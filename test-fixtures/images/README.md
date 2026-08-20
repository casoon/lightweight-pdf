Test-only image fixtures for Phase 5 (`lightweight-pdf-core`'s image validation
tests and `lightweight-pdf`'s facade rendering integration tests). Generated with
Pillow, not sourced from any third party — no license restrictions.
**Not embedded in the crate or the wasm binary**, referenced only via
`std::fs::read` in `#[cfg(test)]` code.

- `logo_rgba.png` / `logo_rgb.png` / `logo_baseline.jpg` / `logo_gray.jpg` —
  valid inputs (RGBA-with-transparency, opaque RGB, baseline RGB JPEG,
  baseline grayscale JPEG).
- `palette.png` (indexed color), `sixteen_bit.png` (16-bit depth),
  `progressive.jpg` (progressive DCT), `cmyk.jpg` (4-component CMYK) —
  each exercises one explicit-rejection path from `05-overflow-and-
  robustness.md` / `phases/phase-5-images.md`.
