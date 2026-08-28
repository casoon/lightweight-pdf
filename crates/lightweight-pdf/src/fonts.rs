//! Bridges `lightweight-pdf-fonts::FontData` to `lightweight-pdf-layout::FontResolver`
//! (ADR-010: "Font-Bridge liegt an der Facade"). `FontRegistry` is a
//! dynamic `FontKey -> RegisteredFont` map (`register()`/`register_named()`
//! register arbitrary keys, not just `SANS_REGULAR`/`SANS_BOLD`). Two
//! convenience constructors cover the common case: `with_defaults()`
//! (bundled Source Sans 3 regular/bold, needs the `default-fonts` feature)
//! and `with_fonts()` (caller-supplied regular/bold bytes, always
//! available). Looking up a key that was never registered (e.g.
//! `SANS_ITALIC`/`SANS_BOLD_ITALIC` without a matching `register()` call)
//! falls back to the registry's default key (`SANS_REGULAR`) rather than
//! erroring — see `entry()`.

use lightweight_pdf_core::FontKey;
use lightweight_pdf_fonts::{EmbeddedFontMetrics, FontData, FontError};
use lightweight_pdf_layout::{FontMetrics, FontResolver};

// Crate-local copy (not the repo-root `assets/fonts/`): `cargo package`
// only bundles files inside the crate's own directory, so a path reaching
// outside it (`../../../assets/...`) silently drops the font files from
// the published tarball — verified missing via `cargo publish --dry-run`,
// which fails the packaged crate's own build with a "file not found" once
// it's extracted and compiled in isolation. The repo-root copy stays too
// (used by `lightweight-pdf-fonts`' own tests and referenced from
// `README.md`), so this does duplicate ~860KB — the accepted cost of a
// crate that must be self-contained once published.
#[cfg(feature = "default-fonts")]
const SANS_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Regular.ttf");
#[cfg(feature = "default-fonts")]
const SANS_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Bold.ttf");

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
        // The layout crate surfaces the miss itself via `has_glyph()` /
        // `LayoutWarningKind::MissingGlyph`, so this no longer fails silently.
        self.0.advance_1000(ch).unwrap_or(500.0)
    }

    fn ascent(&self) -> f32 {
        self.0.ascent
    }

    fn descent(&self) -> f32 {
        self.0.descent
    }

    fn has_glyph(&self, ch: char) -> bool {
        self.0.advance_1000(ch).is_some()
    }
}

pub struct RegisteredFont {
    pub font_data: FontData,
    pub base_font_name: String,
    adapter: MetricsAdapter,
}

impl RegisteredFont {
    fn new(bytes: &[u8], base_font_name: impl Into<String>) -> Result<Self, FontError> {
        let font_data = FontData::load(bytes.to_vec())?;
        let metrics = EmbeddedFontMetrics::from_font_data(&font_data)?;
        Ok(RegisteredFont {
            font_data,
            base_font_name: base_font_name.into(),
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
    fonts: std::collections::HashMap<FontKey, RegisteredFont>,
    default_key: FontKey,
}

impl FontRegistry {
    pub fn empty() -> Self {
        FontRegistry {
            fonts: std::collections::HashMap::new(),
            default_key: FontKey::SANS_REGULAR,
        }
    }

    #[cfg(feature = "default-fonts")]
    pub fn with_defaults() -> Result<Self, FontError> {
        let mut reg = Self::empty();
        reg.register_named(FontKey::SANS_REGULAR, "SourceSans3-Subset", SANS_REGULAR_BYTES)?;
        reg.register_named(FontKey::SANS_BOLD, "SourceSans3-Bold-Subset", SANS_BOLD_BYTES)?;
        Ok(reg)
    }

    /// Builds a registry from caller-supplied static TrueType `glyf` fonts
    /// (ADR-012, same constraint as the bundled defaults) instead of Source
    /// Sans 3 — always available, independent of the `default-fonts`
    /// feature.
    pub fn with_fonts(regular_bytes: &[u8], bold_bytes: &[u8]) -> Result<Self, FontError> {
        let mut reg = Self::empty();
        reg.register_named(FontKey::SANS_REGULAR, "CustomFont-Regular-Subset", regular_bytes)?;
        reg.register_named(FontKey::SANS_BOLD, "CustomFont-Bold-Subset", bold_bytes)?;
        Ok(reg)
    }

    pub fn register(&mut self, key: FontKey, bytes: &[u8]) -> Result<(), FontError> {
        let name = format!("CustomFont-{}-Subset", key.0);
        self.register_named(key, name, bytes)
    }

    pub fn register_named(&mut self, key: FontKey, name: impl Into<String>, bytes: &[u8]) -> Result<(), FontError> {
        let font = RegisteredFont::new(bytes, name)?;
        self.fonts.insert(key, font);
        Ok(())
    }

    /// Order matches how the facade registers PDF fonts — used to build
    /// resource names (`F1`, `F2`, ...) consistently between PDF font
    /// registration and content-stream references.
    pub fn font_entries(&self) -> Vec<(FontKey, &RegisteredFont)> {
        self.fonts.iter().map(|(&k, v)| (k, v)).collect()
    }

    pub fn entry(&self, key: FontKey) -> &RegisteredFont {
        self.fonts
            .get(&key)
            .or_else(|| self.fonts.get(&self.default_key))
            .expect("FontRegistry must contain at least one registered font")
    }
}

impl FontResolver for FontRegistry {
    fn metrics(&self, key: FontKey) -> &dyn FontMetrics {
        &self.entry(key).adapter
    }
}
