//! Bridges validated `lightweight-pdf-core::Image` data to `lightweight-pdf-writer::ImageXObject`
//! (Phase 5, `plan/phases/phase-5-images.md` steps 2-3). JPEG passes
//! through byte-for-byte as `DCTDecode` (`Image::new` already validated
//! baseline-ness/color type in `lightweight-pdf-core`, no re-encoding). PNG is
//! decoded — only PNG actually needs it, to split the alpha channel out
//! into a separate `SMask` — via the feature-reduced, pure-Rust `png`
//! crate (ADR-013); RGB/alpha pixels are then embedded *uncompressed*,
//! consistent with content streams and embedded fonts elsewhere in V1
//! (`plan/progress.md`: no Flate encoder implemented, so nothing here
//! re-compresses either).

use lightweight_pdf_core::ImageFormat;
use lightweight_pdf_writer::{ColorSpace, ImageDataFilter, ImageXObject};

#[derive(Debug)]
pub enum ImageEmbedError {
    /// The document contains a PNG but the crate was built without the
    /// `png` feature — there is no decoder available to split out alpha.
    PngFeatureDisabled,
    /// The `png` crate rejected the data at decode time (should be rare:
    /// `lightweight-pdf-core::Image::new` already validated the header).
    DecodeFailed,
}

impl core::fmt::Display for ImageEmbedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ImageEmbedError::PngFeatureDisabled => write!(f, "PNG image in document, but the `png` feature is disabled"),
            ImageEmbedError::DecodeFailed => write!(f, "failed to decode PNG image data"),
        }
    }
}

/// Guards decompressed PNG output size (ADR-013: "Grenzen für ... dekomprimierte
/// Bytes sind Pflicht") — generous but finite, enforced by the `png` crate
/// itself via `Limits` during decode, not just this module's own pixel-count
/// check in `lightweight-pdf-core`.
#[cfg(feature = "png")]
const MAX_DECOMPRESSED_BYTES: usize = 200_000_000;

pub fn build_pdf_image(bytes: &[u8], format: ImageFormat, components: u8) -> Result<ImageXObject, ImageEmbedError> {
    match format {
        ImageFormat::Jpeg => Ok(build_jpeg(bytes, components)),
        ImageFormat::Png => build_png(bytes),
    }
}

fn build_jpeg(bytes: &[u8], components: u8) -> ImageXObject {
    let color_space = if components == 1 {
        ColorSpace::DeviceGray
    } else {
        ColorSpace::DeviceRgb
    };
    ImageXObject {
        // Width/Height are read from the same validated header
        // `lightweight-pdf-core::Image` already parsed; re-deriving them from the
        // JPEG SOF marker a second time here would just duplicate that
        // parsing, so the facade takes them from the `RenderNode::Image`
        // it's translating (see `render.rs`) and overwrites these
        // placeholders before returning.
        width_px: 0,
        height_px: 0,
        color_space,
        bits_per_component: 8,
        filter: ImageDataFilter::DctDecode,
        bytes: bytes.to_vec(),
        smask: None,
    }
}

#[cfg(feature = "png")]
fn build_png(bytes: &[u8]) -> Result<ImageXObject, ImageEmbedError> {
    let limits = png::Limits {
        bytes: MAX_DECOMPRESSED_BYTES,
    };
    let decoder = png::Decoder::new_with_limits(std::io::Cursor::new(bytes), limits);
    let mut reader = decoder.read_info().map_err(|_| ImageEmbedError::DecodeFailed)?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or(ImageEmbedError::DecodeFailed)?];
    let info = reader.next_frame(&mut buf).map_err(|_| ImageEmbedError::DecodeFailed)?;
    let pixels = &buf[..info.buffer_size()];

    match info.color_type {
        png::ColorType::Rgb => Ok(ImageXObject {
            width_px: info.width,
            height_px: info.height,
            color_space: ColorSpace::DeviceRgb,
            bits_per_component: 8,
            filter: ImageDataFilter::None,
            bytes: pixels.to_vec(),
            smask: None,
        }),
        png::ColorType::Rgba => {
            // `info.width`/`info.height` are `u32`; `usize` is only
            // guaranteed to be at least 16 bits, so the widening is made
            // explicit and fallible rather than an `as` cast. The
            // multiplication is a different story: both values come from
            // the PNG header (caller-supplied, not internally controlled),
            // so a pathological image could overflow `usize` on a 32-bit
            // target — `checked_mul` fails closed via `ImageEmbedError`
            // instead of panicking (debug) or silently wrapping to an
            // undersized allocation (release).
            let width = usize::try_from(info.width).expect("u32 width fits in usize on every supported target");
            let height = usize::try_from(info.height).expect("u32 height fits in usize on every supported target");
            let pixel_count = width.checked_mul(height).ok_or(ImageEmbedError::DecodeFailed)?;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            let mut alpha = Vec::with_capacity(pixel_count);
            for px in pixels.as_chunks::<4>().0 {
                rgb.extend_from_slice(&px[0..3]);
                alpha.push(px[3]);
            }
            let smask = ImageXObject {
                width_px: info.width,
                height_px: info.height,
                color_space: ColorSpace::DeviceGray,
                bits_per_component: 8,
                filter: ImageDataFilter::None,
                bytes: alpha,
                smask: None,
            };
            Ok(ImageXObject {
                width_px: info.width,
                height_px: info.height,
                color_space: ColorSpace::DeviceRgb,
                bits_per_component: 8,
                filter: ImageDataFilter::None,
                bytes: rgb,
                smask: Some(Box::new(smask)),
            })
        }
        // `lightweight-pdf-core::Image::new` only accepts color types 2 (RGB) and
        // 6 (RGBA) — anything else reaching here would be a contract bug,
        // not user input, but fail closed rather than embedding garbage.
        _ => Err(ImageEmbedError::DecodeFailed),
    }
}

#[cfg(not(feature = "png"))]
fn build_png(_bytes: &[u8]) -> Result<ImageXObject, ImageEmbedError> {
    Err(ImageEmbedError::PngFeatureDisabled)
}
