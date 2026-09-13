---
title: Overview
description: What lightweight-pdf does, where it deliberately stops, and how this documentation is organised.
order: 0
---

lightweight-pdf generates PDFs for recurring business documents – invoices, quotes, delivery
notes, reports, certificates – in pure Rust. You describe a document with a builder API over a
fixed set of layout primitives; the library paginates it and writes the PDF. The same code runs
natively and as `wasm32-unknown-unknown`, for example inside a Cloudflare Worker.

The PDF writer (objects, xref, streams) and the TrueType subsetter are part of this repository.
The only required third-party dependency is [`skrifa`](https://crates.io/crates/skrifa) for font
parsing; `miniz_oxide` for Flate compression is optional and on by default.

## What it covers

- Layout primitives – text, rows, columns, tables, lists, images, lines, spacers, page breaks – with
  automatic, page-count-stable pagination, headers and footers, and a table of contents.
- Real Unicode text with embedded, subset fonts; Source Sans 3 is bundled.
- A versioned JSON document format and data-driven templates, plus the `lwpdf` command-line tool.
- A wasm build with JavaScript bindings, a Worker starter and a browser playground.
- PDF/A-3b, ZUGFeRD/Factur-X embedding and Tagged PDF/PDF-UA-1 behind Cargo features.

## Where it stops

This is not a typesetting system and has no markup language. There is no way to hand it HTML/CSS
or a full typesetting language and get a sensible result – a deliberate scope boundary. JSON
documents are a serialisation format, not a template language: no scripting, conditions or
expressions. See the [comparison](reference/comparison/) for how that plays out against other
tools.

## How the docs are organised

- **Getting started**: [install](getting-started/installation/) the crate or the CLI and
  [render a first document](getting-started/quickstart/).
- **Guides**: [layout](guides/layout/), [fonts](guides/fonts/),
  [JSON and templates](guides/json-and-templates/), [the CLI](guides/cli/),
  [WASM and JavaScript](guides/wasm/), [PDF/A, ZUGFeRD and PDF/UA](guides/conformance/) and
  [snapshot testing](guides/testing/).
- **Reference**: [Cargo features](reference/features/), [workspace crates](reference/crates/),
  [comparison](reference/comparison/) and [development](reference/development/). Item-level API
  documentation lives on [docs.rs](https://docs.rs/lightweight-pdf).
