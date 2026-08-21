//! Bridges `lightweight-pdf-fonts::FontData` to `lightweight-pdf-layout::FontResolver`
//! (ADR-010: "Font-Bridge liegt an der Facade"). V1 only ever resolves the
//! two default weights (`FontKey::SANS_REGULAR`/`SANS_BOLD`) — a registry
//! for arbitrary custom fonts is Phase 4+ follow-up scope beyond this
//! document's two-weight default.

use lightweight_pdf_core::FontKey;
use lightweight_pdf_fonts::{EmbeddedFontMetrics, FontData, FontError};
use lightweight_pdf_layout::{FontMetrics, FontResolver};

#[cfg(feature = "default-fonts")]
const SANS_REGULAR_BYTES: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-Regular.ttf");
#[cfg(feature = "default-fonts")]
const SANS_BOLD_BYTES: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-Bold.ttf");

/// Local newtype so `lightweight-pdf-layout`'s `FontMetrics` trait (foreign to this
/// crate) can be implemented for `lightweight-pdf-fonts`' metrics type (also
/// foreign) — Rust's orphan rules require the trait *or* the type to be
/// local, so a thin local wrapper is the standard way to bridge two
/// external crates without either depending on the other.
struct MetricsAdapter(pub(crate) EmbeddedFontMetrics);

impl FontMetrics for MetricsAdapter {
    fn advance(&self, ch: char) -> f32 {
        // Fallback width for characters the font has no glyph for: roughly
        // a notdef-box width, keeps wrapping usable rather than panicking.
        self.0.advance_1000(ch).unwrap_or(500.0)
    }

    fn ascent(&self) -> f32 {
        self.0.ascent
    }

    fn descent(&self) -> f32 {
        self.0.descent
    }
}

pub struct FontEntry {
    pub data: FontData,
    pub base_font_name: &'static str,
    adapter: MetricsAdapter,
}

impl FontEntry {
    // Only called from `with_defaults()` below, which is itself `#[cfg]`-gated
    // on `default-fonts` — see `render.rs`'s doc comment for why an unused
    // `allow` is intentional here rather than a real bug.
    #[cfg_attr(not(feature = "default-fonts"), allow(dead_code))]
    fn new(bytes: &[u8], base_font_name: &'static str) -> Result<Self, FontError> {
        let data = FontData::load(bytes.to_vec())?;
        let metrics = EmbeddedFontMetrics::from_font_data(&data)?;
        Ok(FontEntry {
            data,
            base_font_name,
            adapter: MetricsAdapter(metrics),
        })
    }

    /// FontDescriptor fields (ascent/descent/cap height/bbox/...) come from
    /// here — the same metrics used for layout, not recomputed separately.
    pub fn metrics(&self) -> &EmbeddedFontMetrics {
        &self.adapter.0
    }
}

pub struct FontRegistry {
    pub regular: FontEntry,
    pub bold: FontEntry,
}

impl FontRegistry {
    #[cfg(feature = "default-fonts")]
    pub fn with_defaults() -> Result<Self, FontError> {
        Ok(FontRegistry {
            regular: FontEntry::new(SANS_REGULAR_BYTES, "SourceSans3-Subset")?,
            bold: FontEntry::new(SANS_BOLD_BYTES, "SourceSans3-Bold-Subset")?,
        })
    }

    /// Order matches how the facade registers PDF fonts — used to build
    /// resource names (`F1`, `F2`, ...) consistently between PDF font
    /// registration and content-stream references.
    pub fn font_entries(&self) -> [(FontKey, &FontEntry); 2] {
        [(FontKey::SANS_REGULAR, &self.regular), (FontKey::SANS_BOLD, &self.bold)]
    }

    pub fn entry(&self, key: FontKey) -> &FontEntry {
        if key == FontKey::SANS_BOLD {
            &self.bold
        } else {
            &self.regular
        }
    }
}

impl FontResolver for FontRegistry {
    fn metrics(&self, key: FontKey) -> &dyn FontMetrics {
        &self.entry(key).adapter
    }
}
