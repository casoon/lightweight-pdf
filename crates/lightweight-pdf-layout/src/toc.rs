//! `TableOfContents` (issue #10): self-populates from every
//! `Text::outline_level` heading in the document, with correct page
//! numbers, by riding the two-pass pagination `pagination::paginate`
//! already runs to determine `total_pages` for Header/Footer.
//!
//! - [`prepare_toc`] runs once, before either pass: it walks the
//!   (pre-layout) `Element` tree collecting every heading in document
//!   order, and returns a *clone* of that tree where every heading that
//!   didn't already have an author-set `Text::anchor` gets a synthetic
//!   one — needed so a `TableOfContents` entry can `link_to` it without
//!   the author having to hand-anchor every heading. `LayoutCtx::toc_headings`
//!   is this same list, identical in both passes (it doesn't depend on
//!   page numbers at all — only on the document's own content).
//! - Pass 1 runs (unaware of any of this beyond `toc_headings` being
//!   available), and [`collect_anchor_pages`] walks its resulting
//!   `RenderNode` tree to find which page every anchor (heading or not)
//!   landed on.
//! - Pass 2 sees that map as `LayoutCtx::toc_heading_pages` and a
//!   `TableOfContents::layout` fills in real page numbers.
//!
//! Page-count stability (the two-pass architecture's core invariant,
//! `pagination.rs`'s module doc) holds because a `TableOfContents`'s
//! height only depends on how many headings match `max_depth` — fixed,
//! known content, the same in both passes — never on what page number
//! text ends up printed next to them: entries never wrap (always exactly
//! one line each), so differing digit counts between passes can't shift
//! a line count.

use crate::font_resolver::FontResolver;
use crate::geometry::{Constraints, Rect, Size};
use crate::layoutable::{line_height_pt, LayoutCtx, LayoutResult, Layoutable};
use crate::render_node::RenderNode;
use crate::text::text_width_pt;
use crate::warnings::LayoutWarning;
use lightweight_pdf_core::{Align, Element, TableOfContents, TextStyle};
use std::collections::HashMap;

/// One heading, discovered from the pre-layout `Element` tree by
/// [`prepare_toc`] — everything a `TableOfContents` entry needs except
/// the page number (added later, pass 2 only, via `LayoutCtx::toc_heading_pages`).
#[derive(Clone, Debug)]
pub struct TocHeading {
    pub depth: u8,
    pub title: String,
    /// Either the heading's own `Text::anchor`, or a synthetic one
    /// `prepare_toc` assigned (NUL-prefixed — not something an author can
    /// type via a normal string literal, so it can never collide with a
    /// real anchor name).
    pub anchor: String,
}

fn synthetic_anchor(id: usize) -> String {
    format!("\u{0}toc-heading-{id}")
}

/// Recurses only `Row`/`Column` children — headings inside a `Table`
/// cell or `List` item are out of scope for V1 (same "document this
/// instead of chasing every container" call as rich text's scope notes).
fn prepare_one(element: &Element, headings: &mut Vec<TocHeading>, next_id: &mut usize) -> Element {
    match element {
        Element::Text(t) => {
            let Some(depth) = t.outline_level else {
                return element.clone();
            };
            let mut t = t.clone();
            let anchor = t.anchor.clone().unwrap_or_else(|| {
                let name = synthetic_anchor(*next_id);
                *next_id += 1;
                name
            });
            t.anchor = Some(anchor.clone());
            headings.push(TocHeading {
                depth,
                title: t.content.clone(),
                anchor,
            });
            Element::Text(t)
        }
        Element::Row(r) => {
            let mut r = r.clone();
            r.children = r.children.iter().map(|c| prepare_one(c, headings, next_id)).collect();
            Element::Row(r)
        }
        Element::Column(c) => {
            let mut c = c.clone();
            c.children = c.children.iter().map(|c| prepare_one(c, headings, next_id)).collect();
            Element::Column(c)
        }
        _ => element.clone(),
    }
}

/// Runs once per `paginate()` call, before pass 1. Returns the (possibly
/// anchor-injected) element tree to lay out instead of `doc.children`,
/// plus the heading list both passes see via `LayoutCtx::toc_headings`.
pub fn prepare_toc(elements: &[Element]) -> (Vec<Element>, Vec<TocHeading>) {
    let mut headings = Vec::new();
    let mut next_id = 0usize;
    let prepared = elements.iter().map(|e| prepare_one(e, &mut headings, &mut next_id)).collect();
    (prepared, headings)
}

/// Walks pass 1's resulting pages once, recording the (1-based) page
/// number every anchor (heading or not — cheaper to collect all of them
/// than to filter) first appears on.
pub fn collect_anchor_pages(pages: &[RenderNode]) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for (i, page) in pages.iter().enumerate() {
        collect_anchor_pages_in_node(page, i + 1, &mut out);
    }
    out
}

fn collect_anchor_pages_in_node(node: &RenderNode, page_number: usize, out: &mut HashMap<String, usize>) {
    match node {
        RenderNode::Group { children, .. } => {
            for child in children {
                collect_anchor_pages_in_node(child, page_number, out);
            }
        }
        RenderNode::TextLines { anchor: Some(name), .. } => {
            out.entry(name.clone()).or_insert(page_number);
        }
        _ => {}
    }
}

fn matching_headings<'a>(ctx: &'a LayoutCtx, toc: &TableOfContents) -> impl Iterator<Item = &'a TocHeading> {
    let max_depth = toc.max_depth;
    ctx.toc_headings.iter().filter(move |h| h.depth <= max_depth).skip(toc.skip)
}

/// One entry's full display line: `{indent}{title} {leader...} {page}`,
/// with the leader run sized so the whole line comes as close to
/// `area_width` as an integer number of leader characters allows —
/// there's no true right-alignment machinery here, just enough leader
/// fill to visually line up the page-number column.
fn toc_entry_line(
    resolver: &dyn FontResolver,
    style: &TextStyle,
    heading: &TocHeading,
    page_text: &str,
    area_width: f32,
    leader: char,
) -> String {
    let indent = "  ".repeat(heading.depth.saturating_sub(1) as usize);
    let title = format!("{indent}{}", heading.title);
    let space_w = text_width_pt(resolver, style.font, style.size, " ");
    let base_width = text_width_pt(resolver, style.font, style.size, &title)
        + 2.0 * space_w
        + text_width_pt(resolver, style.font, style.size, page_text);
    let available = (area_width - base_width).max(0.0);
    let leader_w = text_width_pt(resolver, style.font, style.size, &leader.to_string());
    let leader_count = if leader_w > 0.0 {
        (available / leader_w).floor() as usize
    } else {
        0
    };
    let leaders: String = std::iter::repeat_n(leader, leader_count).collect();
    format!("{title} {leaders} {page_text}")
}

fn entry_node(
    resolver: &dyn FontResolver,
    style: &TextStyle,
    heading: &TocHeading,
    page_text: &str,
    area: Rect,
    leader: char,
) -> RenderNode {
    let line = toc_entry_line(resolver, style, heading, page_text, area.width, leader);
    RenderNode::TextLines {
        area,
        style: TextStyle {
            align: Align::Start,
            ..*style
        },
        lines: vec![line],
        paragraph_end: vec![true],
        line_height_pt: area.height,
        url: None,
        anchor: None,
        link_to: Some(heading.anchor.clone()),
        outline_level: None,
    }
}

impl Layoutable for TableOfContents {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        let width = self.common.width.unwrap_or(constraints.max_width);
        let n = matching_headings(ctx, self).count();
        let lh = line_height_pt(&self.style);
        Size {
            width: self.common.width.unwrap_or(width),
            height: self.common.height.unwrap_or(n as f32 * lh),
        }
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, _warnings: &mut Vec<LayoutWarning>, _page: usize) -> LayoutResult {
        let entries: Vec<&TocHeading> = matching_headings(ctx, self).collect();
        let lh = line_height_pt(&self.style);
        let max_fit = (((area.height + 0.01) / lh).floor().max(0.0) as usize).min(entries.len());

        let rendered: Vec<RenderNode> = entries[..max_fit]
            .iter()
            .enumerate()
            .map(|(i, heading)| {
                let page_text = ctx
                    .toc_heading_pages
                    .and_then(|pages| pages.get(&heading.anchor))
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                let entry_area = Rect {
                    x: area.x,
                    y: area.y + i as f32 * lh,
                    width: area.width,
                    height: lh,
                };
                entry_node(ctx.resolver, &self.style, heading, &page_text, entry_area, self.leader)
            })
            .collect();

        let current = if rendered.is_empty() {
            RenderNode::Empty
        } else {
            RenderNode::clipped(
                Rect {
                    height: max_fit as f32 * lh,
                    ..area
                },
                RenderNode::Group {
                    area: Rect {
                        height: max_fit as f32 * lh,
                        ..area
                    },
                    clip: false,
                    background: None,
                    border: None,
                    corner_radius: 0.0,
                    children: rendered,
                },
            )
        };

        if max_fit >= entries.len() {
            LayoutResult::Fit(current)
        } else {
            LayoutResult::Split {
                current,
                remainder: Element::TableOfContents(TableOfContents {
                    skip: self.skip + max_fit,
                    ..self.clone()
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightweight_pdf_core::Text;

    #[test]
    fn prepare_toc_collects_headings_in_document_order_and_assigns_synthetic_anchors() {
        let elements = vec![
            Element::Text(Text::new("Intro").heading1()),
            Element::Text(Text::new("Body copy")),
            Element::Text(Text::new("Details").heading2().anchor("my-anchor")),
        ];
        let (prepared, headings) = prepare_toc(&elements);
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].title, "Intro");
        assert_eq!(headings[0].depth, 1);
        assert!(
            headings[0].anchor.starts_with('\u{0}'),
            "expected a synthetic anchor for the un-anchored heading"
        );
        assert_eq!(headings[1].anchor, "my-anchor", "an author-set anchor must be kept as-is");

        let Element::Text(first) = &prepared[0] else {
            panic!("expected a Text element");
        };
        assert_eq!(first.anchor.as_deref(), Some(headings[0].anchor.as_str()));
    }

    #[test]
    fn prepare_toc_recurses_into_row_and_column_children() {
        let elements = vec![Element::Column(
            lightweight_pdf_core::Column::new().child(Text::new("Nested heading").heading1()),
        )];
        let (_prepared, headings) = prepare_toc(&elements);
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].title, "Nested heading");
    }

    #[test]
    fn collect_anchor_pages_maps_each_anchor_to_its_first_page() {
        let page1 = RenderNode::TextLines {
            area: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 10.0,
            },
            style: TextStyle::default(),
            lines: vec!["Heading".into()],
            paragraph_end: vec![true],
            line_height_pt: 10.0,
            url: None,
            anchor: Some("h1".into()),
            link_to: None,
            outline_level: Some(1),
        };
        let pages = collect_anchor_pages(&[RenderNode::Empty, page1]);
        assert_eq!(pages.get("h1"), Some(&2), "the heading landed on the second page");
    }
}
