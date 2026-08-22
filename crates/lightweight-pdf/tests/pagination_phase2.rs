//! Phase 2 DoD (`plan/phases/phase-2-pagination.md`): long content spans
//! multiple pages, footer shows "Seite X von Y" correctly per page,
//! `PageBreak` forces a break, and an oversized element doesn't hang
//! pagination or escape the page.

use lightweight_pdf_test_support as support;

use lightweight_pdf::*;
use lightweight_pdf_test_support::{page_count, pdftotext_page as page_text};

#[test]
fn footer_shows_correct_page_x_of_y_across_pages() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(56.0, 56.0))
        .footer(Footer::new(20.0, |ctx| {
            Text::new(format!("Seite {} von {}", ctx.page, ctx.total_pages))
                .align(Align::Center)
                .into()
        }));

    for i in 0..80 {
        doc.add(Text::new(format!(
            "Absatz {i}: Dies ist ein Testabsatz mit genug Text, um über mehrere Zeilen zu laufen und irgendwann eine neue Seite zu erzwingen."
        )));
        doc.add(Spacer::new(6.0));
    }

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    let n = page_count(&bytes).unwrap();
    assert!(n > 1, "expected multiple pages, got {n}");

    for page in 1..=n {
        let text = page_text(&bytes, page).unwrap();
        let expected = format!("Seite {page} von {n}");
        assert!(text.contains(&expected), "page {page}/{n} footer missing, got:\n{text}");
    }

    assert!(
        !warnings.iter().any(|w| w.kind == LayoutWarningKind::HeaderFooterOverflow),
        "footer band should not overflow: {warnings:?}"
    );
}

#[test]
fn page_break_element_forces_a_new_page() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Seite eins"));
    doc.add(Element::PageBreak);
    doc.add(Text::new("Seite zwei"));

    let bytes = doc.render().unwrap();
    let n = page_count(&bytes).unwrap();
    assert_eq!(n, 2, "PageBreak must force exactly 2 pages here");
    assert!(page_text(&bytes, 1).unwrap().contains("Seite eins"));
    assert!(!page_text(&bytes, 1).unwrap().contains("Seite zwei"));
    assert!(page_text(&bytes, 2).unwrap().contains("Seite zwei"));
}

#[test]
fn oversized_element_forces_its_own_page_without_hanging() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    doc.add(Text::new("Vor dem riesigen Element"));
    // Taller than a whole A4 body box.
    doc.add(Rect::new().height(3000.0).background(Color::rgb(200, 200, 200)));
    doc.add(Text::new("Nach dem riesigen Element"));

    let (bytes, warnings) = doc.render_with_diagnostics().expect("pagination must terminate, not hang");
    let (ok, log) = support::qpdf_check(&bytes).unwrap();
    assert!(ok, "qpdf --check failed:\n{log}");

    assert!(
        warnings.iter().any(|w| w.kind == LayoutWarningKind::ForcedPageBreak),
        "expected ForcedPageBreak warning: {warnings:?}"
    );

    let n = page_count(&bytes).unwrap();
    assert!(
        n >= 2,
        "the oversized element should push later content onto another page, got {n} pages"
    );
}

#[test]
fn header_and_footer_are_suppressed_on_a_cover_page() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::symmetric(56.0, 56.0))
        .header(Header::new(20.0, |_ctx| Text::new("Firmenkopf").into()))
        .header_visible_from(2);
    doc.add(Text::new("Deckblatt"));
    doc.add(Element::PageBreak);
    doc.add(Text::new("Inhalt"));

    let bytes = doc.render().unwrap();
    assert!(
        !page_text(&bytes, 1).unwrap().contains("Firmenkopf"),
        "cover page must not show the header"
    );
    assert!(page_text(&bytes, 2).unwrap().contains("Firmenkopf"), "page 2 must show the header");
}
