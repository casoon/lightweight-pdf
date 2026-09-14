---
title: Workspace crates
description: The crates in the lightweight-pdf workspace, what each one contains and how they depend on each other.
order: 2
---

Depend on `lightweight-pdf`; the other library crates are its building blocks and are published
so that `cargo publish` can resolve them. All crates share one version.

| Crate | Contents | API docs |
| --- | --- | --- |
| `lightweight-pdf` | Facade: public API, `render()`, `FontRegistry`, `wasm` feature | [docs.rs](https://docs.rs/lightweight-pdf) |
| `lightweight-pdf-core` | Document model, elements, builder API | [docs.rs](https://docs.rs/lightweight-pdf-core) |
| `lightweight-pdf-layout` | `Layoutable` trait, pagination, text wrapping | [docs.rs](https://docs.rs/lightweight-pdf-layout) |
| `lightweight-pdf-writer` | PDF writer core: objects, xref, streams, fonts | [docs.rs](https://docs.rs/lightweight-pdf-writer) |
| `lightweight-pdf-fonts` | Font metrics and parsing (`skrifa`), subsetting | [docs.rs](https://docs.rs/lightweight-pdf-fonts) |
| `lightweight-pdf-cli` | The `lwpdf` binary, see [CLI](../../guides/cli/) | [crates.io](https://crates.io/crates/lightweight-pdf-cli) |
| `lightweight-pdf-testing` | Pixel-diff PDF snapshot testing, see [snapshot testing](../../guides/testing/) | [docs.rs](https://docs.rs/lightweight-pdf-testing) |
| `lightweight-pdf-test-support` | Internal, unpublished: qpdf/pdftotext helpers for the integration tests | – |

`bindings/js` holds the `@casoon/lightweight-pdf` JavaScript package, see
[WASM and JavaScript](../../guides/wasm/).

## Dependency direction

Dependencies point one way only: `core ← layout ← lightweight-pdf`. `writer` and `fonts` are
leaves without a path dependency on `core` or `layout`; CI enforces this with `cargo tree`. The CLI
is a separate crate so `clap` never enters the library's dependency tree.
