//! Greedy word-boundary wrapping with a hard-break fallback for tokens
//! wider than the available width (`plan/05-overflow-and-robustness.md`
//! Grundprinzip 2). No hyphenation.

use crate::font_resolver::FontResolver;
use lightweight_pdf_core::{FontKey, TextStyle};

pub fn text_width_pt(resolver: &dyn FontResolver, font: FontKey, size: f32, text: &str) -> f32 {
    let m = resolver.metrics(font);
    text.chars().map(|c| m.advance(c)).sum::<f32>() / 1000.0 * size
}

/// `text_width_pt` for a `TextStyle`'s font/size — the `style.font,
/// style.size` pair otherwise repeats at every measurement call site below.
fn styled_width_pt(resolver: &dyn FontResolver, style: &TextStyle, text: &str) -> f32 {
    text_width_pt(resolver, style.font, style.size, text)
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
        let w = styled_width_pt(resolver, style, &candidate);
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

/// Starts a fresh line with `word`: if it fits `max_width` whole, it
/// becomes the line's only content so far; otherwise it's hard-broken,
/// with all but the last piece pushed straight into `lines` and the last
/// piece returned as the new line-in-progress. Shared by both places
/// `wrap_text` begins a line (the very first word of a paragraph, and the
/// word right after a line-full break).
fn start_line(resolver: &dyn FontResolver, style: &TextStyle, word: &str, max_width: f32, lines: &mut Vec<String>) -> String {
    let w = styled_width_pt(resolver, style, word);
    if w <= max_width {
        return word.to_string();
    }
    let mut pieces = hard_break_word(resolver, style, word, max_width);
    // `hard_break_word` always returns at least one piece (it pushes
    // `current` unconditionally when `pieces` would otherwise be empty),
    // so popping the last one off can never actually hit the default.
    let last = pieces.pop().expect("hard_break_word always returns at least one piece");
    lines.extend(pieces);
    last
}

/// Wraps `text` to `max_width` points. Explicit `\n` in the source text
/// start a new paragraph/line unconditionally.
pub fn wrap_text(resolver: &dyn FontResolver, style: &TextStyle, text: &str, max_width: f32) -> Vec<String> {
    wrap_text_marking_paragraph_ends(resolver, style, text, max_width).0
}

/// `wrap_text`, plus a same-length `bool` per line: `true` for the last
/// line of its paragraph (the one a `Justify` renderer must leave
/// left-aligned, not stretched), `false` for every other line. A
/// paragraph is a `\n`-separated segment of `text`, same boundary
/// `wrap_text` already breaks on.
pub fn wrap_text_marking_paragraph_ends(
    resolver: &dyn FontResolver,
    style: &TextStyle,
    text: &str,
    max_width: f32,
) -> (Vec<String>, Vec<bool>) {
    let max_width = max_width.max(0.0);
    let mut lines = Vec::new();
    let mut paragraph_end = Vec::new();
    for paragraph in text.split('\n') {
        let words: Vec<&str> = paragraph.split(' ').filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            lines.push(String::new());
        } else {
            let mut current = String::new();
            for word in words {
                if current.is_empty() {
                    current = start_line(resolver, style, word, max_width, &mut lines);
                    continue;
                }
                let candidate = format!("{current} {word}");
                let w = styled_width_pt(resolver, style, &candidate);
                if w <= max_width {
                    current = candidate;
                } else {
                    lines.push(std::mem::take(&mut current));
                    current = start_line(resolver, style, word, max_width, &mut lines);
                }
            }
            lines.push(current);
        }
        // Every line just pushed for this paragraph (including any
        // hard-break pieces `start_line` pushed directly) defaults to
        // `false`; only the last one — the paragraph's actual last line —
        // flips to `true`.
        paragraph_end.resize(lines.len(), false);
        if let Some(last) = paragraph_end.last_mut() {
            *last = true;
        }
    }
    (lines, paragraph_end)
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

    #[test]
    fn marks_only_the_last_line_of_each_paragraph() {
        let style = TextStyle {
            size: 10.0,
            ..Default::default()
        };
        // Two paragraphs: "AAAA BBBB" wraps to 2 lines at width 30, "CCCC"
        // fits on one line by itself.
        let (lines, paragraph_end) = wrap_text_marking_paragraph_ends(&FixedResolver, &style, "AAAA BBBB\nCCCC", 30.0);
        assert_eq!(lines, vec!["AAAA".to_string(), "BBBB".to_string(), "CCCC".to_string()]);
        assert_eq!(paragraph_end, vec![false, true, true]);
    }

    #[test]
    fn empty_paragraph_counts_as_its_own_last_line() {
        let style = TextStyle::default();
        let (lines, paragraph_end) = wrap_text_marking_paragraph_ends(&FixedResolver, &style, "a\n\nb", 1000.0);
        assert_eq!(lines, vec!["a".to_string(), String::new(), "b".to_string()]);
        assert_eq!(paragraph_end, vec![true, true, true]);
    }
}
