---
title: CLI
description: lwpdf renders and validates JSON documents and templates, lists the default fonts and prints the JSON Schema – no Rust code needed.
order: 4
---

Install with `cargo install lightweight-pdf-cli` (see [installation](../../getting-started/installation/)).
The binary is called `lwpdf`.

## Commands

```sh
lwpdf render examples/invoice-template.json --data examples/invoice-data.json -o invoice.pdf
lwpdf validate examples/invoice-template.json --data examples/invoice-data.json  # parse only, no PDF
lwpdf fonts                                                                       # default font weights
lwpdf schema                                                                      # JSON Schema of the format
```

| Command | What it does |
| --- | --- |
| `render <input> -o <file>` | Renders a JSON document to a PDF. With `--data <file>`, `input` is a [template](../json-and-templates/) resolved against that data first. |
| `validate <input>` | Parses (and with `--data`, resolves) the input without rendering. |
| `fonts` | Lists the font weights available by default (bundled Source Sans 3). |
| `schema` | Prints the JSON Schema for the document and template format to stdout. |

`--allow-missing` on `render` and `validate` resolves a missing placeholder to an empty string
instead of failing.

## Output and exit codes

Diagnostics – layout warnings and errors – go to stderr, one line each. This is the output of a
real run, captured while this site was built:

```text
$ lwpdf validate examples/invoice-template.json --data examples/invoice-data.json
ok: examples/invoice-template.json is valid
$ lwpdf render examples/invoice-template.json --data examples/invoice-data.json -o invoice.pdf
wrote invoice.pdf (10435 bytes)
```

| Exit code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | The input was valid, but rendering failed – for example a missing font weight |
| `2` | Input problem – missing file, malformed JSON, an unresolved template placeholder |

The CLI is its own crate, `lightweight-pdf-cli`, so the library never depends on `clap`.
