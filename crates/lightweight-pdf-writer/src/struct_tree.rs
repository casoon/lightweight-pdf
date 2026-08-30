//! Structure tree writing (issue #27, ISO 32000-1 14.7): `/StructTreeRoot`,
//! `/StructElem`, and the `/ParentTree` number tree a reader uses to map a
//! page's marked-content `MCID` back to its owning structure element
//! (14.7.4.4 "Finding Structure Elements from Content Items").
//!
//! Two-pass, same shape as `doc.rs`'s `/Outlines` writer
//! (`alloc_outline_refs`/`write_outline_siblings`): every `/StructElem`
//! needs both its parent's ref (`/P`, required on every element) and its
//! children's refs (`/K`) — allocate one `Ref` per element in a first pass
//! (mirroring the tree shape), then write bodies in a second pass that has
//! both.

use crate::doc::format_pdf_string;
use crate::writer::{PdfWriter, Ref};

/// One node in the logical structure tree the facade builds while
/// rendering (mirrors `PdfOutlineNode`'s role for `/Outlines`) — handed to
/// `PdfDocument` as a whole tree, written once page refs are known.
#[derive(Clone, Debug)]
pub enum PdfStructNode {
    /// A structure element: a PDF standard tag name (`"H1"`, `"P"`,
    /// `"Table"`, ...) plus its children — either more `Elem`s (grouping
    /// roles nest further structure) or `ContentRef`s (leaf roles, one
    /// per marked-content span).
    Elem {
        tag: &'static str,
        /// `/Alt` — only meaningful for `Figure`; `None` elsewhere.
        alt: Option<String>,
        /// `/A << {attrs} >>` — a raw table-attribute-class fragment,
        /// verbatim (only `TH` sets this, `/O /Table /Scope /Column`:
        /// every one of this crate's tables has its header as a single
        /// top row with one `TH` per column, so `/Scope /Column` always
        /// correctly associates that column's `TD`s with it — ISO
        /// 32000-1 14.8.5.7, needed once a table has more than a
        /// trivially simple structure a reader could infer on its own;
        /// found via an actual veraPDF PDF/UA run flagging "TD does not
        /// contain Headers attribute... cannot be determined
        /// algorithmically", not from the spec text alone).
        attrs: Option<&'static str>,
        children: Vec<PdfStructNode>,
    },
    /// A leaf reference to one marked-content sequence: `page_index` is
    /// resolved against `page_refs` at write time (same deferred-
    /// resolution pattern as `PdfOutlineNode::page_index`), `mcid` is
    /// that `BDC` sequence's own marked-content ID on that page.
    ContentRef { page_index: usize, mcid: u32 },
}

/// [`PdfStructNode::Elem`]'s shape mirrored with an allocated [`Ref`] per
/// element (not per `ContentRef` — those are inline `/MCR` dicts, never
/// indirect objects of their own).
struct StructRefTree {
    r: Ref,
    children: Vec<StructRefTree>,
}

fn alloc_struct_refs(w: &mut PdfWriter, node: &PdfStructNode) -> StructRefTree {
    let PdfStructNode::Elem { children, .. } = node else {
        unreachable!("alloc_struct_refs is only ever called on Elem nodes — see write_struct_elem's ContentRef branch, which never recurses into this")
    };
    StructRefTree {
        r: w.alloc(),
        children: children
            .iter()
            .filter(|c| matches!(c, PdfStructNode::Elem { .. }))
            .map(|c| alloc_struct_refs(w, c))
            .collect(),
    }
}

/// Records, for `(page_index, mcid)`, the direct-parent `StructElem`'s ref
/// — what `/ParentTree` is built from once the whole tree is written.
/// Indexed `[page_index][mcid]`; `None` for an mcid never emitted (page
/// has no/fewer tagged content items than that index).
type ParentTree = Vec<Vec<Option<Ref>>>;

fn record_parent(parent_tree: &mut ParentTree, page_index: usize, mcid: u32, elem_ref: Ref) {
    let mcid = mcid as usize;
    let row = &mut parent_tree[page_index];
    if row.len() <= mcid {
        row.resize(mcid + 1, None);
    }
    row[mcid] = Some(elem_ref);
}

fn write_struct_elem(
    w: &mut PdfWriter,
    node: &PdfStructNode,
    ref_tree: &StructRefTree,
    parent_ref: Ref,
    page_refs: &[Ref],
    parent_tree: &mut ParentTree,
) {
    let PdfStructNode::Elem { tag, alt, attrs, children } = node else {
        unreachable!("only ever called on Elem nodes")
    };
    let mut elem_children = ref_tree.children.iter();
    let k_entries: Vec<String> = children
        .iter()
        .map(|child| match child {
            PdfStructNode::Elem { .. } => {
                let child_ref_tree = elem_children
                    .next()
                    .expect("one StructRefTree per Elem child, allocated in the same order");
                write_struct_elem(w, child, child_ref_tree, ref_tree.r, page_refs, parent_tree);
                child_ref_tree.r.write()
            }
            PdfStructNode::ContentRef { page_index, mcid } => {
                let page_ref = page_refs[*page_index];
                record_parent(parent_tree, *page_index, *mcid, ref_tree.r);
                format!("<< /Type /MCR /Pg {} /MCID {} >>", page_ref.write(), mcid)
            }
        })
        .collect();
    let alt_entry = match alt {
        Some(a) => format!(" /Alt {}", format_pdf_string(a)),
        None => String::new(),
    };
    let attrs_entry = match attrs {
        Some(a) => format!(" /A << {a} >>"),
        None => String::new(),
    };
    w.object(
        ref_tree.r,
        &format!(
            "<< /Type /StructElem /S /{tag} /P {} /K [{}]{alt_entry}{attrs_entry} >>",
            parent_ref.write(),
            k_entries.join(" ")
        ),
    );
}

/// Writes the whole structure tree — every `/StructElem` plus
/// `/StructTreeRoot` and `/ParentTree` — and returns
/// `(struct_tree_root_ref, per_page_struct_parents)`, the latter one
/// `/StructParents` integer key per page (in `page_refs` order, always
/// `0..page_refs.len()`) for [`crate::doc::PdfDocument::write`] to put on
/// each `/Page` dict.
pub(crate) fn write_struct_tree(w: &mut PdfWriter, root: &PdfStructNode, page_refs: &[Ref]) -> (Ref, Vec<usize>) {
    let struct_tree_root_ref = w.alloc();
    let root_ref_tree = alloc_struct_refs(w, root);
    let mut parent_tree: ParentTree = vec![Vec::new(); page_refs.len()];
    write_struct_elem(w, root, &root_ref_tree, struct_tree_root_ref, page_refs, &mut parent_tree);

    let parent_tree_ref = w.alloc();
    let mut nums = Vec::with_capacity(parent_tree.len());
    for (page_index, row) in parent_tree.iter().enumerate() {
        let entries: Vec<String> = row
            .iter()
            .map(|r| r.map(|rr| rr.write()).unwrap_or_else(|| "null".to_string()))
            .collect();
        nums.push(format!("{page_index} [{}]", entries.join(" ")));
    }
    w.object(parent_tree_ref, &format!("<< /Nums [{}] >>", nums.join(" ")));

    w.object(
        struct_tree_root_ref,
        &format!(
            "<< /Type /StructTreeRoot /K [{}] /ParentTree {} /ParentTreeNextKey {} >>",
            root_ref_tree.r.write(),
            parent_tree_ref.write(),
            page_refs.len()
        ),
    );

    (struct_tree_root_ref, (0..page_refs.len()).collect())
}
