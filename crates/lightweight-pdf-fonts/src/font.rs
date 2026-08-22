use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use skrifa::attribute::Style;
use skrifa::raw::tables::head::Head;
use skrifa::raw::tables::hhea::Hhea;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider, Tag};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontError {
    /// Not a static TrueType `glyf` font (ADR-012: variable fonts and
    /// CFF/OTF are explicitly rejected in V1).
    UnsupportedFont,
    /// Malformed font data that `skrifa` could not parse at all.
    ParseError,
    /// Structurally inconsistent font tables discovered while subsetting
    /// (e.g. a `loca`/`glyf`/`maxp` mismatch, or a mandatory table `skrifa`
    /// couldn't decode) — distinct from `ParseError` because the font's
    /// sfnt directory itself parsed fine; this is caught by lightweight-pdf's
    /// own table walking.
    MalformedFont,
}

impl core::fmt::Display for FontError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FontError::UnsupportedFont => write!(f, "unsupported font (need static TrueType glyf)"),
            FontError::ParseError => write!(f, "could not parse font data"),
            FontError::MalformedFont => write!(f, "malformed or inconsistent font tables"),
        }
    }
}

/// Resolves a mandatory sfnt table lookup (`head`/`hhea`/`maxp`/...),
/// mapping `skrifa`'s per-table parse failure to `FontError::MalformedFont`
/// — the sfnt directory itself already parsed fine at this point (that's
/// `FontData::load`'s job), so a table that fails to decode here means the
/// font's tables are internally inconsistent, not merely unsupported.
/// Shared by `EmbeddedFontMetrics::from_font_data` and `subset::subset_font`,
/// which both resolve `head`/`hhea`/`maxp` this same way.
pub(crate) fn require_table<T>(table: Result<T, skrifa::raw::ReadError>) -> Result<T, FontError> {
    table.map_err(|_| FontError::MalformedFont)
}

/// Resolves the `head` and `hhea` tables together, via `require_table`.
/// Shared by `EmbeddedFontMetrics::from_font_data` and `subset::subset_font`,
/// which both need exactly this pair (`subset_font` additionally resolves
/// `maxp` on its own, since `from_font_data` doesn't need it).
pub(crate) fn require_head_hhea<'a>(font: &FontRef<'a>) -> Result<(Head<'a>, Hhea<'a>), FontError> {
    let head = require_table(font.head())?;
    let hhea = require_table(font.hhea())?;
    Ok((head, hhea))
}

/// Owns font bytes; a `skrifa::FontRef` is only ever created transiently on
/// access, never stored self-referentially (ADR-010 / contract point 3).
#[derive(Clone, Debug)]
pub struct FontData {
    bytes: Arc<[u8]>,
}

impl FontData {
    /// Accepts only static TrueType fonts with `glyf` outlines (ADR-012).
    /// `FontRef::new` only validates the sfnt directory itself — presence of
    /// `glyf` and absence of `fvar` (the standard variable-font marker) are
    /// checked explicitly here, same as before.
    pub fn load(bytes: impl Into<Arc<[u8]>>) -> Result<Self, FontError> {
        let bytes: Arc<[u8]> = bytes.into();
        let font = FontRef::new(&bytes).map_err(|_| FontError::ParseError)?;
        if font.data_for_tag(Tag::new(b"glyf")).is_none() {
            return Err(FontError::UnsupportedFont);
        }
        if font.data_for_tag(Tag::new(b"fvar")).is_some() {
            return Err(FontError::UnsupportedFont);
        }
        Ok(FontData { bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn with_font<R>(&self, f: impl FnOnce(&FontRef) -> R) -> Result<R, FontError> {
        let font = FontRef::new(&self.bytes).map_err(|_| FontError::ParseError)?;
        Ok(f(&font))
    }
}

/// Metrics for a font, keyed by Unicode character rather than a fixed
/// 256-code table (Phase 4 lifts the WinAnsi-only limitation from Phase
/// 0-2's simple-font text path — Type-0/CID text has no such limit).
/// Advances are computed lazily via `cmap`+`hmtx` lookups and cached per
/// character, since the same handful of glyphs typically repeat often in
/// one document (invoice/report text, not novel-scale volume — this
/// simplicity/performance trade-off is accepted explicitly for V1: "kein
/// Kerning ... zunächst ignorierbar").
#[derive(Debug)]
pub struct EmbeddedFontMetrics {
    font: FontData,
    advance_cache: RefCell<HashMap<char, f32>>,
    pub ascent: f32,
    pub descent: f32,
    pub cap_height: f32,
    pub italic_angle: f32,
    /// FontBBox in 1000-upm glyph space: (xmin, ymin, xmax, ymax).
    pub bbox: (f32, f32, f32, f32),
    pub is_italic: bool,
    pub is_bold: bool,
}

impl Clone for EmbeddedFontMetrics {
    fn clone(&self) -> Self {
        EmbeddedFontMetrics {
            font: self.font.clone(),
            advance_cache: RefCell::new(self.advance_cache.borrow().clone()),
            ascent: self.ascent,
            descent: self.descent,
            cap_height: self.cap_height,
            italic_angle: self.italic_angle,
            bbox: self.bbox,
            is_italic: self.is_italic,
            is_bold: self.is_bold,
        }
    }
}

impl EmbeddedFontMetrics {
    pub fn from_font_data(data: &FontData) -> Result<Self, FontError> {
        data.with_font(|font| -> Result<Self, FontError> {
            let (head, hhea) = require_head_hhea(font)?;
            let upem = head.units_per_em() as f32;
            let scale = 1000.0 / upem;
            let ascent = hhea.ascender().to_i16() as f32 * scale;
            let cap_height = font
                .os2()
                .ok()
                .and_then(|os2| os2.s_cap_height())
                .map(|c| c as f32 * scale)
                .unwrap_or(ascent);
            let italic_angle = font.post().ok().map(|post| post.italic_angle().to_f64() as f32).unwrap_or(0.0);
            // `Style::Oblique` counts as italic too (skrifa's attribute model
            // distinguishes the two; ttf-parser's narrower `is_italic()`
            // only covered the ITALIC bit — a deliberate, documented
            // widening, not a bug, see ADR-015).
            let attrs = font.attributes();
            Ok(EmbeddedFontMetrics {
                font: data.clone(),
                advance_cache: RefCell::new(HashMap::new()),
                ascent,
                descent: hhea.descender().to_i16() as f32 * scale,
                cap_height,
                italic_angle,
                bbox: (
                    head.x_min() as f32 * scale,
                    head.y_min() as f32 * scale,
                    head.x_max() as f32 * scale,
                    head.y_max() as f32 * scale,
                ),
                is_italic: attrs.style != Style::Normal,
                is_bold: attrs.weight >= skrifa::attribute::Weight::BOLD,
            })
        })?
    }

    /// Advance width for a character in 1/1000 em units, or `None` if the
    /// font has no glyph for it.
    pub fn advance_1000(&self, ch: char) -> Option<f32> {
        if let Some(w) = self.advance_cache.borrow().get(&ch) {
            return Some(*w);
        }
        let advance = self
            .font
            .with_font(|font| {
                let upem = font.head().ok()?.units_per_em() as f32;
                let gid = font.charmap().map(ch)?;
                font.hmtx().ok()?.advance(gid).map(|a| a as f32 * 1000.0 / upem)
            })
            .ok()
            .flatten();
        if let Some(w) = advance {
            self.advance_cache.borrow_mut().insert(ch, w);
        }
        advance
    }

    /// The original glyph ID for a character, or `None` if the font has no
    /// glyph for it. Used by the subsetter (`subset::subset_font`) to
    /// determine exactly which glyphs a document needs.
    pub fn glyph_id(&self, ch: char) -> Option<u16> {
        self.font
            .with_font(|font| {
                // A static TrueType font's glyph IDs are inherently <=
                // `maxp.numGlyphs`, itself a u16 field, so this conversion
                // should never fail in practice — but since `glyph_id`
                // already returns `Option`, a failure is folded into `None`
                // rather than reached via a panic.
                font.charmap().map(ch).and_then(|g| u16::try_from(g.to_u32()).ok())
            })
            .ok()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regular_bytes() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/SourceSans3-Regular.ttf")).expect("test font asset present")
    }

    fn bold_bytes() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/SourceSans3-Bold.ttf")).expect("test font asset present")
    }

    #[test]
    fn loads_static_glyf_font() {
        let data = FontData::load(regular_bytes()).unwrap();
        assert!(data.bytes().len() > 1000);
    }

    #[test]
    fn rejects_garbage_bytes() {
        assert_eq!(FontData::load(vec![0u8; 16]).unwrap_err(), FontError::ParseError);
    }

    #[test]
    fn computes_plausible_metrics_for_regular_and_bold() {
        for bytes in [regular_bytes(), bold_bytes()] {
            let data = FontData::load(bytes).unwrap();
            let metrics = EmbeddedFontMetrics::from_font_data(&data).unwrap();
            let w = metrics.advance_1000('H').unwrap();
            assert!(w > 400.0 && w < 900.0, "unexpected advance for 'H': {w}");
            assert!(metrics.ascent > 0.0);
            assert!(metrics.descent < 0.0);
            // German business-document essentials must be present.
            for ch in ['ä', 'ö', 'ü', 'Ä', 'Ö', 'Ü', 'ß', '€', '–', '„', '"'] {
                assert!(metrics.advance_1000(ch).is_some(), "missing glyph for {ch:?}");
                assert!(metrics.glyph_id(ch).is_some(), "missing glyph id for {ch:?}");
            }
        }
    }

    #[test]
    fn advance_lookup_is_cached_and_repeatable() {
        let data = FontData::load(regular_bytes()).unwrap();
        let metrics = EmbeddedFontMetrics::from_font_data(&data).unwrap();
        let first = metrics.advance_1000('x').unwrap();
        let second = metrics.advance_1000('x').unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn unrepresentable_glyph_is_none_not_a_panic() {
        let data = FontData::load(regular_bytes()).unwrap();
        let metrics = EmbeddedFontMetrics::from_font_data(&data).unwrap();
        // A private-use-area codepoint no reasonable text font maps.
        assert_eq!(metrics.advance_1000('\u{E000}'), None);
        assert_eq!(metrics.glyph_id('\u{E000}'), None);
    }
}
