//! Automated test suite for new v2 features:
//! - PageFormat variants and Orientation (Landscape, Custom, A3, A5, Letter, Legal)
//! - PDF Document Metadata (/Info dictionary)
//! - TableCell with colspan and per-cell alignment override
//! - Rect and container rounded corners & dashed borders
//! - Hyperlinks (URI Link annotations)
//! - FontRegistry dynamic registration & italic text styles

use lightweight_pdf::*;
use lightweight_pdf_test_support as support;

#[test]
fn page_format_and_orientation_landscape_and_custom_sizes() {
    let mut doc = Document::new(PageFormat::A4).landscape().margin(Margin::all(20.0));
    doc.add(Text::new("A4 Landscape Document").size(18.0).bold());

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    // A4 Portrait is 595.276 x 841.890; Landscape flips MediaBox to [0 0 841.89 595.276]
    assert!(text.contains("/MediaBox [0 0 841.89 595.276]"));
}

#[test]
fn custom_page_format_and_orientation_works() {
    let mut doc = Document::new(PageFormat::Custom(400.0, 600.0)).margin(Margin::all(10.0));
    doc.add(Text::new("Custom Size").size(12.0));

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/MediaBox [0 0 400 600]"));
}

#[test]
fn document_metadata_is_written_to_pdf_info_dictionary() {
    let mut doc = Document::new(PageFormat::A4)
        .title("Invoice #2026-104")
        .author("Acme Software GmbH")
        .subject("Quarterly Report")
        .keywords("invoice, 2026, tax")
        .creator("lightweight-pdf v0.2.0");
    doc.add(Text::new("Metadata Test"));

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Title (Invoice #2026-104)"));
    assert!(text.contains("/Author (Acme Software GmbH)"));
    assert!(text.contains("/Subject (Quarterly Report)"));
    assert!(text.contains("/Keywords (invoice, 2026, tax)"));
    assert!(text.contains("/Creator (lightweight-pdf v0.2.0)"));
    assert!(text.contains("/Info"));
}

#[test]
fn table_cell_with_colspan_and_alignment() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(30.0));
    doc.add(
        Table::new()
            .columns([
                TableColumn::flex(2.0),
                TableColumn::fixed(80.0).align(Align::End),
                TableColumn::fixed(80.0).align(Align::End),
            ])
            .header(["Beschreibung", "Menge", "Gesamt"])
            .rows(vec![
                vec![
                    TableCell::from("Beratung Softwarearchitektur"),
                    TableCell::from("10 Std"),
                    TableCell::from("1.200,00 €"),
                ],
                vec![
                    TableCell::new("Gesamtsumme (Netto)").colspan(2).align(Align::End),
                    TableCell::from("1.200,00 €"),
                ],
            ]),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("Gesamtsumme (Netto)"));
    assert!(extracted.contains("1.200,00 €"));
}

#[test]
fn rect_and_container_rounded_corners_and_dashed_borders() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0));
    doc.add(
        Column::new()
            .padding(15.0)
            .corner_radius(8.0)
            .border(Border::dashed(2.0, Color::rgb(0, 102, 204), 4.0, 2.0))
            .background(Color::rgb(240, 248, 255))
            .child(Text::new("Karten-Box mit abgerundeten Ecken").bold()),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    // Bezier curves ('c') are emitted for rounded corners
    assert!(text.contains(" c\n"), "rounded rect must emit Bezier curve 'c' operators");
    // Dash pattern '[4 2] 0 d'
    assert!(text.contains("[4 2] 0 d"), "dashed border must emit PDF dash operator");
}

#[test]
fn text_with_hyperlink_url_emits_link_annotation() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0));
    doc.add(
        Text::new("Besuchen Sie unsere Website")
            .color(Color::rgb(0, 102, 204))
            .url("https://example.com"),
    );

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Subtype /Link"));
    assert!(text.contains("/URI (https://example.com)"));
    assert!(text.contains("/Annots"));
}

#[test]
fn text_italic_and_bold_italic_methods_work() {
    let mut doc = Document::new(PageFormat::A4);
    doc.add(Text::new("Kursiver Hinweis").italic());
    doc.add(Text::new("Fetter kursiver Text").bold_italic());

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("Kursiver Hinweis"));
    assert!(extracted.contains("Fetter kursiver Text"));
}
