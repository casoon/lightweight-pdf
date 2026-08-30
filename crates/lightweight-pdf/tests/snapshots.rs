//! Issue #21: pixel-diff snapshot tests — render → rasterize → compare
//! against a checked-in low-DPI grayscale reference PNG per page
//! (`lightweight-pdf-testing`, `pdftoppm`-based, no new system
//! dependency beyond `poppler-utils` this workspace already requires for
//! `pdftotext`).
//!
//! One test per representative document shape rather than literally
//! re-running `examples/demo_*.rs`: those are separate binary crates
//! (Cargo `[[example]]` targets), which an integration test in `tests/`
//! has no way to call into directly — only `cargo run --example ...`
//! them as a subprocess per demo, which would both slow this down (a
//! rebuild+run per demo, the exact CI-time cost issue #21 explicitly
//! rules out) and depend on `cwd` for where each one writes its PDF.
//! These cover the same visual ground the demos do (table, image,
//! watermark, theme, multi-page) directly against the library instead.
//!
//! Run with `UPDATE_SNAPSHOTS=1 cargo test -p lightweight-pdf --test snapshots`
//! to (re)write the reference images after an intentional visual change.

use lightweight_pdf::*;

fn snapshot_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/snapshots"))
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/images/").to_string() + name).expect("test fixture present")
}

#[test]
fn invoice_with_table_and_theme() {
    let mut theme = Theme::default();
    theme.heading1.color = Color::rgb(0, 70, 140);
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0)).theme(theme);
    doc.add(Text::new("Rechnung RE-2026-0100").heading1());
    doc.add(Text::new("Acme Software GmbH"));
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(80.0).align(Align::End)])
            .header(["Beschreibung", "Betrag"])
            .rows(vec![
                vec![TableCell::from("Beratung Softwarearchitektur"), TableCell::from("1.000,00 €")],
                vec![TableCell::from("Reisekosten"), TableCell::from("200,00 €")],
            ]),
    );
    doc.add(Text::new("Gesamtsumme: 1.200,00 €").bold());
    let bytes = doc.render().expect("render should succeed");
    lightweight_pdf_testing::assert_snapshot(&snapshot_dir(), "invoice_with_table_and_theme", &bytes);
}

#[test]
fn document_with_image_and_watermark() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(40.0))
        .watermark(Watermark::new("ENTWURF"));
    doc.add(Text::new("Angebot").heading1());
    doc.add(Image::new(fixture("logo_baseline.jpg")).expect("valid JPEG fixture").width(120.0));
    doc.add(Text::new("Mit freundlichen Grüßen"));
    let bytes = doc.render().expect("render should succeed");
    lightweight_pdf_testing::assert_snapshot(&snapshot_dir(), "document_with_image_and_watermark", &bytes);
}

#[test]
fn multi_page_report_with_list_and_toc() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(40.0))
        .footer(Footer::new(20.0, |ctx| {
            Text::new(format!("Seite {} von {}", ctx.page, ctx.total_pages))
                .align(Align::End)
                .into()
        }));
    doc.add(Text::new("Jahresbericht").heading1());
    doc.add(TableOfContents::new());
    doc.add(Element::PageBreak);
    doc.add(Text::new("Zusammenfassung").heading2());
    doc.add(
        List::new()
            .bullet("Umsatz gestiegen")
            .bullet("Neue Standorte eröffnet")
            .numbered("Ausblick positiv"),
    );
    doc.add(Element::PageBreak);
    doc.add(Text::new("Details").heading2());
    doc.add(Text::new(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
    ));
    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    lightweight_pdf_testing::assert_snapshot(&snapshot_dir(), "multi_page_report_with_list_and_toc", &bytes);
}
