# lightweight-pdf

Lightweight, document-oriented PDF generation in pure Rust — for recurring
business documents (invoices, quotes, delivery notes, reports,
certificates), runnable as `wasm32-unknown-unknown` inside a Cloudflare
Worker. No generic typesetting system, no parser for a custom markup
language — a builder pattern over a fixed set of layout primitives.

Own PDF writer (objects/xref/streams) and own TrueType subsetter; the only
required external dependency is `skrifa` for font parsing. `miniz_oxide`
(FlateDecode compression, see above) is an optional dependency behind the
default-on `compress` feature.

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
concept, API documentation, audit report, and a custom-font demo) — run with `cargo run -p
lightweight-pdf --example demo_invoice` etc.

## Features

- Layout primitives: `Text`, `Column`, `Row`, `Table`, `List`, `Image`,
  `Line`, `Spacer`, `PageBreak` — with `flex`, alignment (including
  `Align::Justify` for `Text`: every line flush with both edges except a
  paragraph's last), padding, border/background, rounded corners
  (`.corner_radius()`), and dashed borders (`Border::dashed(width, color,
  dash, gap)`).
- Page formats `A3`/`A4`/`A5`/`Letter`/`Legal`/`Custom(w, h)` plus
  `Document::landscape()`/`.portrait()` orientation.
- Document metadata (title/author/subject/keywords/creator/creation date/mod
  date) written to the PDF `/Info` dictionary via `Document::title()`/`.author()`/
  `.creation_date(PdfDate::new(..))`/etc., plus a `/Producer` and a
  deterministic `/ID` (hashed from document content, never a random source)
  on every document.
- Hyperlinks: `Text::url(...)` emits a PDF URI link annotation over the
  rendered text; `Text::anchor(name)`/`.link_to(name)` do the same for
  internal jumps (e.g. a table of contents entry to a heading elsewhere
  in the document), resolved to a `/Dest` once pagination is final.
- PDF bookmarks (`/Outlines`): `.heading1()`/`.heading2()`/`.heading3()`
  build the sidebar tree automatically (`.outline_level(n)` for anything
  else that should show up in it); a document with no headings emits no
  `/Outlines` object at all.
- `TableOfContents::new()`: self-populates from every heading
  (`.heading1()`/`2`/`3`/`.outline_level(n)`) in the document, in order,
  with correct page numbers and a clickable entry per heading — the
  two-pass layout already runs (for `{page}/{total}` in Header/Footer)
  determines those before this element ever renders. `.max_depth(n)`
  limits which heading levels become entries (default `3`), `.leader(c)`
  sets the fill character between title and page number (default `.`,
  `' '` for none). Splits across pages like any other content if it has
  enough entries.
- JSON documents (`serde` feature): `Document::from_json(&str)`/`.to_json()`
  (de)serialize the whole document tree — `serde_json::from_str::<DocumentSchema>`/
  `to_string` also works directly for other formats via the same
  `#[derive(Deserialize)]` (YAML/TOML, bring your own crate). The root is
  versioned (`{"schema_version": 1, "document": {...}}`); unknown fields
  anywhere in the tree are a parse error, never silently dropped. Every
  `Element` is tagged by a `"type"` field (`"text"`, `"table"`, ...); see
  `examples/document.json`. Deliberately no scripting/expressions/loops —
  a serialization format, not a template language. Not representable in
  JSON: `Header`/`Footer` (Rust closures) — `to_json()` refuses outright
  if either is set rather than silently dropping them. `Image` embeds as
  base64 (`{"bytes_base64": "...", "common": {...}}`).
- Data-driven templates (`serde` feature, builds on the above):
  `Document::from_template(template_json, data_json, MissingPlaceholder)`
  resolves `"{{path.to.value}}"` placeholders in a template document
  against a separate data document, no Rust code needed — a placeholder
  that's the *entire* string value resolves to the data's own JSON type
  (a number stays a number); embedded in more text it's always a string.
  A missing path is a clear error by default, or an empty string with
  `MissingPlaceholder::Empty`. Row/list repetition is a JSON construct,
  not a text marker: `{"$each": "items", "template": <value>}` wherever
  it appears as an array element expands to one copy of `template` per
  element, with the element's own fields resolved before falling back to
  the outer data — deliberately array-iteration only, no conditions, no
  expressions, no filters. `render_template()` alone (without a
  `Document`) also works, for other consumers of the same JSON schema.
  See `examples/invoice-template.json` + `examples/invoice-data.json`.
- Content streams, embedded font programs, and raw image samples are
  `/FlateDecode`-compressed by default (`compress` feature, on unless
  explicitly disabled) — typically 40-60% smaller output.
- `Document::theme(Theme { .. })`: named style roles (`body`, `caption`,
  `heading1`/`2`/`3`, `table_header`, `muted`) resolved once, when an
  element is added — no cascade. `Text::new()` and the `.heading1()`/`.heading2()`/
  `.heading3()`/`.caption()`/`.muted()`/`.table_header()` presets are
  theme-eligible until any other style call (`.size()`, `.color()`, ...)
  opts them back out; `.align()` is independent of theming either way.
  Table header cells built from plain strings (`Table::header(["A", ...])`)
  pick up `table_header` automatically. No `.theme(..)` call means
  unchanged output.
- Hyphenation: a soft hyphen (U+00AD) anywhere in `Text::new(..)`'s content
  marks an optional break point — used (as a visible `-`) only if the line
  actually needs it there, invisible and absent from extracted text
  otherwise; a hyphenated prefix is preferred over leaving a ragged gap
  and moving the whole word down. `Text::hyphenate(HyphenationLanguage)`
  additionally inserts these break points automatically from Knuth-Liang
  patterns (English/US, German) — requires the `hyphenation` feature (see
  below; substantially increases binary/WASM size, off by default).
- `Text::rich([Span::new("...", style), ...])`: multiple styles (font/size/
  color) inside one text element, wrapped and paginated as a single
  paragraph — mixed sizes on one line share that line's baseline (from the
  tallest word), and a page split can land in the middle of a span. Not
  supported for rich text: `Align::Justify`, `.url()`/`.link_to()`/
  `.outline_level()`.
- Automatic, page-count-stable pagination (two-pass) including
  header/footer bands, widow/orphan rule, and `keep_with_next`.
- Tables that split across page boundaries (header repeats automatically)
  with row striping; cells support `colspan`/`rowspan` and a per-cell
  background/border/padding/alignment override via `TableCell` (cell beats
  row beats column). A `rowspan` never splits across a page break — the
  whole span moves to the next page together.
  `Table::from_rows(&items)` builds rows from anything implementing
  `TableRow` instead of hand-nesting `vec![vec![Element::from(..), ...]]`.
- Own TrueType subsetting (only glyphs actually used are embedded) for
  real Unicode text via Type-0/CIDFontType2; a Source Sans 3 regular/bold
  pair is bundled by default (`default-fonts` feature). `FontRegistry` is a
  dynamic, arbitrary-`FontKey` registry (`register()`/`register_named()`),
  not fixed to two weights — bring your own fonts via
  `FontRegistry::with_fonts(regular_bytes, bold_bytes)` +
  `Document::render_with_fonts()` (works without `default-fonts` too, see
  `examples/demo_custom_font.rs`), or register additional weights such as
  `FontKey::SANS_ITALIC`/`SANS_BOLD_ITALIC` yourself — `default-fonts`
  bundles regular/bold only, no italic. `Text::italic()`/`.bold_italic()`
  render as italic once a font is registered under that key; with no such
  registration, `render()`/`render_with_diagnostics()` return
  `RenderError::MissingFont` instead of silently substituting the
  registry's default font.
- Images: JPEG is passed through unchanged as `DCTDecode`; PNG is decoded
  and re-embedded with a separate `SMask` (alpha channel) — only with the
  `png` feature enabled (see below).
- Watermarking as a document-level feature (rotated, repeated text drawn
  beneath the page content) — deliberately no general transform/rotation
  API.

## Cargo features (crate `lightweight-pdf`)

| Feature           | Default | Purpose                                                       |
|--------------------|:-------:|-----------------------------------------------------------------|
| `default-fonts`     | ✅      | Bundles Source Sans 3 as the default font set for `Document::render()`/`render_with_diagnostics()`. Not needed for `render_with_fonts()` (custom fonts) — `--no-default-features` still compiles the `lib` target and the `demo_custom_font` example, just not the other examples/tests, which call `render()` directly. |
| `compress`          | ✅      | `/FlateDecode`-compresses content streams, embedded font programs, and raw image samples (`miniz_oxide`, see ADR-016). Disabling it falls back to the previous always-uncompressed output — same PDFs, just bigger. |
| `png`               |         | PNG decoding/embedding (`Image::from_png`); without this feature, embedding a PNG fails at runtime with `ImageEmbedError::PngFeatureDisabled`. |
| `hyphenation`       |         | Automatic Knuth-Liang hyphenation (`Text::hyphenate(HyphenationLanguage)`), English/US and German. Pulls in the `hyphenation` crate with all its bundled language dictionaries (no per-language embedding is available upstream), which roughly quadruples release/WASM binary size — measured and not recommended for tight WASM size budgets; see `plan/progress.md`. Soft-hyphen (U+00AD) breaking itself needs no feature and is always on. |
| `serde`             |         | `Document::from_json()`/`.to_json()` (`serde`, `serde_json`, `base64` for `Image`). See the Features list above for schema scope/limitations. |
| `wasm`              |         | Targets `wasm32-unknown-unknown`.                                |
| `wasm-size-probe`   |         | Internal, non-public `extern "C"` function used to measure WASM build size (CI); requires `default-fonts`. |

## Workspace

```
crates/
  lightweight-pdf-core/         Document model, elements, builder API
  lightweight-pdf-layout/       Layoutable trait, pagination, text wrapping
  lightweight-pdf-writer/       PDF writer core (objects, xref, streams, fonts)
  lightweight-pdf-fonts/        Font metrics/parsing (skrifa), subsetting
  lightweight-pdf/              Facade crate, public API + wasm feature
  lightweight-pdf-test-support/ Internal (publish = false): shared qpdf/pdftotext
                                 shell-out helpers for lightweight-pdf's integration
                                 tests, a dev-dependency only.
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

All five published workspace crates (everything except the internal,
`publish = false` `lightweight-pdf-test-support`) are live on crates.io.
They share one workspace version (`workspace.package.version`), so a
release bumps all five together, even if only one crate actually changed —
simpler than tracking independent versions for a one-way dependency chain
this shallow. Path dependencies between them carry a matching `version`
requirement, as `cargo publish` requires.

Because `lightweight-pdf-layout` and `lightweight-pdf` depend on sibling
crates, a release must publish in dependency order — each step only works
once the previous one is live on crates.io (crates.io's index needs a
moment to catch up after each publish; retry the next step if it fails
immediately with "no matching package found"):

```sh
cargo publish -p lightweight-pdf-writer
cargo publish -p lightweight-pdf-core
cargo publish -p lightweight-pdf-fonts
cargo publish -p lightweight-pdf-layout   # needs lightweight-pdf-core live
cargo publish -p lightweight-pdf          # needs all four live
```

## License

The crate code (all workspace members) is MIT-licensed, see `LICENSE`.

The bundled default fonts (Source Sans 3) are under the SIL Open Font
License 1.1 — license text in `assets/fonts/LICENSES/`. The OFL permits
embedding/subsetting in generated documents, so the generated PDFs are not
subject to any OFL obligation.
