# lightweight-pdf browser playground (issue #24)

A static [Astro](https://astro.build) site: a JSON editor on the left, a
live PDF preview on the right, rendered entirely client-side through the
`@casoon/lightweight-pdf` wasm build. No server involved — the acceptance
criterion this example exists to satisfy.

## Running locally

```sh
# from the repo root, build the npm package this example depends on
cd bindings/js && npm install && npm run build && cd ../../examples/playground

npm install
npm run dev       # http://localhost:4321
npm run build && npm run preview   # proves the static-build path works too
```

## What it does

- **Templates** — the picker loads one of four JSON documents adapted
  from the library's own demos (invoice, offer, report, docs). They're
  adaptations, not 1:1 ports of `crates/lightweight-pdf/examples/*.rs`:
  those Rust demos use `Header`/`Footer`, which aren't part of the JSON
  document schema (issue #17) and so can't be represented here.
- **Preview** — rendered bytes become a `blob:` URL loaded into an
  `<iframe>`, so the preview is the browser's own native PDF viewer, not
  a custom one.
- **Download** — the same rendered bytes, offered as a file download.
- **Shareable links** — "Copy shareable link" base64url-encodes the
  current editor text into the URL fragment (`#doc=...`); opening that
  URL restores the same JSON and re-renders it. Plain base64, not
  compressed — simple and robust for the small demo-sized documents this
  playground is meant for.

## Deploying

Static output (`npm run build` → `dist/`), deployable to Cloudflare
Pages or any static host — nothing here needs a Workers runtime.
