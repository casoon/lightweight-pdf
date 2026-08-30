//! Example: PDF/A-3b-conformant output (issue #25) — `Document::pdf_a3b()`
//! plus the `pdf-a` Cargo feature adds XMP metadata (synchronized with
//! `/Info`), an `/OutputIntent` with an embedded sRGB ICC profile, and a
//! transparency-group colour space, then this document's specific mix of
//! content (heading, table, an image with a PNG alpha channel, an external
//! link) exercises every code path issue #25 touched — this is the file
//! CI's `pdf-a-conformance` job runs `verapdf --flavour 3b` against.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_pdf_a3b --features pdf-a,png`
//! Verify: `verapdf --flavour 3b examples/demo_pdf_a3b.pdf` (see
//! `docs.verapdf.org/install` — or `docker run --rm -v "$PWD/examples:/data"
//! verapdf/cli --flavour 3b /data/demo_pdf_a3b.pdf`)

use lightweight_pdf::*;

const LOGO: &[u8] = include_bytes!("../test-fixtures/images/logo_rgba.png");

fn main() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0)).pdf_a3b();
    doc.metadata.title = Some("PDF/A-3b Demo".to_string());
    doc.metadata.author = Some("lightweight-pdf".to_string());
    doc.metadata.subject = Some("Demonstrates PDF/A-3b conformant output".to_string());
    doc.metadata.creation_date = Some(PdfDate::new(2026, 1, 15, 9, 0, 0));

    doc.add(Text::new("PDF/A-3b Demo").heading1());
    doc.add(Text::new(
        "This document is rendered with Document::pdf_a3b() — XMP metadata, an embedded sRGB \
         ICC profile, and a transparency-group colour space for the image below, which carries \
         a PNG alpha channel (SMask).",
    ));
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(80.0).align(Align::End)])
            .header(["Check", "Status"])
            .rows(vec![
                vec![Element::from("Fonts embedded"), Element::from("yes")],
                vec![Element::from("XMP metadata"), Element::from("yes")],
                vec![Element::from("OutputIntent (sRGB)"), Element::from("yes")],
            ]),
    );
    doc.add(Image::new(LOGO).expect("valid PNG fixture").width(100.0));
    doc.add(Text::new("lightweight-pdf on GitHub").url("https://github.com/casoon/lightweight-pdf"));

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_pdf_a3b.pdf", &bytes).expect("write examples/demo_pdf_a3b.pdf");
    println!("wrote examples/demo_pdf_a3b.pdf ({} bytes)", bytes.len());
}
