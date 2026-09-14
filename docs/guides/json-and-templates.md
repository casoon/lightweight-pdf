---
title: JSON and templates
description: The versioned JSON document format, data-driven templates with placeholders and $each, and the JSON Schema they are validated against.
order: 3
---

Both need the `serde` feature on the `lightweight-pdf` crate. The [CLI](../cli/) and the
[JavaScript package](../wasm/) have it built in.

## JSON documents

`Document::from_json(&str)` and `.to_json()` (de)serialise the whole document tree. The root is
versioned:

```json
{
  "schema_version": 1,
  "document": {
    "page_format": "A4",
    "margin": { "top": 40.0, "right": 40.0, "bottom": 40.0, "left": 40.0 },
    "metadata": { "title": "Rechnung RE-2026-0100" },
    "children": [
      { "type": "text", "content": "Rechnung RE-2026-0100", "style": { "size": 24.0, "font": "sans-bold" }, "outline_level": 1 },
      { "type": "table_of_contents" },
      { "type": "list", "items": [{ "marker": "bullet", "content": { "type": "text", "content": "Zahlbar innerhalb 14 Tagen" } }] }
    ]
  }
}
```

(Shortened from `examples/document.json`.)

- Every element is tagged by a `"type"` field (`"text"`, `"table"`, `"list"`, …).
- Unknown fields anywhere in the tree are a parse error, never silently dropped.
- Images embed as base64: `{"bytes_base64": "...", "common": {...}}`.
- Header and footer are Rust closures and cannot be represented; `to_json()` refuses if either is
  set rather than dropping them.
- The same `Deserialize` implementation works with other formats (YAML, TOML) through
  `serde_json::from_str::<DocumentSchema>` and friends – bring your own crate.

It is a serialisation format, not a template language: no scripting, expressions or loops.

## Templates

`Document::from_template(template_json, data_json, MissingPlaceholder)` resolves placeholders in a
template document against a separate data document – no Rust code needed for the content:

```json
{
  "type": "table",
  "rows": [
    {
      "$each": "invoice.items",
      "template": [
        { "element": { "type": "text", "content": "{{description}}" } },
        { "element": { "type": "text", "content": "{{amount}}" } }
      ]
    }
  ]
}
```

- `"{{path.to.value}}"` as the entire string value resolves to the data's own JSON type – a number
  stays a number. Embedded in more text, it is always a string.
- A missing path is an error by default; `MissingPlaceholder::Empty` resolves it to an empty string
  instead.
- `{"$each": "items", "template": <value>}` as an array element expands to one copy of `template`
  per element of `items`. Inside, the element's own fields resolve first, then the outer data.
- Deliberately array iteration only: no conditions, expressions or filters.
- `render_template()` resolves a template without building a `Document`, for other consumers of
  the same JSON.

`examples/invoice-template.json` and `examples/invoice-data.json` are a complete pair; the
[showcase](../../../showcase/cli-template/) renders them with the CLI on every site build.

## JSON Schema

The `schemars` feature generates a JSON Schema for the document and template format from the Rust
types. `lwpdf schema` prints it, and the JavaScript package's TypeScript types are generated from
it – nothing is maintained by hand.
