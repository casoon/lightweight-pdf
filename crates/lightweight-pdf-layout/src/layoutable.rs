use crate::font_resolver::FontResolver;
use crate::geometry::{Constraints, Rect, Size};
use crate::render_node::{align_offset, RenderNode};
use crate::text::{text_width_pt, wrap_text};
use crate::warnings::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_core::{Column, Element, Line, Overflow, Rect as RectElement, Row, Spacer, Text};

/// Threshold for the widow/orphan rule (Grundprinzip 9): a paragraph is
/// never split leaving fewer than `N` lines on either side of the break.
const WIDOW_ORPHAN_N: usize = 2;
const EPS: f32 = 0.01;

pub struct LayoutCtx<'a> {
    pub resolver: &'a dyn FontResolver,
}

/// Result of laying an element out into a bounded area: either it fully
/// fit, or the fitting part plus a materialized remainder element for the
/// next page. `Text`, `Column` and `Table` produce
/// `Split` in V1.
pub enum LayoutResult {
    Fit(RenderNode),
    Split { current: RenderNode, remainder: Element },
}

/// `measure`/`layout`. Implemented for every
/// concrete element type (not `Element` variants with `todo!()`, since all
/// V1-through-Phase-2 variants are implemented) plus a dispatching impl on
/// `Element` itself so containers can recurse over `Vec<Element>` children.
pub trait Layoutable {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size;
    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult;
}

// ---------------------------------------------------------------------
// Element: dispatch to the concrete impls below. `PageBreak` has no
// intrinsic size/rendering of its own — `Column`'s layout loop intercepts
// it before ever calling into this generic path.
// ---------------------------------------------------------------------

impl Layoutable for Element {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        match self {
            Element::Text(t) => t.measure(ctx, constraints),
            Element::Row(r) => r.measure(ctx, constraints),
            Element::Column(c) => c.measure(ctx, constraints),
            Element::Spacer(s) => s.measure(ctx, constraints),
            Element::Line(l) => l.measure(ctx, constraints),
            Element::Rect(r) => r.measure(ctx, constraints),
            Element::Table(t) => t.measure(ctx, constraints),
            Element::Image(i) => i.measure(ctx, constraints),
            Element::List(l) => l.measure(ctx, constraints),
            Element::PageBreak => Size::default(),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        match self {
            Element::Text(t) => t.layout(ctx, area, warnings, page),
            Element::Row(r) => r.layout(ctx, area, warnings, page),
            Element::Column(c) => c.layout(ctx, area, warnings, page),
            Element::Spacer(s) => s.layout(ctx, area, warnings, page),
            Element::Line(l) => l.layout(ctx, area, warnings, page),
            Element::Rect(r) => r.layout(ctx, area, warnings, page),
            Element::Table(t) => t.layout(ctx, area, warnings, page),
            Element::Image(i) => i.layout(ctx, area, warnings, page),
            Element::List(l) => l.layout(ctx, area, warnings, page),
            Element::PageBreak => LayoutResult::Fit(RenderNode::Empty),
        }
    }
}

// ---------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------

fn line_height_pt(style: &lightweight_pdf_core::TextStyle) -> f32 {
    style.size * style.line_height
}

fn text_lines_node(area: Rect, style: lightweight_pdf_core::TextStyle, lines: Vec<String>, lh: f32) -> RenderNode {
    let height = lines.len() as f32 * lh;
    RenderNode::clipped(
        area,
        RenderNode::TextLines {
            area: Rect { height, ..area },
            style,
            lines,
            line_height_pt: lh,
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
        let lines = wrap_text(ctx.resolver, &self.style, &self.content, area.width);
        let lh = line_height_pt(&self.style);
        let total_height = lines.len() as f32 * lh;

        if total_height <= area.height + EPS || lines.len() <= 1 {
            if total_height > area.height + EPS {
                warnings.push(LayoutWarning {
                    kind: LayoutWarningKind::TextClipped,
                    page,
                    element_hint: format!("Text \"{}\"", truncate_hint(&self.content)),
                });
            }
            return LayoutResult::Fit(text_lines_node(area, self.style, lines, lh));
        }

        // An explicit, fixed `.height(...)` means this box's overflow is
        // governed by the `overflow` property (Grundprinzip 3: Clip/
        // Ellipsis), not by pagination — it must never turn into a
        // page-spanning `Split`. Only the ambient, pagination-provided
        // budget (no explicit height) may split.
        if self.common.height.is_some() {
            return LayoutResult::Fit(layout_text_fixed_overflow(self, ctx, area, lines, lh, warnings, page));
        }

        let max_lines_by_height = ((area.height + EPS) / lh).floor().max(0.0) as usize;
        let max_lines_by_height = max_lines_by_height.min(lines.len());

        let mut split_at = max_lines_by_height;
        if lines.len() < 2 * WIDOW_ORPHAN_N {
            // Short paragraph: never split, move as a whole.
            split_at = 0;
        } else if split_at < WIDOW_ORPHAN_N {
            // Orphan: too few lines would remain before the break.
            split_at = 0;
        } else if lines.len() - split_at < WIDOW_ORPHAN_N {
            // Widow: pull lines up so the remainder has >= N lines.
            let adjusted = lines.len().saturating_sub(WIDOW_ORPHAN_N);
            split_at = if adjusted >= WIDOW_ORPHAN_N { adjusted } else { 0 };
        }

        if split_at == 0 {
            return LayoutResult::Split {
                current: RenderNode::Empty,
                remainder: Element::Text(self.clone()),
            };
        }

        let (current_lines, remainder_lines) = lines.split_at(split_at);
        let current = text_lines_node(
            Rect {
                height: current_lines.len() as f32 * lh,
                ..area
            },
            self.style,
            current_lines.to_vec(),
            lh,
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
    lines: Vec<String>,
    lh: f32,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> RenderNode {
    let max_lines = (((area.height + EPS) / lh).floor().max(0.0) as usize).min(lines.len());
    if max_lines >= lines.len() {
        return text_lines_node(area, text.style, lines, lh);
    }
    warnings.push(LayoutWarning {
        kind: LayoutWarningKind::TextClipped,
        page,
        element_hint: format!("Text \"{}\"", truncate_hint(&text.content)),
    });
    let take = if text.common.overflow == Overflow::Ellipsis {
        max_lines.max(1).min(lines.len())
    } else {
        max_lines
    };
    let mut kept: Vec<String> = lines.into_iter().take(take).collect();
    if text.common.overflow == Overflow::Ellipsis {
        if let Some(last) = kept.last_mut() {
            *last = fit_with_ellipsis(ctx, &text.style, last, area.width);
        }
    }
    text_lines_node(area, text.style, kept, lh)
}

/// Trims `line` character by character (from the end) until `line + "…"`
/// fits `max_width`, then appends the ellipsis.
fn fit_with_ellipsis(ctx: &LayoutCtx, style: &lightweight_pdf_core::TextStyle, line: &str, max_width: f32) -> String {
    let mut chars: Vec<char> = line.chars().collect();
    loop {
        let candidate: String = chars.iter().collect::<String>() + "…";
        if text_width_pt(ctx.resolver, style.font, style.size, &candidate) <= max_width || chars.is_empty() {
            return candidate;
        }
        chars.pop();
    }
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
        Size {
            width: self.common.width.unwrap_or(constraints.max_width),
            height: self.common.height.unwrap_or(self.thickness),
        }
    }

    fn layout(&self, _ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        if self.thickness > area.height + EPS {
            warnings.push(LayoutWarning {
                kind: LayoutWarningKind::ContentOverflow,
                page,
                element_hint: "Line".to_string(),
            });
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
        Size {
            width: self.common.width.unwrap_or(constraints.max_width),
            height: self.common.height.unwrap_or(0.0),
        }
    }

    fn layout(&self, _ctx: &LayoutCtx, area: Rect, _warnings: &mut Vec<LayoutWarning>, _page: usize) -> LayoutResult {
        let node = RenderNode::Rect {
            area,
            background: self.common.background,
            border: self.common.border,
        };
        LayoutResult::Fit(RenderNode::clipped(area, node))
    }
}

// ---------------------------------------------------------------------
// Row: horizontal main axis, always bound by the incoming area width.
// Non-flex children measure at that bound (approximation, see
// `plan/03-builder-api-design.md`); flex children share the leftover
// space proportionally (taffy flex-grow analogy, ADR-004). No Row-level
// Split in V1 — only Column/Text split (phase-2 plan, step 1).
// ---------------------------------------------------------------------

impl Layoutable for Row {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        let width = self.common.width.unwrap_or(constraints.max_width);
        let inner_width = (width - 2.0 * self.common.padding).max(0.0);
        let (natural_heights, used_width) = row_natural_layout(self, ctx, inner_width);
        let height = self
            .common
            .height
            .unwrap_or(natural_heights.iter().cloned().fold(0.0f32, f32::max) + 2.0 * self.common.padding);
        Size {
            width: self.common.width.unwrap_or((used_width + 2.0 * self.common.padding).min(width)),
            height,
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        let inner = area.shrink(self.common.padding);
        let resolved_widths = resolve_row_widths(self, ctx, inner.width);

        let mut children_nodes = Vec::with_capacity(self.children.len());
        let mut cursor_x = inner.x;
        let mut max_child_height = 0.0f32;
        let mut child_sizes = Vec::with_capacity(self.children.len());

        for (child, w) in self.children.iter().zip(resolved_widths.iter()) {
            if let Element::Spacer(s) = child {
                cursor_x += s.size + self.gap;
                continue;
            }
            let size = child.measure(
                ctx,
                Constraints {
                    max_width: *w,
                    max_height: f32::INFINITY,
                },
            );
            max_child_height = max_child_height.max(size.height);
            child_sizes.push((cursor_x, *w, size.height));
            cursor_x += w + self.gap;
        }

        let row_height = self.common.height.unwrap_or(max_child_height).max(0.0);
        let bounded_row_height = row_height.min(inner.height.max(row_height));

        let mut idx = 0;
        for child in self.children.iter() {
            if matches!(child, Element::Spacer(_)) {
                continue;
            }
            let (x, w, h) = child_sizes[idx];
            idx += 1;
            let y_offset = align_offset(self.align, bounded_row_height, h);
            let child_area = Rect {
                x,
                y: inner.y + y_offset,
                width: w,
                height: h,
            };
            match child.layout(ctx, child_area, warnings, page) {
                LayoutResult::Fit(node) => children_nodes.push(node),
                LayoutResult::Split { current, .. } => {
                    // Row does not support splitting across pages (V1
                    // scope): keep what fits, clip the rest, warn.
                    warnings.push(LayoutWarning {
                        kind: LayoutWarningKind::ContentOverflow,
                        page,
                        element_hint: "Row child taller than available space".to_string(),
                    });
                    children_nodes.push(current);
                }
            }
        }

        let outer_height = self.common.height.unwrap_or(bounded_row_height + 2.0 * self.common.padding);
        let outer = Rect {
            height: outer_height,
            ..area
        };
        let group = RenderNode::Group {
            area: outer,
            clip: true,
            background: self.common.background,
            border: self.common.border,
            children: children_nodes,
        };
        LayoutResult::Fit(group)
    }
}

/// Natural (non-flex-adjusted) child heights plus the total width used by
/// non-flex children, at a given bound width — used by `measure`.
fn row_natural_layout(row: &Row, ctx: &LayoutCtx, bound_width: f32) -> (Vec<f32>, f32) {
    let mut heights = Vec::new();
    let mut used = 0.0f32;
    let n = row.children.len();
    for (i, child) in row.children.iter().enumerate() {
        if let Element::Spacer(s) = child {
            used += s.size;
        } else {
            let size = child.measure(
                ctx,
                Constraints {
                    max_width: bound_width,
                    max_height: f32::INFINITY,
                },
            );
            heights.push(size.height);
            used += size.width;
        }
        if i + 1 < n {
            used += row.gap;
        }
    }
    (heights, used)
}

/// Resolves each non-spacer child's width: fixed/natural for non-flex
/// children, leftover space shared proportionally among flex children.
fn resolve_row_widths(row: &Row, ctx: &LayoutCtx, bound_width: f32) -> Vec<f32> {
    let n = row.children.len();
    let gaps = if n > 0 { (n - 1) as f32 * row.gap } else { 0.0 };
    let mut natural = vec![0.0f32; n];
    let mut flex_sum = 0.0f32;
    let mut fixed_total = 0.0f32;

    for (i, child) in row.children.iter().enumerate() {
        if let Element::Spacer(s) = child {
            natural[i] = s.size;
            fixed_total += s.size;
            continue;
        }
        let common = child.common();
        if let Some(f) = common.and_then(|c| c.flex) {
            flex_sum += f;
            continue;
        }
        let size = child.measure(
            ctx,
            Constraints {
                max_width: bound_width,
                max_height: f32::INFINITY,
            },
        );
        natural[i] = size.width;
        fixed_total += size.width;
    }

    let leftover = (bound_width - fixed_total - gaps).max(0.0);
    if flex_sum > 0.0 {
        for (i, child) in row.children.iter().enumerate() {
            if let Some(f) = child.common().and_then(|c| c.flex) {
                natural[i] = leftover * (f / flex_sum);
            }
        }
    }
    natural
}

// ---------------------------------------------------------------------
// Column: vertical main axis. Splittable (Text/Column children only, per
// phase-2 plan step 1), widow/orphan + keep_with_next + forced-page-break
// fallback for atomic elements bigger than a page (Grundprinzip 7/9).
// ---------------------------------------------------------------------

impl Layoutable for Column {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        // Cross-axis "auto" width is shrink-to-fit (widest child), not
        // fill-available: `Column::layout` already fills its own children
        // to the full cross-axis width it's given (see below), but *this*
        // method answers "how much space does this Column want", which a
        // `Row` parent needs to distribute flex siblings correctly — always
        // claiming the full bound here would starve any flex sibling.
        let bound_width = self.common.width.unwrap_or(constraints.max_width);
        let inner_width = (bound_width - 2.0 * self.common.padding).max(0.0);
        let mut total_height = 0.0f32;
        let mut max_child_width = 0.0f32;
        let n = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            if let Element::PageBreak = child {
                continue;
            }
            let (w, h) = if let Element::Spacer(s) = child {
                (0.0, s.size)
            } else {
                let size = child.measure(
                    ctx,
                    Constraints {
                        max_width: inner_width,
                        max_height: f32::INFINITY,
                    },
                );
                (size.width, size.height)
            };
            max_child_width = max_child_width.max(w);
            total_height += h;
            if i + 1 < n {
                total_height += self.gap;
            }
        }
        Size {
            width: self
                .common
                .width
                .unwrap_or((max_child_width + 2.0 * self.common.padding).min(bound_width)),
            height: self.common.height.unwrap_or(total_height + 2.0 * self.common.padding),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        let inner = area.shrink(self.common.padding);
        let bound_height = self.common.height.map(|h| h - 2.0 * self.common.padding).unwrap_or(inner.height);

        let mut rendered = Vec::new();
        let mut cursor_y = 0.0f32;

        for (i, child) in self.children.iter().enumerate() {
            if let Element::PageBreak = child {
                let remainder_children: Vec<Element> = self.children[i + 1..].to_vec();
                return finish_page(self, area, cursor_y, rendered, remainder_children, warnings, page);
            }

            let child_width = child
                .common()
                .and_then(|c| c.width)
                .unwrap_or(inner.width - self.common.padding.min(inner.width));
            let child_width = if child.common().and_then(|c| c.width).is_some() {
                child_width
            } else {
                inner.width
            };

            if let Element::Spacer(s) = child {
                cursor_y += s.size + self.gap;
                continue;
            }

            let remaining_height = (bound_height - cursor_y).max(0.0);
            let natural = child.measure(
                ctx,
                Constraints {
                    max_width: child_width,
                    max_height: f32::INFINITY,
                },
            );

            let fits_fully = natural.height <= remaining_height + EPS;
            let keep_ok = !fits_fully
                || !child.common().map(|c| c.keep_with_next).unwrap_or(false)
                || keep_with_next_satisfied(self, ctx, i, child_width, remaining_height - natural.height - self.gap);

            if fits_fully && keep_ok {
                let x_offset = align_offset(self.align, inner.width, child_width);
                let child_area = Rect {
                    x: inner.x + x_offset,
                    y: inner.y + cursor_y,
                    width: child_width,
                    height: natural.height,
                };
                match child.layout(ctx, child_area, warnings, page) {
                    LayoutResult::Fit(node) => {
                        rendered.push(node);
                        cursor_y += natural.height + self.gap;
                    }
                    LayoutResult::Split { current, remainder } => {
                        // Shouldn't normally happen (it fit), but handle
                        // defensively: keep what we got, move the rest on.
                        // `cursor_y` must account for the height `current`
                        // actually consumed — otherwise `finish_page` sees
                        // a stale (too-small) cursor and can wrongly
                        // discard already-rendered content as "empty".
                        let consumed = current.height();
                        if !matches!(current, RenderNode::Empty) {
                            rendered.push(current);
                        }
                        let mut remainder_children = vec![remainder];
                        remainder_children.extend(self.children[i + 1..].to_vec());
                        return finish_page(self, area, cursor_y + consumed, rendered, remainder_children, warnings, page);
                    }
                }
                continue;
            }

            // Does not fit fully on this page (or keep_with_next failed).
            let splittable = matches!(child, Element::Text(_) | Element::Column(_) | Element::Table(_));
            let min_unit = match child {
                Element::Text(t) => line_height_pt(&t.style),
                Element::Table(t) => crate::table::table_min_unit(ctx, t, child_width),
                _ => natural.height,
            };

            if cursor_y > EPS && (remaining_height < min_unit - EPS || !keep_ok) {
                // Not worth attempting here: move this whole child (and
                // everything after it) to the next page.
                let mut remainder_children = vec![child.clone()];
                remainder_children.extend(self.children[i + 1..].to_vec());
                return finish_page(self, area, cursor_y, rendered, remainder_children, warnings, page);
            }

            if !splittable {
                // Either an atomic element that doesn't fit even a full,
                // empty page (force placement, clip, warn — Grundprinzip
                // 7), or one whose `keep_with_next` couldn't be honored at
                // the very start of a page (nothing to defer to, place at
                // its natural size instead of stretching it).
                let x_offset = align_offset(self.align, inner.width, child_width);
                let forced_height = if fits_fully { natural.height } else { remaining_height.max(0.0) };
                let child_area = Rect {
                    x: inner.x + x_offset,
                    y: inner.y + cursor_y,
                    width: child_width,
                    height: forced_height,
                };
                if let LayoutResult::Fit(node) = child.layout(ctx, child_area, warnings, page) {
                    rendered.push(node);
                }
                if !fits_fully {
                    warnings.push(LayoutWarning {
                        kind: LayoutWarningKind::ForcedPageBreak,
                        page,
                        element_hint: "atomic element larger than one page".to_string(),
                    });
                }
                cursor_y += forced_height + self.gap;
                continue;
            }

            // Splittable (Text/Column) and we're at/near the top of a
            // fresh page budget: attempt the real split.
            let x_offset = align_offset(self.align, inner.width, child_width);
            let child_area = Rect {
                x: inner.x + x_offset,
                y: inner.y + cursor_y,
                width: child_width,
                height: remaining_height,
            };
            match child.layout(ctx, child_area, warnings, page) {
                LayoutResult::Fit(node) => {
                    rendered.push(node);
                    cursor_y += natural.height.min(remaining_height) + self.gap;
                }
                LayoutResult::Split { current, remainder } => {
                    // Same fix as above: `current` may hold real, sized
                    // content (the normal case for a child that partially
                    // fits) — `cursor_y` must reflect that before
                    // `finish_page` decides whether this page is empty.
                    let consumed = current.height();
                    if !matches!(current, RenderNode::Empty) {
                        rendered.push(current);
                    }
                    let mut remainder_children = vec![remainder];
                    remainder_children.extend(self.children[i + 1..].to_vec());
                    return finish_page(self, area, cursor_y + consumed, rendered, remainder_children, warnings, page);
                }
            }
        }

        let outer_height = self
            .common
            .height
            .unwrap_or(cursor_y.max(0.0) - self.gap.min(cursor_y) + 2.0 * self.common.padding)
            .max(0.0);
        let outer_height = if cursor_y <= EPS { 0.0 } else { outer_height };
        let outer = Rect {
            height: outer_height,
            ..area
        };
        LayoutResult::Fit(RenderNode::Group {
            area: outer,
            clip: true,
            background: self.common.background,
            border: self.common.border,
            children: rendered,
        })
    }
}

/// Peeks at the sibling right after index `i` to decide whether
/// `keep_with_next` is satisfiable: does its minimal content (one line for
/// Text, full natural height otherwise) fit in the space left after
/// placing the current child?
fn keep_with_next_satisfied(col: &Column, ctx: &LayoutCtx, i: usize, width: f32, leftover: f32) -> bool {
    let Some(next) = col.children.get(i + 1) else {
        return true;
    };
    let min_needed = match next {
        Element::Text(t) => line_height_pt(&t.style),
        Element::Spacer(s) => s.size,
        _ => {
            next.measure(
                ctx,
                Constraints {
                    max_width: width,
                    max_height: f32::INFINITY,
                },
            )
            .height
        }
    };
    leftover + EPS >= min_needed
}

/// Ends the current page's placement for a `Column`. An auto-sized/
/// pagination-driven `Column` (no explicit `.height()`) produces a real
/// `Split` so the remainder continues on the next page. A `Column` with an
/// explicit fixed height instead clips right here and stays a `Fit` — its
/// overflow is governed by `overflow`/Grundprinzip 3, not by pagination
/// (mirrors the same distinction made for `Text`, see `layout_fixed_overflow`).
fn finish_page(
    col: &Column,
    outer_area: Rect,
    cursor_y: f32,
    rendered: Vec<RenderNode>,
    remainder_children: Vec<Element>,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> LayoutResult {
    if let Some(fixed_height) = col.common.height {
        if !remainder_children.is_empty() {
            warnings.push(LayoutWarning {
                kind: LayoutWarningKind::ContentOverflow,
                page,
                element_hint: "Column content exceeds its fixed height".to_string(),
            });
        }
        return LayoutResult::Fit(RenderNode::Group {
            area: Rect {
                height: fixed_height,
                ..outer_area
            },
            clip: true,
            background: col.common.background,
            border: col.common.border,
            children: rendered,
        });
    }

    let outer_height = if cursor_y <= EPS {
        0.0
    } else {
        (cursor_y - col.gap.min(cursor_y) + 2.0 * col.common.padding).max(0.0)
    };
    let current = RenderNode::Group {
        area: Rect {
            height: outer_height,
            ..outer_area
        },
        clip: true,
        background: col.common.background,
        border: col.common.border,
        children: rendered,
    };
    let remainder = Column {
        children: remainder_children,
        gap: col.gap,
        align: col.align,
        common: lightweight_pdf_core::Common {
            height: None,
            ..col.common
        },
    };
    LayoutResult::Split {
        current: if cursor_y <= EPS { RenderNode::Empty } else { current },
        remainder: Element::Column(remainder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pagination::paginate_body;
    use lightweight_pdf_core::{Common, Overflow as OverflowKind, Text as TextEl};

    struct FixedMetrics;
    impl crate::font_resolver::FontMetrics for FixedMetrics {
        fn advance(&self, ch: char) -> f32 {
            if ch == ' ' {
                300.0
            } else {
                600.0
            }
        }
        fn ascent(&self) -> f32 {
            800.0
        }
        fn descent(&self) -> f32 {
            -200.0
        }
    }
    struct FixedResolver;
    impl FontResolver for FixedResolver {
        fn metrics(&self, _key: lightweight_pdf_core::FontKey) -> &dyn crate::font_resolver::FontMetrics {
            &FixedMetrics
        }
    }

    fn ctx() -> LayoutCtx<'static> {
        LayoutCtx { resolver: &FixedResolver }
    }

    // --- Grundprinzip 1: auto-size is the default -----------------------

    #[test]
    fn column_auto_size_grows_with_content() {
        let short = Column::new().child(TextEl::new("Hi").size(10.0).line_height(1.0));
        let long = Column::new().children(vec![
            TextEl::new("Line one").size(10.0).line_height(1.0),
            TextEl::new("Line two").size(10.0).line_height(1.0),
            TextEl::new("Line three").size(10.0).line_height(1.0),
        ]);
        let c = ctx();
        let constraints = Constraints {
            max_width: 400.0,
            max_height: f32::INFINITY,
        };
        let short_size = short.measure(&c, constraints);
        let long_size = long.measure(&c, constraints);
        assert!(long_size.height > short_size.height, "more content must measure taller");
    }

    // --- Grundprinzip 2/3: hard-break + fixed-size Clip (never Split) ---

    #[test]
    fn fixed_height_text_clips_instead_of_splitting() {
        let text = TextEl::new("AAAA BBBB CCCC DDDD").size(10.0).line_height(1.0).height(10.0);
        let c = ctx();
        let mut warnings = Vec::new();
        // Narrow width forces multiple lines; the box is only 1 line tall.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 30.0,
            height: 10.0,
        };
        let result = text.layout(&c, area, &mut warnings, 1);
        assert!(
            matches!(result, LayoutResult::Fit(_)),
            "fixed-size box must Clip, never Split across pages"
        );
        assert!(warnings.iter().any(|w| w.kind == LayoutWarningKind::TextClipped));
    }

    #[test]
    fn fixed_height_column_clips_instead_of_splitting() {
        let col = Column::new().height(10.0).children(vec![
            TextEl::new("Line one").size(10.0).line_height(1.0),
            TextEl::new("Line two").size(10.0).line_height(1.0),
            TextEl::new("Line three").size(10.0).line_height(1.0),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 10.0,
        };
        let result = col.layout(&c, area, &mut warnings, 1);
        assert!(matches!(result, LayoutResult::Fit(_)), "fixed-height Column must Clip, never Split");
        assert!(warnings.iter().any(|w| w.kind == LayoutWarningKind::ContentOverflow));
    }

    // --- Grundprinzip 4/6: containers/children never overlap ------------

    #[test]
    fn row_children_do_not_overlap_horizontally() {
        let row = Row::new()
            .gap(10.0)
            .child(TextEl::new("Left").size(10.0))
            .child(TextEl::new("Right").size(10.0));
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 50.0,
        };
        let result = row.layout(&c, area, &mut warnings, 1);
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = result else {
            panic!("expected a Fit Group");
        };
        assert_eq!(children.len(), 2);
        let rects: Vec<Rect> = children
            .iter()
            .map(|n| match n {
                RenderNode::Group { area, .. } => *area,
                other => panic!("expected nested Group, got {other:?}"),
            })
            .collect();
        assert!(
            rects[0].x + rects[0].width <= rects[1].x + EPS,
            "children must not overlap: {:?} vs {:?}",
            rects[0],
            rects[1]
        );
    }

    // --- Phase 2: PageBreak ----------------------------------------------

    #[test]
    fn page_break_forces_a_split_at_the_marker() {
        let col = Column::new().children(vec![
            Element::Text(TextEl::new("a")),
            Element::PageBreak,
            Element::Text(TextEl::new("b")),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 400.0, // plenty of room — the break must still trigger.
        };
        match col.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { remainder, .. } => match remainder {
                Element::Column(rem) => {
                    assert_eq!(rem.children.len(), 1);
                    match &rem.children[0] {
                        Element::Text(t) => assert_eq!(t.content, "b"),
                        other => panic!("expected Text, got {other:?}"),
                    }
                }
                other => panic!("expected Column remainder, got {other:?}"),
            },
            LayoutResult::Fit(_) => panic!("PageBreak must force a Split even when content would otherwise fit"),
        }
    }

    // --- Grundprinzip 7: atomic element bigger than a page --------------

    #[test]
    fn oversized_atomic_element_is_forced_onto_its_own_page_and_terminates() {
        let children = vec![
            Element::Rect(RectElement::new().height(5000.0).background(lightweight_pdf_core::Color::BLACK)),
            Element::Rect(RectElement::new().height(20.0)),
        ];
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        let pages = paginate_body(&children, area, &c, &mut warnings);
        assert_eq!(
            pages.len(),
            2,
            "oversized element consumes its own page, second Rect starts a fresh one"
        );
        assert_eq!(warnings.iter().filter(|w| w.kind == LayoutWarningKind::ForcedPageBreak).count(), 1);
    }

    // --- Grundprinzip 9: widow/orphan + short-paragraph-never-split -----

    fn line_text(n: usize) -> String {
        (0..n).map(|i| format!("L{i}")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn short_paragraph_is_never_split() {
        // 3 lines < 2*N(=4): must move as a whole even though 2 lines
        // would technically fit.
        let text = TextEl::new(line_text(3)).size(10.0).line_height(1.0);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 20.0, // fits 2 of 3 lines by height alone
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                assert!(
                    matches!(current, RenderNode::Empty),
                    "short paragraph must move whole, nothing placed on this page"
                );
                match remainder {
                    Element::Text(t) => assert_eq!(t.content, line_text(3)),
                    other => panic!("expected Text remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected a Split (paragraph doesn't fully fit)"),
        }
    }

    #[test]
    fn widow_is_avoided_by_pulling_lines_up() {
        // 5 lines, only 4 fit by height -> naive split would leave 1
        // (widow). Rule pulls lines up so >= N=2 remain after the break.
        let text = TextEl::new(line_text(5)).size(10.0).line_height(1.0);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 40.0, // exactly 4 lines at 10pt line-height
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                let RenderNode::Group { children, .. } = current else {
                    panic!("expected the clip-wrapping Group");
                };
                let RenderNode::TextLines { lines, .. } = &children[0] else {
                    panic!("expected TextLines");
                };
                assert_eq!(lines.len(), 3, "must pull one line up so the remainder has >= 2 lines");
                match remainder {
                    Element::Text(t) => assert_eq!(t.content.split(' ').count(), 2),
                    other => panic!("expected Text remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected a Split"),
        }
    }

    #[test]
    fn orphan_moves_whole_paragraph_when_room_is_too_small() {
        // 5 lines, only 1 fits by height -> orphan (< N before break) ->
        // move the whole paragraph.
        let text = TextEl::new(line_text(5)).size(10.0).line_height(1.0);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 10.0,
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, .. } => {
                assert!(matches!(current, RenderNode::Empty));
            }
            LayoutResult::Fit(_) => panic!("expected a Split"),
        }
    }

    // --- Grundprinzip 9: keep_with_next ----------------------------------

    #[test]
    fn keep_with_next_moves_heading_along_with_its_body() {
        let col = Column::new().gap(0.0).children(vec![
            Element::Text(TextEl::new("Filler").size(10.0).line_height(1.0)),
            Element::Text(TextEl::new("Heading").size(10.0).line_height(1.0).keep_with_next()),
            Element::Text(TextEl::new("Body").size(10.0).line_height(1.0)),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        // 10 (filler) + 10 (heading) fits, but leaves only 5pt — not
        // enough for one more 10pt line of body text.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 25.0,
        };
        match col.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                let RenderNode::Group { children, .. } = current else {
                    panic!("expected Group");
                };
                assert_eq!(children.len(), 1, "only the filler should remain on this page");
                match remainder {
                    Element::Column(rem) => {
                        assert_eq!(rem.children.len(), 2);
                        match &rem.children[0] {
                            Element::Text(t) => assert_eq!(t.content, "Heading"),
                            other => panic!("expected Heading Text, got {other:?}"),
                        }
                    }
                    other => panic!("expected Column remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected keep_with_next to force a Split before the heading"),
        }
    }

    #[test]
    fn overflow_ellipsis_truncates_fixed_single_line_text() {
        let text = TextEl::new("AAAAAAAAAAAAAAAA")
            .size(10.0)
            .line_height(1.0)
            .height(10.0)
            .overflow(OverflowKind::Ellipsis);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 10.0,
        };
        let result = text.layout(&c, area, &mut warnings, 1);
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = result else {
            panic!("expected Fit Group (clip wrapper)");
        };
        let RenderNode::TextLines { lines, .. } = &children[0] else {
            panic!("expected TextLines");
        };
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('…'), "expected an ellipsis, got {:?}", lines[0]);
    }

    #[test]
    fn common_default_is_used() {
        // Sanity check that Common::default() means "auto", not zero-sized.
        let c = Common::default();
        assert_eq!(c.width, None);
        assert_eq!(c.height, None);
    }
}
