---
title: Fonts
description: The bundled Source Sans 3, TrueType subsetting, bringing your own fonts, italic font keys and the licence of the default fonts.
order: 2
---

## Default fonts

With the default `default-fonts` feature, Source Sans 3 regular and bold are compiled in and
`Document::render()` uses them. No italic is bundled. `lwpdf fonts` lists the available default
weights:

```sh
$ lwpdf fonts
sans-regular
sans-bold
```

The optional `default-fonts-medium` feature also bundles Source Sans 3 Medium under the
`sans-medium` key, at the size cost listed in [Cargo features](../../reference/features/).

## Subsetting

Text is embedded as real Unicode via Type 0 / CIDFontType2 fonts. The subsetter in this repository
embeds only the glyphs a document actually uses. A character the font has no glyph for is
reported as a `MissingGlyph` layout warning rather than failing silently.

## Your own fonts

`FontRegistry` is a dynamic registry keyed by `FontKey`, not fixed to two weights. Build one with
your regular and bold TrueType files and render with it:

```rust
use lightweight_pdf::*;

let mut fonts = FontRegistry::with_fonts(REGULAR, BOLD)?;
fonts.register(FontKey::SANS_ITALIC, ITALIC)?;

let mut doc = Document::new(PageFormat::A4);
doc.add(Text::new("Set in a custom font."));
doc.add(Text::new("And in its italic.").italic());

let bytes = doc.render_with_fonts(&fonts)?;
```

- `render_with_fonts()` works without the `default-fonts` feature, so the bundled fonts stay out
  of your binary. `examples/demo_custom_font.rs` shows this and runs with
  `--no-default-features`.
- `register()` and `register_named()` add further weights under any key, for example
  `FontKey::SANS_ITALIC` and `FontKey::SANS_BOLD_ITALIC`.
- Any static TrueType font with `glyf` outlines works. Variable fonts and CFF/OTF fonts are
  rejected.

## Missing font weights

`Text::italic()` and `.bold_italic()` render in italic once a font is registered under that key.
Without such a registration, `render()` and `render_with_diagnostics()` return
`RenderError::MissingFont` instead of silently substituting the default font.

## In WASM and JavaScript

The WASM build can bundle the default fonts too, or register fonts at runtime with
`LightweightPdf.registerFont(key, bytes)`. See [WASM and JavaScript](../wasm/) for the trade-off
in a Worker.

## Licence

The bundled Source Sans 3 fonts are under the SIL Open Font License 1.1; the licence text is in
`assets/fonts/LICENSES/`. The OFL permits embedding and subsetting in generated documents, so the
PDFs you generate carry no OFL obligation.
