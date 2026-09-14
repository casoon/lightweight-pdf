---
title: Comparison
description: How lightweight-pdf compares with printpdf, genpdf, Typst, krilla/pdf-writer and browser-based PDF generation.
order: 3
---

Checked against each project's own README and crates.io page in August 2026, not from memory.
Dates and versions are as of then. Numbers that are not published anywhere are marked as such
rather than guessed.

| | Layout and pagination | WASM | WASM/binary size | Dependencies | Fonts/subsetting | Images | Licence | Maintenance |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **lightweight-pdf** | Own layout primitives, automatic page-count-stable pagination (two-pass) | First-class target, tested in CI | ~1.2 MiB / ~590 KiB gzip (`wasm` + `default-fonts` + `compress`, measured) | 1 required (`skrifa`); no `image`/GUI/shaping stack | Own TrueType subsetter, only used glyphs embedded | JPEG passthrough, PNG decode and re-embed (feature-gated) | MIT | Active |
| [printpdf](https://github.com/fschutt/printpdf) | Basic layout system, including automatic page breaking, on top of a lower-level API | Yes, documented, with a hosted demo | Not measured/published | 24 direct deps, including a GUI layout engine (`azul-*`) and `image` | Shaping via `allsorts`; subsetting on save | Via `image` | MIT | Active – v0.12.7, published 2026-08-29 |
| [genpdf](https://git.sr.ht/~ireas/genpdf-rs) | Layout on top of an old printpdf + rusttype pairing | Not documented | Not measured/published | printpdf/rusttype pairing from 2021 | Via rusttype | Via printpdf | Apache-2.0 OR MIT | Unmaintained – v0.2.0, published 2021-06-17 |
| [Typst](https://github.com/typst/typst) | Full typesetting system with its own markup language, not a document-tree API | The web app compiles to WASM; not built primarily as an embeddable "call a function, get PDF bytes" library | Reported WASM builds run tens of MiB | Large compiler: own layout and math engine, shaping | Full shaping and subsetting | Full raster/vector | Apache-2.0 | Active |
| [krilla](https://github.com/LaurenzV/krilla) / [pdf-writer](https://github.com/typst/pdf-writer) | None – krilla lists text layout, tables, page breaking and headers/footers as out of scope; pdf-writer is lower-level still | Not documented (pure Rust, plausible) | Not measured/published | 23 direct deps (krilla) | Full shaping (rustybuzz/skrifa) | Full raster/vector | MIT OR Apache-2.0 | Active – krilla v0.8.2, published 2026-06-04 |
| Headless Chrome / wkhtmltopdf | Full HTML/CSS layout via a browser engine | No – needs a native browser binary | Chromium alone is hundreds of MiB | A whole browser | Whatever the OS/browser provides | Full | Chromium: BSD-style; wkhtmltopdf: LGPLv3 | Headless Chrome: active; wkhtmltopdf: [archived since January 2023](https://github.com/wkhtmltopdf/wkhtmltopdf) |

The size figure for lightweight-pdf comes from `wrangler deploy --dry-run`, see
[WASM and JavaScript](../../guides/wasm/).

## Where lightweight-pdf is behind

- **Conformance levels.** krilla supports Tagged PDF/PDF-UA and PDF/A-1, -2, -3 and -4.
  lightweight-pdf covers PDF/A-3b and PDF/UA-1 (with ZUGFeRD embedding on top) since 0.3.0, and
  nothing else.
- **Input richness.** Every HTML-based approach, and Typst's own language, accepts far richer input
  than this project's fixed builder/JSON document tree. There is no way to hand it arbitrary
  HTML/CSS or a typesetting language. That boundary is deliberate, but it is a real limitation if
  that is what you need.
- **Text shaping.** There is no font-shaping stack, unlike printpdf (`allsorts`), Typst or krilla
  (`rustybuzz`).
