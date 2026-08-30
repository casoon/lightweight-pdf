//! `measure`/`layout`. Implemented for every concrete element type (not
//! `Element` variants with `todo!()`, since all V1-through-Phase-2
//! variants are implemented) plus a dispatching impl on `Element` itself
//! so containers can recurse over `Vec<Element>` children.
//!
//! Split across submodules by concern: `shared` holds the box-model
//! helpers reused by every impl (also re-exported here for `table.rs`,
//! `list.rs`, `pagination.rs`), `row`/`column` hold the two container
//! impls, and `leaf` holds the childless element impls (`Text`, `Spacer`,
//! `Line`, `Rect`).

mod column;
mod leaf;
mod row;
mod shared;

pub(crate) use shared::{
    clip_to_fixed_height, coerce_to_fit, coerce_to_fit_and_warn, finish_fit, line_height_pt, measure_at_width, push_warning,
    resolve_auto_size, resolve_bound, shrink_and_bound_height, wrap_children,
};

use crate::font_resolver::FontResolver;
use crate::geometry::{Constraints, Rect, Size};
use crate::render_node::{RenderNode, StructRole};
use crate::toc::TocHeading;
use crate::warnings::LayoutWarning;
use lightweight_pdf_core::Element;
use std::collections::HashMap;

/// Wraps a `LayoutResult`'s `RenderNode` (the `Fit` case, or `Split`'s
/// `current`/already-fitted part) with `role` — `Split`'s `remainder` is
/// still an `Element`, not a `RenderNode` yet, and gets tagged again on
/// its own when it's laid out on the next page (issue #27: a logical
/// element split across pages becomes sibling `StructElem`s, one per
/// page, rather than one element straddling a page boundary — simpler,
/// and still valid tagged PDF).
pub(crate) fn wrap_result(result: LayoutResult, role: StructRole) -> LayoutResult {
    match result {
        LayoutResult::Fit(node) => LayoutResult::Fit(RenderNode::tagged(role, node)),
        LayoutResult::Split { current, remainder } => LayoutResult::Split {
            current: RenderNode::tagged(role, current),
            remainder,
        },
    }
}

pub struct LayoutCtx<'a> {
    pub resolver: &'a dyn FontResolver,
    /// Every heading (`Text::outline_level`) in the whole document, in
    /// document order — independent of pagination (issue #10's
    /// `TableOfContents`), so identical in both layout passes.
    pub toc_headings: &'a [TocHeading],
    /// `None` during pass 1 (page numbers aren't known yet — a
    /// `TableOfContents` renders its entries without one). `Some` during
    /// pass 2, keyed by each heading's `TocHeading::anchor`, filled in
    /// from pass 1's own result — the same "pass 1 informs pass 2"
    /// mechanism `PageContext.total_pages` uses for Header/Footer.
    pub toc_heading_pages: Option<&'a HashMap<String, usize>>,
}

impl<'a> LayoutCtx<'a> {
    /// A `LayoutCtx` with no `TableOfContents` data — what every caller
    /// outside `pagination::paginate` wants (it fills in the real
    /// per-pass values itself).
    pub fn new(resolver: &'a dyn FontResolver) -> Self {
        LayoutCtx {
            resolver,
            toc_headings: &[],
            toc_heading_pages: None,
        }
    }
}

/// Result of laying an element out into a bounded area: either it fully
/// fit, or the fitting part plus a materialized remainder element for the
/// next page. `Text`, `Column` and `Table` produce
/// `Split` in V1.
pub enum LayoutResult {
    Fit(RenderNode),
    Split { current: RenderNode, remainder: Element },
}

pub trait Layoutable {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size;
    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult;
}

// ---------------------------------------------------------------------
// Element: dispatch to the concrete impls below. `PageBreak` has no
// intrinsic size/rendering of its own — `Column`'s layout loop intercepts
// it before ever calling into this generic path.
// ---------------------------------------------------------------------

impl Layoutable for Element {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        match self {
            Element::Text(t) => t.measure(ctx, constraints),
            Element::Row(r) => r.measure(ctx, constraints),
            Element::Column(c) => c.measure(ctx, constraints),
            Element::Spacer(s) => s.measure(ctx, constraints),
            Element::Line(l) => l.measure(ctx, constraints),
            Element::Rect(r) => r.measure(ctx, constraints),
            Element::Table(t) => t.measure(ctx, constraints),
            Element::Image(i) => i.measure(ctx, constraints),
            Element::List(l) => l.measure(ctx, constraints),
            Element::TableOfContents(t) => t.measure(ctx, constraints),
            Element::PageBreak => Size::default(),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        match self {
            // Structure-tree tagging (issue #27): attached here, at the
            // one place every element's `RenderNode` output already
            // passes through, rather than at each element's own
            // construction site. `Table`/`List`/`TableOfContents` tag
            // their own row/cell/item structure internally (only they
            // know it) — this just adds the outer `Table`/`List`/`Toc`
            // wrapper. `Row`/`Column`/`Spacer`/`Line`/`Rect` are left
            // unwrapped: pure containers contribute no content of their
            // own (their children tag themselves recursively).
            Element::Text(t) => {
                let role = match t.outline_level {
                    Some(n) => StructRole::Heading(n),
                    None => StructRole::Paragraph,
                };
                wrap_result(t.layout(ctx, area, warnings, page), role)
            }
            Element::Row(r) => r.layout(ctx, area, warnings, page),
            Element::Column(c) => c.layout(ctx, area, warnings, page),
            Element::Spacer(s) => s.layout(ctx, area, warnings, page),
            Element::Line(l) => l.layout(ctx, area, warnings, page),
            Element::Rect(r) => r.layout(ctx, area, warnings, page),
            Element::Table(t) => wrap_result(t.layout(ctx, area, warnings, page), StructRole::Table),
            Element::Image(i) => wrap_result(i.layout(ctx, area, warnings, page), StructRole::Figure),
            Element::List(l) => wrap_result(l.layout(ctx, area, warnings, page), StructRole::List),
            Element::TableOfContents(t) => wrap_result(t.layout(ctx, area, warnings, page), StructRole::Toc),
            Element::PageBreak => LayoutResult::Fit(RenderNode::Empty),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shared::EPS;
    use super::*;
    use crate::pagination::paginate_body;
    use crate::warnings::LayoutWarningKind;
    use lightweight_pdf_core::{Column, Common, Overflow as OverflowKind, Rect as RectElement, Row, Span, Text as TextEl, TextStyle};

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
        fn metrics(&self, _key: lightweight_pdf_core::FontKey) -> &dyn crate::font_resolver::FontMetrics {
            &FixedMetrics
        }
    }

    fn ctx() -> LayoutCtx<'static> {
        LayoutCtx::new(&FixedResolver)
    }

    // --- Grundprinzip 1: auto-size is the default -----------------------

    #[test]
    fn column_auto_size_grows_with_content() {
        let short = Column::new().child(TextEl::new("Hi").size(10.0).line_height(1.0));
        let long = Column::new().children(vec![
            TextEl::new("Line one").size(10.0).line_height(1.0),
            TextEl::new("Line two").size(10.0).line_height(1.0),
            TextEl::new("Line three").size(10.0).line_height(1.0),
        ]);
        let c = ctx();
        let constraints = Constraints {
            max_width: 400.0,
            max_height: f32::INFINITY,
        };
        let short_size = short.measure(&c, constraints);
        let long_size = long.measure(&c, constraints);
        assert!(long_size.height > short_size.height, "more content must measure taller");
    }

    // --- Grundprinzip 2/3: hard-break + fixed-size Clip (never Split) ---

    #[test]
    fn fixed_height_text_clips_instead_of_splitting() {
        let text = TextEl::new("AAAA BBBB CCCC DDDD").size(10.0).line_height(1.0).height(10.0);
        let c = ctx();
        let mut warnings = Vec::new();
        // Narrow width forces multiple lines; the box is only 1 line tall.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 30.0,
            height: 10.0,
        };
        let result = text.layout(&c, area, &mut warnings, 1);
        assert!(
            matches!(result, LayoutResult::Fit(_)),
            "fixed-size box must Clip, never Split across pages"
        );
        assert!(warnings.iter().any(|w| w.kind == LayoutWarningKind::TextClipped));
    }

    #[test]
    fn fixed_height_column_clips_instead_of_splitting() {
        let col = Column::new().height(10.0).children(vec![
            TextEl::new("Line one").size(10.0).line_height(1.0),
            TextEl::new("Line two").size(10.0).line_height(1.0),
            TextEl::new("Line three").size(10.0).line_height(1.0),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 10.0,
        };
        let result = col.layout(&c, area, &mut warnings, 1);
        assert!(matches!(result, LayoutResult::Fit(_)), "fixed-height Column must Clip, never Split");
        assert!(warnings.iter().any(|w| w.kind == LayoutWarningKind::ContentOverflow));
    }

    // --- Grundprinzip 4/6: containers/children never overlap ------------

    #[test]
    fn row_children_do_not_overlap_horizontally() {
        let row = Row::new()
            .gap(10.0)
            .child(TextEl::new("Left").size(10.0))
            .child(TextEl::new("Right").size(10.0));
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 50.0,
        };
        let result = row.layout(&c, area, &mut warnings, 1);
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = result else {
            panic!("expected a Fit Group");
        };
        assert_eq!(children.len(), 2);
        let rects: Vec<Rect> = children
            .iter()
            .map(|n| match n.untagged() {
                RenderNode::Group { area, .. } => *area,
                other => panic!("expected nested Group, got {other:?}"),
            })
            .collect();
        assert!(
            rects[0].x + rects[0].width <= rects[1].x + EPS,
            "children must not overlap: {:?} vs {:?}",
            rects[0],
            rects[1]
        );
    }

    // --- Phase 2: PageBreak ----------------------------------------------

    #[test]
    fn page_break_forces_a_split_at_the_marker() {
        let col = Column::new().children(vec![
            Element::Text(TextEl::new("a")),
            Element::PageBreak,
            Element::Text(TextEl::new("b")),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 400.0, // plenty of room — the break must still trigger.
        };
        match col.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { remainder, .. } => match remainder {
                Element::Column(rem) => {
                    assert_eq!(rem.children.len(), 1);
                    match &rem.children[0] {
                        Element::Text(t) => assert_eq!(t.content, "b"),
                        other => panic!("expected Text, got {other:?}"),
                    }
                }
                other => panic!("expected Column remainder, got {other:?}"),
            },
            LayoutResult::Fit(_) => panic!("PageBreak must force a Split even when content would otherwise fit"),
        }
    }

    // --- Grundprinzip 7: atomic element bigger than a page --------------

    #[test]
    fn oversized_atomic_element_is_forced_onto_its_own_page_and_terminates() {
        let children = vec![
            Element::Rect(RectElement::new().height(5000.0).background(lightweight_pdf_core::Color::BLACK)),
            Element::Rect(RectElement::new().height(20.0)),
        ];
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        let pages = paginate_body(&children, area, &c, &mut warnings);
        assert_eq!(
            pages.len(),
            2,
            "oversized element consumes its own page, second Rect starts a fresh one"
        );
        assert_eq!(warnings.iter().filter(|w| w.kind == LayoutWarningKind::ForcedPageBreak).count(), 1);
    }

    // --- Grundprinzip 9: widow/orphan + short-paragraph-never-split -----

    fn line_text(n: usize) -> String {
        (0..n).map(|i| format!("L{i}")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn short_paragraph_is_never_split() {
        // 3 lines < 2*N(=4): must move as a whole even though 2 lines
        // would technically fit.
        let text = TextEl::new(line_text(3)).size(10.0).line_height(1.0);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 20.0, // fits 2 of 3 lines by height alone
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                assert!(
                    matches!(current, RenderNode::Empty),
                    "short paragraph must move whole, nothing placed on this page"
                );
                match remainder {
                    Element::Text(t) => assert_eq!(t.content, line_text(3)),
                    other => panic!("expected Text remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected a Split (paragraph doesn't fully fit)"),
        }
    }

    #[test]
    fn widow_is_avoided_by_pulling_lines_up() {
        // 5 lines, only 4 fit by height -> naive split would leave 1
        // (widow). Rule pulls lines up so >= N=2 remain after the break.
        let text = TextEl::new(line_text(5)).size(10.0).line_height(1.0);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 40.0, // exactly 4 lines at 10pt line-height
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                let RenderNode::Group { children, .. } = current else {
                    panic!("expected the clip-wrapping Group");
                };
                let RenderNode::TextLines { lines, .. } = &children[0] else {
                    panic!("expected TextLines");
                };
                assert_eq!(lines.len(), 3, "must pull one line up so the remainder has >= 2 lines");
                match remainder {
                    Element::Text(t) => assert_eq!(t.content.split(' ').count(), 2),
                    other => panic!("expected Text remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected a Split"),
        }
    }

    #[test]
    fn orphan_moves_whole_paragraph_when_room_is_too_small() {
        // 5 lines, only 1 fits by height -> orphan (< N before break) ->
        // move the whole paragraph.
        let text = TextEl::new(line_text(5)).size(10.0).line_height(1.0);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 10.0,
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, .. } => {
                assert!(matches!(current, RenderNode::Empty));
            }
            LayoutResult::Fit(_) => panic!("expected a Split"),
        }
    }

    // --- Grundprinzip 9: keep_with_next ----------------------------------

    #[test]
    fn keep_with_next_moves_heading_along_with_its_body() {
        let col = Column::new().gap(0.0).children(vec![
            Element::Text(TextEl::new("Filler").size(10.0).line_height(1.0)),
            Element::Text(TextEl::new("Heading").size(10.0).line_height(1.0).keep_with_next()),
            Element::Text(TextEl::new("Body").size(10.0).line_height(1.0)),
        ]);
        let c = ctx();
        let mut warnings = Vec::new();
        // 10 (filler) + 10 (heading) fits, but leaves only 5pt — not
        // enough for one more 10pt line of body text.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 25.0,
        };
        match col.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                let RenderNode::Group { children, .. } = current else {
                    panic!("expected Group");
                };
                assert_eq!(children.len(), 1, "only the filler should remain on this page");
                match remainder {
                    Element::Column(rem) => {
                        assert_eq!(rem.children.len(), 2);
                        match &rem.children[0] {
                            Element::Text(t) => assert_eq!(t.content, "Heading"),
                            other => panic!("expected Heading Text, got {other:?}"),
                        }
                    }
                    other => panic!("expected Column remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected keep_with_next to force a Split before the heading"),
        }
    }

    #[test]
    fn overflow_ellipsis_truncates_fixed_single_line_text() {
        let text = TextEl::new("AAAAAAAAAAAAAAAA")
            .size(10.0)
            .line_height(1.0)
            .height(10.0)
            .overflow(OverflowKind::Ellipsis);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 10.0,
        };
        let result = text.layout(&c, area, &mut warnings, 1);
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = result else {
            panic!("expected Fit Group (clip wrapper)");
        };
        let RenderNode::TextLines { lines, .. } = &children[0] else {
            panic!("expected TextLines");
        };
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('…'), "expected an ellipsis, got {:?}", lines[0]);
    }

    #[test]
    fn common_default_is_used() {
        // Sanity check that Common::default() means "auto", not zero-sized.
        let c = Common::default();
        assert_eq!(c.width, None);
        assert_eq!(c.height, None);
    }

    // --- Text::rich(..) (issue #11) --------------------------------------

    #[test]
    fn rich_text_wraps_words_from_multiple_spans_in_order() {
        let style = TextStyle {
            size: 10.0,
            line_height: 1.0,
            ..Default::default()
        };
        let text = TextEl::rich([Span::new("AAAA", style), Span::new(" BBBB", style)]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 100.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = text.layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        let RenderNode::RichTextLines { lines, .. } = &children[0] else {
            panic!("expected RichTextLines");
        };
        assert_eq!(lines.len(), 1, "both words fit on one line");
        let words: Vec<&str> = lines[0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(words, vec!["AAAA", "BBBB"], "words from both spans, in order, on the same line");
    }

    #[test]
    fn rich_text_mixed_sizes_share_one_line_height_and_ascent() {
        let small = TextStyle {
            size: 10.0,
            line_height: 1.0,
            ..Default::default()
        };
        let big = TextStyle {
            size: 20.0,
            line_height: 1.0,
            ..Default::default()
        };
        let text = TextEl::rich([Span::new("a", small), Span::new(" B", big)]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 100.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = text.layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        let RenderNode::RichTextLines { lines, .. } = &children[0] else {
            panic!("expected RichTextLines");
        };
        assert_eq!(lines.len(), 1);
        // FixedMetrics.ascent() == 800/1000 -> 16pt at size 20; the line's
        // shared baseline reference must come from the larger word, not
        // the smaller one placed first.
        assert_eq!(lines[0].height, 20.0);
        assert_eq!(lines[0].ascent_pt, 16.0);
    }

    #[test]
    fn rich_text_can_split_in_the_middle_of_a_single_span() {
        // One span, 5 short words -> 5 lines at width 15 (each "LN" word is
        // 12pt, two of them plus a 3pt space is 27pt > 15).
        let style = TextStyle {
            size: 10.0,
            line_height: 1.0,
            ..Default::default()
        };
        let text = TextEl::rich([Span::new("L0 L1 L2 L3 L4", style)]);
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 15.0,
            height: 40.0, // fits 4 of 5 lines by height alone
        };
        match text.layout(&c, area, &mut warnings, 1) {
            LayoutResult::Split { current, remainder } => {
                let RenderNode::Group { children, .. } = current else {
                    panic!("expected the clip-wrapping Group");
                };
                let RenderNode::RichTextLines { lines, .. } = &children[0] else {
                    panic!("expected RichTextLines");
                };
                assert_eq!(lines.len(), 3, "widow/orphan rule pulls one line up, same as plain text");
                match remainder {
                    Element::Text(t) => {
                        let spans = t.spans.expect("remainder of a rich Text must still be rich text");
                        assert_eq!(spans.len(), 1, "the single span continues as a single span, split mid-span");
                        assert_eq!(spans[0].text, "L3 L4");
                    }
                    other => panic!("expected Text remainder, got {other:?}"),
                }
            }
            LayoutResult::Fit(_) => panic!("expected a Split"),
        }
    }
}
