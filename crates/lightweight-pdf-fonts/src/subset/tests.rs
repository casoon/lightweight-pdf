use super::sfnt::table_checksum;
use super::subset_font;
use crate::font::{EmbeddedFontMetrics, FontData};
use skrifa::raw::TableProvider;
use skrifa::{GlyphId, MetadataProvider};
use std::collections::BTreeSet;

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
                    // `new_gid` is a u16 (subset.char_to_gid's value type),
                    // so widening to u32 for `GlyphId` cannot truncate.
                    .and_then(|hmtx| hmtx.advance(GlyphId::new(u32::from(new_gid))))
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
