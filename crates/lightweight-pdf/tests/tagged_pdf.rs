//! Issue #27: Tagged PDF/PDF-UA. The structure tree itself is verified
//! with veraPDF (`examples/demo_pdf_ua.rs`, CI's `pdf-a-conformance` job
//! extended to also check `--flavour ua1` — see `.github/workflows/ci.yml`)
//! rather than re-asserted against raw PDF bytes here; this file covers
//! the two things that are genuinely public-API-level behavior:
//! `Document::pdf_ua()` producing structure-tagged content stream markers,
//! and the missing-alt-text warning.

use lightweight_pdf::*;

#[test]
fn pdf_ua_document_contains_marked_content_and_struct_tree() {
    let mut doc = Document::new(PageFormat::A4).pdf_ua().lang("en-US");
    doc.add(Text::new("Heading").heading1());
    doc.add(Text::new("Body text"));

    let bytes = doc.render().expect("render should succeed");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/StructTreeRoot"), "expected a structure tree");
    assert!(text.contains("/MarkInfo << /Marked true >>"));
    assert!(text.contains("/Lang (en-US)"));
    assert!(text.contains("/Type /StructElem /S /H1"));
    assert!(text.contains("/Type /StructElem /S /P"));
}

#[test]
fn image_without_alt_text_produces_a_warning() {
    let logo = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/images/logo_rgb.png")).unwrap();
    let mut doc = Document::new(PageFormat::A4).pdf_ua();
    doc.add(Image::new(logo).expect("valid PNG"));

    let (_bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(
        warnings.iter().any(|w| w.kind == LayoutWarningKind::MissingAltText),
        "expected a MissingAltText warning, got: {warnings:?}"
    );
}

#[test]
fn image_with_alt_text_produces_no_warning() {
    let logo = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/images/logo_rgb.png")).unwrap();
    let mut doc = Document::new(PageFormat::A4).pdf_ua();
    doc.add(Image::new(logo).expect("valid PNG").alt("A sample logo"));

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Alt (A sample logo)"));
}
