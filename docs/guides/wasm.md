---
title: WASM and JavaScript
description: The wasm32 build, the JavaScript API of @casoon/lightweight-pdf, the Cloudflare Worker starter, the browser playground and measured size and speed.
order: 5
---

lightweight-pdf compiles to `wasm32-unknown-unknown`. CI builds the whole workspace for that
target on every run.

## The JavaScript package

`bindings/js` is the `@casoon/lightweight-pdf` package: the Rust engine compiled with
`wasm-bindgen` (features `wasm`, `default-fonts`, `compress`), for Node.js and edge runtimes such
as Cloudflare Workers, with no native dependencies. It is not on the npm registry at the time of
writing – build it from the repository:

```sh
cd bindings/js
npm install
npm run build   # wasm-pack build -> JSON Schema -> generated TypeScript types -> tsc
npm test
```

This needs the Rust toolchain with the `wasm32-unknown-unknown` target, `wasm-pack` and `wasm-opt`
from [binaryen](https://github.com/WebAssembly/binaryen).

```ts
import { render } from "@casoon/lightweight-pdf";
import { writeFileSync } from "node:fs";

const bytes = await render({
  page_format: "A4",
  children: [
    { type: "text", content: "Invoice #2026-100", style: { size: 20, font: "sans-bold" } },
    {
      type: "table",
      columns: [{ width: { flex: 1 } }, { width: { fixed: 80 }, align: "end" }],
      header: [{ element: { type: "text", content: "Item" } }, { element: { type: "text", content: "Amount" } }],
      rows: [[{ element: { type: "text", content: "Consulting" } }, { element: { type: "text", content: "1,200.00 USD" } }]],
    },
  ],
});

writeFileSync("invoice.pdf", bytes);
```

The `Document` type is generated from the same JSON Schema that `lwpdf schema` prints, so it
always matches what the Rust side accepts. The format itself is described in
[JSON and templates](../json-and-templates/).

### Diagnostics, fonts and templates

```ts
import { LightweightPdf } from "@casoon/lightweight-pdf";

const renderer = LightweightPdf.withDefaultFonts(); // bundled Source Sans 3
// or: const renderer = new LightweightPdf(); renderer.registerFont("sans-regular", fontBytes);

const result = renderer.renderWithDiagnostics(JSON.stringify({ schema_version: 1, document }));
result.bytes;    // Uint8Array
result.warnings; // e.g. [{ page: 1, kind: "text_clipped", hint: "..." }]

const pdf = renderer.renderTemplate(templateJson, dataJson, /* allowMissing */ false);
```

## Cloudflare Worker starter

`examples/worker/` is a Worker that turns a `POST` with a JSON document into a PDF response:

```sh
# from the repository root
cd bindings/js && npm install && npm run build && cd ../../examples/worker
npm install
npx wrangler dev      # http://localhost:8787
npx wrangler deploy   # needs `wrangler login` first
```

```sh
curl -X POST http://localhost:8787 \
  -H 'content-type: application/json' \
  -d '{"page_format":"A4","children":[{"type":"text","content":"Hello from the edge"}]}' \
  -o hello.pdf
```

The starter compiles the fonts into the module: one file, no asset lookup on the request path,
but every bundled weight counts towards the upload size. The alternative – registering fonts per
request with `registerFont` from Worker static assets or R2 – keeps the module smaller and loads
only the weights a request uses, at the cost of an extra read before the first render. It pays
off once you need more than a couple of weights.

## Browser playground

`examples/playground/` is a static Astro site: a JSON editor on the left, the browser's own PDF
viewer on the right, rendered entirely client-side through the WASM build. It offers four
templates adapted from the demos (invoice, offer, report, docs), a download button and shareable
links that carry the JSON in the URL fragment. See its README to run it.

## Size and speed

Measured, not estimated:

- **WASM module** (`wasm` + `default-fonts` + `compress`, as shipped in the package):
  1252.95 KiB raw, 590.37 KiB gzip, from `wrangler deploy --dry-run`'s upload-size report.
- **Worker timings** in a local `wrangler dev` (the `workerd` runtime, not the actual edge): cold
  start with the first render ~34–47 ms, warm renders ~11–25 ms including HTTP overhead.
- **Native render time**: about 4 ms per document for the invoice, offer and report demos (release
  build, average of five runs).
- **Output size**: those three demos are about 24–26 KB each with compression on.

Page count, table size and the number of font weights move render times more than anything else;
measure your own documents. CI's `wasm-size` job writes the module size to every run's job summary
and fails if the gzip size grows beyond the documented tolerance without an updated baseline.
