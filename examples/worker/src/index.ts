// POST a JSON document (or template) to `/` and get a PDF back. Fonts are
// compiled into the wasm module (`default-fonts`) rather than fetched as a
// separate asset — see this example's README for the size/cold-start
// trade-off that choice makes, and the alternative.
//
// The wasm module is imported directly (wrangler's built-in `.wasm` module
// support) and passed to `render()` explicitly — Workers have no
// filesystem and no `fetch` for local assets, so the package's own
// Node-vs-browser `fetch` auto-detection is bypassed entirely here.
import wasmModule from "@casoon/lightweight-pdf/pkg/lightweight_pdf_bg.wasm";
import { render, type Document } from "@casoon/lightweight-pdf";

export default {
  async fetch(request: Request): Promise<Response> {
    if (request.method !== "POST") {
      return new Response("POST a JSON document to render a PDF — see this worker's README.md for an example.", {
        status: 405,
      });
    }

    let document: Document;
    try {
      document = await request.json();
    } catch {
      return new Response("invalid JSON body", { status: 400 });
    }

    try {
      const bytes = await render(document, wasmModule);
      return new Response(bytes, { headers: { "content-type": "application/pdf" } });
    } catch (err) {
      return new Response(`render failed: ${err}`, { status: 422 });
    }
  },
} satisfies ExportedHandler;
