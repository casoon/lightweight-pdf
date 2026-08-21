//! Bridges `lightweight-pdf-fonts::FontData` to `lightweight-pdf-layout::FontResolver`
//! (ADR-010: "Font-Bridge liegt an der Facade"). `FontRegistry` always
//! resolves exactly the two weights `FontKey::SANS_REGULAR`/`SANS_BOLD` —
//! either the bundled default (`with_defaults()`, needs the `default-fonts`
//! feature) or caller-supplied bytes (`with_fonts()`, always available). An
//! arbitrary-weight/arbitrary-`FontKey` registry beyond this fixed pair is
//! out of scope here (see the tracking issue linked from `with_fonts`).

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

    /// Builds a registry from caller-supplied static TrueType `glyf` fonts
    /// (ADR-012, same constraint as the bundled defaults) instead of Source
    /// Sans 3 — always available, independent of the `default-fonts`
    /// feature. Still exactly the two-weight `SANS_REGULAR`/`SANS_BOLD`
    /// model; an arbitrary-weight/arbitrary-`FontKey` registry is tracked
    /// separately (github.com/casoon/lightweight-pdf/issues/1).
    pub fn with_fonts(regular_bytes: &[u8], bold_bytes: &[u8]) -> Result<Self, FontError> {
        Ok(FontRegistry {
            regular: FontEntry::new(regular_bytes, "CustomFont-Regular-Subset")?,
            bold: FontEntry::new(bold_bytes, "CustomFont-Bold-Subset")?,
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
