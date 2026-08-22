//! `loca`/`glyf` parsing and rewriting: `loca` table parsing, composite
//! glyph component walking, and the composite-glyph closure + glyph
//! renumbering used to rebuild `glyf`/`loca` for the subset.

use super::sfnt::{read_i16, read_u16, read_u32};
use crate::font::FontError;
use std::collections::{BTreeSet, HashMap, VecDeque};

pub(super) fn parse_loca(loca: &[u8], num_glyphs: u16, long_format: bool) -> Result<Vec<u32>, FontError> {
    // `num_glyphs` is a u16 (maxp.numGlyphs), so widening to usize cannot
    // truncate.
    let count = usize::from(num_glyphs) + 1;
    if long_format {
        if loca.len() < count * 4 {
            return Err(FontError::MalformedFont);
        }
        Ok((0..count).map(|i| read_u32(loca, i * 4)).collect())
    } else {
        if loca.len() < count * 2 {
            return Err(FontError::MalformedFont);
        }
        // short-format loca stores offsets/2 as u16; widening to u32 before
        // doubling cannot truncate (u16::MAX * 2 fits comfortably in u32).
        Ok((0..count).map(|i| u32::from(read_u16(loca, i * 2)) * 2).collect())
    }
}

/// A component reference inside a composite glyph: `gid_offset` is where
/// its glyph-index field starts within the glyph's raw bytes (so it can be
/// patched in place after remapping to a new subset GID).
pub(super) struct Component {
    pub(super) gid_offset: usize,
    pub(super) gid: u16,
}

/// Walks a single glyph's component records (empty for simple glyphs).
/// TrueType composite glyph flags: `ARG_1_AND_2_ARE_WORDS` (0x0001),
/// `WE_HAVE_A_SCALE` (0x0008), `MORE_COMPONENTS` (0x0020),
/// `WE_HAVE_AN_X_AND_Y_SCALE` (0x0040), `WE_HAVE_A_TWO_BY_TWO` (0x0080).
pub(super) fn glyph_components(glyph: &[u8]) -> Vec<Component> {
    let mut out = Vec::new();
    if glyph.len() < 10 {
        return out;
    }
    if read_i16(glyph, 0) >= 0 {
        return out; // simple glyph (>= 0 contours), never composite
    }
    let mut pos = 10usize;
    loop {
        if pos + 4 > glyph.len() {
            break;
        }
        let flags = read_u16(glyph, pos);
        let gid_offset = pos + 2;
        let gid = read_u16(glyph, gid_offset);
        out.push(Component { gid_offset, gid });
        pos = gid_offset + 2;
        pos += if flags & 0x0001 != 0 { 4 } else { 2 };
        if flags & 0x0008 != 0 {
            pos += 2;
        } else if flags & 0x0040 != 0 {
            pos += 4;
        } else if flags & 0x0080 != 0 {
            pos += 8;
        }
        if flags & 0x0020 == 0 {
            break; // no MORE_COMPONENTS
        }
    }
    out
}

/// Resolves the byte range for glyph `gid` within `loca` (glyph `gid`'s
/// bytes are then `glyf_raw[start..end]`). `loca` entries are u32 byte
/// offsets; usize is at least 32 bits on every platform this crate targets,
/// so the widening to `usize` cannot fail in practice — checked anyway,
/// since `usize` has no infallible `From<u32>` impl. Shared by
/// `composite_glyph_closure` and `rebuild_glyf_and_loca`, which both do this
/// same `loca[gid]`/`loca[gid + 1]` lookup.
pub(super) fn glyph_range(loca: &[u32], gid: u16) -> Result<(usize, usize), FontError> {
    let start = usize::try_from(*loca.get(usize::from(gid)).ok_or(FontError::MalformedFont)?).map_err(|_| FontError::MalformedFont)?;
    let end = usize::try_from(*loca.get(usize::from(gid) + 1).ok_or(FontError::MalformedFont)?).map_err(|_| FontError::MalformedFont)?;
    Ok((start, end))
}

/// Expands a seed set of glyph IDs to include every transitively-referenced
/// composite-glyph component (a component can itself be composite).
/// Component references that fall outside `loca`'s bounds are dropped —
/// treated as corrupt/inconsistent data to skip, not a hard error. Seed
/// members themselves (e.g. `.notdef` or a cmap-derived glyph ID) are never
/// dropped this way; if one of them turns out to be out of range, later
/// table-building steps that actually index into `loca`/`glyf` with it will
/// surface `FontError::MalformedFont` instead.
pub(super) fn composite_glyph_closure(mut used: BTreeSet<u16>, loca: &[u32], glyf_raw: &[u8]) -> Result<BTreeSet<u16>, FontError> {
    // `gid` is a u16, so widening it to usize cannot truncate.
    let gid_in_range = |gid: u16| usize::from(gid) + 1 < loca.len();
    let mut queue: VecDeque<u16> = used.iter().copied().collect();
    while let Some(gid) = queue.pop_front() {
        if !gid_in_range(gid) {
            continue; // corrupt/out-of-range reference, skip defensively
        }
        // `gid_in_range` above already guarantees `glyph_range` succeeds.
        let (start, end) = glyph_range(loca, gid)?;
        let glyph_slice = glyf_raw.get(start..end).ok_or(FontError::MalformedFont)?;
        for comp in glyph_components(glyph_slice) {
            if gid_in_range(comp.gid) && used.insert(comp.gid) {
                queue.push_back(comp.gid);
            }
        }
    }
    Ok(used)
}

/// Rebuilds `glyf` and `loca` (always long-format, 4-byte-offset `loca`)
/// containing exactly `ordered`'s glyphs in that order, remapping composite
/// component references in place from their original glyph IDs to their new
/// ones via `orig_to_new` (a component whose original GID isn't in the
/// subset — should not happen, since `composite_glyph_closure` already
/// pulled every reachable component in — falls back to `.notdef`, glyph 0).
pub(super) fn rebuild_glyf_and_loca(
    ordered: &[u16],
    loca: &[u32],
    glyf_raw: &[u8],
    orig_to_new: &HashMap<u16, u16>,
) -> Result<(Vec<u8>, Vec<u8>), FontError> {
    let mut new_glyf = Vec::new();
    let mut new_loca: Vec<u32> = Vec::with_capacity(ordered.len() + 1);
    for &orig_gid in ordered {
        new_loca.push(u32::try_from(new_glyf.len()).map_err(|_| FontError::MalformedFont)?);
        let (start, end) = glyph_range(loca, orig_gid)?;
        let mut glyph_bytes = glyf_raw.get(start..end).ok_or(FontError::MalformedFont)?.to_vec();
        for comp in glyph_components(&glyph_bytes) {
            let new_gid = orig_to_new.get(&comp.gid).copied().unwrap_or(0);
            // `glyph_components` only ever returns offsets it already
            // validated against this same slice's length, but treat a
            // violation as malformed input rather than trusting it blindly.
            glyph_bytes
                .get_mut(comp.gid_offset..comp.gid_offset + 2)
                .ok_or(FontError::MalformedFont)?
                .copy_from_slice(&new_gid.to_be_bytes());
        }
        new_glyf.extend_from_slice(&glyph_bytes);
    }
    new_loca.push(u32::try_from(new_glyf.len()).map_err(|_| FontError::MalformedFont)?);
    let mut new_loca_bytes = Vec::with_capacity(new_loca.len() * 4);
    for off in &new_loca {
        new_loca_bytes.extend_from_slice(&off.to_be_bytes());
    }
    Ok((new_glyf, new_loca_bytes))
}
