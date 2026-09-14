---
title: PDF/A, ZUGFeRD and PDF/UA
description: PDF/A-3b output, embedding ZUGFeRD/Factur-X invoice XML and Tagged PDF/PDF-UA-1, each behind a Cargo feature and checked against reference validators.
order: 6
---

All three are opt-in Cargo features. Calling the method without its feature does not silently
produce a non-conformant file – `render()` returns `RenderError::PdfAFeatureDisabled`,
`ZugferdFeatureDisabled` or `TaggedPdfFeatureDisabled`.

## PDF/A-3b

Feature `pdf-a`, method `Document::pdf_a3b()`:

```rust
let mut doc = Document::new(PageFormat::A4).margin(Margin::all(40.0)).pdf_a3b();
doc.metadata.title = Some("PDF/A-3b Demo".to_string());
```

- XMP metadata kept in sync with `/Info`.
- An `/OutputIntent` with an embedded sRGB ICC profile – the ICC's own 3 KiB `sRGB2014.icc`
  reference profile, so the feature costs about 4.3 KiB gzip in the WASM build.
- A transparency-group colour space on any page with an alpha-channel image.

Verified with the [veraPDF](https://verapdf.org/) validator, locally and in CI (the
`pdf-a-conformance` job renders `examples/demo_pdf_a3b.rs`).

## ZUGFeRD / Factur-X

Feature `zugferd` (implies `pdf-a`), method `Document::zugferd_xml(bytes)`:

```rust
let doc = Document::new(PageFormat::A4).zugferd_xml(INVOICE_XML);
```

It embeds a caller-supplied EN 16931 invoice XML as an associated file (`/AF`,
`/Names/EmbeddedFiles`) with the Factur-X XMP extension schema. This crate embeds only – it never
generates or validates the XML. `examples/demo_zugferd.rs` embeds a sample from the ZUGFeRD
reference corpus; the result was checked with veraPDF (PDF/A-3b container) and the
[Mustang](https://www.mustangproject.org/) validator (the PDF and the embedded XML against
EN 16931). The [showcase](../../../showcase/zugferd/) has the PDF.

## Tagged PDF / PDF/UA-1

Feature `tagged-pdf` (implies `pdf-a`), method `Document::pdf_ua()`:

```rust
let mut doc = Document::new(PageFormat::A4)
    .pdf_ua()
    .lang("en-US")
    .watermark(Watermark::new("SAMPLE"));
doc.add(Image::new(LOGO)?.width(100.0).alt("lightweight-pdf logo"));
```

- A real structure tree (`/StructTreeRoot`) with one structure element per heading, paragraph,
  table row and cell, list item and figure.
- Marked content (`BDC`/`EMC` with MCIDs) in every content stream, and `/Lang`.
- Watermark, header and footer are marked as artifacts – excluded from the reading order.
- `Image::alt(text)` sets `/Alt`. Without it, an empty `/Alt` keeps the structure tree well-formed,
  but `render_with_diagnostics()` warns and the file is not PDF/UA-1-conformant until you supply
  real alt text. The crate does not invent placeholder text.

`pdf_ua()` implies `pdf_a3b()`, so the output is both. Verified with veraPDF's `ua1` and `3b`
profiles, locally and in CI (`pdf-a-conformance` job, `examples/demo_pdf_ua.rs`). The
[showcase](../../../showcase/pdf-ua/) has the PDF.
