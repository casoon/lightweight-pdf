# Changelog

All notable changes to this project are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions are the ones published on
crates.io; all workspace crates share one version.

## [Unreleased]

### Added

- Table cell vertical alignment (`VerticalAlign::Top`, `Middle`, `Bottom`) on `TableCell`, `TableColumn`, and `Table`, plus `Table::min_row_height` so fixed-size grids can centre their content (#35).
- Source Sans 3 Medium font weight behind optional `default-fonts-medium` feature (#36).
- Measurement API: `measure_text` and `measure_element` in Rust and JavaScript WASM bindings (#37).

## [0.3.0] - 2026-09-01

### Added

- PDF/A-3b output with `Document::pdf_a3b()` (`pdf-a` feature), verified with veraPDF (#25).
- ZUGFeRD/Factur-X invoice XML embedding with `Document::zugferd_xml()` (`zugferd` feature),
  verified with veraPDF and Mustang (#26).
- Tagged PDF/PDF-UA-1 output with `Document::pdf_ua()` (`tagged-pdf` feature), verified with
  veraPDF (#27).
- `lwpdf` command-line tool: `render`, `validate`, `fonts` and `schema` (#19).
- Data-driven JSON templates with `{{placeholders}}` and `$each` (#18).
- JSON (de)serialisation of documents (`serde` feature) (#17).
- wasm-bindgen bindings and the `@casoon/lightweight-pdf` package sources in `bindings/js` (#22).
- Cloudflare Worker starter with measured size and timings (#23) and a browser playground (#24).
- `lightweight-pdf-testing`: pixel-diff snapshot testing for PDFs (#21).
- `TableOfContents`, filled from the document's headings (#10).
- Soft hyphens and automatic hyphenation (`hyphenation` feature) (#13).
- `Text::rich()` with inline spans (#11).
- `Document::theme()` with named style roles (#16).
- Per-cell table styling (#15), table `rowspan` (#14) and `Table::from_rows()` (#20).
- PDF bookmarks from the heading hierarchy (#9) and internal links with `Text::anchor()` and
  `.link_to()` (#8).
- `Align::Justify` for text (#12).
- Complete `/Info` metadata with creation and modification date, `/Producer` and a deterministic
  `/ID` (#3).
- Flate compression of content streams, font programs and image samples (`compress` feature, on
  by default) (#2).
- CI tracks WASM size and fails on an unannounced gzip size regression (#31).

### Changed

- A missing font variant is a typed error, `RenderError::MissingFont`, instead of a silent
  fallback (#5).
- Missing glyphs are reported as a layout warning instead of being dropped silently (#6).

### Fixed

- Non-deterministic PDF output when a document used several font weights.
- A panic when rendering with an empty font registry.

## [0.2.1] - 2026-08-24

### Added

- Page formats A3, A5, Letter, Legal and custom sizes, plus landscape and portrait orientation.
- PDF document metadata.
- Hyperlinks on text.
- Rounded and dashed borders.
- Table `colspan`.
- Italic font keys.

## [0.2.0] - 2026-08-22

### Breaking

- `lightweight-pdf-writer`: `PdfImage` renamed to `ImageXObject` and its `data` field to `bytes`;
  `ContentBuilder::text_rotated` takes a `TextRotation`; `PdfWriter::finish` is no longer public.

### Fixed

- A composite glyph with an invalid glyph ID could panic on malformed fonts.
- Missing overflow check on PNG dimensions.

## [0.1.0] - 2026-08-21

### Added

- First release: builder API over layout primitives, automatic pagination, own PDF writer and
  TrueType subsetter (`skrifa` for parsing), bundled Source Sans 3 fonts and a custom font
  registry (#1).
