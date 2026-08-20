//! Phase 0 DoD (`plan/phases/phase-0-spike.md`): a minimal document with one
//! embedded font and fixed text renders to a valid, byte-stable PDF.

mod support;

use lightweight_pdf::*;

fn minimal_doc() -> Document {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(72.0, 72.0));
    doc.add(Text::new("Hallo Rechnung").size(18.0));
    doc
}

#[test]
fn renders_valid_pdf_with_expected_text() {
    let doc = minimal_doc();
    let bytes = doc.render().expect("render should succeed");

    assert!(bytes.starts_with(b"%PDF-1.7"));
    assert!(bytes.ends_with(b"%%EOF"));

    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes);
    assert!(text.contains("Hallo Rechnung"), "pdftotext output was:\n{text}");
}

#[test]
fn output_is_byte_stable_across_runs() {
    let a = minimal_doc().render().unwrap();
    let b = minimal_doc().render().unwrap();
    assert_eq!(a, b, "rendering the same document twice must be byte-identical");
}

#[test]
fn matches_golden_reference() {
    let bytes = minimal_doc().render().unwrap();
    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/spike.pdf");
    if std::env::var("LIGHTWEIGHT_PDF_UPDATE_GOLDEN").is_ok() {
        std::fs::write(golden_path, &bytes).unwrap();
    }
    let golden = std::fs::read(golden_path).expect("golden reference file present");
    assert_eq!(
        bytes, golden,
        "PDF output changed — run with LIGHTWEIGHT_PDF_UPDATE_GOLDEN=1 to accept intentional changes"
    );
}
