use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontError {
    /// Not a static TrueType `glyf` font (ADR-012: variable fonts and
    /// CFF/OTF are explicitly rejected in V1).
    UnsupportedFont,
    /// Malformed font data that `ttf-parser` could not parse at all.
    ParseError,
    /// Structurally inconsistent font tables discovered while subsetting
    /// (e.g. a `loca`/`glyf`/`maxp` mismatch) — distinct from `ParseError`
    /// because `ttf-parser` itself accepted the font; this is caught by
    /// lightweight-pdf's own table walking.
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

/// Owns font bytes; a `ttf_parser::Face` is only ever created transiently
/// on access, never stored self-referentially (ADR-010 / contract point 3).
#[derive(Clone, Debug)]
pub struct FontData {
    bytes: Arc<[u8]>,
}

impl FontData {
    /// Accepts only static TrueType fonts with `glyf` outlines (ADR-012).
    pub fn load(bytes: impl Into<Arc<[u8]>>) -> Result<Self, FontError> {
        let bytes: Arc<[u8]> = bytes.into();
        let face = ttf_parser::Face::parse(&bytes, 0).map_err(|_| FontError::ParseError)?;
        if face.tables().glyf.is_none() {
            return Err(FontError::UnsupportedFont);
        }
        if face.is_variable() {
            return Err(FontError::UnsupportedFont);
        }
        Ok(FontData { bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn with_face<R>(&self, f: impl FnOnce(&ttf_parser::Face) -> R) -> Result<R, FontError> {
        let face = ttf_parser::Face::parse(&self.bytes, 0).map_err(|_| FontError::ParseError)?;
        Ok(f(&face))
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
        data.with_face(|face| {
            let upem = face.units_per_em() as f32;
            let scale = 1000.0 / upem;
            let bb = face.global_bounding_box();
            EmbeddedFontMetrics {
                font: data.clone(),
                advance_cache: RefCell::new(HashMap::new()),
                ascent: face.ascender() as f32 * scale,
                descent: face.descender() as f32 * scale,
                cap_height: face
                    .capital_height()
                    .map(|c| c as f32 * scale)
                    .unwrap_or(face.ascender() as f32 * scale),
                italic_angle: face.italic_angle().unwrap_or(0.0),
                bbox: (
                    bb.x_min as f32 * scale,
                    bb.y_min as f32 * scale,
                    bb.x_max as f32 * scale,
                    bb.y_max as f32 * scale,
                ),
                is_italic: face.is_italic(),
                is_bold: face.is_bold(),
            }
        })
    }

    /// Advance width for a character in 1/1000 em units, or `None` if the
    /// font has no glyph for it.
    pub fn advance_1000(&self, ch: char) -> Option<f32> {
        if let Some(w) = self.advance_cache.borrow().get(&ch) {
            return Some(*w);
        }
        let advance = self
            .font
            .with_face(|face| {
                let upem = face.units_per_em() as f32;
                face.glyph_index(ch)
                    .and_then(|gid| face.glyph_hor_advance(gid))
                    .map(|a| a as f32 * 1000.0 / upem)
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
        self.font.with_face(|face| face.glyph_index(ch).map(|g| g.0)).ok().flatten()
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
