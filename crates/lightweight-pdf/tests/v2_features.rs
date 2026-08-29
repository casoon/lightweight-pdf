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
        .creator("lightweight-pdf v0.2.0")
        .creation_date(PdfDate::new(2026, 1, 15, 9, 30, 0))
        .mod_date(PdfDate::new(2026, 1, 16, 10, 0, 0));
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
    assert!(text.contains("/CreationDate (D:20260115093000Z)"));
    assert!(text.contains("/ModDate (D:20260116100000Z)"));
    assert!(text.contains(concat!("/Producer (lightweight-pdf ", env!("CARGO_PKG_VERSION"), ")")));
    assert!(text.contains("/ID [<"));
    assert!(text.contains("/Info"));

    let info = support::pdfinfo(&bytes).unwrap();
    assert!(info.contains("Invoice #2026-104"), "pdfinfo output was:\n{info}");
    assert!(info.contains("Acme Software GmbH"), "pdfinfo output was:\n{info}");
    assert!(info.contains("CreationDate:"), "pdfinfo output was:\n{info}");
    assert!(info.contains("ModDate:"), "pdfinfo output was:\n{info}");
}

#[test]
fn document_id_is_deterministic_across_identical_renders() {
    let build = || {
        let mut doc = Document::new(PageFormat::A4).title("Determinism Check");
        doc.add(Text::new("Same content, same /ID"));
        doc.render().expect("render should succeed")
    };
    assert_eq!(
        build(),
        build(),
        "two renders of the same Document must be byte-identical, including /ID"
    );
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

/// Every `Tj` (text-showing) op in the content stream, across the whole
/// document — a justified line draws one op per word (composite/CID fonts
/// can't use PDF word spacing, see `render/text.rs`), a non-justified
/// line draws exactly one op for the whole line.
fn count_tj_ops(bytes: &[u8]) -> usize {
    bytes.windows(5).filter(|w| *w == b"Tj ET").count()
}

#[test]
fn justified_paragraph_stretches_every_line_but_the_last() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0));
    doc.add(
        Text::new("Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor")
            .align(Align::Justify)
            .width(220.0),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    let line_count = extracted.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(
        line_count >= 2,
        "expected the paragraph to wrap onto multiple lines, got:\n{extracted}"
    );

    let tj_count = count_tj_ops(&bytes);
    assert!(
        tj_count > line_count,
        "expected more than one text run per justified line (one per word), got {tj_count} Tj ops across {line_count} lines"
    );

    for word in ["Lorem", "ipsum", "dolor", "tempor"] {
        assert!(extracted.contains(word), "missing {word:?} in extracted text:\n{extracted}");
    }
}

#[test]
fn justify_never_stretches_a_single_line_paragraph() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0));
    doc.add(Text::new("Kurzer Satz").align(Align::Justify).width(300.0));

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    // A paragraph that fits on one line is entirely its own last line —
    // never stretched — so it draws as a single text-showing run, same as
    // Align::Start.
    assert_eq!(count_tj_ops(&bytes), 1);
}
