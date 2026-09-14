---
title: Quickstart
description: Build a first document, add a page footer, check layout warnings, and render the same kind of document from JSON with the CLI.
order: 2
---

## A first document

```rust
use lightweight_pdf::*;

let mut doc = Document::new(PageFormat::A4)
    .margin(Margin::all(20.0))
    .footer(Footer::new(20.0, |ctx| {
        Text::new(format!("Page {} of {}", ctx.page, ctx.total_pages)).into()
    }));

doc.add(Text::new("Invoice").heading1());
doc.add(
    Table::new()
        .columns([TableColumn::flex(1.0), TableColumn::fixed(60.0).align(Align::End)])
        .header(["Item", "Amount"])
        .rows(vec![vec![Element::from("Consulting"), Element::from("1,200.00 USD")]]),
);

let bytes = doc.render().expect("render should succeed");
std::fs::write("invoice.pdf", bytes).unwrap();
```

- `Document::new` takes a page format; margins and sizes are in PDF points (1/72 inch).
- `doc.add` appends elements to the document flow. Pagination happens at render time.
- The footer is a closure that receives the page number and the total page count. Layout runs in
  two passes, so `total_pages` is already final when the closure is called.

## Check what the layout had to do

`render()` returns the bytes. `render_with_diagnostics()` also returns layout warnings – clipped
text, content larger than a page, a missing glyph, an image without alt text in a tagged PDF:

```rust
let (bytes, warnings) = doc.render_with_diagnostics()?;
for w in &warnings {
    eprintln!("page {}: {:?} ({})", w.page, w.kind, w.element_hint);
}
```

## Without Rust: JSON and the CLI

The same kind of document can be described as JSON and rendered with `lwpdf`. The repository
ships a template and a data file:

```sh
lwpdf validate examples/invoice-template.json --data examples/invoice-data.json
lwpdf render examples/invoice-template.json --data examples/invoice-data.json -o invoice.pdf
```

The [showcase](../../../showcase/cli-template/) shows both files next to the PDF they produce.

## Larger examples

`examples/demo_*.rs` in the repository are complete sample documents – invoice, quote,
credentials hand-off, concept, API documentation, audit report, a custom-font demo and the
PDF/A-3b, ZUGFeRD and PDF/UA demos. Run one with:

```sh
cargo run -p lightweight-pdf --example demo_invoice
```

`crates/lightweight-pdf/examples/invoice.rs` (a German DIN 5008-style invoice with a multi-page
table) and `report.rs` are further examples. Most of them are rendered on the
[showcase](../../../showcase/) page during every site build.

Next: the [layout guide](../../guides/layout/).
