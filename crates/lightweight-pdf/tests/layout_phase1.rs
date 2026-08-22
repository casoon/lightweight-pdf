//! Phase 1 DoD (`plan/phases/phase-1-layout-core.md`): a mixed-content
//! single-page document (Row/Column/Text/Spacer/Line/Rect) renders
//! correctly, and the documented edge cases don't overlap or escape the
//! page.

use lightweight_pdf_test_support as support;

use lightweight_pdf::*;

fn sample_doc() -> Document {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Row::new().gap(12.0).children(vec![
        Element::from(Column::new().children(vec![
            Element::from(Text::new("Rechnung").size(22.0).bold()),
            Element::from(Text::new("RE-2026-0042")),
        ])),
        Element::from(Text::new("Muster GmbH").align(Align::End).flex(1.0)),
    ]));
    doc.add(Spacer::new(12.0));
    doc.add(Line::new());
    doc.add(Spacer::new(12.0));
    doc.add(Text::new(
        "Dies ist ein längerer Flie\u{df}text, der \u{fc}ber mehrere Zeilen umbrechen soll, \
         damit der Wortumbruch inklusive Sonderzeichen wie \u{e4}\u{f6}\u{fc} und \u{20ac} \
         sichtbar getestet werden kann.",
    ));
    doc.add(Spacer::new(12.0));
    doc.add(Rect::new().width(100.0).height(40.0).background(Color::rgb(230, 230, 230)));
    doc
}

#[test]
fn renders_mixed_content_document() {
    let doc = sample_doc();
    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes).unwrap();
    assert!(text.contains("Rechnung"), "missing heading, got:\n{text}");
    assert!(text.contains("RE-2026-0042"));
    assert!(text.contains("Muster GmbH"));
    assert!(text.contains("Wortumbruch"));
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
}

#[test]
fn hard_break_handles_a_token_wider_than_the_page() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(400.0, 56.0)); // narrow body
    doc.add(Text::new("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"));
    let (bytes, _warnings) = doc
        .render_with_diagnostics()
        .expect("render should succeed even with a token wider than the column");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");
}

#[test]
fn fixed_size_element_clips_and_reports_a_warning() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(
        Text::new("Ein sehr sehr sehr sehr sehr sehr sehr sehr sehr sehr langer Text der garantiert nicht in eine Zeile passt")
            .width(80.0)
            .height(12.0),
    );
    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");
    assert!(
        warnings.iter().any(|w| w.kind == LayoutWarningKind::TextClipped),
        "expected a TextClipped warning, got {warnings:?}"
    );
}
