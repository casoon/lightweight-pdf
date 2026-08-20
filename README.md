# lightweight-pdf

Lightweight, document-oriented PDF generation in pure Rust — for recurring
business documents (invoices, quotes, delivery notes, reports,
certificates), runnable as `wasm32-unknown-unknown` inside a Cloudflare
Worker. No generic typesetting system, no parser for a custom markup
language — a builder pattern over a fixed set of layout primitives.

Own PDF writer (objects/xref/streams) and own TrueType subsetter; the only
required external dependency is `ttf-parser` for font parsing.

## Example

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

More complete examples: `crates/lightweight-pdf/examples/invoice.rs` (a
German DIN-5008-style invoice with a multi-page table) and
`.../examples/report.rs`.

## Features

- Layout primitives: `Text`, `Column`, `Row`, `Table`, `List`, `Image`,
  `Line`, `Spacer`, `PageBreak` — with `flex`, alignment, padding,
  border/background.
- Automatic, page-count-stable pagination (two-pass) including
  header/footer bands, widow/orphan rule, and `keep_with_next`.
- Tables that split across page boundaries (header repeats automatically)
  with row striping.
- Own TrueType subsetting (only glyphs actually used are embedded) for
  real Unicode text via Type-0/CIDFontType2; a Source Sans 3 font set is
  bundled by default (`default-fonts` feature).
- Images: JPEG is passed through unchanged as `DCTDecode`; PNG is decoded
  and re-embedded with a separate `SMask` (alpha channel) — only with the
  `png` feature enabled (see below).
- Watermarking as a document-level feature (rotated, repeated text drawn
  beneath the page content) — deliberately no general transform/rotation
  API.

Currently only one page size (A4, portrait); see
`crates/lightweight-pdf-core/src/document.rs`.

## Cargo features (crate `lightweight-pdf`)

| Feature           | Default | Purpose                                                       |
|--------------------|:-------:|-----------------------------------------------------------------|
| `default-fonts`     | ✅      | Bundles Source Sans 3 as the default font set.                  |
| `png`               |         | PNG decoding/embedding (`Image::from_png`); without this feature, embedding a PNG fails at runtime with `ImageEmbedError::PngFeatureDisabled`. |
| `wasm`              |         | Targets `wasm32-unknown-unknown`.                                |
| `wasm-size-probe`   |         | Internal, non-public `extern "C"` function used to measure WASM build size (CI); requires `default-fonts`. |

## Workspace

```
crates/
  lightweight-pdf-core/    Document model, elements, builder API
  lightweight-pdf-layout/  Layoutable trait, pagination, text wrapping
  lightweight-pdf-writer/  PDF writer core (objects, xref, streams, fonts)
  lightweight-pdf-fonts/   Font metrics/parsing (ttf-parser), subsetting
  lightweight-pdf/         Facade crate, public API + wasm feature
```

Dependency direction is strictly one-way: `core ← layout ← facade`;
`writer` and `fonts` are leaves with no path dependency on `core`/`layout`
(enforced in CI via `cargo tree`).

Architecture, decisions (ADRs) and the work plan live locally in `plan/`
(not part of this repo/its history).

## Building & testing

```sh
cargo test --workspace                          # unit + integration tests
cargo test -p lightweight-pdf --features png     # incl. PNG path
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --target wasm32-unknown-unknown --release
```

## License

The crate code (all workspace members) is MIT-licensed, see `LICENSE`.

The bundled default fonts (Source Sans 3) are under the SIL Open Font
License 1.1 — license text in `assets/fonts/LICENSES/`. The OFL permits
embedding/subsetting in generated documents, so the generated PDFs are not
subject to any OFL obligation.
