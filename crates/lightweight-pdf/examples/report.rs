//! Example: a report/certificate-style document — cover page (header
//! suppressed via `.header_visible_from(2)`), heading presets, a `List`,
//! and an optional watermark. (Phase 6 DoD, `plan/phases/phase-6-business-
//! polish.md`.)
//!
//! Run: `cargo run -p lightweight-pdf --example report`

use lightweight_pdf::*;

fn main() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(56.0, 56.0))
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
    doc.add(Text::new("vorgelegt am 20. August 2026").align(Align::Center));
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

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("report.pdf", &bytes).expect("write report.pdf");
    println!("wrote report.pdf ({} bytes)", bytes.len());
}
