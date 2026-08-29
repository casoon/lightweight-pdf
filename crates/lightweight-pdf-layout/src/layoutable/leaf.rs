//! Leaf `Layoutable` impls: `Text`, `Spacer`, `Line`, `Rect`. None of these
//! have children of their own to recurse into.

use super::shared::{line_height_pt, push_warning, size_with_defaults, EPS};
use super::{LayoutCtx, LayoutResult, Layoutable};
use crate::geometry::{Constraints, Rect, Size};
use crate::render_node::RenderNode;
use crate::text::{text_width_pt, wrap_text, wrap_text_marking_paragraph_ends};
use crate::warnings::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_core::{Element, Line, Overflow, Rect as RectElement, Spacer, Text, TextStyle};

/// Threshold for the widow/orphan rule (Grundprinzip 9): a paragraph is
/// never split leaving fewer than `N` lines on either side of the break.
const WIDOW_ORPHAN_N: usize = 2;

/// How many lines of height `lh` fit within `area_height` (never more than
/// `available`) — the line-count budget shared by `Text::layout`'s
/// pagination split point and `layout_text_fixed_overflow`'s clip point.
fn max_lines_fitting(area_height: f32, lh: f32, available: usize) -> usize {
    (((area_height + EPS) / lh).floor().max(0.0) as usize).min(available)
}

/// `wrap_text_marking_paragraph_ends`'s two parallel `Vec`s, bundled so
/// they travel together as a single parameter through `Text::layout`'s
/// split/overflow helpers instead of two.
struct WrappedLines {
    lines: Vec<String>,
    paragraph_end: Vec<bool>,
}

impl WrappedLines {
    fn len(&self) -> usize {
        self.lines.len()
    }
}

/// `Text`'s three link-related fields, bundled for the same reason as
/// `WrappedLines`: one parameter through the split/overflow helpers
/// instead of three, all cloned together at each `Text::layout` exit
/// point.
struct TextLinks {
    url: Option<String>,
    anchor: Option<String>,
    link_to: Option<String>,
}

impl TextLinks {
    fn from(text: &Text) -> Self {
        TextLinks {
            url: text.url.clone(),
            anchor: text.anchor.clone(),
            link_to: text.link_to.clone(),
        }
    }
}

fn text_lines_node(area: Rect, style: TextStyle, wrapped: WrappedLines, lh: f32, links: TextLinks) -> RenderNode {
    let height = wrapped.lines.len() as f32 * lh;
    RenderNode::clipped(
        area,
        RenderNode::TextLines {
            area: Rect { height, ..area },
            style,
            lines: wrapped.lines,
            paragraph_end: wrapped.paragraph_end,
            line_height_pt: lh,
            url: links.url,
            anchor: links.anchor,
            link_to: links.link_to,
        },
    )
}

impl Layoutable for Text {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        let width = self.common.width.unwrap_or(constraints.max_width);
        let lines = wrap_text(ctx.resolver, &self.style, &self.content, width);
        let lh = line_height_pt(&self.style);
        let actual_width = lines
            .iter()
            .map(|l| text_width_pt(ctx.resolver, self.style.font, self.style.size, l))
            .fold(0.0f32, f32::max);
        Size {
            width: self.common.width.unwrap_or(actual_width.min(width)),
            height: self.common.height.unwrap_or(lines.len() as f32 * lh),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        push_missing_glyph_warnings(ctx, &self.style, &self.content, warnings, page);
        let (lines, paragraph_end) = wrap_text_marking_paragraph_ends(ctx.resolver, &self.style, &self.content, area.width);
        let wrapped = WrappedLines { lines, paragraph_end };
        let lh = line_height_pt(&self.style);
        let total_height = wrapped.len() as f32 * lh;

        if total_height <= area.height + EPS || wrapped.len() <= 1 {
            if total_height > area.height + EPS {
                push_warning(warnings, LayoutWarningKind::TextClipped, page, text_clipped_hint(&self.content));
            }
            return LayoutResult::Fit(text_lines_node(area, self.style, wrapped, lh, TextLinks::from(self)));
        }

        // An explicit, fixed `.height(...)` means this box's overflow is
        // governed by the `overflow` property (Grundprinzip 3: Clip/
        // Ellipsis), not by pagination — it must never turn into a
        // page-spanning `Split`. Only the ambient, pagination-provided
        // budget (no explicit height) may split.
        if self.common.height.is_some() {
            return LayoutResult::Fit(layout_text_fixed_overflow(self, ctx, area, wrapped, lh, warnings, page));
        }

        let max_lines_by_height = max_lines_fitting(area.height, lh, wrapped.len());

        let mut split_at = max_lines_by_height;
        if wrapped.len() < 2 * WIDOW_ORPHAN_N {
            // Short paragraph: never split, move as a whole.
            split_at = 0;
        } else if split_at < WIDOW_ORPHAN_N {
            // Orphan: too few lines would remain before the break.
            split_at = 0;
        } else if wrapped.len() - split_at < WIDOW_ORPHAN_N {
            // Widow: pull lines up so the remainder has >= N lines.
            let adjusted = wrapped.len().saturating_sub(WIDOW_ORPHAN_N);
            split_at = if adjusted >= WIDOW_ORPHAN_N { adjusted } else { 0 };
        }

        if split_at == 0 {
            return LayoutResult::Split {
                current: RenderNode::Empty,
                remainder: Element::Text(self.clone()),
            };
        }

        let (current_lines, remainder_lines) = wrapped.lines.split_at(split_at);
        let (current_paragraph_end, _) = wrapped.paragraph_end.split_at(split_at);
        let current = text_lines_node(
            Rect {
                height: current_lines.len() as f32 * lh,
                ..area
            },
            self.style,
            WrappedLines {
                lines: current_lines.to_vec(),
                paragraph_end: current_paragraph_end.to_vec(),
            },
            lh,
            TextLinks::from(self),
        );
        let remainder_text = remainder_lines.join(" ");
        let mut remainder = self.clone();
        remainder.content = remainder_text;
        LayoutResult::Split {
            current,
            remainder: Element::Text(remainder),
        }
    }
}

/// Overflow handling for an explicitly, fixed-size text box (Grundprinzip
/// 3): `Clip` drops lines that don't fit, `Ellipsis` truncates the last
/// visible line with a trailing "…" (single-line use case: a long label in
/// a narrow, fixed column). Free function (not an inherent impl) because
/// `Text` is defined in `lightweight-pdf-core`, outside this crate.
fn layout_text_fixed_overflow(
    text: &Text,
    ctx: &LayoutCtx,
    area: Rect,
    wrapped: WrappedLines,
    lh: f32,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> RenderNode {
    let max_lines = max_lines_fitting(area.height, lh, wrapped.len());
    if max_lines >= wrapped.len() {
        return text_lines_node(area, text.style, wrapped, lh, TextLinks::from(text));
    }
    push_warning(warnings, LayoutWarningKind::TextClipped, page, text_clipped_hint(&text.content));
    let take = if text.common.overflow == Overflow::Ellipsis {
        max_lines.max(1).min(wrapped.len())
    } else {
        max_lines
    };
    let mut kept: Vec<String> = wrapped.lines.into_iter().take(take).collect();
    let mut kept_paragraph_end: Vec<bool> = wrapped.paragraph_end.into_iter().take(take).collect();
    if text.common.overflow == Overflow::Ellipsis {
        if let Some(last) = kept.last_mut() {
            *last = fit_with_ellipsis(ctx, &text.style, last, area.width);
        }
        // An ellipsis-truncated line is never stretched, regardless of
        // whether it happened to be its paragraph's real last line.
        if let Some(last) = kept_paragraph_end.last_mut() {
            *last = true;
        }
    }
    text_lines_node(
        area,
        text.style,
        WrappedLines {
            lines: kept,
            paragraph_end: kept_paragraph_end,
        },
        lh,
        TextLinks::from(text),
    )
}

/// Trims `line` character by character (from the end) until `line + "…"`
/// fits `max_width`, then appends the ellipsis.
fn fit_with_ellipsis(ctx: &LayoutCtx, style: &TextStyle, line: &str, max_width: f32) -> String {
    let mut chars: Vec<char> = line.chars().collect();
    loop {
        let candidate: String = chars.iter().collect::<String>() + "…";
        if text_width_pt(ctx.resolver, style.font, style.size, &candidate) <= max_width || chars.is_empty() {
            return candidate;
        }
        chars.pop();
    }
}

/// The `element_hint` used for both `Text::layout`'s and
/// `layout_text_fixed_overflow`'s `TextClipped` warning.
/// Emits `LayoutWarningKind::MissingGlyph` for every character in `content`
/// the resolved font has no glyph for — deduplicated per (`ch`, `font`)
/// against everything already in `warnings`, not per occurrence (a
/// document-wide repeated character/font miss would otherwise drown the
/// diagnosis in noise).
fn push_missing_glyph_warnings(ctx: &LayoutCtx, style: &TextStyle, content: &str, warnings: &mut Vec<LayoutWarning>, page: usize) {
    let metrics = ctx.resolver.metrics(style.font);
    for ch in content.chars() {
        if ch.is_whitespace() || metrics.has_glyph(ch) {
            continue;
        }
        let kind = LayoutWarningKind::MissingGlyph { ch, font: style.font };
        if !warnings.iter().any(|w| w.kind == kind) {
            push_warning(warnings, kind, page, format!("missing glyph for {ch:?}"));
        }
    }
}

fn text_clipped_hint(content: &str) -> String {
    format!("Text \"{}\"", truncate_hint(content))
}

fn truncate_hint(s: &str) -> String {
    if s.len() > 24 {
        format!("{}…", &s[..24])
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------
// Spacer (special-cased by Row/Column before generic dispatch — the axis
// a Spacer consumes depends on its parent, which a standalone measure/
// layout call cannot know).
// ---------------------------------------------------------------------

impl Layoutable for Spacer {
    fn measure(&self, _ctx: &LayoutCtx, _constraints: Constraints) -> Size {
        Size {
            width: self.size,
            height: self.size,
        }
    }

    fn layout(&self, _ctx: &LayoutCtx, _area: Rect, _warnings: &mut Vec<LayoutWarning>, _page: usize) -> LayoutResult {
        LayoutResult::Fit(RenderNode::Empty)
    }
}

// ---------------------------------------------------------------------
// Line
// ---------------------------------------------------------------------

impl Layoutable for Line {
    fn measure(&self, _ctx: &LayoutCtx, constraints: Constraints) -> Size {
        size_with_defaults(&self.common, constraints, self.thickness)
    }

    fn layout(&self, _ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        if self.thickness > area.height + EPS {
            push_warning(warnings, LayoutWarningKind::ContentOverflow, page, "Line");
        }
        let y_mid = area.y + (self.thickness / 2.0).min(area.height);
        let node = RenderNode::Line {
            x1: area.x,
            y1: y_mid,
            x2: area.x + area.width,
            y2: y_mid,
            thickness: self.thickness,
            color: self.color,
        };
        LayoutResult::Fit(RenderNode::clipped(area, node))
    }
}

// ---------------------------------------------------------------------
// Rect
// ---------------------------------------------------------------------

impl Layoutable for RectElement {
    fn measure(&self, _ctx: &LayoutCtx, constraints: Constraints) -> Size {
        size_with_defaults(&self.common, constraints, 0.0)
    }

    fn layout(&self, _ctx: &LayoutCtx, area: Rect, _warnings: &mut Vec<LayoutWarning>, _page: usize) -> LayoutResult {
        let node = RenderNode::Rect {
            area,
            background: self.common.background,
            border: self.common.border,
            corner_radius: self.common.corner_radius,
        };
        LayoutResult::Fit(RenderNode::clipped(area, node))
    }
}
