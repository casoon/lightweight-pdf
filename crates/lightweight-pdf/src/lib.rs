//! Public facade for `lightweight-pdf`: re-exports the `lightweight-pdf-core` builder API
//! and adds `Document::render()` (ADR-002 — this is the crate users add to
//! `Cargo.toml`, the place `render()` becomes public).

mod fonts;
mod images;
mod render;

pub use fonts::FontRegistry;
pub use images::ImageEmbedError;
pub use lightweight_pdf_core::*;
pub use lightweight_pdf_fonts::FontError;
pub use lightweight_pdf_layout::{LayoutWarning, LayoutWarningKind};
pub use render::{DocumentExt, RenderError};

#[cfg(all(feature = "wasm-size-probe", not(feature = "default-fonts")))]
compile_error!(
    "wasm-size-probe needs a font source: enable `default-fonts` alongside it (see plan/00a-contracts-and-artifacts.md, point 2)"
);

/// Internal, non-public measurement export (`plan/00a-contracts-and-artifacts.md`
/// point 1): renders a small but complete document end-to-end so the real
/// render path isn't dead-code-eliminated from the size measurement. Not a
/// public runtime API (ADR-009: no JS-facing API in V1).
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
