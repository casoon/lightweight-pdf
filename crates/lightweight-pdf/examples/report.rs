//! Example: a report/certificate-style document — cover page (header
//! suppressed via `.header_visible_from(2)`), heading presets, a `List`,
//! and an optional watermark. (Phase 6 DoD, `plan/phases/phase-6-business-
//! polish.md`.)
//!
//! Also demonstrates `Document::theme(..)` (issue #16): headings get a
//! brand color and the cover subtitle uses `.muted()` — set once, here,
//! instead of a `.color(..)` call on every heading this document adds.
//!
//! Run: `cargo run -p lightweight-pdf --example report`

use lightweight_pdf::*;

#[path = "common/mod.rs"]
mod common;

/// A brand-colored variant of the default theme — only `heading1`/
/// `heading2`/`heading3` differ from `Theme::default()`, so every
/// `.heading1()`/`.heading2()`/`.heading3()` in this document picks up
/// the accent color automatically; nothing else about them (size, bold,
/// `keep_with_next`, outline level) changes.
fn brand_theme() -> Theme {
    let accent = Color::rgb(0x1a, 0x3c, 0x6e);
    let defaults = Theme::default();
    Theme {
        heading1: TextStyle {
            color: accent,
            ..defaults.heading1
        },
        heading2: TextStyle {
            color: accent,
            ..defaults.heading2
        },
        heading3: TextStyle {
            color: accent,
            ..defaults.heading3
        },
        ..defaults
    }
}

fn main() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(56.0, 56.0))
        .theme(brand_theme())
        .header(Header::new(20.0, |_ctx| {
            Text::new("Jahresbericht 2026 \u{2014} Muster GmbH").size(9.0).into()
        }))
        .header_visible_from(2)
        .footer(Footer::new(20.0, |ctx| {
            Text::new(format!("Seite {} von {}", ctx.page, ctx.total_pages))
                .size(9.0)
                .align(Align::Center)
                .into()
        }))
        .watermark(Watermark::new("ENTWURF"));

    // --- cover page -----------------------------------------------------
    doc.add(Spacer::new(220.0));
    doc.add(Text::new("Jahresbericht 2026").heading1().align(Align::Center));
    doc.add(Spacer::new(8.0));
    doc.add(Text::new("Muster GmbH").align(Align::Center));
    doc.add(Text::new("vorgelegt am 20. August 2026").muted().align(Align::Center));
    doc.add(Element::PageBreak);

    // --- content ---------------------------------------------------------
    doc.add(Text::new("1. Zusammenfassung").heading1());
    doc.add(Text::new(
        "Das Geschäftsjahr 2026 war geprägt von stabilem Wachstum in allen \
         Kernbereichen. Die folgenden Abschnitte fassen die wichtigsten \
         Kennzahlen und Entwicklungen zusammen.",
    ));
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("2. Wichtigste Ereignisse").heading2());
    doc.add(
        List::new()
            .bullet(Text::new("Markteinführung des neuen Produkts im zweiten Quartal"))
            .bullet(Text::new("Erweiterung des Teams um 12 neue Mitarbeitende"))
            .bullet(Text::new("Eröffnung eines zweiten Standorts")),
    );
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("3. Zeitplan").heading2());
    doc.add(
        List::new()
            .numbered(Text::new("Kickoff und Planung (Januar \u{2013} Februar)"))
            .numbered(Text::new("Umsetzung Phase 1 (M\u{e4}rz \u{2013} Juni)"))
            .numbered(Text::new("Umsetzung Phase 2 (Juli \u{2013} Oktober)"))
            .numbered(Text::new("Abschluss und Auswertung (November \u{2013} Dezember)")),
    );
    doc.add(Spacer::new(12.0));

    doc.add(Text::new("3.1 Details").heading3());
    doc.add(Text::new(
        "Weitere Details zu den einzelnen Phasen finden sich im Anhang. \
         Diese Überschrift bleibt dank keep_with_next garantiert mit \
         diesem Absatz auf derselben Seite zusammen.",
    ));

    common::write_pdf(&doc, "report.pdf");
}
