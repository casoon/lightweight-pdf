import { test } from "node:test";
import assert from "node:assert/strict";
import { render } from "../dist/index.js";

test("render() produces a valid PDF from a plain object document", async () => {
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

  assert.ok(bytes instanceof Uint8Array, "expected a Uint8Array");
  assert.ok(bytes.length > 0, "expected a non-empty result");
  const header = Buffer.from(bytes.subarray(0, 5)).toString("ascii");
  assert.equal(header, "%PDF-", "expected a PDF header");
});
