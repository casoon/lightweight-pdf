//! `wasm-bindgen` bindings (issue #22, ADR-009 v2 — superseding the
//! "no JS-facing API in V1" note `lightweight_pdf_wasm_size_probe` still
//! carries from ADR-002; that pre-serde deferral is exactly what #17's
//! versioned JSON format resolved).
//!
//! Minimal surface, on purpose: a document/template goes in as a JSON
//! *string* (`Document::from_json` already does the real parsing/
//! validation — no reason to duplicate that at the JS boundary with a
//! second, less strict object-walking path), fonts register as raw
//! bytes, and a render call returns either a `Uint8Array` or a
//! `RenderResult` carrying both the bytes and structured diagnostics —
//! never a bare string a caller has to parse themselves. The npm
//! package's TypeScript wrapper is what gives callers the nicer
//! `render(document: Document)` object-based API the issue describes;
//! `JSON.stringify` before crossing into wasm is cheap compared to the
//! render itself and keeps this Rust-side surface tiny.

use crate::{Document, DocumentExt, FontKey, FontRegistry};
use lightweight_pdf_layout::{LayoutWarning, LayoutWarningKind};
use wasm_bindgen::prelude::*;

fn to_js_error(err: impl std::fmt::Display) -> JsError {
    JsError::new(&err.to_string())
}

#[derive(serde::Serialize)]
struct WasmWarning {
    page: usize,
    kind: &'static str,
    hint: String,
}

impl From<&LayoutWarning> for WasmWarning {
    fn from(w: &LayoutWarning) -> Self {
        let kind = match w.kind {
            LayoutWarningKind::TextClipped => "text_clipped",
            LayoutWarningKind::ContentOverflow => "content_overflow",
            LayoutWarningKind::ForcedPageBreak => "forced_page_break",
            LayoutWarningKind::HeaderFooterOverflow => "header_footer_overflow",
            LayoutWarningKind::MissingGlyph { .. } => "missing_glyph",
            LayoutWarningKind::TableRowOverflow => "table_row_overflow",
            LayoutWarningKind::MissingAltText => "missing_alt_text",
        };
        WasmWarning {
            page: w.page,
            kind,
            hint: w.element_hint.clone(),
        }
    }
}

/// `render_with_diagnostics`'s return value: `bytes` is a real
/// `Uint8Array` (wasm-bindgen's built-in `Vec<u8>` mapping, not a JSON
/// array of numbers), `warnings` a structured array of objects — a
/// caller never has to parse a string for either.
#[wasm_bindgen]
pub struct RenderResult {
    bytes: Vec<u8>,
    warnings: Vec<WasmWarning>,
}

#[wasm_bindgen]
impl RenderResult {
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn warnings(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(&self.warnings).map_err(to_js_error)
    }
}

/// The JS-facing entry point: holds the registered fonts (rendering
/// needs at least one), otherwise stateless per call.
#[wasm_bindgen]
pub struct LightweightPdf {
    fonts: FontRegistry,
}

impl Default for LightweightPdf {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl LightweightPdf {
    /// No fonts registered yet — call `registerFont` at least once
    /// before `render`/`renderWithDiagnostics`, or use
    /// `withDefaultFonts()` for the bundled Source Sans 3 pair.
    #[wasm_bindgen(constructor)]
    pub fn new() -> LightweightPdf {
        LightweightPdf {
            fonts: FontRegistry::empty(),
        }
    }

    #[cfg(feature = "default-fonts")]
    #[wasm_bindgen(js_name = withDefaultFonts)]
    pub fn with_default_fonts() -> Result<LightweightPdf, JsError> {
        Ok(LightweightPdf {
            fonts: FontRegistry::with_defaults().map_err(to_js_error)?,
        })
    }

    /// Registers a TrueType font under `key` (e.g. `"sans-regular"`,
    /// `"sans-bold"`, or any caller-chosen name referenced from a
    /// document's `style.font`). `key` is leaked once per distinct value
    /// to satisfy `FontKey`'s `'static` string requirement — the same
    /// documented, one-time-per-name tradeoff `Document::from_json`
    /// already makes for font keys read out of JSON (see `FontKey`'s own
    /// doc comment); a wasm module registering a bounded, small set of
    /// fonts once at startup is exactly that acceptable case.
    #[wasm_bindgen(js_name = registerFont)]
    pub fn register_font(&mut self, key: &str, bytes: &[u8]) -> Result<(), JsError> {
        let key = FontKey(Box::leak(key.to_string().into_boxed_str()));
        self.fonts.register(key, bytes).map_err(to_js_error)
    }

    /// Parses `document_json` (`Document::from_json`) and renders it,
    /// returning the PDF bytes as a `Uint8Array`.
    pub fn render(&self, document_json: &str) -> Result<Vec<u8>, JsError> {
        let doc = Document::from_json(document_json).map_err(to_js_error)?;
        doc.render_with_fonts(&self.fonts).map_err(to_js_error)
    }

    /// As `render`, plus layout diagnostics as a `RenderResult` instead
    /// of a bare byte array.
    #[wasm_bindgen(js_name = renderWithDiagnostics)]
    pub fn render_with_diagnostics(&self, document_json: &str) -> Result<RenderResult, JsError> {
        let doc = Document::from_json(document_json).map_err(to_js_error)?;
        let (bytes, warnings) = doc.render_with_fonts_and_diagnostics(&self.fonts).map_err(to_js_error)?;
        Ok(RenderResult {
            bytes,
            warnings: warnings.iter().map(WasmWarning::from).collect(),
        })
    }

    /// Resolves `template_json`'s `{{placeholders}}`/`$each` against
    /// `data_json` (issue #18) and renders the result — the same
    /// `Document::from_template` the CLI's `render --data` uses.
    #[wasm_bindgen(js_name = renderTemplate)]
    pub fn render_template(&self, template_json: &str, data_json: &str, allow_missing: bool) -> Result<Vec<u8>, JsError> {
        let on_missing = if allow_missing {
            crate::MissingPlaceholder::Empty
        } else {
            crate::MissingPlaceholder::Error
        };
        let doc = Document::from_template(template_json, data_json, on_missing).map_err(to_js_error)?;
        doc.render_with_fonts(&self.fonts).map_err(to_js_error)
    }
}
