# lightweight-pdf

Lightweight, document-oriented PDF generation in pure Rust — for recurring
business documents (invoices, quotes, delivery notes, reports,
certificates), runnable as `wasm32-unknown-unknown` inside a Cloudflare
Worker. No generic typesetting system, no parser for a custom markup
language — a builder pattern over a fixed set of layout primitives.

**Website and documentation:** [casoon.github.io/lightweight-pdf](https://casoon.github.io/lightweight-pdf/)

Own PDF writer (objects/xref/streams) and own TrueType subsetter; the only
required external dependency is `skrifa` for font parsing. `miniz_oxide`
(FlateDecode compression) is an optional dependency behind the default-on
`compress` feature.

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

`examples/demo_*.rs` are complete sample documents (invoice, quote,
credentials hand-off, concept, API documentation, audit report, custom
font, PDF/A-3b, ZUGFeRD/Factur-X, Tagged PDF/PDF-UA) — run with
`cargo run -p lightweight-pdf --example demo_invoice` etc. The
[showcase](https://casoon.github.io/lightweight-pdf/showcase/) renders
them on every site build, source next to PDF.

Page 1 of three of those demos, rendered — regenerate with
`scripts/render-readme-previews.sh` whenever their output changes:

<p>
  <img src="assets/demo_invoice.png" alt="Rendered invoice demo, page 1" width="260">
  <img src="assets/demo_offer.png" alt="Rendered quote/offer demo, page 1" width="260">
  <img src="assets/demo_report.png" alt="Rendered report demo, page 1" width="260">
</p>

## Install

```sh
cargo add lightweight-pdf            # the library
cargo install lightweight-pdf-cli    # the `lwpdf` CLI: JSON document/template in, PDF out
```

The `@casoon/lightweight-pdf` JavaScript/WASM package is built from
`bindings/js` (see its README); it is not on the npm registry yet.

## Documentation

The full documentation lives on the [website](https://casoon.github.io/lightweight-pdf/docs/);
its sources are in [`docs/`](docs/).

- [Installation](https://casoon.github.io/lightweight-pdf/docs/getting-started/installation/) and
  [quickstart](https://casoon.github.io/lightweight-pdf/docs/getting-started/quickstart/)
- [Layout](https://casoon.github.io/lightweight-pdf/docs/guides/layout/) — primitives, pagination,
  tables, headers/footers, links, bookmarks, table of contents, themes, watermarks
- [Fonts](https://casoon.github.io/lightweight-pdf/docs/guides/fonts/) — bundled Source Sans 3,
  subsetting, custom fonts
- [JSON and templates](https://casoon.github.io/lightweight-pdf/docs/guides/json-and-templates/) and
  the [CLI](https://casoon.github.io/lightweight-pdf/docs/guides/cli/)
- [WASM and JavaScript](https://casoon.github.io/lightweight-pdf/docs/guides/wasm/) — npm package,
  Cloudflare Worker starter, browser playground, measured size and speed
- [PDF/A, ZUGFeRD and PDF/UA](https://casoon.github.io/lightweight-pdf/docs/guides/conformance/)
- [Snapshot testing](https://casoon.github.io/lightweight-pdf/docs/guides/testing/) with
  `lightweight-pdf-testing`
- [Cargo features](https://casoon.github.io/lightweight-pdf/docs/reference/features/),
  [workspace crates](https://casoon.github.io/lightweight-pdf/docs/reference/crates/),
  [comparison](https://casoon.github.io/lightweight-pdf/docs/reference/comparison/) with
  printpdf/genpdf/Typst/krilla/headless Chrome,
  [development and releasing](https://casoon.github.io/lightweight-pdf/docs/reference/development/)
- [Architecture decisions (ADRs)](https://casoon.github.io/lightweight-pdf/docs/adr/overview/) —
  design choices, trade-offs, and constraints
- API reference on [docs.rs](https://docs.rs/lightweight-pdf); changes in [CHANGELOG.md](CHANGELOG.md)

## Workspace

```
crates/
  lightweight-pdf-core/         Document model, elements, builder API
  lightweight-pdf-layout/       Layoutable trait, pagination, text wrapping
  lightweight-pdf-writer/       PDF writer core (objects, xref, streams, fonts)
  lightweight-pdf-fonts/        Font metrics/parsing (skrifa), subsetting
  lightweight-pdf/              Facade crate, public API + wasm feature
  lightweight-pdf-cli/          `lwpdf` binary (own crate, so the library never depends on clap)
  lightweight-pdf-testing/      Pixel-diff PDF snapshot testing, usable for any PDF
  lightweight-pdf-test-support/ Internal (publish = false): test helpers
bindings/
  js/                           `@casoon/lightweight-pdf` (wasm-bindgen + generated TypeScript types)
site/                           Website (Astro), deployed by .github/workflows/pages.yml
```

Dependency direction is strictly one-way: `core ← layout ← facade`;
`writer` and `fonts` are leaves with no path dependency on `core`/`layout`
(enforced in CI via `cargo tree`).

Architecture decisions (ADRs) are published under [`docs/adr/`](docs/adr/overview.md)
and on the website; the day-to-day work journal lives locally in `plan/`.

## Building & testing

```sh
cargo test --workspace                          # unit + integration tests
cargo test -p lightweight-pdf --features png     # incl. PNG path
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --target wasm32-unknown-unknown --release
```

The tests need `qpdf` and `poppler-utils`. Snapshot references, CI and
the crates.io release order are described under
[development](https://casoon.github.io/lightweight-pdf/docs/reference/development/).

## License

The crate code (all workspace members) is MIT-licensed, see `LICENSE`.

The bundled default fonts (Source Sans 3) are under the SIL Open Font
License 1.1 — license text in `assets/fonts/LICENSES/`. The OFL permits
embedding/subsetting in generated documents, so the generated PDFs are not
subject to any OFL obligation.
