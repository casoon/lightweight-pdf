//! Own TrueType subsetting code (Phase 4, `skrifa`/`read-fonts` only parse —
//! they don't subset, ADR-003/ADR-015). Rewrites `glyf`, `loca`, `hmtx`,
//! `head`, `hhea`, `maxp` and `cmap` for exactly the glyphs a document uses
//! (plus `.notdef` and the full composite-glyph closure), with correct
//! 4-byte table alignment and `head.checkSumAdjustment` (plan/phases/
//! phase-4-fonts-subsetting.md, step 4). All of this operates on raw table
//! bytes obtained via `TableProvider::data_for_tag` — `skrifa` is only used
//! to locate those tables and to resolve `cmap`/`maxp`/`head`/`hhea`
//! metadata, not for the byte-level rewriting itself.
//!
//! Split across `sfnt` (byte-level table directory / checksum plumbing),
//! `glyf` (`loca`/`glyf` parsing, composite-glyph closure, glyph
//! renumbering) and `cmap` (the rebuilt format-4 `cmap` subtable); this
//! module orchestrates them into the public `subset_font` entry point.

mod cmap;
mod glyf;
mod sfnt;

#[cfg(test)]
mod tests;

use crate::font::{require_head_hhea, require_table, FontData, FontError};
use skrifa::charmap::Charmap;
use skrifa::raw::TableProvider;
use skrifa::{MetadataProvider, Tag};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

/// Rewritten `head`/`hhea`/`maxp` table bytes, as produced by
/// `patch_head_hhea_maxp`.
struct PatchedTables {
    head: Vec<u8>,
    hhea: Vec<u8>,
    maxp: Vec<u8>,
}

/// Rewrites the fixed-layout fields of `head`, `hhea`, and `maxp` that
/// depend on the subset's new glyph count: forces `head.indexToLocFormat`
/// to long (matching `sfnt::build_sfnt`'s always-u32 `loca`), and writes
/// `num_glyphs_new` into both `hhea.numberOfHMetrics` (the subset's `hmtx`
/// always has one full entry per glyph, no trailing-advance compression)
/// and `maxp.numGlyphs`. All three tables are truncated/copied only after
/// an explicit length check, so every slice access below is in-bounds.
fn patch_head_hhea_maxp(head_raw: &[u8], hhea_raw: &[u8], maxp_raw: &[u8], num_glyphs_new: u16) -> Result<PatchedTables, FontError> {
    if head_raw.len() < 54 {
        return Err(FontError::MalformedFont);
    }
    let mut head = head_raw.get(..54).ok_or(FontError::MalformedFont)?.to_vec();
    head.get_mut(50..52)
        .ok_or(FontError::MalformedFont)?
        .copy_from_slice(&1u16.to_be_bytes()); // indexToLocFormat = long

    if hhea_raw.len() < 36 {
        return Err(FontError::MalformedFont);
    }
    let mut hhea = hhea_raw.get(..36).ok_or(FontError::MalformedFont)?.to_vec();
    hhea.get_mut(34..36)
        .ok_or(FontError::MalformedFont)?
        .copy_from_slice(&num_glyphs_new.to_be_bytes());

    if maxp_raw.len() < 6 {
        return Err(FontError::MalformedFont);
    }
    let mut maxp = maxp_raw.to_vec();
    maxp.get_mut(4..6)
        .ok_or(FontError::MalformedFont)?
        .copy_from_slice(&num_glyphs_new.to_be_bytes());

    Ok(PatchedTables { head, hhea, maxp })
}

/// Characters -> original glyph IDs (unrepresentable characters are simply
/// omitted, not an error), seeding `used` with `.notdef` plus every mapped
/// glyph.
fn map_chars_to_glyphs(charmap: &Charmap, chars: &BTreeSet<char>) -> Result<(BTreeMap<char, u16>, BTreeSet<u16>), FontError> {
    let mut char_to_orig_gid: BTreeMap<char, u16> = BTreeMap::new();
    let mut used: BTreeSet<u16> = BTreeSet::new();
    used.insert(0); // .notdef, always included
    for &ch in chars {
        if let Some(gid) = charmap.map(ch) {
            // A static TrueType font's glyph IDs are inherently <=
            // `maxp.numGlyphs`, itself a u16 field, so this cannot
            // truncate — checked explicitly rather than assumed, so a
            // future change to that invariant produces
            // `FontError::MalformedFont` instead of a panic.
            let gid = u16::try_from(gid.to_u32()).map_err(|_| FontError::MalformedFont)?;
            char_to_orig_gid.insert(ch, gid);
            used.insert(gid);
        }
    }
    Ok((char_to_orig_gid, used))
}

/// The subset's glyph ordering: the new-GID-indexed list of original GIDs,
/// the subset's new glyph count, and the original-GID -> new-GID mapping.
type GlyphOrder = (Vec<u16>, u16, HashMap<u16, u16>);

/// Assigns new, sequential glyph IDs to `used` (0 is guaranteed to sort
/// first, i.e. `.notdef` stays glyph 0), returning the ordering itself, the
/// subset's new glyph count, and the original-GID -> new-GID mapping.
fn build_glyph_order(used: BTreeSet<u16>) -> Result<GlyphOrder, FontError> {
    let ordered: Vec<u16> = used.into_iter().collect();
    // `ordered` came from a `BTreeSet<u16>`, so its length can never exceed
    // 65536 — but 65536 itself doesn't fit the u16 `numGlyphs` field, so
    // this is a checked conversion rather than a silent one.
    let num_glyphs_new = u16::try_from(ordered.len()).map_err(|_| FontError::MalformedFont)?;
    let mut orig_to_new: HashMap<u16, u16> = HashMap::with_capacity(ordered.len());
    for (new_gid, &orig_gid) in ordered.iter().enumerate() {
        // `new_gid` < ordered.len() <= u16::MAX, checked above — still a
        // checked conversion, since `usize -> u16` has no infallible `From`.
        let new_gid = u16::try_from(new_gid).map_err(|_| FontError::MalformedFont)?;
        orig_to_new.insert(orig_gid, new_gid);
    }
    Ok((ordered, num_glyphs_new, orig_to_new))
}

/// `hmtx`: always writes a full (advance, lsb) pair per new glyph —
/// simpler and always spec-valid, at the cost of a few bytes vs. the
/// optional trailing-glyphs-share-last-advance compression. Also returns
/// each new glyph's advance width in 1/1000 em, for `FontSubset::widths_1000`.
fn rebuild_hmtx(ordered: &[u16], hmtx_raw: &[u8], num_hmetrics: u16, upem: f32) -> (Vec<u8>, Vec<f32>) {
    let read_orig_metric = |orig_gid: u16| -> (u16, i16) {
        if orig_gid < num_hmetrics {
            // `orig_gid` is a u16; widening to usize cannot truncate.
            let off = usize::from(orig_gid) * 4;
            (sfnt::read_u16(hmtx_raw, off), sfnt::read_i16(hmtx_raw, off + 2))
        } else {
            // `num_hmetrics` is a u16 checked non-zero by the caller, so the
            // widening and the `- 1` below cannot underflow.
            let advance_off = (usize::from(num_hmetrics) - 1) * 4;
            let advance = sfnt::read_u16(hmtx_raw, advance_off);
            // This `else` branch means `orig_gid >= num_hmetrics` (the `if`
            // above was false), so the subtraction below (after widening
            // both u16 operands to usize) cannot underflow.
            let lsb_off = usize::from(num_hmetrics) * 4 + (usize::from(orig_gid) - usize::from(num_hmetrics)) * 2;
            let lsb = if lsb_off + 2 <= hmtx_raw.len() {
                sfnt::read_i16(hmtx_raw, lsb_off)
            } else {
                0
            };
            (advance, lsb)
        }
    };
    let mut new_hmtx = Vec::with_capacity(ordered.len() * 4);
    let mut widths_1000 = Vec::with_capacity(ordered.len());
    for &orig_gid in ordered {
        let (advance, lsb) = read_orig_metric(orig_gid);
        new_hmtx.extend_from_slice(&advance.to_be_bytes());
        new_hmtx.extend_from_slice(&lsb.to_be_bytes());
        widths_1000.push(advance as f32 * 1000.0 / upem);
    }
    (new_hmtx, widths_1000)
}

/// Remaps `char_to_orig_gid`'s values from original to new glyph IDs.
/// `char_to_orig_gid` entries are always inserted into `used` too (by
/// `map_chars_to_glyphs`), and `composite_glyph_closure` only ever adds to
/// `used`, so every `orig` below is present in `orig_to_new` — checked
/// explicitly rather than assumed, so a future change to that invariant
/// produces `FontError::MalformedFont` instead of a panic.
fn remap_char_to_gid(char_to_orig_gid: &BTreeMap<char, u16>, orig_to_new: &HashMap<u16, u16>) -> Result<BTreeMap<char, u16>, FontError> {
    let mut char_to_gid: BTreeMap<char, u16> = BTreeMap::new();
    for (&ch, &orig) in char_to_orig_gid {
        let new_gid = *orig_to_new.get(&orig).ok_or(FontError::MalformedFont)?;
        char_to_gid.insert(ch, new_gid);
    }
    Ok(char_to_gid)
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

        let (head, hhea) = require_head_hhea(font)?;
        let num_glyphs_orig = require_table(font.maxp())?.num_glyphs();
        let long_loca = head.index_to_loc_format() == 1;
        let loca = glyf::parse_loca(loca_raw, num_glyphs_orig, long_loca)?;
        let num_hmetrics = hhea.number_of_h_metrics();
        if num_hmetrics == 0 {
            return Err(FontError::MalformedFont);
        }

        let charmap = font.charmap();
        let (char_to_orig_gid, used) = map_chars_to_glyphs(&charmap, chars)?;

        // Composite-glyph closure: pull in every component glyph,
        // transitively (a component can itself be composite).
        let used = glyf::composite_glyph_closure(used, &loca, glyf_raw)?;

        let (ordered, num_glyphs_new, orig_to_new) = build_glyph_order(used)?;

        // glyf + loca, remapping composite component references in place.
        let (new_glyf, new_loca_bytes) = glyf::rebuild_glyf_and_loca(&ordered, &loca, glyf_raw, &orig_to_new)?;

        let upem = head.units_per_em() as f32;
        let (new_hmtx, widths_1000) = rebuild_hmtx(&ordered, hmtx_raw, num_hmetrics, upem);

        let patched = patch_head_hhea_maxp(head_raw, hhea_raw, maxp_raw, num_glyphs_new)?;
        let char_to_gid = remap_char_to_gid(&char_to_orig_gid, &orig_to_new)?;
        let new_cmap = cmap::build_cmap_format4(&char_to_gid)?;

        let tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
            (b"cmap", new_cmap),
            (b"glyf", new_glyf),
            (b"head", patched.head),
            (b"hhea", patched.hhea),
            (b"hmtx", new_hmtx),
            (b"loca", new_loca_bytes),
            (b"maxp", patched.maxp),
        ];
        let font_data = sfnt::build_sfnt(&tables)?;

        Ok(FontSubset {
            font_data,
            char_to_gid,
            num_glyphs: num_glyphs_new,
            widths_1000,
        })
    })?
}
