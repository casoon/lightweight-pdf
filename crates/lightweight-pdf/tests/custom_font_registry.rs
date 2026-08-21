//! `FontRegistry::with_fonts()` / `Document::render_with_fonts()`
//! (github.com/casoon/lightweight-pdf/issues/1): rendering with a
//! caller-supplied font instead of the bundled Source Sans 3 default.

mod support;

use lightweight_pdf::*;

const CUSTOM_FONT: &[u8] = include_bytes!("../../lightweight-pdf-fonts/tests/fixtures/custom-test-font.ttf");

#[test]
fn renders_with_a_caller_supplied_font_instead_of_the_bundled_default() {
    let fonts = FontRegistry::with_fonts(CUSTOM_FONT, CUSTOM_FONT).expect("valid custom font");

    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Hallo Rechnung \u{e4}\u{f6}\u{fc}").size(14.0));

    let (bytes, warnings) = doc.render_with_fonts_and_diagnostics(&fonts).expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes);
    assert!(
        text.contains("Hallo Rechnung äöü"),
        "missing expected text in extracted content:\n{text}"
    );
}

#[test]
fn unused_bold_weight_is_not_embedded_when_only_regular_text_is_used() {
    // Same reasoning as `fonts_phase4.rs`'s selective-embedding test, just
    // for the custom-font path: passing the same bytes for both slots
    // would double the embedded size if the (unreferenced) bold slot were
    // embedded regardless of use.
    let fonts = FontRegistry::with_fonts(CUSTOM_FONT, CUSTOM_FONT).expect("valid custom font");
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Hallo").size(14.0));

    let bytes = doc.render_with_fonts(&fonts).expect("render should succeed");
    assert!(
        bytes.len() < CUSTOM_FONT.len() / 2,
        "expected only a small regular-weight subset to be embedded, got {} bytes (source font is {} bytes)",
        bytes.len(),
        CUSTOM_FONT.len()
    );
}

#[test]
fn rejects_a_font_without_glyf_outlines_the_same_way_as_the_default_path() {
    let garbage = vec![0u8; 16];
    match FontRegistry::with_fonts(&garbage, &garbage) {
        Err(e) => assert_eq!(e, FontError::ParseError),
        Ok(_) => panic!("expected garbage bytes to be rejected"),
    }
}
