import { test } from "node:test";
import assert from "node:assert/strict";
import { measureText, measure } from "../dist/index.js";

test("measureText() measures single and multiline text", async () => {
  const short = await measureText("Hello World", { size: 14 }, 200);
  assert.ok(short.width > 0, "width should be positive");
  assert.ok(short.height > 0, "height should be positive");
  assert.equal(short.lines, 1, "expected 1 line");

  const long = await measureText(
    "A longer sentence intended to wrap across multiple lines when given a narrow width limit.",
    { size: 14 },
    80
  );
  assert.ok(long.lines > 1, "expected multiple lines when wrapped");
  assert.ok(long.height > short.height, "multiline height should exceed single line");
});

test("measure() measures elements", async () => {
  const textElem = await measure({ type: "text", content: "Sample element" }, 300);
  assert.ok(textElem.width > 0);
  assert.ok(textElem.height > 0);

  const tableElem = await measure(
    {
      type: "table",
      columns: [{ width: { fixed: 50 } }, { width: { fixed: 50 } }],
      rows: [
        [
          { element: { type: "text", content: "Cell 1" } },
          { element: { type: "text", content: "Cell 2" } },
        ],
      ],
    },
    200
  );
  assert.ok(tableElem.height > 0);
  assert.equal(tableElem.width, 200);
});
