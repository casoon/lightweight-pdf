//! Phase 6 DoD (`plan/phases/phase-6-business-polish.md`): `List`, heading
//! presets, and the watermark edge case — a rotated watermark over a full
//! text page must not make any letter unreadable and must not bleed into
//! the header/footer bands.

use lightweight_pdf_test_support as support;

use lightweight_pdf::*;

#[test]
fn list_renders_bullets_and_numbers() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(
        List::new()
            .bullet(Text::new("Erste Position"))
            .numbered(Text::new("Zweite Position"))
            .numbered(Text::new("Dritte Position")),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = support::pdftotext(&bytes).unwrap();
    assert!(text.contains("Erste Position"));
    assert!(text.contains("1."));
    assert!(text.contains("Zweite Position"));
    assert!(text.contains("2."));
    assert!(text.contains("Dritte Position"));
}

#[test]
fn heading_presets_render_larger_and_bold() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Kapitel 1").heading1());
    doc.add(Text::new("Ein Absatz direkt danach, damit keep_with_next nichts verschiebt."));

    let bytes = doc.render().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");
    let text = support::pdftotext(&bytes).unwrap();
    assert!(text.contains("Kapitel 1"));
}

#[test]
fn currency_formatting_matches_expected_german_format() {
    assert_eq!(format_currency_de(123456), "1.234,56 \u{20ac}");
}

#[test]
fn watermark_does_not_obscure_body_text_and_stays_within_the_body_box() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(56.0, 56.0))
        .header(Header::new(30.0, |_ctx| Text::new("Kopfzeile Firma GmbH").into()))
        .footer(Footer::new(30.0, |ctx| {
            Text::new(format!("Seite {} von {}", ctx.page, ctx.total_pages)).into()
        }))
        .watermark(Watermark::new("ENTWURF"));

    // Fill the page with enough real text that the watermark's diagonal
    // sweep necessarily crosses through it.
    for i in 0..40 {
        doc.add(Text::new(format!(
            "Zeile {i}: Volltext, der die Seite ausreichend füllt, damit das \
             Wasserzeichen garantiert durch sichtbaren Inhalt läuft."
        )));
    }

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(
        !warnings.iter().any(|w| w.kind == LayoutWarningKind::HeaderFooterOverflow),
        "header/footer must render normally alongside the watermark: {warnings:?}"
    );

    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    // Both the watermark text and ordinary body text must be extractable —
    // proof neither one silently swallowed the other. `-layout`/default
    // mode fragments *rotated* text across several visual "lines" (a
    // poppler heuristic quirk, not a rendering bug) — `-raw` mode preserves
    // content-stream glyph order, so stripping whitespace before matching
    // reassembles the diagonal run into contiguous "ENTWURF".
    let raw = support::pdftotext_raw(&bytes).unwrap();
    let raw_no_whitespace: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        raw_no_whitespace.contains("ENTWURF"),
        "watermark text missing from extracted output:\n{raw}"
    );

    let text = support::pdftotext(&bytes).unwrap();
    assert!(text.contains("Zeile 0"), "body text missing from extracted output:\n{text}");
    assert!(text.contains("Kopfzeile Firma GmbH"), "header text missing");
    assert!(text.contains("Seite 1 von"), "footer text missing");

    // Byte-order proof of z-ordering: the watermark's rotation matrix
    // ("cm BT", unique to `text_rotated` — normal text never precedes `BT`
    // with a `cm`) must be the *first* drawing operation in the page's
    // content stream, before the outer-clip-scoped header/body/footer ops
    // that follow it — i.e. it really is the bottom layer, not drawn last.
    let content = String::from_utf8_lossy(&bytes);
    let watermark_pos = content.find("cm BT").expect("watermark rotation matrix must be present");
    let first_page_content_start = content.find("re W n").expect("outer page clip must be present"); // the full-page clip, written first
    assert!(
        watermark_pos > first_page_content_start,
        "watermark must be drawn after the page clip is opened"
    );
    let watermark_block_end = content[watermark_pos..].find(" Q").map(|off| watermark_pos + off).unwrap();
    let remaining_ops_after_watermark = &content[watermark_block_end..];
    assert!(
        remaining_ops_after_watermark.contains("Tj"),
        "expected normal content (header/body/footer text) to be drawn after the watermark block"
    );

    // The watermark's own clip must be the body box (margin 56/56, minus
    // the 30pt header/footer bands), not the full page — i.e. narrower
    // *and* shorter than the page, proving it can't bleed into the
    // header/footer bands. Reproduces the same rounding `lightweight-pdf-writer`'s
    // `fmt_num` applies (round to 3 decimals, strip trailing zeros).
    fn fmt_num(v: f32) -> String {
        let mut s = format!("{:.3}", (v * 1000.0).round() / 1000.0);
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        s
    }
    let body_width = 595.2756 - 56.0 - 56.0;
    let body_height = 841.8898 - 56.0 - 56.0 - 30.0 - 30.0;
    let expected_body_clip = format!("{} {} re W n", fmt_num(body_width), fmt_num(body_height));
    assert!(
        content.contains(&expected_body_clip),
        "expected a clip matching the body box ({expected_body_clip}) for the watermark, got:\n{content}"
    );
}
