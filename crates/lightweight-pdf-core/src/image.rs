//! `Image` element (Phase 5, `plan/phases/phase-5-images.md`): JPEG/PNG
//! embedding for logos, no image processing.
//! Validates the header (dimensions, baseline-ness, color type) eagerly at
//! construction — same "fail fast on unsupported input" spirit as
//! `lightweight_pdf_fonts::FontData::load` — rather than deferring rejection to
//! render time. Only header bytes are parsed here; no dependency, no pixel
//! decoding (that's the facade's job for PNG, and unneeded for JPEG since
//! it's embedded byte-for-byte as `DCTDecode`).

use crate::style::Common;
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

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Parses just enough of a JPEG to validate it and extract
/// `(width, height, components)`. Scans markers up to the first
/// start-of-frame; `SOF0` (0xFFC0) is baseline, any other `SOFn` is
/// rejected as unsupported (progressive, extended sequential, lossless,
/// arithmetic-coded, ...). `components` is 1 (Gray) or 3 (RGB); 4 (CMYK)
/// or anything else is rejected.
fn parse_jpeg(bytes: &[u8]) -> Result<(u32, u32, u8), ImageError> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(ImageError::Malformed);
    }
    let mut i = 2usize;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            return Err(ImageError::Malformed);
        }
        let mut marker_pos = i + 1;
        while marker_pos < bytes.len() && bytes[marker_pos] == 0xFF {
            marker_pos += 1; // fill bytes between markers are legal
        }
        if marker_pos >= bytes.len() {
            return Err(ImageError::Malformed);
        }
        let marker = bytes[marker_pos];
        i = marker_pos + 1;

        // Standalone markers carry no length field.
        if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            continue;
        }
        if i + 2 > bytes.len() {
            return Err(ImageError::Malformed);
        }
        let length = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if length < 2 || i + length > bytes.len() {
            return Err(ImageError::Malformed);
        }

        let is_sof = (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC;
        if is_sof {
            if marker != 0xC0 {
                return Err(ImageError::UnsupportedJpeg);
            }
            let payload = &bytes[i + 2..i + length];
            if payload.len() < 6 {
                return Err(ImageError::Malformed);
            }
            let height = u16::from_be_bytes([payload[1], payload[2]]) as u32;
            let width = u16::from_be_bytes([payload[3], payload[4]]) as u32;
            let components = payload[5];
            if components != 1 && components != 3 {
                return Err(ImageError::UnsupportedJpeg); // CMYK (4) or exotic
            }
            return Ok((width, height, components));
        }
        if marker == 0xDA {
            return Err(ImageError::Malformed); // reached scan data, no SOF seen
        }
        i += length;
    }
    Err(ImageError::Malformed)
}

/// Parses a PNG's `IHDR` chunk (always the first chunk) and validates V1's
/// supported subset: non-interlaced, 8-bit, RGB or RGBA (ADR-013).
fn parse_png(bytes: &[u8]) -> Result<(u32, u32, u8), ImageError> {
    if bytes.len() < 8 + 8 + 13 || bytes[0..8] != PNG_SIGNATURE {
        return Err(ImageError::Malformed);
    }
    let chunk_len = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if &bytes[12..16] != b"IHDR" || chunk_len != 13 {
        return Err(ImageError::Malformed);
    }
    let ihdr = &bytes[16..16 + 13];
    let width = u32::from_be_bytes(ihdr[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(ihdr[4..8].try_into().unwrap());
    let bit_depth = ihdr[8];
    let color_type = ihdr[9];
    let interlace = ihdr[12];
    if width == 0 || height == 0 {
        return Err(ImageError::Malformed);
    }
    if interlace != 0 || bit_depth != 8 {
        return Err(ImageError::UnsupportedPng);
    }
    let components = match color_type {
        2 => 3,                                      // RGB
        6 => 4,                                      // RGBA
        _ => return Err(ImageError::UnsupportedPng), // grayscale(0)/palette(3)/gray+alpha(4) — not V1 scope
    };
    Ok((width, height, components))
}

/// A validated, embeddable JPEG or PNG. `bytes` are the original file
/// bytes, kept as-is — pixel decoding (only ever needed for PNG, to split
/// out the alpha channel as a `SMask`) happens later, in the facade.
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

impl Image {
    /// Validates `bytes` as a supported JPEG or PNG and extracts the
    /// metadata layout needs (dimensions, color components). Rejects
    /// anything outside V1's explicit scope instead of a silent
    /// best-effort attempt (`phases/phase-5-images.md` step 2-3).
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Result<Self, ImageError> {
        let bytes: Arc<[u8]> = bytes.into();
        let (format, width_px, height_px, components) = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
            let (w, h, c) = parse_jpeg(&bytes)?;
            (ImageFormat::Jpeg, w, h, c)
        } else if bytes.len() >= 8 && bytes[0..8] == PNG_SIGNATURE {
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
