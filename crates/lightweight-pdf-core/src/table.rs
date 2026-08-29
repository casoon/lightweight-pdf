//! `Table` element (Phase 3, `plan/phases/phase-3-tables.md`): the element
//! that matters most for invoices. Cells are plain `Element`s so they reuse
//! the exact same `Layoutable`/text-wrap machinery as everything else —
//! no separate cell-content model.

use crate::element::Element;
use crate::style::{Align, Border, Color, Common};

/// A column's width: `fixed(w)` reserves an exact width, `flex(weight)`
/// shares the leftover space proportionally (taffy `flex-grow` analogy,
/// ADR-004 / `03-builder-api-design.md`) — the same distribution step as
/// `Row`, not a generic flex implementation.
#[derive(Clone, Copy, Debug)]
pub enum ColumnWidth {
    Fixed(f32),
    Flex(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct TableColumn {
    pub width: ColumnWidth,
    pub align: Align,
}

impl TableColumn {
    pub fn fixed(width: f32) -> Self {
        TableColumn {
            width: ColumnWidth::Fixed(width),
            align: Align::Start,
        }
    }

    pub fn flex(weight: f32) -> Self {
        TableColumn {
            width: ColumnWidth::Flex(weight),
            align: Align::Start,
        }
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

#[derive(Clone, Debug)]
pub struct TableCell {
    pub element: Element,
    pub colspan: usize,
    /// How many rows (including this one) this cell's box extends down
    /// through. A continuation row (one a `rowspan > 1` cell from an
    /// earlier row still covers) simply omits a `TableCell` for that
    /// column — same convention as HTML `<tr>`/`<td>`, not a separate
    /// "placeholder" cell type.
    pub rowspan: usize,
    pub align: Option<Align>,
    /// Overrides the row's zebra stripe for this cell only (precedence:
    /// cell beats row beats column — the same order `.align()` already
    /// follows against `TableColumn::align`).
    pub background: Option<Color>,
    pub border: Option<Border>,
    /// Overrides `Table::cell_padding` for this cell only.
    pub padding: Option<f32>,
}

impl TableCell {
    pub fn new(element: impl Into<Element>) -> Self {
        TableCell {
            element: element.into(),
            colspan: 1,
            rowspan: 1,
            align: None,
            background: None,
            border: None,
            padding: None,
        }
    }

    pub fn colspan(mut self, colspan: usize) -> Self {
        self.colspan = colspan.max(1);
        self
    }

    pub fn rowspan(mut self, rowspan: usize) -> Self {
        self.rowspan = rowspan.max(1);
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = Some(align);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = Some(padding);
        self
    }
}

impl<T: Into<Element>> From<T> for TableCell {
    fn from(value: T) -> Self {
        TableCell::new(value)
    }
}

/// Implemented by a domain type (an invoice line item, a report row, ...)
/// that knows how to render itself as one table row — lets callers write
/// `Table::new()...from_rows(&items)` instead of hand-building
/// `vec![vec![Element::from(..), ...]]` per row, where the column order is
/// invisible at the call site and only checked at runtime. The plain
/// `.rows(vec![vec![..]])` form stays available for ad hoc tables.
pub trait TableRow {
    fn cells(&self) -> Vec<TableCell>;
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub columns: Vec<TableColumn>,
    pub header: Option<Vec<TableCell>>,
    pub rows: Vec<Vec<TableCell>>,
    /// Alternating row background ("Zebra-Streifen"), see
    /// `02-elementcatalog-and-features.md`. Applies to data rows only (a
    /// striped header would be indistinguishable from a striped data row).
    pub striped: Option<Color>,
    /// Inner spacing on every side of each cell's content, same default
    /// (4pt) header and data rows.
    pub cell_padding: f32,
    /// Absolute index of `rows[0]` within the *original*, unsplit table —
    /// 0 unless this `Table` is itself the remainder produced by a
    /// previous page's `LayoutResult::Split`. Not part of the public
    /// builder surface; exists purely so `.striped()` keeps alternating
    /// correctly across a page break instead of resetting per page.
    pub row_offset: usize,
    pub common: Common,
}

impl Table {
    pub fn new() -> Self {
        Table {
            cell_padding: 4.0,
            ..Default::default()
        }
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = TableColumn>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    pub fn header(mut self, cells: impl IntoIterator<Item = impl Into<TableCell>>) -> Self {
        self.header = Some(cells.into_iter().map(Into::into).collect());
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = impl IntoIterator<Item = impl Into<TableCell>>>) -> Self {
        self.rows = rows.into_iter().map(|row| row.into_iter().map(Into::into).collect()).collect();
        self
    }

    /// `.rows(..)` for anything implementing `TableRow` — one row per item,
    /// in order.
    pub fn from_rows<T: TableRow>(mut self, items: &[T]) -> Self {
        self.rows = items.iter().map(TableRow::cells).collect();
        self
    }

    pub fn striped(mut self, color: Color) -> Self {
        self.striped = Some(color);
        self
    }

    pub fn cell_padding(mut self, padding: f32) -> Self {
        self.cell_padding = padding;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.common.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.common.height = Some(height);
        self
    }

    pub fn flex(mut self, factor: f32) -> Self {
        self.common.flex = Some(factor);
        self
    }

    pub fn keep_with_next(mut self) -> Self {
        self.common.keep_with_next = true;
        self
    }
}
