//! Public facade for `lightweight-pdf`: re-exports the `lightweight-pdf-core` builder API
//! and adds `Document::render()` (ADR-002 — this is the crate users add to
//! `Cargo.toml`, the place `render()` becomes public).

mod fonts;
mod images;
mod render;
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
mod wasm_bindings;

pub use fonts::FontRegistry;
pub use images::ImageEmbedError;
pub use lightweight_pdf_core::*;
pub use lightweight_pdf_fonts::FontError;
pub use lightweight_pdf_layout::{measure_element, measure_text, LayoutWarning, LayoutWarningKind};
pub use render::{DocumentExt, RenderError};
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub use wasm_bindings::{LightweightPdf, RenderResult};

/// Measures `text` with the bundled default fonts. Builds a fresh
/// `FontRegistry` (parses the font files) on every call — for repeated
/// measurements create one with `FontRegistry::with_defaults()` and call
/// its `measure_text` instead.
#[cfg(feature = "default-fonts")]
pub fn measure_text_default(text: &str, style: &TextStyle, max_width: f32) -> Result<TextMeasurement, FontError> {
    let registry = FontRegistry::with_defaults()?;
    Ok(registry.measure_text(text, style, max_width))
}

/// Measures `element` with the bundled default fonts. Same per-call cost
/// as [`measure_text_default`]; reuse a `FontRegistry` for repeated calls.
#[cfg(feature = "default-fonts")]
pub fn measure_element_default(element: &Element, max_width: f32) -> Result<ElementMeasurement, FontError> {
    let registry = FontRegistry::with_defaults()?;
    Ok(registry.measure_element(element, max_width))
}

#[cfg(all(feature = "wasm-size-probe", not(feature = "default-fonts")))]
compile_error!(
    "wasm-size-probe needs a font source: enable `default-fonts` alongside it (see plan/00a-contracts-and-artifacts.md, point 2)"
);

/// Internal, non-public measurement export (`plan/00a-contracts-and-artifacts.md`
/// point 1): renders a small but complete document end-to-end so the real
/// render path isn't dead-code-eliminated from the size measurement.
/// Independent of the `wasm` feature's actual `wasm_bindings` module
/// (ADR-009 v2, issue #22) — this probe exists purely to catch dead-code
/// elimination regressions in the size measurement itself, not to be a
/// runtime API.
#[cfg(feature = "wasm-size-probe")]
#[no_mangle]
pub extern "C" fn lightweight_pdf_wasm_size_probe() -> i32 {
    let mut doc = Document::new(PageFormat::A4);
    doc.add(Text::new("Hallo Rechnung").size(18.0).bold());
    let cell = |s: &str| Text::new(s).size(10.0);
    doc.add(Row::new().gap(8.0).child(cell("Menge")).child(cell("Preis")));
    doc.add(Line::new());
    match doc.render() {
        // `bytes` comes from rendering the small, fixed document literally
        // constructed above (not attacker/caller-supplied input), so its
        // length is bounded far below `i32::MAX` in practice — `unwrap_or`
        // keeps that a non-panicking fallback rather than an `.expect()` on
        // this `pub extern "C"` FFI boundary.
        Ok(bytes) => i32::try_from(bytes.len()).unwrap_or(i32::MAX),
        Err(_) => -1,
    }
}
