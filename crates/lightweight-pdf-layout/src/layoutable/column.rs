//! Column: vertical main axis. Splittable (Text/Column children only, per
//! phase-2 plan step 1), widow/orphan + keep_with_next + forced-page-break
//! fallback for atomic elements bigger than a page (Grundprinzip 7/9).

use super::shared::{
    clip_to_fixed_height, line_height_pt, measure_at_width, push_warning, resolve_auto_size, resolve_bound, shrink_and_bound_height,
    wrap_children, EPS,
};
use super::{LayoutCtx, LayoutResult, Layoutable};
use crate::geometry::{Constraints, Rect, Size};
use crate::render_node::{align_offset, RenderNode};
use crate::warnings::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_core::{Align, Column, Common, Element};

impl Layoutable for Column {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        // Cross-axis "auto" width is shrink-to-fit (widest child), not
        // fill-available: `Column::layout` already fills its own children
        // to the full cross-axis width it's given (see below), but *this*
        // method answers "how much space does this Column want", which a
        // `Row` parent needs to distribute flex siblings correctly — always
        // claiming the full bound here would starve any flex sibling.
        let (bound_width, inner_width) = resolve_bound(self.common.width, constraints.max_width, self.common.padding);
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
                let size = measure_at_width(ctx, child, inner_width);
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
            height: resolve_auto_size(self.common.height, total_height, self.common.padding),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        let (inner, bound_height) = shrink_and_bound_height(area, self.common.height, self.common.padding);

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
            let natural = measure_at_width(ctx, child, child_width);

            let fits_fully = natural.height <= remaining_height + EPS;
            let keep_ok = !fits_fully
                || !child.common().map(|c| c.keep_with_next).unwrap_or(false)
                || keep_with_next_satisfied(self, ctx, i, child_width, remaining_height - natural.height - self.gap);

            if fits_fully && keep_ok {
                let child_area = column_child_rect(inner, self.align, cursor_y, child_width, natural.height);
                match child.layout(ctx, child_area, warnings, page) {
                    LayoutResult::Fit(node) => {
                        rendered.push(node);
                        cursor_y += natural.height + self.gap;
                    }
                    // Shouldn't normally happen (it fit), but handle
                    // defensively: keep what we got, move the rest on.
                    LayoutResult::Split { current, remainder } => {
                        let split = ChildSplit {
                            current,
                            remainder,
                            remaining_siblings: &self.children[i + 1..],
                        };
                        return split_child_and_finish_page(self, area, cursor_y, rendered, split, warnings, page);
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
                let forced_height = if fits_fully { natural.height } else { remaining_height.max(0.0) };
                let child_area = column_child_rect(inner, self.align, cursor_y, child_width, forced_height);
                if let LayoutResult::Fit(node) = child.layout(ctx, child_area, warnings, page) {
                    rendered.push(node);
                }
                if !fits_fully {
                    push_warning(
                        warnings,
                        LayoutWarningKind::ForcedPageBreak,
                        page,
                        "atomic element larger than one page",
                    );
                }
                cursor_y += forced_height + self.gap;
                continue;
            }

            // Splittable (Text/Column) and we're at/near the top of a
            // fresh page budget: attempt the real split.
            let child_area = column_child_rect(inner, self.align, cursor_y, child_width, remaining_height);
            match child.layout(ctx, child_area, warnings, page) {
                LayoutResult::Fit(node) => {
                    rendered.push(node);
                    cursor_y += natural.height.min(remaining_height) + self.gap;
                }
                // `current` may hold real, sized content (the normal case
                // for a child that partially fits) — folded into `cursor_y`
                // by the helper so `finish_page` doesn't see a stale
                // (too-small) cursor and wrongly discard it as "empty".
                LayoutResult::Split { current, remainder } => {
                    let split = ChildSplit {
                        current,
                        remainder,
                        remaining_siblings: &self.children[i + 1..],
                    };
                    return split_child_and_finish_page(self, area, cursor_y, rendered, split, warnings, page);
                }
            }
        }

        let outer_height = self
            .common
            .height
            .unwrap_or(cursor_y.max(0.0) - self.gap.min(cursor_y) + 2.0 * self.common.padding)
            .max(0.0);
        let outer_height = if cursor_y <= EPS { 0.0 } else { outer_height };
        LayoutResult::Fit(wrap_children(area, outer_height, &self.common, rendered))
    }
}

/// Builds a child's placement `Rect` within `Column::layout`'s content
/// box: cross-axis-aligned (`align`) within `inner`'s width, stacked at
/// `cursor_y` — all three of `Column::layout`'s placement sites (fits
/// fully, forced atomic, split attempt) build this identically, differing
/// only in the height they hand the child.
fn column_child_rect(inner: Rect, align: Align, cursor_y: f32, child_width: f32, height: f32) -> Rect {
    let x_offset = align_offset(align, inner.width, child_width);
    Rect {
        x: inner.x + x_offset,
        y: inner.y + cursor_y,
        width: child_width,
        height,
    }
}

/// A child's `Split` halves plus the siblings still waiting after it — the
/// data `split_child_and_finish_page` needs, grouped so the function
/// doesn't need one positional parameter per field (mirrors
/// `RowRenderParams` in `table.rs`).
struct ChildSplit<'a> {
    current: RenderNode,
    remainder: Element,
    remaining_siblings: &'a [Element],
}

/// Handles a child that itself `Split` mid-placement inside
/// `Column::layout` (both the "fits fully" and "near top of page, real
/// split attempt" call sites end up here identically): keeps whatever
/// fit, folds its already-consumed height into `cursor_y`, and ends the
/// page. `cursor_y` must include `current`'s consumed height — otherwise
/// `finish_page` sees a stale (too-small) cursor and can wrongly discard
/// already-rendered content as "empty".
fn split_child_and_finish_page(
    col: &Column,
    area: Rect,
    cursor_y: f32,
    mut rendered: Vec<RenderNode>,
    split: ChildSplit,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> LayoutResult {
    let consumed = split.current.height();
    if !matches!(split.current, RenderNode::Empty) {
        rendered.push(split.current);
    }
    let mut remainder_children = vec![split.remainder];
    remainder_children.extend(split.remaining_siblings.to_vec());
    finish_page(col, area, cursor_y + consumed, rendered, remainder_children, warnings, page)
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
        _ => measure_at_width(ctx, next, width).height,
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
        let overflow_hint = (!remainder_children.is_empty()).then_some("Column content exceeds its fixed height");
        return clip_to_fixed_height(outer_area, fixed_height, &col.common, rendered, warnings, page, overflow_hint);
    }

    let outer_height = if cursor_y <= EPS {
        0.0
    } else {
        (cursor_y - col.gap.min(cursor_y) + 2.0 * col.common.padding).max(0.0)
    };
    let current = wrap_children(outer_area, outer_height, &col.common, rendered);
    let remainder = Column {
        children: remainder_children,
        gap: col.gap,
        align: col.align,
        common: Common {
            height: None,
            ..col.common
        },
    };
    LayoutResult::Split {
        current: if cursor_y <= EPS { RenderNode::Empty } else { current },
        remainder: Element::Column(remainder),
    }
}
