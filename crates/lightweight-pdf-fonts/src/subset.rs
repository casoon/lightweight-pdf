//! Own TrueType subsetting code (Phase 4, `skrifa`/`read-fonts` only parse —
//! they don't subset, ADR-003/ADR-015). Rewrites `glyf`, `loca`, `hmtx`,
//! `head`, `hhea`, `maxp` and `cmap` for exactly the glyphs a document uses
//! (plus `.notdef` and the full composite-glyph closure), with correct
//! 4-byte table alignment and `head.checkSumAdjustment` (plan/phases/
//! phase-4-fonts-subsetting.md, step 4). All of this operates on raw table
//! bytes obtained via `TableProvider::data_for_tag` — `skrifa` is only used
//! to locate those tables and to resolve `cmap`/`maxp`/`head`/`hhea`
//! metadata, not for the byte-level rewriting itself.

use crate::font::{FontData, FontError};
use skrifa::raw::TableProvider;
use skrifa::{MetadataProvider, Tag};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// Result of subsetting: a standalone, valid sfnt binary plus the mapping
/// from each requested character to its *new* glyph ID in that binary
/// (used both to encode PDF content-stream CIDs and to build the
/// `ToUnicode` CMap — CID space equals the subset's own glyph-index space,
/// so `CIDToGIDMap` can stay `/Identity`).
pub struct FontSubset {
    pub font_data: Vec<u8>,
    pub char_to_gid: BTreeMap<char, u16>,
    pub num_glyphs: u16,
    /// Advance width per new glyph ID (= CID), in 1/1000 em — exactly what
    /// a PDF `/W` array needs, computed once here instead of re-parsing
    /// the freshly-built subset just to read its own `hmtx` back out.
    pub widths_1000: Vec<f32>,
}

fn read_u16(d: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([d[off], d[off + 1]])
}

fn read_u32(d: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}

fn read_i16(d: &[u8], off: usize) -> i16 {
    i16::from_be_bytes([d[off], d[off + 1]])
}

fn parse_loca(loca: &[u8], num_glyphs: u16, long_format: bool) -> Result<Vec<u32>, FontError> {
    let count = num_glyphs as usize + 1;
    if long_format {
        if loca.len() < count * 4 {
            return Err(FontError::MalformedFont);
        }
        Ok((0..count).map(|i| read_u32(loca, i * 4)).collect())
    } else {
        if loca.len() < count * 2 {
            return Err(FontError::MalformedFont);
        }
        Ok((0..count).map(|i| read_u16(loca, i * 2) as u32 * 2).collect())
    }
}

/// A component reference inside a composite glyph: `gid_offset` is where
/// its glyph-index field starts within the glyph's raw bytes (so it can be
/// patched in place after remapping to a new subset GID).
struct Component {
    gid_offset: usize,
    gid: u16,
}

/// Walks a single glyph's component records (empty for simple glyphs).
/// TrueType composite glyph flags: `ARG_1_AND_2_ARE_WORDS` (0x0001),
/// `WE_HAVE_A_SCALE` (0x0008), `MORE_COMPONENTS` (0x0020),
/// `WE_HAVE_AN_X_AND_Y_SCALE` (0x0040), `WE_HAVE_A_TWO_BY_TWO` (0x0080).
fn glyph_components(glyph: &[u8]) -> Vec<Component> {
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

/// TrueType table checksum: sum of 4-byte big-endian words, the final
/// partial word (if any) treated as zero-padded (OpenType spec).
fn table_checksum(data: &[u8]) -> u32 {
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

fn sfnt_search_params(num_tables: u16) -> (u16, u16, u16) {
    let mut entry_selector = 0u16;
    while (1u16 << (entry_selector + 1)) <= num_tables {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 16;
    let range_shift = num_tables.wrapping_mul(16).wrapping_sub(search_range);
    (search_range, entry_selector, range_shift)
}

/// Assembles a complete sfnt binary from already-built table buffers.
/// `tables` must be sorted by tag ascending (OpenType spec requirement).
/// Pads every table to a 4-byte boundary and finally patches
/// `head.checkSumAdjustment` so the whole file's checksum resolves to the
/// spec's magic constant `0xB1B0AFBA`.
fn build_sfnt(tables: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let num_tables = tables.len() as u16;
    let (search_range, entry_selector, range_shift) = sfnt_search_params(num_tables);

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
        let offset = out.len() as u32;
        let checksum = table_checksum(data);
        records.push((*tag, checksum, offset, data.len() as u32));
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
    let head_offset = records[head_idx].2 as usize;
    out[head_offset + 8..head_offset + 12].copy_from_slice(&0u32.to_be_bytes());
    let file_checksum = table_checksum(&out);
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(file_checksum);
    out[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    out
}

/// Builds a `cmap` table with a single format-4 (segmented BMP) subtable
/// covering exactly the given characters. Format 4 (not the simpler format
/// 6) because subset glyph IDs are generally *not* contiguous with
/// codepoint order, so segments always use the `idRangeOffset` +
/// `glyphIdArray` indirection rather than `idDelta` arithmetic.
fn build_cmap_format4(char_to_new_gid: &BTreeMap<char, u16>) -> Vec<u8> {
    let mut pairs: Vec<(u32, u16)> = char_to_new_gid
        .iter()
        .filter(|(&ch, _)| (ch as u32) <= 0xFFFF)
        .map(|(&ch, &gid)| (ch as u32, gid))
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
    let seg_count_x2 = (seg_count * 2) as u16;
    let (search_range, entry_selector, range_shift) = sfnt_search_params_16(seg_count as u16);

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
    let mut sub = vec![0u8; length];
    sub[0..2].copy_from_slice(&4u16.to_be_bytes());
    sub[2..4].copy_from_slice(&(length as u16).to_be_bytes());
    sub[4..6].copy_from_slice(&0u16.to_be_bytes()); // language
    sub[6..8].copy_from_slice(&seg_count_x2.to_be_bytes());
    sub[8..10].copy_from_slice(&search_range.to_be_bytes());
    sub[10..12].copy_from_slice(&entry_selector.to_be_bytes());
    sub[12..14].copy_from_slice(&range_shift.to_be_bytes());

    for (i, run) in runs.iter().enumerate() {
        sub[end_code_start + i * 2..end_code_start + i * 2 + 2].copy_from_slice(&(run.end as u16).to_be_bytes());
        sub[start_code_start + i * 2..start_code_start + i * 2 + 2].copy_from_slice(&(run.start as u16).to_be_bytes());
        sub[id_delta_start + i * 2..id_delta_start + i * 2 + 2].copy_from_slice(&0i16.to_be_bytes());
        let id_range_offset_pos = id_range_offset_start + i * 2;
        let glyph_id_array_byte_offset = glyph_id_array_start + run_glyph_offsets[i] * 2;
        let id_range_offset_value = (glyph_id_array_byte_offset - id_range_offset_pos) as u16;
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
    cmap
}

fn sfnt_search_params_16(count: u16) -> (u16, u16, u16) {
    let mut entry_selector = 0u16;
    while (1u16 << (entry_selector + 1)) <= count {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 2;
    let range_shift = (count * 2).wrapping_sub(search_range);
    (search_range, entry_selector, range_shift)
}

/// Subsets `font` down to `.notdef` plus exactly the glyphs needed for
/// `chars` (including the full composite-glyph closure). Characters the
/// font has no glyph for are silently omitted from the result — the
/// caller decides how to handle that (see the facade's `?` fallback).
pub fn subset_font(font: &FontData, chars: &BTreeSet<char>) -> Result<FontSubset, FontError> {
    font.with_font(|font| -> Result<FontSubset, FontError> {
        let glyf_raw = font.data_for_tag(Tag::new(b"glyf")).ok_or(FontError::UnsupportedFont)?.as_bytes();
        let loca_raw = font.data_for_tag(Tag::new(b"loca")).ok_or(FontError::UnsupportedFont)?.as_bytes();
        let hmtx_raw = font.data_for_tag(Tag::new(b"hmtx")).ok_or(FontError::UnsupportedFont)?.as_bytes();
        let head_raw = font.data_for_tag(Tag::new(b"head")).ok_or(FontError::MalformedFont)?.as_bytes();
        let hhea_raw = font.data_for_tag(Tag::new(b"hhea")).ok_or(FontError::MalformedFont)?.as_bytes();
        let maxp_raw = font.data_for_tag(Tag::new(b"maxp")).ok_or(FontError::MalformedFont)?.as_bytes();

        let head = font.head().map_err(|_| FontError::MalformedFont)?;
        let hhea = font.hhea().map_err(|_| FontError::MalformedFont)?;
        let num_glyphs_orig = font.maxp().map_err(|_| FontError::MalformedFont)?.num_glyphs();
        let long_loca = head.index_to_loc_format() == 1;
        let loca = parse_loca(loca_raw, num_glyphs_orig, long_loca)?;
        let num_hmetrics = hhea.number_of_h_metrics();
        if num_hmetrics == 0 {
            return Err(FontError::MalformedFont);
        }

        // Characters -> original glyph IDs (unrepresentable characters are
        // simply omitted, not an error).
        let charmap = font.charmap();
        let mut char_to_orig_gid: BTreeMap<char, u16> = BTreeMap::new();
        let mut used: BTreeSet<u16> = BTreeSet::new();
        used.insert(0); // .notdef, always included
        for &ch in chars {
            if let Some(gid) = charmap.map(ch) {
                let gid = gid.to_u32() as u16;
                char_to_orig_gid.insert(ch, gid);
                used.insert(gid);
            }
        }

        // Composite-glyph closure: pull in every component glyph,
        // transitively (a component can itself be composite).
        let mut queue: VecDeque<u16> = used.iter().copied().collect();
        while let Some(gid) = queue.pop_front() {
            if gid as usize + 1 >= loca.len() {
                continue; // corrupt/out-of-range reference, skip defensively
            }
            let start = loca[gid as usize] as usize;
            let end = loca[gid as usize + 1] as usize;
            if start > end || end > glyf_raw.len() {
                return Err(FontError::MalformedFont);
            }
            for comp in glyph_components(&glyf_raw[start..end]) {
                if used.insert(comp.gid) {
                    queue.push_back(comp.gid);
                }
            }
        }

        // New, sequential glyph IDs (0 is guaranteed to sort first).
        let ordered: Vec<u16> = used.into_iter().collect();
        let mut orig_to_new: HashMap<u16, u16> = HashMap::with_capacity(ordered.len());
        for (new_gid, &orig_gid) in ordered.iter().enumerate() {
            orig_to_new.insert(orig_gid, new_gid as u16);
        }
        let num_glyphs_new = ordered.len() as u16;

        // glyf + loca, remapping composite component references in place.
        let mut new_glyf = Vec::new();
        let mut new_loca: Vec<u32> = Vec::with_capacity(ordered.len() + 1);
        for &orig_gid in &ordered {
            new_loca.push(new_glyf.len() as u32);
            let start = loca[orig_gid as usize] as usize;
            let end = loca[orig_gid as usize + 1] as usize;
            let mut glyph_bytes = glyf_raw[start..end].to_vec();
            for comp in glyph_components(&glyph_bytes) {
                let new_gid = orig_to_new.get(&comp.gid).copied().unwrap_or(0);
                glyph_bytes[comp.gid_offset..comp.gid_offset + 2].copy_from_slice(&new_gid.to_be_bytes());
            }
            new_glyf.extend_from_slice(&glyph_bytes);
        }
        new_loca.push(new_glyf.len() as u32);
        let mut new_loca_bytes = Vec::with_capacity(new_loca.len() * 4);
        for off in &new_loca {
            new_loca_bytes.extend_from_slice(&off.to_be_bytes());
        }

        // hmtx: always write a full (advance, lsb) pair per new glyph —
        // simpler and always spec-valid, at the cost of a few bytes vs. the
        // optional trailing-glyphs-share-last-advance compression.
        let read_orig_metric = |orig_gid: u16| -> (u16, i16) {
            if orig_gid < num_hmetrics {
                let off = orig_gid as usize * 4;
                (read_u16(hmtx_raw, off), read_i16(hmtx_raw, off + 2))
            } else {
                let advance_off = (num_hmetrics as usize - 1) * 4;
                let advance = read_u16(hmtx_raw, advance_off);
                let lsb_off = num_hmetrics as usize * 4 + (orig_gid as usize - num_hmetrics as usize) * 2;
                let lsb = if lsb_off + 2 <= hmtx_raw.len() {
                    read_i16(hmtx_raw, lsb_off)
                } else {
                    0
                };
                (advance, lsb)
            }
        };
        let upem = head.units_per_em() as f32;
        let mut new_hmtx = Vec::with_capacity(ordered.len() * 4);
        let mut widths_1000 = Vec::with_capacity(ordered.len());
        for &orig_gid in &ordered {
            let (advance, lsb) = read_orig_metric(orig_gid);
            new_hmtx.extend_from_slice(&advance.to_be_bytes());
            new_hmtx.extend_from_slice(&lsb.to_be_bytes());
            widths_1000.push(advance as f32 * 1000.0 / upem);
        }

        if head_raw.len() < 54 {
            return Err(FontError::MalformedFont);
        }
        let mut new_head = head_raw[..54].to_vec();
        new_head[50..52].copy_from_slice(&1u16.to_be_bytes()); // indexToLocFormat = long

        if hhea_raw.len() < 36 {
            return Err(FontError::MalformedFont);
        }
        let mut new_hhea = hhea_raw[..36].to_vec();
        new_hhea[34..36].copy_from_slice(&num_glyphs_new.to_be_bytes());

        if maxp_raw.len() < 6 {
            return Err(FontError::MalformedFont);
        }
        let mut new_maxp = maxp_raw.to_vec();
        new_maxp[4..6].copy_from_slice(&num_glyphs_new.to_be_bytes());

        let char_to_gid: BTreeMap<char, u16> = char_to_orig_gid
            .iter()
            .map(|(&ch, &orig)| (ch, *orig_to_new.get(&orig).expect("orig gid always mapped")))
            .collect();
        let new_cmap = build_cmap_format4(&char_to_gid);

        let tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
            (b"cmap", new_cmap),
            (b"glyf", new_glyf),
            (b"head", new_head),
            (b"hhea", new_hhea),
            (b"hmtx", new_hmtx),
            (b"loca", new_loca_bytes),
            (b"maxp", new_maxp),
        ];
        let font_data = build_sfnt(&tables);

        Ok(FontSubset {
            font_data,
            char_to_gid,
            num_glyphs: num_glyphs_new,
            widths_1000,
        })
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::EmbeddedFontMetrics;
    use skrifa::GlyphId;

    fn regular() -> FontData {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/SourceSans3-Regular.ttf")).unwrap();
        FontData::load(bytes).unwrap()
    }

    fn chars(s: &str) -> BTreeSet<char> {
        s.chars().collect()
    }

    #[test]
    fn subset_is_much_smaller_than_the_original() {
        let font = regular();
        let subset = subset_font(&font, &chars("Hallo Rechnung")).unwrap();
        assert!(
            subset.font_data.len() < font.bytes().len() / 4,
            "expected the subset ({} bytes) to be well under a quarter of the original ({} bytes)",
            subset.font_data.len(),
            font.bytes().len()
        );
    }

    #[test]
    fn subset_round_trips_through_skrifa_and_is_internally_consistent() {
        let font = regular();
        let subset = subset_font(&font, &chars("Hallo Rechnung äöüßÄÖÜ€")).unwrap();
        let reloaded = FontData::load(subset.font_data.clone()).expect("subset must itself be a valid, loadable static glyf TTF");
        reloaded
            .with_font(|font| {
                assert_eq!(font.maxp().unwrap().num_glyphs(), subset.num_glyphs);
            })
            .unwrap();
    }

    #[test]
    fn subset_checksum_adjustment_makes_the_whole_file_checksum_correct() {
        let font = regular();
        let subset = subset_font(&font, &chars("Test")).unwrap();
        // Per the OpenType spec, summing the *entire* file as big-endian
        // u32 words must equal the magic constant once checkSumAdjustment
        // is set correctly.
        assert_eq!(table_checksum(&subset.font_data), 0xB1B0_AFBAu32);
    }

    #[test]
    fn subset_preserves_advance_widths_for_used_glyphs() {
        let font = regular();
        let metrics = EmbeddedFontMetrics::from_font_data(&font).unwrap();
        let used = chars("HRi");
        let subset = subset_font(&font, &used).unwrap();
        let reloaded = FontData::load(subset.font_data.clone()).unwrap();
        for &ch in &used {
            let new_gid = *subset.char_to_gid.get(&ch).unwrap();
            let original_advance = metrics.advance_1000(ch).unwrap();
            let subset_advance = reloaded
                .with_font(|font| {
                    let upem = font.head().unwrap().units_per_em() as f32;
                    font.hmtx()
                        .ok()
                        .and_then(|hmtx| hmtx.advance(GlyphId::new(new_gid as u32)))
                        .map(|a| a as f32 * 1000.0 / upem)
                })
                .unwrap()
                .unwrap();
            assert!(
                (original_advance - subset_advance).abs() < 0.5,
                "advance mismatch for {ch:?}: original {original_advance} vs subset {subset_advance}"
            );
            assert!(
                (subset.widths_1000[new_gid as usize] - original_advance).abs() < 0.5,
                "widths_1000[{new_gid}] mismatch for {ch:?}"
            );
        }
    }

    #[test]
    fn subset_cmap_lookup_matches_char_to_gid_mapping() {
        let font = regular();
        let used = chars("Hallo äöü");
        let subset = subset_font(&font, &used).unwrap();
        let reloaded = FontData::load(subset.font_data.clone()).unwrap();
        reloaded
            .with_font(|font| {
                for (&ch, &expected_gid) in &subset.char_to_gid {
                    let gid = font.charmap().map(ch).expect("subset cmap must resolve every included character");
                    assert_eq!(gid.to_u32() as u16, expected_gid, "cmap lookup mismatch for {ch:?}");
                }
            })
            .unwrap();
    }

    #[test]
    fn composite_glyph_closure_pulls_in_component_glyphs() {
        let font = regular();
        // 'ä' (U+00E4) is a composite glyph in Source Sans 3 (base 'a' +
        // combining diaeresis component) — verified via direct table
        // inspection. Subsetting it alone must still produce a font with
        // more than just {.notdef, ä} because its components are pulled
        // in too, and the result must remain loadable/consistent.
        let subset = subset_font(&font, &chars("ä")).unwrap();
        assert!(
            subset.num_glyphs > 2,
            "expected composite closure to include component glyphs, got only {} glyphs",
            subset.num_glyphs
        );
        FontData::load(subset.font_data).expect("subset with composite glyphs must still be a valid font");
    }

    #[test]
    fn unrepresentable_characters_are_omitted_not_an_error() {
        let font = regular();
        let mut used = chars("Hallo");
        used.insert('\u{E000}'); // private-use area, no glyph in this font
        let subset = subset_font(&font, &used).unwrap();
        assert!(!subset.char_to_gid.contains_key(&'\u{E000}'));
        assert!(subset.char_to_gid.contains_key(&'H'));
    }

    #[test]
    fn empty_char_set_still_produces_a_loadable_notdef_only_font() {
        let font = regular();
        let subset = subset_font(&font, &BTreeSet::new()).unwrap();
        assert_eq!(subset.num_glyphs, 1); // .notdef only
        FontData::load(subset.font_data).expect("a notdef-only subset must still be a valid font");
    }

    #[test]
    fn works_on_a_non_default_font() {
        // Regression guard against "only works by luck with the bundled
        // Source Sans 3 table layout" — Source Serif 4 has different
        // table sizes/glyph counts/hmtx compression entirely.
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/custom-test-font.ttf")).unwrap();
        let font = FontData::load(bytes).unwrap();
        let subset = subset_font(&font, &chars("Hallo Rechnung äöüß")).unwrap();
        assert!(subset.font_data.len() < font.bytes().len());
        let reloaded = FontData::load(subset.font_data).expect("custom font subset must also be valid");
        reloaded
            .with_font(|font| {
                for (&ch, &gid) in &subset.char_to_gid {
                    assert_eq!(font.charmap().map(ch).unwrap().to_u32() as u16, gid);
                }
            })
            .unwrap();
    }
}
