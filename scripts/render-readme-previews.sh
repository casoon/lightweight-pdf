#!/usr/bin/env bash
# Regenerates assets/demo_*.png: a PNG of page 1 of the invoice/offer/report
# demos, embedded in the README (issue #29). Run manually and commit the
# result whenever those demos' visual output changes — not wired into CI
# (see issue #32: keep CI from growing scope with every new example).
#
# Requires: cargo, pdftoppm (poppler-utils) — the same tool the
# lightweight-pdf-testing snapshot tests (issue #21) rasterize PDFs with.
set -euo pipefail
cd "$(dirname "$0")/.."

DPI=100
DEMOS=(demo_invoice demo_offer demo_report)

for demo in "${DEMOS[@]}"; do
  echo "==> cargo run -p lightweight-pdf --example $demo"
  cargo run -p lightweight-pdf --example "$demo" >/dev/null
  pdf="examples/${demo}.pdf"
  out="assets/${demo}"
  pdftoppm -png -r "$DPI" -f 1 -l 1 "$pdf" "$out"
  # pdftoppm names single-page output "$out-1.png"; drop the page suffix.
  mv "${out}-1.png" "${out}.png"
  echo "==> wrote assets/${demo}.png"
done
