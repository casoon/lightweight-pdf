//! PNG header parsing: reads the `IHDR` chunk (always the first chunk after
//! the signature) and validates V1's supported subset — non-interlaced,
//! 8-bit, RGB or RGBA (ADR-013). No pixel decoding here; that's the facade's
//! job (to split out the alpha channel as a `SMask`).

use super::ImageError;

pub(super) const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Parses a PNG's `IHDR` chunk (always the first chunk) and validates V1's
/// supported subset: non-interlaced, 8-bit, RGB or RGBA (ADR-013).
pub(super) fn parse_png(bytes: &[u8]) -> Result<(u32, u32, u8), ImageError> {
    if bytes.len() < 8 + 8 + 13 || bytes[0..8] != PNG_SIGNATURE {
        return Err(ImageError::Malformed);
    }
    // `bytes[8..12]` are 4 bytes: the length check above guarantees
    // `bytes.len() >= 8 + 8 + 13`, so all four indices are in bounds. Built
    // from an array literal (not `try_into()`) so there is no panicking
    // conversion here. `usize::try_from` (rather than `as`) covers the
    // hypothetical case of a target where `usize` is narrower than `u32`.
    let chunk_len = usize::try_from(u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]])).map_err(|_| ImageError::Malformed)?;
    if &bytes[12..16] != b"IHDR" || chunk_len != 13 {
        return Err(ImageError::Malformed);
    }
    let ihdr = &bytes[16..16 + 13];
    // `ihdr` is exactly 13 bytes (guaranteed by the length check above), so
    // the two 4-byte reads below are in bounds. Built from array literals
    // (not `try_into()`) so there is no panicking conversion here.
    let width = u32::from_be_bytes([ihdr[0], ihdr[1], ihdr[2], ihdr[3]]);
    let height = u32::from_be_bytes([ihdr[4], ihdr[5], ihdr[6], ihdr[7]]);
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
