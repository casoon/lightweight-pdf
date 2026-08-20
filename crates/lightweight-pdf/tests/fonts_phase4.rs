//! Phase 4 DoD (`plan/phases/phase-4-fonts-subsetting.md`): German text
//! with the full set of required special characters renders correctly,
//! embedded font data is demonstrably subset (smaller than the full
//! source font), and extracted text round-trips through the `ToUnicode`
//! CMap / CID mapping.

mod support;

use lightweight_pdf::*;

const FULL_REGULAR_FONT_BYTES: usize = 431_196; // assets/fonts/SourceSans3-Regular.ttf on disk
const FULL_BOLD_FONT_BYTES: usize = 428_176; // assets/fonts/SourceSans3-Bold.ttf on disk

#[test]
fn german_special_characters_render_and_extract_correctly() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Umlaute: \u{e4}\u{f6}\u{fc}\u{c4}\u{d6}\u{dc}\u{df}").size(14.0));
    doc.add(Text::new("Preis: 1.234,56 \u{20ac}").size(14.0));
    doc.add(Text::new("Anf\u{fc}hrung: \u{201e}Zitat\u{201c} \u{2013} Gedankenstrich").size(14.0));
    doc.add(Text::new("Fett: \u{e4}\u{f6}\u{fc} \u{20ac}").bold().size(14.0));

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes);
    for expected in ["äöüÄÖÜß", "1.234,56 €", "„Zitat“", "–", "äöü €"] {
        assert!(text.contains(expected), "missing {expected:?} in extracted text:\n{text}");
    }
}

#[test]
fn embedded_fonts_are_subset_not_fully_embedded() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Hallo Rechnung").size(14.0));
    doc.add(Text::new("Fett").bold().size(14.0));

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    // A handful of glyphs subset from each weight must add up to a small
    // fraction of embedding both full fonts (roughly 860KB combined).
    assert!(
        bytes.len() < (FULL_REGULAR_FONT_BYTES + FULL_BOLD_FONT_BYTES) / 4,
        "expected a subset PDF well under a quarter of the full-font size, got {} bytes",
        bytes.len()
    );
}

#[test]
fn only_referenced_weights_are_embedded() {
    // Bold is never used — only the Regular weight should be embedded at
    // all (no PDF font object emitted for a weight nobody referenced).
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Nur Regular hier").size(14.0));

    let bytes = doc.render().expect("render should succeed");
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(
        text.matches("/Subtype /Type0").count(),
        1,
        "expected exactly one embedded font (Regular only)"
    );
}
