//! Issue #13, Stage 2: automatic (Knuth-Liang) hyphenation, feature-gated
//! behind `hyphenation` because its pattern data measurably grows the
//! compiled artifact (see the crate's `Cargo.toml`/the workspace README's
//! Cargo features table — relevant for the `wasm` target in particular).
//!
//! This module's only job is inserting a soft hyphen (U+00AD) at each
//! dictionary break point *before* a word ever reaches `text::wrap_text`/
//! `wrap_text_marking_paragraph_ends` — actual line breaking and hyphen
//! rendering stays exactly Stage 1's always-on soft-hyphen logic in
//! `text.rs` either way, so an author's own soft hyphens and dictionary
//! ones are indistinguishable by the time wrapping sees them.

use hyphenation::{Hyphenator, Language, Load, Standard};
use lightweight_pdf_core::HyphenationLanguage;
use std::sync::OnceLock;

fn dictionary_for(lang: HyphenationLanguage) -> &'static Standard {
    static EN_US: OnceLock<Standard> = OnceLock::new();
    static GERMAN: OnceLock<Standard> = OnceLock::new();
    match lang {
        HyphenationLanguage::EnglishUs => {
            EN_US.get_or_init(|| Standard::from_embedded(Language::EnglishUS).expect("embedded en-US hyphenation dictionary"))
        }
        HyphenationLanguage::German => {
            GERMAN.get_or_init(|| Standard::from_embedded(Language::German1996).expect("embedded de-1996 hyphenation dictionary"))
        }
    }
}

/// Inserts a soft hyphen at each dictionary break point in `word` — a
/// no-op for words the dictionary has no opinion on (too short, foreign
/// alphabet, ...). A word that already contains an author-placed soft
/// hyphen (Stage 1) is left untouched rather than consulted: the
/// dictionary itself prioritizes existing soft hyphens over its own
/// breaks, and its `segments()` for that case already carry the original
/// marker inline (meant for direct display, not for joining with another
/// separator) — simplest and safest is to just not double up on a choice
/// the author already made.
fn hyphenate_word(dict: &Standard, word: &str) -> String {
    if word.contains('\u{AD}') {
        return word.to_string();
    }
    let segments: Vec<&str> = dict.hyphenate(word).into_iter().segments().collect();
    segments.join("\u{AD}")
}

/// Runs `hyphenate_word` over every whitespace-delimited run of `text`,
/// copying whitespace through unchanged.
pub fn auto_hyphenate(text: &str, lang: HyphenationLanguage) -> String {
    let dict = dictionary_for(lang);
    let mut out = String::with_capacity(text.len());
    let mut word_start = None;
    // `start` and `i` below are always char-boundary offsets into this same
    // `text` (`char_indices()`'s own contract), so `&text[start..i]` /
    // `&text[start..]` can never panic on a non-boundary or out-of-range
    // index.
    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = word_start.take() {
                out.push_str(&hyphenate_word(dict, &text[start..i]));
            }
            out.push(ch);
        } else if word_start.is_none() {
            word_start = Some(i);
        }
    }
    if let Some(start) = word_start {
        out.push_str(&hyphenate_word(dict, &text[start..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_soft_hyphens_at_dictionary_break_points_in_a_long_word() {
        let hyphenated = auto_hyphenate("Silbentrennung", HyphenationLanguage::German);
        assert!(
            hyphenated.contains('\u{AD}'),
            "expected at least one soft hyphen, got {hyphenated:?}"
        );
        assert_eq!(
            hyphenated.replace('\u{AD}', ""),
            "Silbentrennung",
            "hyphenation must not change the word itself"
        );
    }

    #[test]
    fn preserves_whitespace_and_word_boundaries() {
        let hyphenated = auto_hyphenate("Hyphenation example", HyphenationLanguage::EnglishUs);
        assert_eq!(hyphenated.replace('\u{AD}', ""), "Hyphenation example");
        assert!(hyphenated.contains(' '), "the space between words must survive untouched");
    }
}
