# @casoon/lightweight-pdf

PDF generation from a JSON document (or `{{template}}` + data, see the
main [lightweight-pdf](https://github.com/casoon/lightweight-pdf) repo)
— compiled to WASM from the Rust [`lightweight-pdf`](https://crates.io/crates/lightweight-pdf)
crate. No native dependencies, no headless browser: runs in Node.js and
edge runtimes (Cloudflare Workers, ...).

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

`Document` (the type used above) is generated from the same JSON Schema
`lwpdf schema` prints — not hand-maintained — so it always matches
exactly what the Rust side accepts. See the main repo's README for the
full element catalog (tables, images, themes, table of contents, ...).

## Diagnostics and font registration

```ts
import { LightweightPdf } from "@casoon/lightweight-pdf";

const renderer = LightweightPdf.withDefaultFonts(); // bundled Source Sans 3
// or: const renderer = new LightweightPdf(); renderer.registerFont("sans-regular", fontBytes);

const result = renderer.renderWithDiagnostics(JSON.stringify({ schema_version: 1, document }));
result.bytes; // Uint8Array
result.warnings; // structured array, e.g. [{ page: 1, kind: "text_clipped", hint: "..." }]
```

## Building from source

```sh
npm install
npm run build   # wasm-pack build -> JSON Schema -> generated TypeScript types -> tsc
npm test
```

Requires the Rust toolchain (`wasm32-unknown-unknown` target),
`wasm-pack`, and `wasm-opt` (part of [binaryen](https://github.com/WebAssembly/binaryen)).
