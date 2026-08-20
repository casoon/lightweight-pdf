//! Phase 3 DoD (`plan/phases/phase-3-tables.md`): the invoice table example
//! (Beschreibung/Menge/Preis/Gesamt, right-aligned price columns, striped
//! rows) renders correctly, and a table
//! longer than one page repeats its header without losing or duplicating
//! rows.

mod support;

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
    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes);
    assert!(text.contains("Beschreibung"), "missing header, got:\n{text}");
    assert!(text.contains("Beratung"));
    assert!(text.contains("Lizenz"));
    assert!(text.contains("499.00"));
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
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
    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    let n = support::page_count(&bytes);
    assert!(n > 1, "expected the table to span multiple pages");

    let full_text = support::pdftotext(&bytes);
    let header_count = full_text.matches("Beschreibung").count();
    assert_eq!(header_count, n, "header must repeat exactly once per page");
    for i in [0, 30, 59] {
        assert!(full_text.contains(&format!("Position {i:03}")), "row {i} missing from output");
    }
}
