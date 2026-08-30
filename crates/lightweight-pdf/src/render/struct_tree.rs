//! Structure-tree building during rendering (issue #27) — mirrors how
//! `text::build_outline` builds `pdf.outline`, but this needs a stack: a
//! `Table`/`List`/heading can nest arbitrarily deep, and `enter`/`exit`
//! calls (from `tree::render_node`'s `RenderNode::Tagged` handling) track
//! exactly where the walk currently is in that nesting.

use lightweight_pdf_writer::PdfStructNode;

/// `stack[0]` is the `Document` root's children; [`Self::enter`] pushes a
/// fresh accumulator (a new open ancestor), [`Self::exit`] pops it, wraps
/// it as a [`PdfStructNode::Elem`], and pushes that into the new top (its
/// parent's accumulator). [`Self::next_content_ref`] is the leaf case:
/// records one `ContentRef` in whatever accumulator is currently open —
/// `tree::render_node` always calls it between a matching `enter`/`exit`
/// pair (see that function's `RenderNode::Tagged` arm), so a leaf role
/// (`H1`..`H6`/`P`/`Figure`) ends up with exactly one `ContentRef` child,
/// structurally identical to a grouping role that happened to have only
/// one child.
pub(super) struct StructTreeBuilder {
    stack: Vec<Vec<PdfStructNode>>,
    next_mcid: u32,
}

impl StructTreeBuilder {
    pub(super) fn new() -> Self {
        StructTreeBuilder {
            stack: vec![Vec::new()],
            next_mcid: 0,
        }
    }

    /// Resets the per-page MCID counter — call once per page, before
    /// rendering it (MCIDs are only unique within one page, ISO 32000-1
    /// 14.6.2), not once per document.
    pub(super) fn start_page(&mut self) {
        self.next_mcid = 0;
    }

    pub(super) fn enter(&mut self) {
        self.stack.push(Vec::new());
    }

    pub(super) fn exit(&mut self, tag: &'static str, alt: Option<String>) {
        let children = self.stack.pop().expect("exit without a matching enter");
        // `/Scope /Column` on every `TH` (issue #27, ISO 32000-1
        // 14.8.5.7) — every table this crate produces has its header as
        // a single top row with one `TH` per column, so this always
        // correctly associates that column's `TD`s with it. Derived
        // here from `tag` rather than threaded through every
        // `render_tagged` call site, since it's the same fixed value
        // for every `TH`, always.
        let attrs = (tag == "TH").then_some("/O /Table /Scope /Column");
        let elem = PdfStructNode::Elem { tag, alt, attrs, children };
        self.stack.last_mut().expect("Document root accumulator is never popped").push(elem);
    }

    /// Allocates the next MCID for the current page and records a
    /// `ContentRef` for it in the currently-open accumulator. Returns the
    /// MCID so the caller can emit the matching `BDC`/`EMC` pair around
    /// it in the content stream.
    pub(super) fn next_content_ref(&mut self, page_index: usize) -> u32 {
        let mcid = self.next_mcid;
        self.next_mcid += 1;
        self.stack
            .last_mut()
            .expect("Document root accumulator is never popped")
            .push(PdfStructNode::ContentRef { page_index, mcid });
        mcid
    }

    /// Finishes the tree — call once, after every page has been
    /// rendered. Panics on unbalanced `enter`/`exit` calls, a bug in this
    /// crate (every `enter` in `tree::render_node` has exactly one
    /// `exit`), never something a caller's `Document` content can trigger.
    pub(super) fn finish(mut self) -> PdfStructNode {
        let children = self.stack.pop().expect("Document root accumulator");
        assert!(self.stack.is_empty(), "unbalanced enter/exit calls building the structure tree");
        PdfStructNode::Elem {
            tag: "Document",
            alt: None,
            attrs: None,
            children,
        }
    }
}

/// Finds the first `Image`'s `alt` text in `node` — used when a `Figure`
/// role's `/Alt` value lives a level or two down (`Image::layout` wraps
/// its `RenderNode::Image` in a clipping `Group`, see `image.rs`), not
/// because a `Figure` could ever legitimately contain more than one
/// image.
pub(super) fn find_image_alt(node: &lightweight_pdf_layout::RenderNode) -> Option<String> {
    use lightweight_pdf_layout::RenderNode;
    match node {
        RenderNode::Image { alt, .. } => alt.clone(),
        RenderNode::Group { children, .. } => children.iter().find_map(find_image_alt),
        RenderNode::Tagged { inner, .. } => find_image_alt(inner),
        _ => None,
    }
}
