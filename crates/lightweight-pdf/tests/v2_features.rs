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

    // Content streams are compressed by default (ADR-016); decompress to
    // string-search their operators.
    let decompressed = support::decompressed(&bytes).unwrap();
    let text = String::from_utf8_lossy(&decompressed);
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

/// The object number referenced by the first `N 0 R` inside a
/// `/Dest [...]` array.
fn dest_object_number(text: &str) -> u32 {
    let after = text.split("/Dest [").nth(1).expect("expected a /Dest array in the output");
    after
        .split(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .expect("expected an object number right after '/Dest ['")
}

#[test]
fn link_to_jumps_to_a_different_page_than_a_same_page_anchor() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Sprung zum Anhang").link_to("appendix"));
    doc.add(Text::new("Toter Link").link_to("nonexistent-anchor"));
    // Enough filler content to force the anchor onto a later page.
    for i in 0..120 {
        doc.add(Text::new(format!("Fuelltext Zeile {i:03}")));
    }
    // Plain text, not a heading: this test is about link_to/anchor, kept
    // independent of outline_level/heading behavior (see outline tests).
    doc.add(Text::new("Anhang").anchor("appendix"));

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let page_count = support::page_count(&bytes).unwrap();
    assert!(page_count > 1, "expected the filler content to force a page break");

    let text = String::from_utf8_lossy(&bytes);
    // Exactly one /Dest: the valid link_to resolved, the dangling one
    // silently produced no annotation at all (degrades to plain text).
    assert_eq!(
        text.matches("/Dest [").count(),
        1,
        "expected exactly one resolved internal link, got:\n{text}"
    );

    // The destination is a real /Type /Page object, and not page 1 itself
    // (the first entry of /Pages' /Kids) — i.e. it genuinely jumps forward
    // to the page the anchor ended up on, not back to the link's own page.
    let dest_obj = dest_object_number(&text);
    let dest_obj_decl = format!("\n{dest_obj} 0 obj");
    assert!(text.contains(&dest_obj_decl), "destination object {dest_obj} not found in output");
    let dest_obj_body = text.split(&dest_obj_decl).nth(1).unwrap().split("endobj").next().unwrap();
    assert!(
        dest_obj_body.contains("/Type /Page"),
        "destination object {dest_obj} is not a /Type /Page object: {dest_obj_body}"
    );

    let kids = text.split("/Kids [").nth(1).unwrap().split(']').next().unwrap();
    let first_page_obj: u32 = kids.split(' ').next().unwrap().parse().unwrap();
    assert_ne!(
        dest_obj, first_page_obj,
        "internal link must not point back at its own (first) page"
    );
}

/// The full `N 0 obj ... endobj` block of whichever object's body
/// contains `needle` — objects never legitimately contain a bare
/// `\nendobj\n`, so splitting on it is a safe-enough object boundary for
/// tests without a real PDF parser.
fn find_object_containing<'a>(text: &'a str, needle: &str) -> &'a str {
    let idx = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in output:\n{text}"));
    let start = text[..idx].rfind("\nendobj\n").map(|i| i + "\nendobj\n".len()).unwrap_or(0);
    let end = text[idx..].find("\nendobj").map(|i| idx + i).unwrap_or(text.len());
    &text[start..end]
}

#[test]
fn document_without_headings_has_no_outlines_object() {
    let mut doc = Document::new(PageFormat::A4);
    doc.add(Text::new("Ganz gewoehnlicher Text, keine Ueberschrift"));

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("/Outlines"),
        "a document with no headings must not emit an /Outlines object"
    );
}

#[test]
fn heading_hierarchy_produces_a_nested_outline_tree() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0));
    doc.add(Text::new("Kapitel 1").heading1());
    doc.add(Text::new("Abschnitt 1.1").heading2());
    doc.add(Text::new("Abschnitt 1.2").heading2());
    doc.add(Text::new("Kapitel 2").heading1());

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(
        text.matches("/Title (").count(),
        4,
        "expected one outline entry per heading, got:\n{text}"
    );
    // The whole-tree /Count on the root /Outlines object: 4 headings total.
    assert!(
        text.contains("/Type /Outlines /Count 4"),
        "expected the root outline count to be 4, got:\n{text}"
    );

    // "Kapitel 1" is a root with two children (its /Count) and a /Next
    // sibling ("Kapitel 2"), but no /Prev (it's first).
    let kapitel1 = find_object_containing(&text, "/Title (Kapitel 1)");
    assert!(kapitel1.contains("/Count 2"), "Kapitel 1 should have 2 children:\n{kapitel1}");
    assert!(kapitel1.contains("/Next"), "Kapitel 1 should have a /Next sibling:\n{kapitel1}");
    assert!(
        !kapitel1.contains("/Prev"),
        "Kapitel 1 is the first root, must not have /Prev:\n{kapitel1}"
    );

    // "Abschnitt 1.2" is a leaf (no /Count/First/Last) with a /Prev
    // sibling ("Abschnitt 1.1") but no /Next (last child of Kapitel 1).
    let abschnitt_1_2 = find_object_containing(&text, "/Title (Abschnitt 1.2)");
    assert!(
        abschnitt_1_2.contains("/Prev"),
        "Abschnitt 1.2 should have a /Prev sibling:\n{abschnitt_1_2}"
    );
    assert!(
        !abschnitt_1_2.contains("/Next"),
        "Abschnitt 1.2 is the last child, must not have /Next:\n{abschnitt_1_2}"
    );
    assert!(
        !abschnitt_1_2.contains("/Count"),
        "a leaf outline entry must not have /Count:\n{abschnitt_1_2}"
    );
}

#[test]
fn heading_on_a_later_page_jumps_to_the_correct_page() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Start").heading1());
    for i in 0..120 {
        doc.add(Text::new(format!("Fuelltext Zeile {i:03}")));
    }
    doc.add(Text::new("Ende").heading1());

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let page_count = support::page_count(&bytes).unwrap();
    assert!(page_count > 1, "expected the filler content to force a page break");

    let text = String::from_utf8_lossy(&bytes);
    let ende_obj = find_object_containing(&text, "/Title (Ende)");
    let dest_obj = dest_object_number(ende_obj);
    let dest_obj_decl = format!("\n{dest_obj} 0 obj");
    let dest_obj_body = text.split(&dest_obj_decl).nth(1).unwrap().split("endobj").next().unwrap();
    assert!(
        dest_obj_body.contains("/Type /Page"),
        "'Ende' heading's /Dest {dest_obj} is not a /Type /Page object: {dest_obj_body}"
    );

    let kids = text.split("/Kids [").nth(1).unwrap().split(']').next().unwrap();
    let first_page_obj: u32 = kids.split(' ').next().unwrap().parse().unwrap();
    assert_ne!(
        dest_obj, first_page_obj,
        "'Ende' heading landed on a later page, its bookmark must not point at page 1"
    );
}

const CUSTOM_FONT: &[u8] = include_bytes!("../../lightweight-pdf-fonts/tests/fixtures/custom-test-font.ttf");

#[test]
fn italic_with_nothing_registered_under_the_key_is_a_typed_error_not_a_silent_fallback() {
    // default-fonts bundles regular/bold only, no italic (see README).
    let mut doc = Document::new(PageFormat::A4);
    doc.add(Text::new("Kursiver Hinweis").italic());

    let err = doc
        .render()
        .expect_err("no SANS_ITALIC registered — render must not silently substitute regular");
    assert!(
        matches!(err, RenderError::MissingFont(key) if key == FontKey::SANS_ITALIC),
        "expected RenderError::MissingFont(SANS_ITALIC), got: {err:?}"
    );
}

#[test]
fn italic_and_bold_italic_methods_work_once_a_font_is_registered_under_the_key() {
    let mut fonts = FontRegistry::with_fonts(CUSTOM_FONT, CUSTOM_FONT).unwrap();
    fonts.register(FontKey::SANS_ITALIC, CUSTOM_FONT).unwrap();
    fonts.register(FontKey::SANS_BOLD_ITALIC, CUSTOM_FONT).unwrap();

    let mut doc = Document::new(PageFormat::A4);
    doc.add(Text::new("Kursiver Hinweis").italic());
    doc.add(Text::new("Fetter kursiver Text").bold_italic());

    let bytes = doc
        .render_with_fonts(&fonts)
        .expect("render should succeed once SANS_ITALIC/SANS_BOLD_ITALIC are registered");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");

    let extracted = support::pdftotext(&bytes).unwrap();
    assert!(extracted.contains("Kursiver Hinweis"));
    assert!(extracted.contains("Fetter kursiver Text"));
}

/// Every `Tj` (text-showing) op in the content stream, across the whole
/// document — a justified line draws one op per word (composite/CID fonts
/// can't use PDF word spacing, see `render/text.rs`), a non-justified
/// line draws exactly one op for the whole line. Content streams are
/// compressed by default (ADR-016), so this decompresses first.
fn count_tj_ops(bytes: &[u8]) -> usize {
    let decompressed = support::decompressed(bytes).expect("qpdf --stream-data=uncompress should succeed");
    decompressed.windows(5).filter(|w| *w == b"Tj ET").count()
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
