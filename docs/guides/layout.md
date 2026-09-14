---
title: Layout
description: Layout primitives, page setup, text, tables, headers and footers, links, bookmarks, the table of contents, themes and watermarks.
order: 1
---

A document is a flow of elements. You add them in reading order; the layout engine measures them,
breaks them into pages and writes each page.

## Primitives

| Element | Purpose |
| --- | --- |
| `Text` | A paragraph: wrapping, alignment, styles, links, hyphenation |
| `Row`, `Column` | Horizontal and vertical containers with `gap` and `flex` sizing |
| `Table` | Columns, header, rows; splits across pages |
| `List` | Bullet and numbered items |
| `Image` | JPEG or PNG |
| `Line` | A horizontal rule |
| `Spacer` | Fixed vertical space |
| `PageBreak` | Forces a new page |
| `TableOfContents` | Filled from the document's headings |

Containers and elements share `flex`, alignment (`Align::Start`, `Center`, `End`, and
`Align::Justify` for text), padding, border and background. Borders can have rounded corners
(`.corner_radius()`) and can be dashed (`Border::dashed(width, color, dash, gap)`).

```rust
let cell = |s: &str| Text::new(s).size(10.0);
doc.add(Row::new().gap(8.0).child(cell("Quantity")).child(cell("Price")));
doc.add(Line::new());
doc.add(Spacer::new(10.0));
```

## Page setup

- Formats: `PageFormat::A3`, `A4`, `A5`, `Letter`, `Legal` and `Custom(width, height)` in points.
- Orientation: `Document::landscape()` and `.portrait()`.
- Margins: `Margin::all(..)`, `Margin::symmetric(..)`.

## Text

`Text::new("…")` takes styles such as `.size()`, `.bold()`, `.italic()`, `.bold_italic()`,
`.color(Color::rgb(..))`, `.font(FontKey)` and `.align(..)`. Presets: `.heading1()`,
`.heading2()`, `.heading3()`, `.caption()`, `.muted()`, `.table_header()`.

- **Justify**: `Align::Justify` sets every line flush with both edges except the last line of the
  paragraph.
- **Rich text**: `Text::rich([Span::new("…", style), …])` mixes fonts, sizes and colours in one
  paragraph. Mixed sizes on a line share its baseline, and a page break can fall inside a span.
  Rich text does not support `Align::Justify`, `.url()`, `.link_to()` or `.outline_level()`.
- **Hyphenation**: a soft hyphen (U+00AD) marks an optional break point. It is shown as `-` only
  if the line breaks there and is absent from extracted text otherwise. `Text::hyphenate(..)`
  inserts break points automatically from Knuth-Liang patterns for US English and German – this
  needs the `hyphenation` feature, which roughly quadruples the binary and WASM size.

## Tables

```rust
Table::new()
    .columns([TableColumn::flex(1.0), TableColumn::fixed(80.0).align(Align::End)])
    .header(["Check", "Status"])
    .rows(vec![
        vec![Element::from("Structure tree"), Element::from("yes")],
        vec![Element::from("Reading order"), Element::from("yes")],
    ])
```

- Tables split across pages and repeat the header automatically; rows can be striped.
- Cells support `colspan` and `rowspan`. A `rowspan` never splits across a page break – the whole
  span moves to the next page.
- `TableCell` overrides background, border, padding and alignment per cell: cell beats row beats
  column.
- `vertical_align` (`top`, `middle`, `bottom`) positions content inside a taller row; set it on the
  table, a column or a cell (cell beats column beats table). `min_row_height` gives every row a
  minimum height, so one-letter cells can form a square grid; taller content still grows its row.
- Header cells built from plain strings pick up the theme's `table_header` style.
- `Table::from_rows(&items)` builds rows from anything that implements `TableRow`.

## Lists and images

```rust
doc.add(
    List::new()
        .bullet(Text::new("Headings tagged H1/H2"))
        .numbered(Text::new("Image tagged Figure with alt text")),
);
doc.add(Image::new(LOGO_JPEG)?.width(100.0).alt("Company logo"));
```

`Image::new` validates the bytes. JPEG is embedded unchanged (`DCTDecode`). PNG is decoded and
re-embedded with a separate soft mask for the alpha channel – only with the `png` feature;
without it, embedding a PNG fails with `ImageEmbedError::PngFeatureDisabled`.

## Pagination, headers and footers

Pagination is automatic and runs in two passes, so page counts in headers and footers are stable.
`Header::new(height, |ctx| …)` and `Footer::new(height, |ctx| …)` reserve a band on every page and
receive `ctx.page` and `ctx.total_pages`. The engine applies widow and orphan rules, and
`.keep_with_next()` keeps an element on the same page as the one after it.

## Links, bookmarks and table of contents

- `Text::url("https://…")` adds a URI link annotation over the text.
- `Text::anchor(name)` and `.link_to(name)` create internal jumps, resolved once pagination is
  final.
- `.heading1()`, `.heading2()` and `.heading3()` build the PDF bookmark tree (`/Outlines`);
  `.outline_level(n)` adds any other element. A document without headings has no `/Outlines`.
- `TableOfContents::new()` lists every heading in order with its page number and a clickable
  entry. `.max_depth(n)` limits the levels (default 3), `.leader(c)` sets the fill character
  between title and page number (default `.`, `' '` for none). It splits across pages like any
  other content.

## Themes

`Document::theme(Theme { .. })` defines named style roles – `body`, `caption`, `heading1`,
`heading2`, `heading3`, `table_header`, `muted` – resolved once, when an element is added. There
is no cascade. `Text::new()` and the presets use the theme until another style call such as
`.size()` or `.color()` opts them out; `.align()` is independent of theming. Without a `.theme(..)`
call, output is unchanged. `Theme` implements `Default`, so you can override single roles.

## Watermark and metadata

`Document::watermark(Watermark::new("DRAFT"))` draws rotated, repeated text beneath the page
content; `Watermark` has `rotation_deg`, `size`, `color` and `font` fields. There is deliberately
no general rotation or transform API.

Document metadata – title, author, subject, keywords, creator, creation and modification date –
goes to the PDF `/Info` dictionary via `Document::title()`, `.author()`,
`.creation_date(PdfDate::new(..))` and friends, or the `doc.metadata` fields. Every document also
gets a `/Producer` entry and a deterministic `/ID` hashed from the document content, never from a
random source.

## Layout warnings

`render_with_diagnostics()` reports what the layout had to compromise on, each with a page number
and an element hint: text clipped, content overflow, an element larger than a page forced onto
its own page, header or footer content taller than its band, a missing glyph, a table row taller
than the available space, and an image without alt text in a tagged PDF.
