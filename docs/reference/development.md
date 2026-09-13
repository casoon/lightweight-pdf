---
title: Development
description: Building and testing the workspace, regenerating snapshots and previews, CI, and the release order for crates.io.
order: 4
---

## Build and test

```sh
cargo test --workspace                           # unit and integration tests
cargo test -p lightweight-pdf --features png     # including the PNG path
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --target wasm32-unknown-unknown --release
```

The integration tests shell out to `qpdf` and `pdftotext`, the snapshot tests to `pdftoppm` – all
from the `qpdf` and `poppler-utils` packages.

`crates/lightweight-pdf/tests/snapshots.rs` pixel-diffs a handful of representative documents
against low-DPI grayscale references in `test-fixtures/snapshots/` using
[lightweight-pdf-testing](../../guides/testing/). After an intentional visual change:

```sh
UPDATE_SNAPSHOTS=1 cargo test -p lightweight-pdf --test snapshots
```

## Generated files

- `scripts/render-readme-previews.sh` regenerates `assets/demo_*.png`, the page previews in the
  README.
- `scripts/render-site-showcase.sh` renders the PDFs and page previews for this site into
  `site/public/pdf/`. The Pages workflow runs it before every site build; run it locally before
  `pnpm build` in `site/`.
- The demo examples write `examples/demo_*.pdf`, which are checked in as sample output.

## CI

`.github/workflows/ci.yml` runs formatting, clippy with all features, the test suite with default
and all features, a `--no-default-features` build, the wasm32 build, the invoice and report
examples, crate boundary checks, `cargo audit`, and the veraPDF conformance checks.

The `wasm-size` job builds the WASM module in two configurations and writes raw, `wasm-opt -Oz`
and gzip sizes to the job summary. It compares the gzip size against
`.github/wasm-size-baseline.json` and fails if it grows beyond the documented tolerance without
the baseline being updated in the same commit.

## Releasing

All published crates share `workspace.package.version`, so a release bumps them together. Path
dependencies between them carry a matching `version` requirement, as `cargo publish` requires.
Publish in dependency order; each step works only once the previous crate is live (retry if
crates.io's index has not caught up yet):

```sh
cargo publish -p lightweight-pdf-writer
cargo publish -p lightweight-pdf-core
cargo publish -p lightweight-pdf-fonts
cargo publish -p lightweight-pdf-layout   # needs lightweight-pdf-core
cargo publish -p lightweight-pdf          # needs all four
cargo publish -p lightweight-pdf-cli      # needs lightweight-pdf
cargo publish -p lightweight-pdf-testing
```

The npm package is published by `.github/workflows/release-npm.yml`, triggered by an `npm-v*.*.*`
tag only – never part of the pull-request path.
