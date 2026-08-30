//! Example: ZUGFeRD/Factur-X output (issue #26) — `Document::zugferd_xml()`
//! embeds a caller-supplied EN 16931 invoice XML as an associated file
//! (implies `.pdf_a3b()`: ZUGFeRD *is* a PDF/A-3 file with an embedded
//! invoice, not an independent format). This crate embeds only — it
//! never generates or validates the XML itself (ADR-018 in the local
//! `plan/00-decisions.md`); the sample XML below is a real EN
//! 16931-conformant invoice from the ZUGFeRD reference test corpus, not
//! something this crate produced.
//!
//! Run: `cargo run -p lightweight-pdf --example demo_zugferd --features zugferd`
//! Verify: any ZUGFeRD/Factur-X validator (e.g. the [Mustang
//! Project](https://www.mustangproject.org/) validator, or
//! <https://www.itb.ec.europa.eu/invoice/upload> for EN 16931).

use lightweight_pdf::*;

const INVOICE_XML: &[u8] = include_bytes!("../test-fixtures/zugferd/en16931-sample.xml");

fn main() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0)).zugferd_xml(INVOICE_XML);
    doc.metadata.title = Some("ZUGFeRD Demo Invoice".to_string());
    doc.metadata.author = Some("lightweight-pdf".to_string());

    doc.add(Text::new("Invoice RE-20201121/508").heading1());
    doc.add(Text::new(
        "This PDF embeds a machine-readable EN 16931 invoice (factur-x.xml) alongside this \
         human-readable rendering — the hybrid format ZUGFeRD/Factur-X e-invoicing requires. \
         Open this file's attachments panel in a PDF reader to see the embedded XML.",
    ));
    doc.add(
        Table::new()
            .columns([TableColumn::flex(1.0), TableColumn::fixed(80.0).align(Align::End)])
            .header(["Item", "Amount"])
            .rows(vec![vec![Element::from("See embedded XML for line items"), Element::from("—")]]),
    );

    let bytes = doc.render().expect("render should succeed");
    std::fs::write("examples/demo_zugferd.pdf", &bytes).expect("write examples/demo_zugferd.pdf");
    println!("wrote examples/demo_zugferd.pdf ({} bytes)", bytes.len());
}
