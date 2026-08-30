//! `Image` element (Phase 5, `plan/phases/phase-5-images.md`): JPEG/PNG
//! embedding for logos, no image processing.
//! Validates the header (dimensions, baseline-ness, color type) eagerly at
//! construction — same "fail fast on unsupported input" spirit as
//! `lightweight_pdf_fonts::FontData::load` — rather than deferring rejection to
//! render time. Only header bytes are parsed here; no dependency, no pixel
//! decoding (that's the facade's job for PNG, and unneeded for JPEG since
//! it's embedded byte-for-byte as `DCTDecode`).
//!
//! Format-specific header parsing lives in the `jpeg`/`png` submodules; this
//! module owns the public `Image` type and dispatches to whichever parser
//! matches the file's magic bytes.

mod jpeg;
mod png;

use crate::style::Common;
use jpeg::parse_jpeg;
use png::{parse_png, PNG_SIGNATURE};
use std::sync::Arc;

/// Generous but finite: guards against a maliciously/accidentally huge
/// declared pixel count before any decoding happens (ADR-013: "Grenzen für
/// Pixelzahl ... sind Pflicht"). ~6300x6300 — comfortably more than any
/// realistic invoice logo or letterhead graphic.
const MAX_PIXELS: u64 = 40_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageError {
    /// Not a JPEG or PNG (bad magic bytes).
    UnsupportedFormat,
    /// Truncated or structurally broken header.
    Malformed,
    /// Not baseline, or not Gray/RGB (progressive, CMYK, ...).
    UnsupportedJpeg,
    /// Not non-interlaced 8-bit RGB/RGBA (palette, 16-bit, interlaced, ...).
    UnsupportedPng,
    /// Declared pixel count exceeds `MAX_PIXELS`.
    ImageTooLarge,
}

impl core::fmt::Display for ImageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ImageError::UnsupportedFormat => write!(f, "unsupported image format (need JPEG or PNG)"),
            ImageError::Malformed => write!(f, "malformed image header"),
            ImageError::UnsupportedJpeg => write!(f, "unsupported JPEG variant (need baseline Gray/RGB)"),
            ImageError::UnsupportedPng => write!(f, "unsupported PNG variant (need non-interlaced 8-bit RGB/RGBA)"),
            ImageError::ImageTooLarge => write!(f, "image pixel count exceeds the supported limit"),
        }
    }
}

/// A validated, embeddable JPEG or PNG. `bytes` are the original file
/// bytes, kept as-is — pixel decoding (only ever needed for PNG, to split
/// out the alpha channel as a `SMask`) happens later, in the facade.
///
/// `serde` (issue #17): serializes as `{"bytes_base64": "...", "common":
/// {..}}` — `format`/`width_px`/`height_px`/`components` are re-derived
/// by re-running `Image::new`'s own header validation on deserialize
/// rather than trusting redundant JSON fields that could disagree with
/// the actual bytes.
#[derive(Clone, Debug)]
pub struct Image {
    pub bytes: Arc<[u8]>,
    pub format: ImageFormat,
    pub width_px: u32,
    pub height_px: u32,
    /// 1 = Gray, 3 = RGB, 4 = RGBA (JPEG is always 1 or 3, never 4).
    pub components: u8,
    pub common: Common,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Image {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use base64::Engine;
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Image", 2)?;
        state.serialize_field("bytes_base64", &base64::engine::general_purpose::STANDARD.encode(&self.bytes))?;
        state.serialize_field("common", &self.common)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Image {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use base64::Engine;

        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            bytes_base64: String,
            #[serde(default)]
            common: Common,
        }

        let raw = Raw::deserialize(deserializer)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw.bytes_base64.as_bytes())
            .map_err(serde::de::Error::custom)?;
        let mut image = Image::new(bytes).map_err(serde::de::Error::custom)?;
        image.common = raw.common;
        Ok(image)
    }
}

/// Hand-written to match the custom `Serialize`/`Deserialize` above.
#[cfg(feature = "schemars")]
impl schemars::JsonSchema for Image {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Image".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "properties": {
                "bytes_base64": { "type": "string" },
                "common": generator.subschema_for::<Common>()
            },
            "required": ["bytes_base64"],
            "additionalProperties": false
        })
    }
}

impl Image {
    /// Validates `bytes` as a supported JPEG or PNG and extracts the
    /// metadata layout needs (dimensions, color components). Rejects
    /// anything outside V1's explicit scope instead of a silent
    /// best-effort attempt (`phases/phase-5-images.md` step 2-3).
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Result<Self, ImageError> {
        let bytes: Arc<[u8]> = bytes.into();
        let (format, width_px, height_px, components) = if bytes.starts_with(&[0xFF, 0xD8]) {
            let (w, h, c) = parse_jpeg(&bytes)?;
            (ImageFormat::Jpeg, w, h, c)
        } else if bytes.starts_with(&PNG_SIGNATURE) {
            let (w, h, c) = parse_png(&bytes)?;
            (ImageFormat::Png, w, h, c)
        } else {
            return Err(ImageError::UnsupportedFormat);
        };
        if (width_px as u64) * (height_px as u64) > MAX_PIXELS {
            return Err(ImageError::ImageTooLarge);
        }
        Ok(Image {
            bytes,
            format,
            width_px,
            height_px,
            components,
            common: Common::default(),
        })
    }

    pub fn width(mut self, width: f32) -> Self {
        self.common.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.common.height = Some(height);
        self
    }

    pub fn flex(mut self, factor: f32) -> Self {
        self.common.flex = Some(factor);
        self
    }

    pub fn keep_with_next(mut self) -> Self {
        self.common.keep_with_next = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/images/{}"), name)).expect("test fixture present")
    }

    #[test]
    fn accepts_rgba_png_with_transparency() {
        let img = Image::new(fixture("logo_rgba.png")).unwrap();
        assert_eq!(img.format, ImageFormat::Png);
        assert_eq!(img.components, 4);
        assert_eq!((img.width_px, img.height_px), (64, 48));
    }

    #[test]
    fn accepts_opaque_rgb_png() {
        let img = Image::new(fixture("logo_rgb.png")).unwrap();
        assert_eq!(img.components, 3);
        assert_eq!((img.width_px, img.height_px), (40, 30));
    }

    #[test]
    fn accepts_baseline_rgb_jpeg() {
        let img = Image::new(fixture("logo_baseline.jpg")).unwrap();
        assert_eq!(img.format, ImageFormat::Jpeg);
        assert_eq!(img.components, 3);
        assert_eq!((img.width_px, img.height_px), (80, 60));
    }

    #[test]
    fn accepts_baseline_gray_jpeg() {
        let img = Image::new(fixture("logo_gray.jpg")).unwrap();
        assert_eq!(img.components, 1);
        assert_eq!((img.width_px, img.height_px), (32, 32));
    }

    #[test]
    fn rejects_progressive_jpeg() {
        assert_eq!(Image::new(fixture("progressive.jpg")).unwrap_err(), ImageError::UnsupportedJpeg);
    }

    #[test]
    fn rejects_cmyk_jpeg() {
        assert_eq!(Image::new(fixture("cmyk.jpg")).unwrap_err(), ImageError::UnsupportedJpeg);
    }

    #[test]
    fn rejects_palette_png() {
        assert_eq!(Image::new(fixture("palette.png")).unwrap_err(), ImageError::UnsupportedPng);
    }

    #[test]
    fn rejects_sixteen_bit_png() {
        assert_eq!(Image::new(fixture("sixteen_bit.png")).unwrap_err(), ImageError::UnsupportedPng);
    }

    #[test]
    fn rejects_interlaced_png() {
        // Real Adam7-interlaced fixture generation is unreliable across
        // PNG encoders; a minimal synthetic IHDR with the interlace byte
        // set is sufficient here since interlacing must be rejected
        // before any pixel decoding is ever attempted.
        let mut bytes = fixture("logo_rgb.png");
        assert_eq!(&bytes[12..16], b"IHDR");
        bytes[16 + 12] = 1; // interlace method = Adam7
        assert_eq!(Image::new(bytes).unwrap_err(), ImageError::UnsupportedPng);
    }

    #[test]
    fn rejects_garbage_bytes() {
        assert_eq!(Image::new(vec![0u8; 16]).unwrap_err(), ImageError::UnsupportedFormat);
    }

    #[test]
    fn rejects_oversized_declared_dimensions() {
        let mut bytes = fixture("logo_rgb.png");
        bytes[16..20].copy_from_slice(&10_000u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&10_000u32.to_be_bytes());
        assert_eq!(Image::new(bytes).unwrap_err(), ImageError::ImageTooLarge);
    }

    #[test]
    fn builder_methods_set_common_fields() {
        let img = Image::new(fixture("logo_rgb.png"))
            .unwrap()
            .width(100.0)
            .height(50.0)
            .flex(1.0)
            .keep_with_next();
        assert_eq!(img.common.width, Some(100.0));
        assert_eq!(img.common.height, Some(50.0));
        assert_eq!(img.common.flex, Some(1.0));
        assert!(img.common.keep_with_next);
    }
}
