//! Phase 3 DoD (`plan/phases/phase-3-tables.md`): the invoice table example
//! (Beschreibung/Menge/Preis/Gesamt, right-aligned price columns, striped
//! rows) renders correctly, and a table
//! longer than one page repeats its header without losing or duplicating
//! rows.

use lightweight_pdf_test_support as support;

use lightweight_pdf::*;

struct LineItem {
    description: &'static str,
    qty: u32,
    price: f32,
}

fn line_to_row(item: &LineItem) -> Vec<Element> {
    vec![
        Text::new(item.description).into(),
        Text::new(item.qty.to_string()).into(),
        Text::new(format!("{:.2}", item.price)).into(),
        Text::new(format!("{:.2}", item.qty as f32 * item.price)).into(),
    ]
}

#[test]
fn renders_the_invoice_table_example() {
    let items = [
        LineItem {
            description: "Beratung",
            qty: 3,
            price: 120.0,
        },
        LineItem {
            description: "Lizenz",
            qty: 1,
            price: 499.0,
        },
    ];

    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(
        Table::new()
            .columns([
                TableColumn::flex(1.0),
                TableColumn::fixed(60.0).align(Align::End),
                TableColumn::fixed(60.0).align(Align::End),
                TableColumn::fixed(60.0).align(Align::End),
            ])
            .header(["Beschreibung", "Menge", "Preis", "Gesamt"])
            .striped(Color::rgb(0xF5, 0xF5, 0xF5))
            .rows(items.iter().map(line_to_row)),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes).unwrap();
    assert!(text.contains("Beschreibung"), "missing header, got:\n{text}");
    assert!(text.contains("Beratung"));
    assert!(text.contains("Lizenz"));
    assert!(text.contains("499.00"));
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
}

impl TableRow for LineItem {
    fn cells(&self) -> Vec<TableCell> {
        line_to_row(self).into_iter().map(TableCell::from).collect()
    }
}

#[test]
fn from_rows_matches_the_equivalent_rows_call() {
    let items = [
        LineItem {
            description: "Beratung",
            qty: 3,
            price: 120.0,
        },
        LineItem {
            description: "Lizenz",
            qty: 1,
            price: 499.0,
        },
    ];

    let build = |table: Table| {
        let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
        doc.add(table);
        doc.render().expect("render should succeed")
    };

    let via_rows = build(
        Table::new()
            .columns([
                TableColumn::flex(1.0),
                TableColumn::fixed(60.0),
                TableColumn::fixed(60.0),
                TableColumn::fixed(60.0),
            ])
            .rows(items.iter().map(line_to_row)),
    );
    let via_from_rows = build(
        Table::new()
            .columns([
                TableColumn::flex(1.0),
                TableColumn::fixed(60.0),
                TableColumn::fixed(60.0),
                TableColumn::fixed(60.0),
            ])
            .from_rows(&items),
    );
    assert_eq!(
        via_rows, via_from_rows,
        "from_rows must produce the same table as the equivalent .rows() call"
    );
}

#[test]
fn row_with_more_cells_than_columns_reports_a_layout_warning() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0)])
            .rows(vec![vec![Element::from("A"), Element::from("B"), Element::from("C")]]),
    );

    let (_bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(
        warnings.iter().any(|w| w.kind == LayoutWarningKind::TableRowOverflow),
        "expected a TableRowOverflow warning, got: {warnings:?}"
    );
}

#[test]
fn rowspan_cell_renders_once_and_covering_row_omits_that_column() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(
        Table::new()
            .columns([TableColumn::fixed(140.0), TableColumn::fixed(140.0)])
            .header(["Position", "Betrag"])
            .rows(vec![
                vec![TableCell::new("Gesamtsumme").rowspan(2), TableCell::from("Netto: 100,00 €")],
                vec![TableCell::from("Brutto: 119,00 €")],
            ]),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("Gesamtsumme"), "missing spanning cell text, got:\n{extracted}");
    assert!(extracted.contains("Netto: 100,00 €"));
    assert!(extracted.contains("Brutto: 119,00 €"));
    // The spanning cell's text must appear exactly once, not once per row.
    assert_eq!(
        extracted.matches("Gesamtsumme").count(),
        1,
        "a rowspan cell must render once, not once per spanned row"
    );
}

#[test]
fn per_cell_background_and_border_render_and_override_the_stripe() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(
        Table::new()
            .columns([TableColumn::fixed(140.0), TableColumn::fixed(140.0)])
            .striped(Color::rgb(240, 240, 240))
            .rows(vec![
                vec![TableCell::from("Position"), TableCell::from("Menge")],
                vec![
                    TableCell::new("Rueckstand")
                        .background(Color::rgb(255, 0, 0))
                        .border(Border::solid(1.0, Color::rgb(120, 0, 0))),
                    TableCell::from("-3"),
                ],
            ]),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("Rueckstand"));
    assert!(extracted.contains("-3"));

    // The cell's own fill color (1.0/0/0) and stroke color (120/255=0.47..)
    // must show up as PDF `rg`/`RG` color operators — proof the cell-level
    // Rect actually got emitted, not just the row's zebra stripe. Content
    // streams are compressed by default (ADR-016), so decompress first.
    let decompressed = support::decompressed(&bytes).unwrap();
    let text = String::from_utf8_lossy(&decompressed);
    assert!(text.contains("1 0 0 rg"), "expected the cell's own red fill color, got:\n{text}");
    assert!(
        text.contains(" RG"),
        "expected a stroked border operator for the cell's own border, got:\n{text}"
    );
}

#[test]
fn table_spanning_multiple_pages_repeats_header_without_losing_rows() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    let rows: Vec<Vec<Element>> = (0..60)
        .map(|i| vec![Element::from(format!("Position {i:03}")), Element::from(format!("{i}"))])
        .collect();
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(40.0).align(Align::End)])
            .header(["Beschreibung", "Nr"])
            .rows(rows),
    );

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let n = support::page_count(&bytes).unwrap();
    assert!(n > 1, "expected the table to span multiple pages");

    let full_text = support::pdftotext(&bytes).unwrap();
    let header_count = full_text.matches("Beschreibung").count();
    assert_eq!(header_count, n, "header must repeat exactly once per page");
    for i in [0, 30, 59] {
        assert!(full_text.contains(&format!("Position {i:03}")), "row {i} missing from output");
    }
}
