//! Row: horizontal main axis, always bound by the incoming area width.
//! Non-flex children measure at that bound (approximation, see
//! `plan/03-builder-api-design.md`); flex children share the leftover
//! space proportionally (taffy flex-grow analogy, ADR-004). No Row-level
//! Split in V1 — only Column/Text split (phase-2 plan, step 1).

use super::shared::{coerce_to_fit_and_warn, finish_fit, measure_at_width, resolve_auto_size, resolve_bound};
use super::{LayoutCtx, LayoutResult, Layoutable};
use crate::geometry::{Constraints, Rect, Size};
use crate::render_node::align_offset;
use crate::warnings::LayoutWarning;
use lightweight_pdf_core::{Element, Row};

impl Layoutable for Row {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        let (width, inner_width) = resolve_bound(self.common.width, constraints.max_width, self.common.padding);
        let (natural_heights, used_width) = row_natural_layout(self, ctx, inner_width);
        let height = resolve_auto_size(
            self.common.height,
            natural_heights.iter().cloned().fold(0.0f32, f32::max),
            self.common.padding,
        );
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
            let size = measure_at_width(ctx, child, *w);
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
            // Row does not support splitting across pages (V1 scope): keep
            // what fits, clip the rest, warn.
            let result = child.layout(ctx, child_area, warnings, page);
            children_nodes.push(coerce_to_fit_and_warn(
                result,
                warnings,
                page,
                "Row child taller than available space",
            ));
        }

        finish_fit(&self.common, area, bounded_row_height, children_nodes)
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
            let size = measure_at_width(ctx, child, bound_width);
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
        let size = measure_at_width(ctx, child, bound_width);
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
