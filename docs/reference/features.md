---
title: Cargo features
description: Features of the lightweight-pdf crate, what they enable and what they cost.
order: 1
---

| Feature | Default | Purpose |
| --- | :---: | --- |
| `default-fonts` | yes | Bundles Source Sans 3 (regular, bold) for `Document::render()` and `render_with_diagnostics()`. Not needed for `render_with_fonts()`. |
| `compress` | yes | Flate-compresses content streams, embedded font programs and raw image samples (`miniz_oxide`), typically 40–60% smaller output. Without it, the same PDFs uncompressed. |
| `png` | | PNG decoding and embedding. Without it, embedding a PNG fails with `ImageEmbedError::PngFeatureDisabled`. |
| `hyphenation` | | Automatic Knuth-Liang hyphenation for US English and German via `Text::hyphenate(..)`. Pulls in all bundled dictionaries of the `hyphenation` crate and roughly quadruples release and WASM binary size. Soft-hyphen breaking needs no feature. |
| `serde` | | `Document::from_json()`, `.to_json()` and templates (`serde`, `serde_json`, `base64`). See [JSON and templates](../../guides/json-and-templates/). |
| `schemars` | | Generates the JSON Schema for the document and template format (implies `serde`). |
| `wasm` | | `wasm32-unknown-unknown` build with `wasm-bindgen` bindings (implies `serde`). See [WASM and JavaScript](../../guides/wasm/). |
| `wasm-size-probe` | | Internal: an `extern "C"` export used to measure WASM size in CI. Requires `default-fonts`. |
| `pdf-a` | | `Document::pdf_a3b()`: PDF/A-3b output. About 4.3 KiB gzip in the WASM build. |
| `zugferd` | | `Document::zugferd_xml(bytes)`: embeds ZUGFeRD/Factur-X invoice XML (implies `pdf-a`). |
| `tagged-pdf` | | `Document::pdf_ua()`: Tagged PDF/PDF-UA-1 output (implies `pdf-a`). |

The last three are described in [PDF/A, ZUGFeRD and PDF/UA](../../guides/conformance/).

Without default features, the library still compiles and `render_with_fonts()` works; most
examples and tests call `render()` directly and need `default-fonts`.
