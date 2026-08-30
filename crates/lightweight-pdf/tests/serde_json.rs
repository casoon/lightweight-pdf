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
