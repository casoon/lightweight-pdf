// Thin wrapper over the wasm-bindgen bindings (`../pkg`, built by
// `npm run build:wasm`): the Rust side takes a JSON *string* (see
// `lightweight-pdf`'s `wasm_bindings` module doc comment for why), this
// gives callers the `render(document: Document)` object-based API the
// issue describes instead.

import init, { LightweightPdf, RenderResult } from "../pkg/lightweight_pdf.js";
import type { InitInput } from "../pkg/lightweight_pdf.js";
import type { Document } from "./document.js";

export type { Document } from "./document.js";
export { LightweightPdf, RenderResult };

let ready: Promise<unknown> | null = null;
let defaultRenderer: LightweightPdf | null = null;

/**
 * Node has no `fetch` support for `file://` URLs (the `--target web`
 * build's default `init()` loading path), so read the wasm bytes
 * directly there instead. Browsers/bundlers/edge runtimes either
 * polyfill `fetch` for their own asset pipeline or pass their own
 * `wasmInput` to `render`/`getDefaultRenderer` (e.g. a Cloudflare Worker
 * importing the `.wasm` file as an ES module).
 */
async function defaultWasmInput(): Promise<InitInput | undefined> {
  if (typeof process !== "undefined" && process.versions?.node) {
    const { readFile } = await import("node:fs/promises");
    return readFile(new URL("../pkg/lightweight_pdf_bg.wasm", import.meta.url));
  }
  return undefined;
}

/**
 * Loads the wasm module (once) and returns a renderer with the bundled
 * default fonts (Source Sans 3) already registered. `wasmInput` is
 * passed straight through to the generated `init()` — omit it to use
 * `defaultWasmInput()`'s Node-vs-fetch detection.
 */
export async function getDefaultRenderer(wasmInput?: InitInput): Promise<LightweightPdf> {
  ready ??= init({ module_or_path: wasmInput ?? (await defaultWasmInput()) });
  await ready;
  defaultRenderer ??= LightweightPdf.withDefaultFonts();
  return defaultRenderer;
}

/** Renders `document` to a PDF using the bundled default fonts. */
export async function render(document: Document, wasmInput?: InitInput): Promise<Uint8Array> {
  const renderer = await getDefaultRenderer(wasmInput);
  return renderer.render(JSON.stringify({ schema_version: 1, document }));
}
