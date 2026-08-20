//! Greedy word-boundary wrapping with a hard-break fallback for tokens
//! wider than the available width (`plan/05-overflow-and-robustness.md`
//! Grundprinzip 2). No hyphenation.

use crate::font_resolver::FontResolver;
use lightweight_pdf_core::{FontKey, TextStyle};

pub fn text_width_pt(resolver: &dyn FontResolver, font: FontKey, size: f32, text: &str) -> f32 {
    let m = resolver.metrics(font);
    text.chars().map(|c| m.advance(c)).sum::<f32>() / 1000.0 * size
}

/// Splits a single word into pieces that each fit `max_width`, breaking on
/// character boundaries as a last resort (never truncated, never drawn
/// past the edge).
fn hard_break_word(resolver: &dyn FontResolver, style: &TextStyle, word: &str, max_width: f32) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    for ch in word.chars() {
        let mut candidate = current.clone();
        candidate.push(ch);
        let w = text_width_pt(resolver, style.font, style.size, &candidate);
        if w > max_width && !current.is_empty() {
            pieces.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    if !current.is_empty() || pieces.is_empty() {
        pieces.push(current);
    }
    pieces
}

/// Wraps `text` to `max_width` points. Explicit `\n` in the source text
/// start a new paragraph/line unconditionally.
pub fn wrap_text(resolver: &dyn FontResolver, style: &TextStyle, text: &str, max_width: f32) -> Vec<String> {
    let max_width = max_width.max(0.0);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let words: Vec<&str> = paragraph.split(' ').filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in words {
            if current.is_empty() {
                let w = text_width_pt(resolver, style.font, style.size, word);
                if w > max_width {
                    let mut pieces = hard_break_word(resolver, style, word, max_width);
                    let last = pieces.pop().unwrap_or_default();
                    lines.extend(pieces);
                    current = last;
                } else {
                    current = word.to_string();
                }
                continue;
            }
            let candidate = format!("{current} {word}");
            let w = text_width_pt(resolver, style.font, style.size, &candidate);
            if w <= max_width {
                current = candidate;
            } else {
                lines.push(std::mem::take(&mut current));
                let word_width = text_width_pt(resolver, style.font, style.size, word);
                if word_width > max_width {
                    let mut pieces = hard_break_word(resolver, style, word, max_width);
                    let last = pieces.pop().unwrap_or_default();
                    lines.extend(pieces);
                    current = last;
                } else {
                    current = word.to_string();
                }
            }
        }
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedMetrics;
    impl crate::font_resolver::FontMetrics for FixedMetrics {
        fn advance(&self, ch: char) -> f32 {
            if ch == ' ' {
                300.0
            } else {
                600.0
            }
        }
        fn ascent(&self) -> f32 {
            800.0
        }
        fn descent(&self) -> f32 {
            -200.0
        }
    }
    struct FixedResolver;
    impl FontResolver for FixedResolver {
        fn metrics(&self, _key: FontKey) -> &dyn crate::font_resolver::FontMetrics {
            &FixedMetrics
        }
    }

    #[test]
    fn wraps_on_word_boundaries() {
        let style = TextStyle {
            size: 10.0,
            ..Default::default()
        };
        // Each char = 6pt at size 10 (600/1000*10). "AAAA BBBB" at width 30
        // -> "AAAA" is 24pt, fits; adding " BBBB" would be way over.
        let lines = wrap_text(&FixedResolver, &style, "AAAA BBBB", 30.0);
        assert_eq!(lines, vec!["AAAA".to_string(), "BBBB".to_string()]);
    }

    #[test]
    fn hard_breaks_a_single_too_wide_token() {
        let style = TextStyle {
            size: 10.0,
            ..Default::default()
        };
        // A single 10-char token, each char 6pt, max width 18pt -> 3 chars/line.
        let lines = wrap_text(&FixedResolver, &style, "ABCDEFGHIJ", 18.0);
        assert_eq!(lines, vec!["ABC", "DEF", "GHI", "J"]);
    }

    #[test]
    fn respects_explicit_newlines() {
        let style = TextStyle::default();
        let lines = wrap_text(&FixedResolver, &style, "a\nb", 1000.0);
        assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
    }
}
