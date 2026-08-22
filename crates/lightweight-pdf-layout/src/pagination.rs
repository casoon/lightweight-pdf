//! Two-pass pagination: pass 1 counts pages, pass 2
//! runs again with `total_pages` known so `Header`/`Footer` closures see
//! correct values. Header/Footer bands are fixed at document-creation time
//! (ADR-011) — the body content-box is therefore identical across both
//! passes and every page, which is what makes the page count invariant.

use crate::geometry::Rect;
use crate::layoutable::{coerce_to_fit, measure_at_width, push_warning, LayoutCtx, LayoutResult, Layoutable};
use crate::render_node::RenderNode;
use crate::warnings::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_core::{Align, Column, Common, Document, Element, PageContext};

const EPS: f32 = 0.01;
/// Safety valve against a pathological layout bug spinning forever
/// (Grundprinzip 7's "harte Obergrenze" principle applied to pagination
/// itself, not just a single oversized element).
const HARD_PAGE_LIMIT: usize = 10_000;

pub struct PageRender {
    pub page_number: usize,
    pub header: Option<RenderNode>,
    pub footer: Option<RenderNode>,
    pub body: RenderNode,
}

pub struct PaginatedDocument {
    pub page_width: f32,
    pub page_height: f32,
    /// The body content box, identical on every page (ADR-011: fixed
    /// header/footer bands make it page-count-invariant). Exposed so the
    /// facade can clip a document-level watermark to it (Phase 6) without
    /// recomputing margins/band heights itself.
    pub body_area: Rect,
    pub pages: Vec<PageRender>,
    pub warnings: Vec<LayoutWarning>,
}

/// Repeatedly lays the document body out into an identical, fixed-size box
/// per page until every child has been placed. Returns one `RenderNode`
/// per page.
pub fn paginate_body(children: &[Element], body_area: Rect, ctx: &LayoutCtx, warnings: &mut Vec<LayoutWarning>) -> Vec<RenderNode> {
    let mut remaining = Element::Column(Column {
        children: children.to_vec(),
        gap: 0.0,
        align: Align::Start,
        common: Common::default(),
    });
    let mut pages = Vec::new();
    let mut page_num = 1usize;
    loop {
        match remaining.layout(ctx, body_area, warnings, page_num) {
            LayoutResult::Fit(node) => {
                pages.push(node);
                break;
            }
            LayoutResult::Split { current, remainder } => {
                pages.push(current);
                remaining = remainder;
                page_num += 1;
                if page_num > HARD_PAGE_LIMIT {
                    break;
                }
            }
        }
    }
    pages
}

fn layout_band(el: &Element, area: Rect, ctx: &LayoutCtx, warnings: &mut Vec<LayoutWarning>, page: usize) -> RenderNode {
    let natural = measure_at_width(ctx, el, area.width);
    if natural.height > area.height + EPS {
        push_warning(
            warnings,
            LayoutWarningKind::HeaderFooterOverflow,
            page,
            "Header/Footer content taller than reserved band",
        );
    }
    // Header/Footer never spans pages: keep whatever fit, the overflow
    // warning above already flagged the clipped remainder.
    coerce_to_fit(el.layout(ctx, area, warnings, page))
}

pub fn paginate(doc: &Document, ctx: &LayoutCtx) -> PaginatedDocument {
    let (page_w, page_h) = doc.page_format.size();
    let header_h = doc.header.as_ref().map(|h| h.height).unwrap_or(0.0);
    let footer_h = doc.footer.as_ref().map(|h| h.height).unwrap_or(0.0);
    let body_w = (page_w - doc.margin.left - doc.margin.right).max(0.0);
    let body_h = (page_h - doc.margin.top - doc.margin.bottom - header_h - footer_h).max(0.0);
    let body_area = Rect {
        x: doc.margin.left,
        y: doc.margin.top + header_h,
        width: body_w,
        height: body_h,
    };

    // Pass 1: layout without `total_pages` — only used to determine the
    // page count.
    let mut pass1_warnings = Vec::new();
    let pass1_pages = paginate_body(&doc.children, body_area, ctx, &mut pass1_warnings);
    let total_pages = pass1_pages.len().max(1);

    // Pass 2: independent re-run, now with `total_pages` available to
    // Header/Footer closures. Same measure/layout code path as pass 1; the
    // body box is identical, so this reproduces the exact same split
    // points (verified by a dedicated test).
    let mut warnings = Vec::new();
    let pass2_pages = paginate_body(&doc.children, body_area, ctx, &mut warnings);

    let mut pages = Vec::with_capacity(total_pages);
    for (i, body_node) in pass2_pages.into_iter().enumerate() {
        let page_number = i + 1;
        let pc = PageContext {
            page: page_number,
            total_pages,
        };

        let header = if page_number >= doc.header_visible_from {
            doc.header.as_ref().map(|h| {
                let el = (h.content)(&pc);
                let area = Rect {
                    x: doc.margin.left,
                    y: doc.margin.top,
                    width: body_w,
                    height: h.height,
                };
                layout_band(&el, area, ctx, &mut warnings, page_number)
            })
        } else {
            None
        };

        let footer = if page_number >= doc.footer_visible_from {
            doc.footer.as_ref().map(|f| {
                let el = (f.content)(&pc);
                let area = Rect {
                    x: doc.margin.left,
                    y: page_h - doc.margin.bottom - footer_h,
                    width: body_w,
                    height: f.height,
                };
                layout_band(&el, area, ctx, &mut warnings, page_number)
            })
        } else {
            None
        };

        pages.push(PageRender {
            page_number,
            header,
            footer,
            body: body_node,
        });
    }

    PaginatedDocument {
        page_width: page_w,
        page_height: page_h,
        body_area,
        pages,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedMetrics;
    impl crate::font_resolver::FontMetrics for FixedMetrics {
        fn advance(&self, _ch: char) -> f32 {
            600.0
        }
        fn ascent(&self) -> f32 {
            800.0
        }
        fn descent(&self) -> f32 {
            -200.0
        }
    }
    struct FixedResolver;
    impl crate::font_resolver::FontResolver for FixedResolver {
        fn metrics(&self, _key: lightweight_pdf_core::FontKey) -> &dyn crate::font_resolver::FontMetrics {
            &FixedMetrics
        }
    }

    #[test]
    fn pass1_and_pass2_page_counts_match() {
        let ctx = LayoutCtx { resolver: &FixedResolver };
        let children: Vec<Element> = (0..40)
            .map(|i| Element::Text(lightweight_pdf_core::Text::new(format!("Zeile {i} mit etwas Text drumherum."))))
            .collect();
        let body_area = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 200.0,
        };
        let mut w1 = Vec::new();
        let mut w2 = Vec::new();
        let p1 = paginate_body(&children, body_area, &ctx, &mut w1);
        let p2 = paginate_body(&children, body_area, &ctx, &mut w2);
        assert_eq!(p1.len(), p2.len());
        assert!(p1.len() > 1, "expected the long body to span multiple pages");
    }
}
