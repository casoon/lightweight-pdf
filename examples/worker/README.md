# lightweight-pdf Cloudflare Worker starter (issue #23)

A Worker that turns a `POST` with a JSON document/template body into a PDF
response — the use case `lightweight-pdf` is explicitly built for ("runnable
as `wasm32-unknown-unknown` inside a Cloudflare Worker"), deployable in
about five minutes.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/casoon/lightweight-pdf)

The button clones the whole monorepo; if your Cloudflare account doesn't
pick up `examples/worker` as the deploy target automatically, the manual
path below is the one this README's own measurements were taken with —
build the `@casoon/lightweight-pdf` package once, then deploy this
directory:

```sh
# from the repo root
cd bindings/js && npm install && npm run build && cd ../../examples/worker
npm install
npx wrangler dev      # local dev server, http://localhost:8787
npx wrangler deploy   # real deploy — needs `wrangler login` first
```

```sh
curl -X POST http://localhost:8787 \
  -H 'content-type: application/json' \
  -d '{"page_format":"A4","children":[{"type":"text","content":"Hello from the edge"}]}' \
  -o hello.pdf
```

## Fonts: compiled in, not fetched as an asset

This template uses `default-fonts` (Source Sans 3 regular/bold compiled
directly into the wasm module) rather than fetching font bytes as a
separate Worker asset at request time. Trade-off, both real:

- **Compiled in (this template):** the whole module — engine + fonts — is
  one wasm file, uploaded once at deploy time. No extra request/asset
  lookup on the request path, simplest to reason about; but every font
  weight you bundle is permanently part of the uploaded module size,
  whether or not a given request uses it.
- **Fetched as an asset (not implemented here):** register fonts at
  request time via `LightweightPdf.registerFont(key, bytes)` (see the npm
  package's README) from a [Worker Static Asset](https://developers.cloudflare.com/workers/static-assets/)
  or R2 bucket. Smaller core module, fonts only loaded for the weights
  actually used per-request — at the cost of an extra asset read before
  the first render (and its own cold-start/caching behavior to reason
  about). Worth it once you need more than a couple of font weights, or
  fonts too large to justify baking into every deploy.

## Measured numbers (issue #23's acceptance criterion)

Measured locally via `wrangler dev`/`wrangler deploy --dry-run` (the same
`workerd` runtime Cloudflare's edge runs, but not an actual edge
deployment — this repo has no Cloudflare account to deploy from):

- **Module size** (`wrangler deploy --dry-run`, the authoritative number
  Cloudflare's own upload-size limits are checked against): **1252.95 KiB
  raw / 590.37 KiB gzip.**
- **Cold start** (first request against a freshly started `wrangler dev`
  process — wasm compile + instantiate + font registry setup + one
  render, averaged over 3 runs): **~34–47 ms.**
- **Warm render** (subsequent requests against the same running worker,
  same document): **~11–25 ms**, including HTTP overhead from `curl`
  itself.

These are directional, not a substitute for measuring your own documents
on Cloudflare's actual edge — page count, table size, and font-weight
count all move the render-time number more than anything else here does.
