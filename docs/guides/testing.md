---
title: Snapshot testing
description: lightweight-pdf-testing pins rendered PDFs against reference images – for this library's documents or any other PDF.
order: 7
---

`lightweight-pdf-testing` renders a PDF's pages to PNG with `pdftoppm` (poppler-utils) and compares
each page against a checked-in reference image, with a small per-pixel tolerance for renderer
noise. It works on any PDF, not only ones built with this library.

```toml
[dev-dependencies]
lightweight-pdf-testing = "0.3"
```

```rust
let dir = std::path::Path::new("test-fixtures/snapshots");
lightweight_pdf_testing::assert_snapshot(dir, "invoice", &render());
```

- `assert_snapshot(dir, name, pdf_bytes)` panics on a mismatch.
- `check_snapshot(dir, name, pdf_bytes, dpi, tolerance)` returns a `Result` instead, with your own
  resolution and tolerance.
- Reference images are low-DPI grayscale PNGs on purpose: a regression trip-wire, not a
  print-quality proof, and small enough to keep the repository history small.
- On a failed comparison a `.diff.png` is written next to the reference.

After an intentional visual change, rewrite the references:

```sh
UPDATE_SNAPSHOTS=1 cargo test
```

This repository uses it in `crates/lightweight-pdf/tests/snapshots.rs` for a handful of
representative documents (table and theme, image and watermark, multi-page list and table of
contents); see [development](../../reference/development/).
