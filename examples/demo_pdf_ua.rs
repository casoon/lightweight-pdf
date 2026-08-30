//! Example: Tagged PDF/PDF-UA output (issue #27) — `Document::pdf_ua()`
//! writes a structure tree (`/StructTreeRoot`, one `/StructElem` per
//! heading/paragraph/table/list/figure), marked content (`BDC`/`EMC` with
//! MCIDs) in every content stream, and marks the watermark/footer as
//! artifacts (pagination decoration, excluded from reading order) rather
//! than structure. Implies `.pdf_a3b()` (ADR-019) — this document is both
//! PDF/A-3b and PDF/UA-1 conformant.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_pdf_ua --features tagged-pdf,png`
//! Verify: `verapdf --flavour ua1 examples/demo_pdf_ua.pdf` (also passes
//! `--flavour 3b`) — see `docs.verapdf.org/install`, or `docker run --rm
//! -v "$PWD/examples:/data" verapdf/cli --flavour ua1 /data/demo_pdf_ua.pdf`

use lightweight_pdf::*;

const LOGO: &[u8] = include_bytes!("../test-fixtures/images/logo_rgba.png");

fn main() {
    let mut doc = Document::new(PageFormat::A4)
        .margin(Margin::all(40.0))
        .pdf_ua()
        .lang("en-US")
        .watermark(Watermark::new("SAMPLE"))
        .footer(Footer::new(20.0, |ctx| {
            Text::new(format!("Page {} of {}", ctx.page, ctx.total_pages)).into()
        }));
    doc.metadata.title = Some("Tagged PDF / PDF-UA Demo".to_string());
    doc.metadata.author = Some("lightweight-pdf".to_string());

    doc.add(Text::new("Tagged PDF / PDF-UA Demo").heading1());
    doc.add(Text::new(
        "This document has a real structure tree: headings, this paragraph, the table and \
         list below, and the image all have their own tagged structure element and reading \
         order that follows document order, not render order. The watermark and page-number \
         footer are marked as artifacts, excluded from that reading order entirely.",
    ));

    doc.add(Text::new("Accessibility checklist").heading2());
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(70.0).align(Align::End)])
            .header(["Check", "Status"])
            .rows(vec![
                vec![Element::from("Structure tree"), Element::from("yes")],
                vec![Element::from("Reading order"), Element::from("yes")],
                vec![Element::from("Alt text"), Element::from("yes")],
            ]),
    );

    doc.add(Text::new("Highlights").heading2());
    doc.add(
        List::new()
            .bullet(Text::new("Headings tagged H1/H2"))
            .bullet(Text::new("Table tagged Table/TR/TH/TD"))
            .numbered(Text::new("Image tagged Figure with Alt text")),
    );

    doc.add(
        Image::new(LOGO)
            .expect("valid PNG fixture")
            .width(100.0)
            .alt("lightweight-pdf logo"),
    );

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    std::fs::write("examples/demo_pdf_ua.pdf", &bytes).expect("write examples/demo_pdf_ua.pdf");
    println!("wrote examples/demo_pdf_ua.pdf ({} bytes)", bytes.len());
}
