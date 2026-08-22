//! Low-level sfnt byte encoding: big-endian field readers, the TrueType
//! table checksum, the shared binary-search-parameter formula, and final
//! assembly of a complete sfnt binary from already-built table buffers.

use crate::font::FontError;

pub(super) fn read_u16(d: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([d[off], d[off + 1]])
}

pub(super) fn read_u32(d: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}

pub(super) fn read_i16(d: &[u8], off: usize) -> i16 {
    i16::from_be_bytes([d[off], d[off + 1]])
}

/// TrueType table checksum: sum of 4-byte big-endian words, the final
/// partial word (if any) treated as zero-padded (OpenType spec).
pub(super) fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i < data.len() {
        let mut buf = [0u8; 4];
        let n = (data.len() - i).min(4);
        buf[..n].copy_from_slice(&data[i..i + n]);
        sum = sum.wrapping_add(u32::from_be_bytes(buf));
        i += 4;
    }
    sum
}

/// Shared binary-search-parameter formula from the sfnt/cmap spec:
/// `searchRange = 2^floor(log2(count)) * unit_size`, `entrySelector =
/// floor(log2(count))`, `rangeShift = count*unit_size - searchRange`.
/// Parameterized by `unit_size` so it covers both the sfnt table directory
/// (`unit_size = 16`, one 16-byte directory record per table) and the cmap
/// format-4 segment arrays (`unit_size = 2`, one u16 per segment).
pub(super) fn sfnt_search_params(count: u16, unit_size: u16) -> (u16, u16, u16) {
    let mut entry_selector = 0u16;
    while (1u16 << (entry_selector + 1)) <= count {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector).wrapping_mul(unit_size);
    let range_shift = count.wrapping_mul(unit_size).wrapping_sub(search_range);
    (search_range, entry_selector, range_shift)
}

/// Assembles a complete sfnt binary from already-built table buffers.
/// `tables` must be sorted by tag ascending (OpenType spec requirement).
/// Pads every table to a 4-byte boundary and finally patches
/// `head.checkSumAdjustment` so the whole file's checksum resolves to the
/// spec's magic constant `0xB1B0AFBA`.
pub(super) fn build_sfnt(tables: &[(&[u8; 4], Vec<u8>)]) -> Result<Vec<u8>, FontError> {
    // `tables` is always the fixed 7-entry list built in `subset_font`
    // (cmap/glyf/head/hhea/hmtx/loca/maxp), far under u16::MAX — checked
    // rather than assumed, since `usize -> u16` has no infallible `From`.
    let num_tables = u16::try_from(tables.len()).map_err(|_| FontError::MalformedFont)?;
    let (search_range, entry_selector, range_shift) = sfnt_search_params(num_tables, 16);

    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&num_tables.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    let dir_start = out.len();
    out.resize(dir_start + tables.len() * 16, 0);

    let mut records = Vec::with_capacity(tables.len());
    for (tag, data) in tables {
        // Table offsets/lengths are u32 sfnt directory fields; a subset
        // output realistically never gets close to 4 GiB, but reject
        // cleanly instead of silently wrapping if it somehow did.
        let offset = u32::try_from(out.len()).map_err(|_| FontError::MalformedFont)?;
        let checksum = table_checksum(data);
        let len = u32::try_from(data.len()).map_err(|_| FontError::MalformedFont)?;
        records.push((*tag, checksum, offset, len));
        out.extend_from_slice(data);
        let pad = (4 - data.len() % 4) % 4;
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    for (i, (tag, checksum, offset, len)) in records.iter().enumerate() {
        let pos = dir_start + i * 16;
        out[pos..pos + 4].copy_from_slice(*tag);
        out[pos + 4..pos + 8].copy_from_slice(&checksum.to_be_bytes());
        out[pos + 8..pos + 12].copy_from_slice(&offset.to_be_bytes());
        out[pos + 12..pos + 16].copy_from_slice(&len.to_be_bytes());
    }

    let head_idx = tables.iter().position(|(t, _)| **t == *b"head").expect("head table always present");
    // `records[head_idx].2` came from `u32::try_from(out.len())` above, and
    // usize is at least 32 bits on every platform this crate targets, so
    // this cannot fail in practice — but `usize` has no infallible `From<u32>`
    // impl (some in-principle 16-bit target could exist), so it stays a
    // checked conversion.
    let head_offset = usize::try_from(records[head_idx].2).map_err(|_| FontError::MalformedFont)?;
    out[head_offset + 8..head_offset + 12].copy_from_slice(&0u32.to_be_bytes());
    let file_checksum = table_checksum(&out);
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(file_checksum);
    out[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    Ok(out)
}
