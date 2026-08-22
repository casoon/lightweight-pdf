//! Builds the subset's `cmap` table: a single format-4 (segmented BMP)
//! subtable covering exactly the characters the subset includes.

use super::sfnt::sfnt_search_params;
use crate::font::FontError;
use std::collections::BTreeMap;

/// Builds a `cmap` table with a single format-4 (segmented BMP) subtable
/// covering exactly the given characters. Format 4 (not the simpler format
/// 6) because subset glyph IDs are generally *not* contiguous with
/// codepoint order, so segments always use the `idRangeOffset` +
/// `glyphIdArray` indirection rather than `idDelta` arithmetic.
pub(super) fn build_cmap_format4(char_to_new_gid: &BTreeMap<char, u16>) -> Result<Vec<u8>, FontError> {
    // `char` is a Unicode scalar value, always <= 0x10FFFF, so widening to
    // u32 here (both in the filter and the map) can never truncate.
    let mut pairs: Vec<(u32, u16)> = char_to_new_gid
        .iter()
        .filter(|(&ch, _)| u32::from(ch) <= 0xFFFF)
        .map(|(&ch, &gid)| (u32::from(ch), gid))
        .collect();
    pairs.sort_unstable_by_key(|&(cp, _)| cp);

    struct Run {
        start: u32,
        end: u32,
        gids: Vec<u16>,
    }
    let mut runs: Vec<Run> = Vec::new();
    for (cp, gid) in pairs {
        if let Some(last) = runs.last_mut() {
            if cp == last.end + 1 {
                last.end = cp;
                last.gids.push(gid);
                continue;
            }
        }
        runs.push(Run {
            start: cp,
            end: cp,
            gids: vec![gid],
        });
    }

    let seg_count = runs.len() + 1; // +1 terminator segment
                                    // A format-4 subtable's segment count is a u16 field (`segCountX2`
                                    // holds 2x it); an implausibly large character set could exceed that,
                                    // so this is a checked conversion rather than a truncating one.
    let seg_count_x2 = u16::try_from(seg_count * 2).map_err(|_| FontError::MalformedFont)?;
    let seg_count_u16 = seg_count_x2 / 2;
    let (search_range, entry_selector, range_shift) = sfnt_search_params(seg_count_u16, 2);

    let end_code_start = 14usize;
    let start_code_start = end_code_start + seg_count * 2 + 2; // +2 reservedPad
    let id_delta_start = start_code_start + seg_count * 2;
    let id_range_offset_start = id_delta_start + seg_count * 2;
    let glyph_id_array_start = id_range_offset_start + seg_count * 2;

    let mut glyph_id_array: Vec<u16> = Vec::new();
    let mut run_glyph_offsets = Vec::with_capacity(runs.len());
    for run in &runs {
        run_glyph_offsets.push(glyph_id_array.len());
        glyph_id_array.extend_from_slice(&run.gids);
    }

    let length = glyph_id_array_start + glyph_id_array.len() * 2;
    // The subtable `length` field is also a u16 (format-4 spec); reject
    // rather than silently truncate if an implausibly large character set
    // would overflow it.
    let length_u16 = u16::try_from(length).map_err(|_| FontError::MalformedFont)?;
    let mut sub = vec![0u8; length];
    sub[0..2].copy_from_slice(&4u16.to_be_bytes());
    sub[2..4].copy_from_slice(&length_u16.to_be_bytes());
    sub[4..6].copy_from_slice(&0u16.to_be_bytes()); // language
    sub[6..8].copy_from_slice(&seg_count_x2.to_be_bytes());
    sub[8..10].copy_from_slice(&search_range.to_be_bytes());
    sub[10..12].copy_from_slice(&entry_selector.to_be_bytes());
    sub[12..14].copy_from_slice(&range_shift.to_be_bytes());

    for (i, run) in runs.iter().enumerate() {
        // `run.end`/`run.start` are codepoints filtered to <= 0xFFFF above
        // (format 4 only covers the BMP), so these always fit in u16 — still
        // a checked conversion, consistent with the rest of this function.
        let run_end = u16::try_from(run.end).map_err(|_| FontError::MalformedFont)?;
        let run_start = u16::try_from(run.start).map_err(|_| FontError::MalformedFont)?;
        sub[end_code_start + i * 2..end_code_start + i * 2 + 2].copy_from_slice(&run_end.to_be_bytes());
        sub[start_code_start + i * 2..start_code_start + i * 2 + 2].copy_from_slice(&run_start.to_be_bytes());
        sub[id_delta_start + i * 2..id_delta_start + i * 2 + 2].copy_from_slice(&0i16.to_be_bytes());
        let id_range_offset_pos = id_range_offset_start + i * 2;
        let glyph_id_array_byte_offset = glyph_id_array_start + run_glyph_offsets[i] * 2;
        // Both offsets are positions within `sub`, whose total length was
        // already checked to fit in u16 above, so their difference does too.
        let id_range_offset_value =
            u16::try_from(glyph_id_array_byte_offset - id_range_offset_pos).map_err(|_| FontError::MalformedFont)?;
        sub[id_range_offset_pos..id_range_offset_pos + 2].copy_from_slice(&id_range_offset_value.to_be_bytes());
    }
    // Terminator segment: code 0xFFFF -> notdef via idDelta (spec convention).
    let term = seg_count - 1;
    sub[end_code_start + term * 2..end_code_start + term * 2 + 2].copy_from_slice(&0xFFFFu16.to_be_bytes());
    sub[start_code_start + term * 2..start_code_start + term * 2 + 2].copy_from_slice(&0xFFFFu16.to_be_bytes());
    sub[id_delta_start + term * 2..id_delta_start + term * 2 + 2].copy_from_slice(&1i16.to_be_bytes());
    sub[id_range_offset_start + term * 2..id_range_offset_start + term * 2 + 2].copy_from_slice(&0u16.to_be_bytes());

    for (i, gid) in glyph_id_array.iter().enumerate() {
        let pos = glyph_id_array_start + i * 2;
        sub[pos..pos + 2].copy_from_slice(&gid.to_be_bytes());
    }

    // Full `cmap` table: header + one (3,1) Windows-Unicode-BMP encoding
    // record pointing at the format-4 subtable right after it.
    let mut cmap = Vec::with_capacity(12 + sub.len());
    cmap.extend_from_slice(&0u16.to_be_bytes()); // version
    cmap.extend_from_slice(&1u16.to_be_bytes()); // numTables
    cmap.extend_from_slice(&3u16.to_be_bytes()); // platformID: Windows
    cmap.extend_from_slice(&1u16.to_be_bytes()); // encodingID: Unicode BMP
    cmap.extend_from_slice(&12u32.to_be_bytes()); // offset to subtable
    cmap.extend_from_slice(&sub);
    Ok(cmap)
}
