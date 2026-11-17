//! Issue #17: `Document` ↔ JSON round-trip via the `serde` feature.

use lightweight_pdf::*;
use lightweight_pdf_test_support as support;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/images/").to_string() + name).expect("test fixture present")
}

/// Exercises most element kinds in one document: `Row`/`Column`, `Table`
/// (with a per-cell override), `List`, `TableOfContents`, `Image`
/// (base64), `Line`, `Rect`, `Spacer` — everything the JSON schema
/// supports except `Header`/`Footer` (out of scope, see `Document::header`'s
/// doc comment).
fn sample_document() -> Document {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(40.0))
        .title("Rechnung RE-2026-0100")
        .watermark(Watermark::new("ENTWURF"));

    doc.add(Text::new("Rechnung").heading1());
    doc.add(TableOfContents::new());
    doc.add(
        Row::new()
            .gap(10.0)
            .child(Text::new("Links"))
            .child(Text::new("Rechts").align(Align::End)),
    );
    doc.add(
        Column::new()
            .padding(8.0)
            .background(Color::rgb(240, 240, 240))
            .child(Text::new("In einer Box")),
    );
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(80.0).align(Align::End)])
            .header(["Beschreibung", "Betrag"])
            .rows(vec![vec![
                TableCell::from("Beratung").background(Color::rgb(255, 240, 240)),
                TableCell::from("100,00 €"),
            ]]),
    );
    doc.add(List::new().bullet("Erster Punkt").numbered("Zweiter Punkt"));
    doc.add(Line::new().thickness(2.0).color(Color::rgb(0, 0, 0)));
    doc.add(Rect::new().height(20.0).background(Color::rgb(200, 200, 200)));
    doc.add(Spacer::new(10.0));
    doc.add(Image::new(fixture("logo_baseline.jpg")).expect("valid JPEG fixture").width(100.0));
    doc
}

#[test]
fn round_trip_through_json_renders_a_byte_identical_pdf() {
    let original = sample_document();
    let json = original.to_json().expect("to_json should succeed");
    assert!(json.contains("\"schema_version\":1"));

    let restored = Document::from_json(&json).expect("from_json should succeed");

    let original_bytes = original.render().expect("original should render");
    let restored_bytes = restored.render().expect("restored should render");
    assert_eq!(
        original_bytes, restored_bytes,
        "a round-tripped Document must render byte-identically"
    );

    let (ok, log) = support::qpdf_check(&restored_bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");
}

#[test]
fn from_json_rejects_an_unknown_field_with_a_clear_error() {
    let json = r#"{"schema_version":1,"document":{"page_format":"A4","children":[{"type":"text","content":"hi","bogus":true}]}}"#;
    let Err(err) = Document::from_json(json) else {
        panic!("an unknown field on a nested element must be rejected");
    };
    let message = err.to_string();
    assert!(
        message.contains("bogus"),
        "expected the error to name the unknown field, got: {message}"
    );
}

#[test]
fn from_json_documented_example_parses_and_renders() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/document.json"))
        .expect("examples/document.json should exist");
    let doc = Document::from_json(&json).expect("the documented example should parse");
    let (bytes, warnings) = doc.render_with_diagnostics().expect("the documented example should render");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf check failed: {log}");
}

/// Issue #39: a footer and a header in JSON, with page placeholders.
fn document_with_json_footer() -> String {
    let paragraphs: Vec<String> = (0..80)
        .map(|i| {
            format!(
                r#"{{"type":"text","content":"Absatz {i}: Dies ist ein Testabsatz mit genug Text, um über mehrere Zeilen zu laufen und irgendwann eine neue Seite zu erzwingen."}}"#
            )
        })
        .collect();
    format!(
        r#"{{"schema_version":1,"document":{{
            "page_format":"A4",
            "margin":{{"top":56.0,"right":56.0,"bottom":56.0,"left":56.0}},
            "header":{{"height":20.0,"content":{{"type":"text","content":"Kopfzeile"}}}},
            "footer":{{"height":20.0,"visible_from":2,"content":{{"type":"text","content":"Seite {{page}} von {{total_pages}}"}}}},
            "children":[{}]
        }}}}"#,
        paragraphs.join(",")
    )
}

#[test]
fn json_footer_shows_page_and_total_from_the_second_page_on() {
    let doc = Document::from_json(&document_with_json_footer()).expect("header/footer JSON should parse");
    let bytes = doc.render().expect("render should succeed");
    let n = support::page_count(&bytes).unwrap();
    assert!(n > 2, "expected several pages, got {n}");
    for page in 1..=n {
        let text = support::pdftotext_page(&bytes, page).unwrap();
        assert!(text.contains("Kopfzeile"), "page {page}: header missing:\n{text}");
        if page == 1 {
            assert!(!text.contains("Seite 1 von"), "footer must not be drawn on page 1:\n{text}");
        } else {
            let expected = format!("Seite {page} von {n}");
            assert!(text.contains(&expected), "page {page}/{n}: footer missing, got:\n{text}");
        }
    }
}

#[test]
fn json_header_and_footer_round_trip_to_a_byte_identical_pdf() {
    let original = Document::from_json(&document_with_json_footer()).unwrap();
    let json = original.to_json().expect("a header/footer from JSON is written back");
    assert!(json.contains("\"footer\"") && json.contains("\"visible_from\":2"), "got: {json}");
    let again = Document::from_json(&json).unwrap();
    assert_eq!(original.render().unwrap(), again.render().unwrap());
}

#[test]
fn to_json_still_refuses_a_closure_footer() {
    let doc = Document::new(PageFormat::A4).footer(Footer::new(20.0, |_| Text::new("x").into()));
    assert!(matches!(doc.to_json(), Err(DocumentJsonError::HeaderOrFooterNotSupported)));
}

#[test]
fn a_footer_built_from_an_element_is_written_to_json() {
    let doc = Document::new(PageFormat::A4)
        .footer(Footer::from_element(20.0, Text::new("S. {page}").into()))
        .footer_visible_from(3);
    let json = doc.to_json().unwrap();
    assert!(json.contains("S. {page}") && json.contains("\"visible_from\":3"), "got: {json}");
}

#[test]
fn page_placeholders_are_replaced_in_spans_tables_and_lists_and_other_braces_stay() {
    let json = r#"{"schema_version":1,"document":{"page_format":"A4","footer":{"height":60.0,"content":{"type":"column","children":[
        {"type":"text","content":"","spans":[{"text":"rich {page}"},{"text":" / {total_pages}"}]},
        {"type":"table","columns":[{"width":{"flex":1.0}}],"rows":[[{"element":{"type":"text","content":"cell {page}"}}]]},
        {"type":"list","items":[{"marker":"bullet","content":{"type":"text","content":"item {page}"}}]},
        {"type":"text","content":"keep {other} {{page}}"}
    ]}}}}"#;
    let doc = Document::from_json(json).expect("should parse");
    let footer = doc.footer.as_ref().unwrap();
    let element = (footer.content)(&PageContext { page: 7, total_pages: 9 });
    let rendered = format!("{element:?}");
    for expected in ["rich 7", " / 9", "cell 7", "item 7", "keep {other} {7}"] {
        assert!(rendered.contains(expected), "missing {expected:?} in {rendered}");
    }
}
