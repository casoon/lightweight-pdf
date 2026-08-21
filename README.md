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
`.../examples/report.rs`. `examples/demo_*.rs` at the repo root are further,
English-language sample documents (invoice, quote, credentials hand-off,
concept, API documentation, audit report) — run with `cargo run -p
lightweight-pdf --example demo_invoice` etc.

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
| `default-fonts`     | ✅      | Bundles Source Sans 3 as the default font set. Currently the only font source in V1 (no custom-font API yet), so `Document::render()` isn't available without it — `--no-default-features` still compiles the `lib` target, but not the examples/tests. |
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

## Publishing

All five workspace crates are meant to be published to crates.io (the
`publish = false` guard that used to sit on the four internal crates was
only a placeholder for "before v1 ships", not a permanent decision — see
the local `plan/00-decisions.md`, ADR-002/ADR-015 for context that isn't
part of this repo's git history). Path dependencies between them now also
carry a `version` requirement, as `cargo publish` requires.

Because `lightweight-pdf-layout` and `lightweight-pdf` depend on
not-yet-published sibling crates, a first release must publish in
dependency order — each step only works once the previous one is live on
crates.io:

```sh
cargo publish -p lightweight-pdf-writer
cargo publish -p lightweight-pdf-core
cargo publish -p lightweight-pdf-fonts
cargo publish -p lightweight-pdf-layout   # needs lightweight-pdf-core live
cargo publish -p lightweight-pdf          # needs all four live
```

`cargo package -p lightweight-pdf-writer/-core/-fonts --allow-dirty`
already package and verify cleanly today (no unpublished dependencies).
`lightweight-pdf-layout` and `lightweight-pdf` will fail `cargo package`/
`cargo publish --dry-run` until their dependencies actually exist on
crates.io — that's expected for a first multi-crate release, not a
configuration error.

## License

The crate code (all workspace members) is MIT-licensed, see `LICENSE`.

The bundled default fonts (Source Sans 3) are under the SIL Open Font
License 1.1 — license text in `assets/fonts/LICENSES/`. The OFL permits
embedding/subsetting in generated documents, so the generated PDFs are not
subject to any OFL obligation.
