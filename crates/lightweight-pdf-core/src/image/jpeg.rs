//! JPEG header parsing: scans marker segments up to the first start-of-frame
//! and extracts `(width, height, components)` for a baseline (`SOF0`)
//! Gray/RGB image. No pixel decoding — a valid JPEG is embedded byte-for-byte
//! as `DCTDecode`, so only enough of the header is parsed to validate it and
//! read the metadata layout needs.

use super::ImageError;

/// Reads the 2-byte big-endian length field of a marker segment starting at
/// `at` (immediately after the marker byte) and validates it against
/// `bytes`' bounds. The returned length includes the 2 length bytes
/// themselves, per the JPEG spec.
fn read_segment_length(bytes: &[u8], at: usize) -> Result<usize, ImageError> {
    if at + 2 > bytes.len() {
        return Err(ImageError::Malformed);
    }
    let length = usize::from(u16::from_be_bytes([bytes[at], bytes[at + 1]]));
    if length < 2 || at + length > bytes.len() {
        return Err(ImageError::Malformed);
    }
    Ok(length)
}

/// Scans forward from `i` (which must satisfy `i + 1 < bytes.len()`) for the
/// next JPEG marker, skipping the legal 0xFF fill bytes markers may be
/// padded with. Returns the marker byte and the index of the first byte
/// after it (i.e. where a length field, if any, would start).
fn next_marker(bytes: &[u8], i: usize) -> Result<(u8, usize), ImageError> {
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
    Ok((bytes[marker_pos], marker_pos + 1))
}

/// Extracts `(width, height, components)` from a baseline `SOF0` payload
/// (the bytes between the 2-byte length field and the end of the segment).
/// `components` is 1 (Gray) or 3 (RGB); 4 (CMYK) or anything else is
/// rejected.
fn parse_sof0_payload(payload: &[u8]) -> Result<(u32, u32, u8), ImageError> {
    if payload.len() < 6 {
        return Err(ImageError::Malformed);
    }
    let height = u32::from(u16::from_be_bytes([payload[1], payload[2]]));
    let width = u32::from(u16::from_be_bytes([payload[3], payload[4]]));
    let components = payload[5];
    if components != 1 && components != 3 {
        return Err(ImageError::UnsupportedJpeg); // CMYK (4) or exotic
    }
    Ok((width, height, components))
}

/// Parses just enough of a JPEG to validate it and extract
/// `(width, height, components)`. Scans markers up to the first
/// start-of-frame; `SOF0` (0xFFC0) is baseline, any other `SOFn` is
/// rejected as unsupported (progressive, extended sequential, lossless,
/// arithmetic-coded, ...).
pub(super) fn parse_jpeg(bytes: &[u8]) -> Result<(u32, u32, u8), ImageError> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(ImageError::Malformed);
    }
    let mut i = 2usize;
    while i + 1 < bytes.len() {
        let (marker, next_i) = next_marker(bytes, i)?;
        i = next_i;

        // Standalone markers carry no length field.
        if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            continue;
        }
        let length = read_segment_length(bytes, i)?;

        let is_sof = (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC;
        if is_sof {
            if marker != 0xC0 {
                return Err(ImageError::UnsupportedJpeg);
            }
            return parse_sof0_payload(&bytes[i + 2..i + length]);
        }
        if marker == 0xDA {
            return Err(ImageError::Malformed); // reached scan data, no SOF seen
        }
        i += length;
    }
    Err(ImageError::Malformed)
}
