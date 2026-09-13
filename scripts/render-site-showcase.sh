#!/usr/bin/env bash
# Renders the PDFs shown on the project site (site/src/showcase.ts) with this
# repository's own code, plus a PNG preview of every page. The Pages workflow
# (.github/workflows/pages.yml) runs this before the site build; run it
# locally before `pnpm build` in site/.
#
# Output goes to site/public/pdf/ (gitignored). The demo examples also
# rewrite their checked-in examples/demo_*.pdf as a side effect; rendering is
# deterministic, so a diff there means the demo's output really changed.
#
# Requires: cargo, pdftoppm (poppler-utils).
set -euo pipefail
cd "$(dirname "$0")/.."

OUT=site/public/pdf
DPI=110
DEMOS=(demo_invoice demo_offer demo_report demo_documentation demo_zugferd demo_pdf_ua)

rm -rf "$OUT"
mkdir -p "$OUT"

for demo in "${DEMOS[@]}"; do
  echo "==> cargo run -p lightweight-pdf --example $demo"
  cargo run --quiet -p lightweight-pdf --features zugferd,tagged-pdf,png --example "$demo" >/dev/null
  cp "examples/${demo}.pdf" "$OUT/${demo}.pdf"
done

# The CLI renders the JSON template + data from the README, run from the repo
# root exactly as documented; its terminal output is kept for the site.
echo "==> lwpdf render examples/invoice-template.json"
cargo build --quiet -p lightweight-pdf-cli
{
  echo "\$ lwpdf validate examples/invoice-template.json --data examples/invoice-data.json"
  cargo run --quiet -p lightweight-pdf-cli -- validate examples/invoice-template.json --data examples/invoice-data.json 2>&1
  echo "\$ lwpdf render examples/invoice-template.json --data examples/invoice-data.json -o invoice.pdf"
  cargo run --quiet -p lightweight-pdf-cli -- render examples/invoice-template.json --data examples/invoice-data.json -o invoice.pdf 2>&1
} >"$OUT/lwpdf-invoice-template.txt"
mv invoice.pdf "$OUT/lwpdf-invoice-template.pdf"

for pdf in "$OUT"/*.pdf; do
  pdftoppm -png -r "$DPI" "$pdf" "${pdf%.pdf}"
done

echo "==> wrote $(ls "$OUT" | wc -l | tr -d ' ') files to $OUT"
