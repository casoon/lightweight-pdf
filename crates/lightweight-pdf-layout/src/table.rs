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
use lightweight_pdf_core::{Color, ColumnWidth, Common, Element, Table, TableColumn};

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

fn measure_row_height(ctx: &LayoutCtx, cells: &[Element], col_widths: &[f32], cell_padding: f32) -> f32 {
    cells
        .iter()
        .zip(col_widths.iter())
        .map(|(cell, w)| {
            let inner_w = (w - 2.0 * cell_padding).max(0.0);
            measure_at_width(ctx, cell, inner_w).height + 2.0 * cell_padding
        })
        .fold(0.0f32, f32::max)
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
    cells: &[Element],
    col_widths: &[f32],
    row_area: Rect,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
) -> Vec<RenderNode> {
    let cell_padding = table.cell_padding;
    let mut nodes = Vec::with_capacity(cells.len());
    let mut cursor_x = row_area.x;
    for ((cell, col), w) in cells.iter().zip(table.columns.iter()).zip(col_widths.iter()) {
        let inner_w = (w - 2.0 * cell_padding).max(0.0);
        let content_h = (row_area.height - 2.0 * cell_padding).max(0.0);
        let cell_size = measure_at_width(ctx, cell, inner_w);
        let box_width = cell_size.width.min(inner_w).max(0.0);
        let x_offset = align_offset(col.align, inner_w, box_width);
        let cell_area = Rect {
            x: cursor_x + cell_padding + x_offset,
            y: row_area.y + cell_padding,
            width: box_width,
            height: content_h,
        };
        match cell.layout(ctx, cell_area, warnings, page) {
            LayoutResult::Fit(node) => nodes.push(node),
            LayoutResult::Split { current, .. } => {
                // Cells never split (a row is an atomic unit,
                // Grundprinzip 5's table addendum) — keep what fit,
                // ContentOverflow already implied by TextClipped from the
                // cell's own fixed-size handling if applicable.
                nodes.push(current);
            }
        }
        cursor_x += *w;
    }
    nodes
}

/// The per-row data that varies across `render_row` call sites (header vs.
/// data row vs. forced/oversized row), grouped so the function itself
/// doesn't need one positional `f32`/`Option<Color>` parameter per field.
struct RowRenderParams<'a> {
    cells: &'a [Element],
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
        children: nodes,
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

        for (i, row) in self.rows.iter().enumerate() {
            let absolute_i = self.row_offset + i;
            let row_height = measure_row_height(ctx, row, &col_widths, self.cell_padding);
            let remaining = (bound_height - cursor_y).max(0.0);
            let stripe = self.striped.filter(|_| absolute_i % 2 == 1);

            if row_height <= remaining + EPS {
                rendered.push(render_row(
                    ctx,
                    self,
                    &col_widths,
                    &inner,
                    warnings,
                    page,
                    RowRenderParams {
                        cells: row,
                        y: cursor_y,
                        row_height,
                        background: stripe,
                    },
                ));
                cursor_y += row_height;
                continue;
            }

            if cursor_y <= header_height + EPS {
                // Only the header (or nothing) placed so far: this row is
                // atomic and doesn't fit even a fresh page — force it,
                // clip, warn (Grundprinzip 7), then move on.
                rendered.push(render_row(
                    ctx,
                    self,
                    &col_widths,
                    &inner,
                    warnings,
                    page,
                    RowRenderParams {
                        cells: row,
                        y: cursor_y,
                        row_height: remaining,
                        background: stripe,
                    },
                ));
                push_warning(
                    warnings,
                    LayoutWarningKind::ForcedPageBreak,
                    page,
                    format!("Table row {absolute_i} larger than one page"),
                );
                cursor_y = bound_height;
                continue;
            }

            // Doesn't fit — move this row and everything after it to a
            // continuation page, which repeats the header.
            if let Some(fixed_height) = self.common.height {
                let overflow_hint = (!self.rows[i..].is_empty()).then_some("Table content exceeds its fixed height");
                return clip_to_fixed_height(area, fixed_height, &self.common, rendered, warnings, page, overflow_hint);
            }

            let remainder = Table {
                columns: self.columns.clone(),
                header: self.header.clone(),
                rows: self.rows[i..].to_vec(),
                striped: self.striped,
                cell_padding: self.cell_padding,
                row_offset: absolute_i,
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
        LayoutCtx { resolver: &FixedResolver }
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
}
