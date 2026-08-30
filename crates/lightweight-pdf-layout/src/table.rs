//! `Table` layout (Phase 3, `plan/phases/phase-3-tables.md`): fixed/flex
//! column widths, cell content reuses the ordinary `Layoutable` machinery
//! (word-wrap included), row height auto-grows with the tallest cell
//! (Grundprinzip 5), a row is never split mid-row (atomic unit), the
//! header repeats on every continuation page.

use crate::geometry::{Constraints, Rect, Size};
use crate::layoutable::{
    clip_to_fixed_height, finish_fit, measure_at_width, push_warning, resolve_auto_size, resolve_bound, shrink_and_bound_height,
    wrap_children, LayoutCtx, LayoutResult, Layoutable,
};
use crate::render_node::{align_offset, RenderNode};
use crate::warnings::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_core::{Color, ColumnWidth, Common, Element, Table, TableCell, TableColumn};

const EPS: f32 = 0.01;

/// Fixed columns keep their exact width; the leftover space is shared
/// proportionally among flex columns by weight (taffy `flex-grow`
/// analogy). The last flex column absorbs any float-rounding remainder so
/// the widths sum *exactly* to `available_width` (phase-3 DoD).
fn resolve_column_widths(columns: &[TableColumn], available_width: f32) -> Vec<f32> {
    let fixed_total: f32 = columns
        .iter()
        .filter_map(|c| match c.width {
            ColumnWidth::Fixed(w) => Some(w),
            ColumnWidth::Flex(_) => None,
        })
        .sum();
    let flex_sum: f32 = columns
        .iter()
        .filter_map(|c| match c.width {
            ColumnWidth::Flex(w) => Some(w),
            ColumnWidth::Fixed(_) => None,
        })
        .sum();
    let leftover = (available_width - fixed_total).max(0.0);

    let mut widths = Vec::with_capacity(columns.len());
    let mut last_flex_idx = None;
    for (i, c) in columns.iter().enumerate() {
        match c.width {
            ColumnWidth::Fixed(w) => widths.push(w),
            ColumnWidth::Flex(weight) => {
                widths.push(if flex_sum > 0.0 { leftover * (weight / flex_sum) } else { 0.0 });
                last_flex_idx = Some(i);
            }
        }
    }
    if let Some(i) = last_flex_idx {
        let sum: f32 = widths.iter().sum();
        widths[i] += available_width - sum;
    }
    widths
}

fn measure_row_height(ctx: &LayoutCtx, cells: &[TableCell], col_widths: &[f32], cell_padding: f32) -> f32 {
    let mut max_h = 0.0f32;
    let mut col_idx = 0;
    for cell in cells {
        if col_idx >= col_widths.len() {
            break;
        }
        let span = cell.colspan.max(1);
        let end_idx = (col_idx + span).min(col_widths.len());
        let total_w: f32 = col_widths[col_idx..end_idx].iter().sum();
        let padding = cell.padding.unwrap_or(cell_padding);
        let inner_w = (total_w - 2.0 * padding).max(0.0);
        let h = measure_at_width(ctx, &cell.element, inner_w).height + 2.0 * padding;
        max_h = max_h.max(h);
        col_idx = end_idx;
    }
    max_h
}

/// The header row's height, or `0.0` if the table has no header — shared
/// by `table_min_unit` and `Table::layout`.
fn header_row_height(ctx: &LayoutCtx, table: &Table, col_widths: &[f32]) -> f32 {
    table
        .header
        .as_ref()
        .map(|h| measure_row_height(ctx, h, col_widths, table.cell_padding))
        .unwrap_or(0.0)
}

/// The smallest worthwhile placement for a `Table` when there's already
/// other content on the page (used by `Column`'s "is it worth starting
/// here" check, mirroring `Text`'s per-line granularity instead of
/// treating a whole table as one atomic block).
pub fn table_min_unit(ctx: &LayoutCtx, table: &Table, width: f32) -> f32 {
    let col_widths = resolve_column_widths(&table.columns, (width - 2.0 * table.common.padding).max(0.0));
    let first_row_h = table
        .rows
        .first()
        .map(|r| measure_row_height(ctx, r, &col_widths, table.cell_padding))
        .unwrap_or(0.0);
    header_row_height(ctx, table, &col_widths) + first_row_h
}

fn layout_row_cells(
    ctx: &LayoutCtx,
    table: &Table,
    cells: &[TableCell],
    col_widths: &[f32],
    row_area: Rect,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> Vec<RenderNode> {
    let cell_padding = table.cell_padding;
    let mut nodes = Vec::with_capacity(cells.len());
    let mut cursor_x = row_area.x;
    let mut col_idx = 0;
    for cell in cells {
        if col_idx >= table.columns.len() {
            push_warning(
                warnings,
                LayoutWarningKind::TableRowOverflow,
                page,
                format!(
                    "table row has {} cell(s), table has {} column(s) — extra cells dropped",
                    cells.len(),
                    table.columns.len()
                ),
            );
            break;
        }
        let span = cell.colspan.max(1);
        let end_idx = (col_idx + span).min(table.columns.len());
        let total_w: f32 = col_widths[col_idx..end_idx].iter().sum();
        let col_align = cell.align.unwrap_or(table.columns[col_idx].align);
        let padding = cell.padding.unwrap_or(cell_padding);

        let inner_w = (total_w - 2.0 * padding).max(0.0);
        let content_h = (row_area.height - 2.0 * padding).max(0.0);
        let cell_size = measure_at_width(ctx, &cell.element, inner_w);
        let box_width = cell_size.width.min(inner_w).max(0.0);
        let x_offset = align_offset(col_align, inner_w, box_width);
        if cell.background.is_some() || cell.border.is_some() {
            nodes.push(RenderNode::Rect {
                area: Rect {
                    x: cursor_x,
                    y: row_area.y,
                    width: total_w,
                    height: row_area.height,
                },
                background: cell.background,
                border: cell.border,
                corner_radius: 0.0,
            });
        }
        let cell_area = Rect {
            x: cursor_x + padding + x_offset,
            y: row_area.y + padding,
            width: box_width,
            height: content_h,
        };
        match cell.element.layout(ctx, cell_area, warnings, page) {
            LayoutResult::Fit(node) => nodes.push(node),
            LayoutResult::Split { current, .. } => {
                nodes.push(current);
            }
        }
        cursor_x += total_w;
        col_idx = end_idx;
    }
    nodes
}

/// The per-row data that varies across `render_row` call sites (header vs.
/// data row vs. forced/oversized row), grouped so the function itself
/// doesn't need one positional `f32`/`Option<Color>` parameter per field.
struct RowRenderParams<'a> {
    cells: &'a [TableCell],
    y: f32,
    row_height: f32,
    background: Option<Color>,
}

fn render_row(
    ctx: &LayoutCtx,
    table: &Table,
    col_widths: &[f32],
    inner: &Rect,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
    row: RowRenderParams,
) -> RenderNode {
    let row_area = Rect {
        x: inner.x,
        y: inner.y + row.y,
        width: inner.width,
        height: row.row_height,
    };
    let nodes = layout_row_cells(ctx, table, row.cells, col_widths, row_area, warnings, page);
    RenderNode::Group {
        area: row_area,
        clip: true,
        background: row.background,
        border: None,
        corner_radius: 0.0,
        children: nodes,
    }
}

// ---------------------------------------------------------------------
// rowspan: body rows only (the header repeats per page independently and
// stays on the simple per-row path above — a rowspan cell "hanging over"
// into a repeated header makes no sense). A `rowspan` cell blocks its
// column(s) for the rows below it; a row with no free cell for a given
// column simply omits a `TableCell` there (same convention as HTML
// `<tr>`/`<td>`). The page-break rule (`plan/02-elementcatalog-and-features.md`
// discussion for #14): a rowspan may never be split across a page boundary
// — the whole "block" of rows it covers moves to the next page together.
// ---------------------------------------------------------------------

/// One cell's resolved grid position, computed once per `Table::layout`
/// call by [`plan_grid`] and reused for both row-height accounting and
/// rendering.
struct CellPlacement<'a> {
    row: usize,
    col_start: usize,
    col_end: usize,
    cell: &'a TableCell,
}

/// Simulates column occupancy row by row: a cell with `rowspan > 1`
/// placed at row `r` blocks its column(s) through row `r + rowspan - 1`.
/// Returns every cell's resolved `(row, col_start, col_end)` and, per row,
/// whether it's a "continuation" row (some column still blocked by an
/// earlier row's `rowspan`) — a continuation row is never a legal
/// page-break point on its own; see [`atomic_blocks`].
fn plan_grid<'a>(
    rows: &'a [Vec<TableCell>],
    num_columns: usize,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> (Vec<CellPlacement<'a>>, Vec<bool>) {
    // blocked_until[col] = column `col` is occupied by an earlier row's
    // rowspan for every row index strictly less than this value.
    let mut blocked_until = vec![0usize; num_columns];
    let mut placements = Vec::new();
    let mut is_continuation = Vec::with_capacity(rows.len());

    for (row_idx, row) in rows.iter().enumerate() {
        is_continuation.push((0..num_columns).any(|c| blocked_until[c] > row_idx));

        let mut col_idx = 0;
        for cell in row {
            while col_idx < num_columns && blocked_until[col_idx] > row_idx {
                col_idx += 1;
            }
            if col_idx >= num_columns {
                push_warning(
                    warnings,
                    LayoutWarningKind::TableRowOverflow,
                    page,
                    format!("table row {row_idx} has more cells than the table has free column(s) after rowspan — extra cells dropped"),
                );
                break;
            }
            let col_end = (col_idx + cell.colspan.max(1)).min(num_columns);
            let rowspan = cell.rowspan.max(1);
            if rowspan > 1 {
                for slot in blocked_until[col_idx..col_end].iter_mut() {
                    *slot = row_idx + rowspan;
                }
            }
            placements.push(CellPlacement {
                row: row_idx,
                col_start: col_idx,
                col_end,
                cell,
            });
            col_idx = col_end;
        }
    }
    (placements, is_continuation)
}

/// Each row's natural height from its own (non-`rowspan`-owning) cells —
/// `rowspan` cells are handled separately by [`apply_rowspan_deficits`],
/// since their content can spread across several rows.
fn natural_row_heights(ctx: &LayoutCtx, placements: &[CellPlacement], num_rows: usize, col_widths: &[f32], cell_padding: f32) -> Vec<f32> {
    let mut heights = vec![0.0f32; num_rows];
    for p in placements {
        if p.cell.rowspan.max(1) > 1 {
            continue;
        }
        let padding = p.cell.padding.unwrap_or(cell_padding);
        let inner_w = (col_widths[p.col_start..p.col_end].iter().sum::<f32>() - 2.0 * padding).max(0.0);
        let h = measure_at_width(ctx, &p.cell.element, inner_w).height + 2.0 * padding;
        heights[p.row] = heights[p.row].max(h);
    }
    heights
}

/// If a `rowspan` cell's own content needs more height than the rows it
/// spans already provide, the shortfall is added to the *last* spanned
/// row — simple, predictable, and keeps every other row's height
/// undisturbed by a tall spanning cell.
fn apply_rowspan_deficits(ctx: &LayoutCtx, placements: &[CellPlacement], heights: &mut [f32], col_widths: &[f32], cell_padding: f32) {
    for p in placements {
        let span = p.cell.rowspan.max(1);
        if span <= 1 {
            continue;
        }
        let padding = p.cell.padding.unwrap_or(cell_padding);
        let inner_w = (col_widths[p.col_start..p.col_end].iter().sum::<f32>() - 2.0 * padding).max(0.0);
        let needed = measure_at_width(ctx, &p.cell.element, inner_w).height + 2.0 * padding;
        let end_row = (p.row + span).min(heights.len());
        let available: f32 = heights[p.row..end_row].iter().sum();
        if needed > available {
            if let Some(last) = heights[p.row..end_row].last_mut() {
                *last += needed - available;
            }
        }
    }
}

/// Groups row indices into maximal runs where every row after the first
/// is a continuation row of the one before it — the atomic units the
/// page-break loop in `Table::layout` treats as indivisible.
fn atomic_blocks(is_continuation: &[bool]) -> Vec<std::ops::Range<usize>> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < is_continuation.len() {
        let start = i;
        i += 1;
        while i < is_continuation.len() && is_continuation[i] {
            i += 1;
        }
        blocks.push(start..i);
    }
    blocks
}

/// Reduces a forced block's row heights to fit `budget`: rows keep their
/// natural height from the top until the budget runs out, everything
/// after collapses to zero — "clip the tail," the same read-order
/// priority a single oversized row already got before `rowspan` existed.
fn shrink_row_heights_to_fit(natural: &[f32], budget: f32) -> Vec<f32> {
    let mut remaining_budget = budget.max(0.0);
    natural
        .iter()
        .map(|&h| {
            let take = h.min(remaining_budget);
            remaining_budget -= take;
            take
        })
        .collect()
}

/// Everything a block's rendering needs that stays the same across every
/// cell/row in it — bundled so `render_cells_starting_at`/`render_block`
/// don't grow past clippy's argument-count limit.
struct TableRenderCtx<'a> {
    ctx: &'a LayoutCtx<'a>,
    table: &'a Table,
    col_widths: &'a [f32],
    placements: &'a [CellPlacement<'a>],
    page: usize,
}

/// Lays out every cell placed at `row_idx`, using `block_heights[local_row_idx..]`
/// for cells that reach beyond this single row (`rowspan > 1`) —
/// `block_heights` is local to the block currently being rendered
/// (already resolved to either natural or forced-page-clipped values by
/// the caller), not the table-wide array.
fn render_cells_starting_at(
    trc: &TableRenderCtx,
    row_area: &Rect,
    warnings: &mut Vec<LayoutWarning>,
    row_idx: usize,
    local_row_idx: usize,
    block_heights: &[f32],
) -> Vec<RenderNode> {
    let cell_padding = trc.table.cell_padding;
    let mut nodes = Vec::new();
    for p in trc.placements.iter().filter(|p| p.row == row_idx) {
        let span = p.cell.rowspan.max(1);
        let local_end = (local_row_idx + span).min(block_heights.len());
        let cell_h: f32 = block_heights[local_row_idx..local_end].iter().sum();
        let total_w: f32 = trc.col_widths[p.col_start..p.col_end].iter().sum();
        let col_align = p.cell.align.unwrap_or(trc.table.columns[p.col_start].align);
        let cursor_x = row_area.x + trc.col_widths[..p.col_start].iter().sum::<f32>();
        let padding = p.cell.padding.unwrap_or(cell_padding);

        let cell_box = Rect {
            x: cursor_x,
            y: row_area.y,
            width: total_w,
            height: cell_h,
        };
        // A cell's own background/border paints over its *whole* box
        // (not just the padded content area), and — precedence: cell
        // beats row beats column — over whatever the row's zebra stripe
        // already painted underneath it.
        if p.cell.background.is_some() || p.cell.border.is_some() {
            nodes.push(RenderNode::Rect {
                area: cell_box,
                background: p.cell.background,
                border: p.cell.border,
                corner_radius: 0.0,
            });
        }

        let inner_w = (total_w - 2.0 * padding).max(0.0);
        let content_h = (cell_h - 2.0 * padding).max(0.0);
        let cell_size = measure_at_width(trc.ctx, &p.cell.element, inner_w);
        let box_width = cell_size.width.min(inner_w).max(0.0);
        let x_offset = align_offset(col_align, inner_w, box_width);
        let cell_area = Rect {
            x: cursor_x + padding + x_offset,
            y: row_area.y + padding,
            width: box_width,
            height: content_h,
        };
        match p.cell.element.layout(trc.ctx, cell_area, warnings, trc.page) {
            LayoutResult::Fit(node) => nodes.push(node),
            LayoutResult::Split { current, .. } => nodes.push(current),
        }
    }
    nodes
}

/// Renders one atomic block (`block.len() == 1` for the overwhelmingly
/// common non-`rowspan` case — identical `RenderNode` shape to
/// `render_row` above, so existing single-row tests/output are
/// unaffected).
///
/// A `block.len() > 1` block wraps every row in one non-striped, clipping
/// outer `Group` (its area spans the whole block, so a `rowspan` cell
/// drawn from an earlier row is never clipped by a row boundary it
/// extends past) with each row's own stripe painted as a plain
/// background `Rect` child instead of the `Group.background` field.
fn render_block(
    trc: &TableRenderCtx,
    inner: &Rect,
    warnings: &mut Vec<LayoutWarning>,
    block: std::ops::Range<usize>,
    block_heights: &[f32], // one entry per row in `block`, in order
    block_top_y: f32,
    row_backgrounds: &[Option<Color>], // indexed by absolute row index
) -> RenderNode {
    if block.len() == 1 {
        let row_idx = block.start;
        let row_area = Rect {
            x: inner.x,
            y: inner.y + block_top_y,
            width: inner.width,
            height: block_heights[0],
        };
        let nodes = render_cells_starting_at(trc, &row_area, warnings, row_idx, 0, block_heights);
        return RenderNode::Group {
            area: row_area,
            clip: true,
            background: row_backgrounds[row_idx],
            border: None,
            corner_radius: 0.0,
            children: nodes,
        };
    }

    let block_area = Rect {
        x: inner.x,
        y: inner.y + block_top_y,
        width: inner.width,
        height: block_heights.iter().sum(),
    };
    let mut children = Vec::new();
    let mut cursor_y = block_top_y;
    for (local_idx, row_idx) in block.clone().enumerate() {
        let row_h = block_heights[local_idx];
        let row_area = Rect {
            x: inner.x,
            y: inner.y + cursor_y,
            width: inner.width,
            height: row_h,
        };
        if let Some(bg) = row_backgrounds[row_idx] {
            children.push(RenderNode::Rect {
                area: row_area,
                background: Some(bg),
                border: None,
                corner_radius: 0.0,
            });
        }
        children.extend(render_cells_starting_at(
            trc,
            &row_area,
            warnings,
            row_idx,
            local_idx,
            block_heights,
        ));
        cursor_y += row_h;
    }
    RenderNode::Group {
        area: block_area,
        clip: true,
        background: None,
        border: None,
        corner_radius: 0.0,
        children,
    }
}

impl Layoutable for Table {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        let (width, inner_width) = resolve_bound(self.common.width, constraints.max_width, self.common.padding);
        let col_widths = resolve_column_widths(&self.columns, inner_width);
        let mut total = header_row_height(ctx, self, &col_widths);
        for row in &self.rows {
            total += measure_row_height(ctx, row, &col_widths, self.cell_padding);
        }
        Size {
            width,
            height: resolve_auto_size(self.common.height, total, self.common.padding),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        let (inner, bound_height) = shrink_and_bound_height(area, self.common.height, self.common.padding);
        let col_widths = resolve_column_widths(&self.columns, inner.width);
        let header_height = header_row_height(ctx, self, &col_widths);

        let mut rendered = Vec::new();
        let mut cursor_y = 0.0f32;

        if let Some(header) = &self.header {
            rendered.push(render_row(
                ctx,
                self,
                &col_widths,
                &inner,
                warnings,
                page,
                RowRenderParams {
                    cells: header,
                    y: cursor_y,
                    row_height: header_height,
                    background: None,
                },
            ));
            cursor_y += header_height;
        }

        let (placements, is_continuation) = plan_grid(&self.rows, self.columns.len(), warnings, page);
        let mut row_heights = natural_row_heights(ctx, &placements, self.rows.len(), &col_widths, self.cell_padding);
        apply_rowspan_deficits(ctx, &placements, &mut row_heights, &col_widths, self.cell_padding);
        let row_backgrounds: Vec<Option<Color>> = (0..self.rows.len())
            .map(|i| self.striped.filter(|_| (self.row_offset + i) % 2 == 1))
            .collect();
        let trc = TableRenderCtx {
            ctx,
            table: self,
            col_widths: &col_widths,
            placements: &placements,
            page,
        };

        for block in atomic_blocks(&is_continuation) {
            let block_height: f32 = row_heights[block.clone()].iter().sum();
            let remaining = (bound_height - cursor_y).max(0.0);

            if block_height <= remaining + EPS {
                rendered.push(render_block(
                    &trc,
                    &inner,
                    warnings,
                    block.clone(),
                    &row_heights[block.clone()],
                    cursor_y,
                    &row_backgrounds,
                ));
                cursor_y += block_height;
                continue;
            }

            if cursor_y <= header_height + EPS {
                // Only the header (or nothing) placed so far: this block
                // is atomic (a rowspan may never split across a page,
                // #14) and doesn't fit even a fresh page — force it,
                // clip from the bottom up, warn (Grundprinzip 7), then
                // move on.
                let forced_heights = shrink_row_heights_to_fit(&row_heights[block.clone()], remaining);
                rendered.push(render_block(
                    &trc,
                    &inner,
                    warnings,
                    block.clone(),
                    &forced_heights,
                    cursor_y,
                    &row_backgrounds,
                ));
                let start = self.row_offset + block.start;
                let end = self.row_offset + block.end - 1;
                let hint = if start == end {
                    format!("Table row {start} larger than one page")
                } else {
                    format!("Table rows {start}-{end} (rowspan) larger than one page")
                };
                push_warning(warnings, LayoutWarningKind::ForcedPageBreak, page, hint);
                cursor_y = bound_height;
                continue;
            }

            // Doesn't fit — move this block (and everything after it) to
            // a continuation page, which repeats the header. Never split
            // mid-block: that's exactly the invariant `atomic_blocks`
            // exists to protect.
            if let Some(fixed_height) = self.common.height {
                let overflow_hint = (!self.rows[block.start..].is_empty()).then_some("Table content exceeds its fixed height");
                return clip_to_fixed_height(area, fixed_height, &self.common, rendered, warnings, page, overflow_hint);
            }

            let remainder = Table {
                columns: self.columns.clone(),
                header: self.header.clone(),
                rows: self.rows[block.start..].to_vec(),
                striped: self.striped,
                cell_padding: self.cell_padding,
                row_offset: self.row_offset + block.start,
                common: Common {
                    height: None,
                    ..self.common
                },
            };
            let current = wrap_children(area, cursor_y, &self.common, rendered);
            return LayoutResult::Split {
                current,
                remainder: Element::Table(remainder),
            };
        }

        finish_fit(&self.common, area, cursor_y, rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warnings::LayoutWarningKind;
    use lightweight_pdf_core::{Align, Element, Text as TextEl};

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
    impl crate::font_resolver::FontResolver for FixedResolver {
        fn metrics(&self, _key: lightweight_pdf_core::FontKey) -> &dyn crate::font_resolver::FontMetrics {
            &FixedMetrics
        }
    }
    fn ctx() -> LayoutCtx<'static> {
        LayoutCtx::new(&FixedResolver)
    }

    fn row(cells: &[&str]) -> Vec<Element> {
        cells.iter().map(|c| Element::Text(TextEl::new(*c))).collect()
    }

    #[test]
    fn column_widths_sum_exactly_to_available_width() {
        let columns = vec![
            TableColumn::flex(1.0),
            TableColumn::fixed(37.3),
            TableColumn::flex(2.0),
            TableColumn::fixed(19.9),
        ];
        let widths = resolve_column_widths(&columns, 400.0);
        let sum: f32 = widths.iter().sum();
        assert!((sum - 400.0).abs() < 1e-3, "widths must sum exactly to available width, got {sum}");
        assert_eq!(widths[1], 37.3);
        assert_eq!(widths[3], 19.9);
    }

    #[test]
    fn header_repeats_and_all_rows_survive_a_page_split() {
        let table = Table::new()
            .columns([TableColumn::flex(1.0)])
            .header(["Beschreibung"])
            .rows((0..20).map(|i| row(&[Box::leak(format!("Zeile {i}").into_boxed_str())])));
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 60.0, // room for header + a couple of rows only
        };
        let mut pages = Vec::new();
        let mut current = Element::Table(table);
        loop {
            match current.layout(&c, area, &mut warnings, pages.len() + 1) {
                LayoutResult::Fit(node) => {
                    pages.push(node);
                    break;
                }
                LayoutResult::Split { current: node, remainder } => {
                    pages.push(node);
                    current = remainder;
                }
            }
            if pages.len() > 100 {
                panic!("pagination did not terminate");
            }
        }
        assert!(pages.len() > 1, "expected the table to span multiple pages");

        // Every page (after the first) must repeat the header as its
        // first row, and every original data row must appear exactly
        // once across all pages, in order.
        let mut seen_rows = Vec::new();
        for page in &pages {
            let RenderNode::Group { children, .. } = page else {
                panic!("expected a Group");
            };
            assert!(!children.is_empty(), "every page must render at least the header");
            for row_node in children {
                let RenderNode::Group { children: cells, .. } = row_node else {
                    panic!("expected row Group");
                };
                let RenderNode::Group { children: text_wrap, .. } = &cells[0] else {
                    panic!("expected clipped text wrapper");
                };
                let RenderNode::TextLines { lines, .. } = &text_wrap[0] else {
                    panic!("expected TextLines");
                };
                seen_rows.push(lines.join(" "));
            }
        }
        let header_count = seen_rows.iter().filter(|s| *s == "Beschreibung").count();
        assert_eq!(header_count, pages.len(), "header must repeat on every page exactly once");
        let data_rows: Vec<_> = seen_rows.iter().filter(|s| *s != "Beschreibung").collect();
        assert_eq!(data_rows.len(), 20, "no row may be lost or duplicated across the split");
        for (i, row) in data_rows.iter().enumerate() {
            assert_eq!(*row, &format!("Zeile {i}"), "rows must stay in order");
        }
    }

    #[test]
    fn cell_hard_breaks_a_token_wider_than_the_column() {
        let table = Table::new().columns([TableColumn::fixed(30.0)]).rows([row(&["ABCDEFGHIJ"])]); // 10 chars * 6pt = 60pt, column inner width ~22pt
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 30.0,
            height: 200.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        let RenderNode::Group { children: cells, .. } = &children[0] else {
            panic!("expected row group");
        };
        let RenderNode::Group { children: text_wrap, .. } = &cells[0] else {
            panic!("expected clipped text wrapper");
        };
        let RenderNode::TextLines { lines, .. } = &text_wrap[0] else {
            panic!("expected TextLines");
        };
        assert!(lines.len() > 1, "a token wider than the column must hard-break onto multiple lines");
    }

    #[test]
    fn row_height_grows_with_tallest_cell_without_moving_other_rows() {
        let table = Table::new().columns([TableColumn::fixed(30.0), TableColumn::fixed(30.0)]).rows([
            row(&["kurz", "kurz"]),
            row(&["ein sehr sehr sehr sehr langer Zellinhalt der umbricht", "kurz"]),
            row(&["kurz", "kurz"]),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 400.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: rows, .. }) = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        assert_eq!(rows.len(), 3);
        let heights: Vec<f32> = rows
            .iter()
            .map(|r| match r {
                RenderNode::Group { area, .. } => area.height,
                _ => panic!("expected Group"),
            })
            .collect();
        assert!(heights[1] > heights[0], "the row with more content must be taller");
        assert_eq!(heights[0], heights[2], "unrelated rows keep their own (equal) height");

        // Rows must not overlap vertically: each row's y must be >= the
        // previous row's y + height.
        let ys: Vec<f32> = rows
            .iter()
            .map(|r| match r {
                RenderNode::Group { area, .. } => area.y,
                _ => unreachable!(),
            })
            .collect();
        assert!(ys[1] >= ys[0] + heights[0] - EPS);
        assert!(ys[2] >= ys[1] + heights[1] - EPS);
    }

    #[test]
    fn striped_alternates_and_survives_a_split() {
        let table = Table::new()
            .columns([TableColumn::flex(1.0)])
            .header(["H"])
            .striped(Color::rgb(240, 240, 240))
            .rows((0..6).map(|i| row(&[Box::leak(format!("R{i}").into_boxed_str())])));
        let c = ctx();
        let mut warnings = Vec::new();
        // Force a split after the header + 2 rows (each ~22.4pt: 14.4pt
        // line height + 2*4pt cell padding).
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 3.5 * 22.4,
        };
        let LayoutResult::Split { remainder, .. } = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected a Split");
        };
        let Element::Table(remainder_table) = remainder else {
            panic!("expected Table remainder");
        };
        // Row 2 (0-indexed) is the first row on the continuation page;
        // row_offset must reflect its true absolute index so striping
        // continues correctly instead of resetting.
        assert_eq!(remainder_table.row_offset, 2);
    }

    #[test]
    fn oversized_row_forces_its_own_page() {
        let table = Table::new()
            .columns([TableColumn::flex(1.0)])
            .rows([row(&["normal"]), row(&["a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np"])]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let mut pages = 0;
        let mut current = Element::Table(table);
        loop {
            match current.layout(&c, area, &mut warnings, pages + 1) {
                LayoutResult::Fit(_) => {
                    pages += 1;
                    break;
                }
                LayoutResult::Split { remainder, .. } => {
                    pages += 1;
                    current = remainder;
                }
            }
            if pages > 50 {
                panic!("pagination did not terminate");
            }
        }
        assert!(pages >= 2, "the oversized row should push onto its own page");
        assert!(warnings.iter().any(|w| w.kind == LayoutWarningKind::ForcedPageBreak));
    }

    #[test]
    fn table_column_align_positions_short_content_in_the_column() {
        let table = Table::new()
            .columns([TableColumn::fixed(100.0).align(Align::End)])
            .rows([row(&["42"])]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: rows, .. }) = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        let RenderNode::Group { children: cells, .. } = &rows[0] else {
            panic!("expected row group");
        };
        let RenderNode::Group { area: cell_area, .. } = &cells[0] else {
            panic!("expected clipped cell wrapper");
        };
        // "42" is much narrower than the 100pt column; End-align must
        // push it toward the right edge, not leave it at x=0.
        assert!(cell_area.x > 50.0, "expected right-aligned cell, got x={}", cell_area.x);
    }

    fn cell(text: &str) -> TableCell {
        TableCell::from(text)
    }

    #[test]
    fn rowspan_cell_spans_multiple_rows_as_one_atomic_block() {
        let table = Table::new()
            .columns([TableColumn::fixed(30.0), TableColumn::fixed(30.0)])
            .rows(vec![
                vec![TableCell::new("Summe").rowspan(2), cell("Zeile 1")],
                // Column 0 is covered by the rowspan cell above — this row
                // supplies only its own (column 1) cell, same convention as
                // HTML <tr>/<td>.
                vec![cell("Zeile 2")],
            ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 400.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: blocks, .. }) = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        // Both rows must merge into one atomic block, not two separate
        // row groups.
        assert_eq!(
            blocks.len(),
            1,
            "expected the 2-row span to render as a single block, got {} top-level children",
            blocks.len()
        );
        let RenderNode::Group {
            area: block_area,
            children: cells,
            ..
        } = &blocks[0]
        else {
            panic!("expected block group");
        };
        // 3 rendered cells total: "Summe" (once, spanning both rows),
        // "Zeile 1", "Zeile 2".
        assert_eq!(cells.len(), 3, "expected 3 rendered cells, got {}", cells.len());
        // The block covers both rows' height (single-row height is
        // 22.4pt, same constant as `striped_alternates_and_survives_a_split`).
        assert!(
            block_area.height > 30.0,
            "block should cover both rows, got height {}",
            block_area.height
        );
    }

    #[test]
    fn rowspan_never_splits_across_a_page_boundary() {
        let mut rows: Vec<Vec<TableCell>> = (0..3).map(|i| vec![cell(&format!("R{i}")), cell("x")]).collect();
        rows.push(vec![TableCell::new("Spanned").rowspan(2), cell("y0")]);
        rows.push(vec![cell("y1")]);
        let table = Table::new()
            .columns([TableColumn::fixed(30.0), TableColumn::fixed(30.0)])
            .rows(rows);
        let c = ctx();
        let mut warnings = Vec::new();
        // Single-row height is 22.4pt (14.4pt line height + 2*4pt cell
        // padding, same constant as `striped_alternates_and_survives_a_split`).
        // 4 rows' worth of budget lands the natural split right in the
        // middle of the 2-row span if it weren't treated as atomic.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 4.0 * 22.4,
        };
        let LayoutResult::Split { current, remainder } = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected a Split");
        };
        let RenderNode::Group { children, .. } = current else {
            panic!("expected Group");
        };
        assert_eq!(
            children.len(),
            3,
            "the rowspan block must not partially fit onto the current page, got {} rows",
            children.len()
        );
        let Element::Table(remainder_table) = remainder else {
            panic!("expected Table remainder");
        };
        assert_eq!(
            remainder_table.rows.len(),
            2,
            "the whole 2-row span must move to the continuation page together"
        );
        assert_eq!(remainder_table.row_offset, 3);
    }

    #[test]
    fn continuation_row_with_too_many_cells_still_reports_overflow() {
        let table = Table::new()
            .columns([TableColumn::fixed(30.0), TableColumn::fixed(30.0)])
            .rows(vec![
                vec![TableCell::new("Spanned").rowspan(2), cell("a")],
                // Column 0 is blocked by the rowspan above (only column 1 is
                // free); this row wrongly supplies 2 cells anyway.
                vec![cell("b"), cell("c")],
            ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 400.0,
        };
        let _ = Element::Table(table).layout(&c, area, &mut warnings, 1);
        assert!(warnings.iter().any(|w| w.kind == LayoutWarningKind::TableRowOverflow));
    }

    #[test]
    fn cell_background_overrides_the_row_stripe() {
        let stripe = Color::rgb(240, 240, 240);
        let cell_bg = Color::rgb(255, 0, 0);
        let table = Table::new()
            .columns([TableColumn::fixed(30.0), TableColumn::fixed(30.0)])
            .striped(stripe)
            .rows(vec![
                vec![cell("a"), cell("b")],
                vec![TableCell::new("c").background(cell_bg), cell("d")], // striped row (index 1)
            ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 400.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: blocks, .. }) = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        let RenderNode::Group {
            background: row_bg,
            children: cells,
            ..
        } = &blocks[1]
        else {
            panic!("expected row group");
        };
        // The row's own Group.background still carries the stripe...
        assert_eq!(*row_bg, Some(stripe));
        // ...but the styled cell paints its own Rect on top of it, with
        // its own color, not the stripe's.
        let has_cell_rect = cells
            .iter()
            .any(|n| matches!(n, RenderNode::Rect { background: Some(bg), .. } if *bg == cell_bg));
        assert!(
            has_cell_rect,
            "expected a cell-level background Rect overriding the stripe, got: {cells:?}"
        );
    }

    #[test]
    fn cell_padding_overrides_the_table_default() {
        let table = Table::new()
            .columns([TableColumn::fixed(60.0)])
            .cell_padding(4.0)
            .rows(vec![vec![TableCell::new("x").padding(20.0)]]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 400.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: blocks, .. }) = Element::Table(table).layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        // Row height must reflect the cell's own (larger) padding, not
        // the table's default 4.0 — content height (14.4pt line) + 2*20pt.
        let RenderNode::Group { area: row_area, .. } = &blocks[0] else {
            panic!("expected row group");
        };
        assert!(
            row_area.height > 14.4 + 2.0 * 20.0 - EPS,
            "expected the cell's own padding to grow the row, got height {}",
            row_area.height
        );
    }
}
