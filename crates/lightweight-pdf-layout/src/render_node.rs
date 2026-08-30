use crate::geometry::Rect;
use crate::text::RichLine;
use lightweight_pdf_core::{Align, Border, Color, ImageFormat, TextStyle};
use std::sync::Arc;

/// Positioned, resolved layout output ready for the facade to translate
/// into `lightweight-pdf-writer` content-stream operations. Never seen by
/// `lightweight-pdf-writer` directly (`plan/00a-contracts-and-artifacts.md` point 3).
#[derive(Clone, Debug)]
pub enum RenderNode {
    Empty,
    /// Already-wrapped lines, one per output line, top-aligned within `area`.
    TextLines {
        area: Rect,
        style: TextStyle,
        lines: Vec<String>,
        /// Same length as `lines`: `true` for the last line of its source
        /// paragraph. Only consulted when `style.align == Align::Justify`
        /// (that line stays left-aligned instead of being stretched); every
        /// other alignment ignores it.
        paragraph_end: Vec<bool>,
        line_height_pt: f32,
        url: Option<String>,
        anchor: Option<String>,
        link_to: Option<String>,
        /// `Text::outline_level` — the PDF bookmark tree is built from
        /// these, in document order, once pagination is final.
        outline_level: Option<u8>,
    },
    /// `Text::rich(..)` (issue #11) — the multi-style counterpart to
    /// `TextLines`. Deliberately a separate variant rather than a shape
    /// change to `TextLines`: every existing `TextLines` consumer (facade
    /// rendering, font-collection, outline/anchor collection) keeps
    /// working unmodified for plain `Text`, and this variant just isn't
    /// handled by any of the outline/anchor/link machinery yet (rich text
    /// doesn't support those in V1, see `Text::spans`' doc comment).
    RichTextLines {
        area: Rect,
        align: Align,
        lines: Vec<RichLine>,
    },
    Rect {
        area: Rect,
        background: Option<Color>,
        border: Option<Border>,
        corner_radius: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        thickness: f32,
        color: Color,
    },
    /// A validated JPEG/PNG placed at its final, Contain-fit size. `bytes`
    /// are the original file bytes (facade decides how to embed them —
    /// JPEG passes through as `DCTDecode`, PNG gets decoded once here to
    /// split out the alpha channel as a `SMask`).
    Image {
        area: Rect,
        bytes: Arc<[u8]>,
        format: ImageFormat,
        width_px: u32,
        height_px: u32,
        components: u8,
        /// `Image::alt` (issue #27) — carried through layout so the
        /// facade can both write `/Alt` and warn when it's missing.
        alt: Option<String>,
    },
    /// A container's own box: clipped (Grundprinzip 4), optionally painted
    /// with a background/border, holding its children.
    Group {
        area: Rect,
        clip: bool,
        background: Option<Color>,
        border: Option<Border>,
        corner_radius: f32,
        children: Vec<RenderNode>,
    },
    /// Wraps `inner` with a structure-tree role (issue #27) — a wrapper
    /// variant rather than a field on every other variant, so tagging
    /// support doesn't require touching every `RenderNode` construction
    /// site in this crate. Attached at `Element::layout`'s dispatch (see
    /// `layoutable::mod`) plus, for `Table`/`List`/`TableOfContents`,
    /// inside their own layout code where row/cell/item structure is
    /// known.
    Tagged {
        role: StructRole,
        inner: Box<RenderNode>,
    },
}

/// The PDF standard structure types (ISO 32000-1 14.8.4, Table 333/334)
/// this crate tags content with — no `RoleMap` needed since all of these
/// are standard types. `Grouping` roles (see `StructRole::is_grouping`)
/// nest child `StructElem`s and own no marked content of their own;
/// `Artifact` is excluded from the structure tree entirely (watermarks,
/// running headers/footers — PDF/UA pagination artifacts, not content).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StructRole {
    Document,
    /// `Text::outline_level` 1-6, clamped (`.outline_level(7+)` still
    /// produces valid output, just tagged as the deepest standard
    /// heading level rather than inventing a 7th).
    Heading(u8),
    Paragraph,
    Figure,
    Table,
    TableRow,
    TableHeaderCell,
    TableCell,
    List,
    ListItem,
    ListItemLabel,
    ListItemBody,
    Toc,
    TocItem,
    /// Not structure content at all — `BDC`/`EMC` with no MCID, entirely
    /// excluded from `/StructTreeRoot` (ISO 32000-1 14.8.2.2).
    Artifact,
}

impl StructRole {
    /// `true` for roles that only nest child `StructElem`s (no marked
    /// content of their own — see `RenderNode::Tagged`'s doc comment).
    pub fn is_grouping(self) -> bool {
        matches!(
            self,
            StructRole::Document
                | StructRole::Table
                | StructRole::TableRow
                | StructRole::TableHeaderCell
                | StructRole::TableCell
                | StructRole::List
                | StructRole::ListItem
                | StructRole::ListItemLabel
                | StructRole::ListItemBody
                | StructRole::Toc
        )
    }

    /// The PDF structure type name (ISO 32000-1 Table 333/334).
    pub fn tag_name(self) -> &'static str {
        match self {
            StructRole::Document => "Document",
            StructRole::Heading(n) => match n.clamp(1, 6) {
                1 => "H1",
                2 => "H2",
                3 => "H3",
                4 => "H4",
                5 => "H5",
                _ => "H6",
            },
            StructRole::Paragraph => "P",
            StructRole::Figure => "Figure",
            StructRole::Table => "Table",
            StructRole::TableRow => "TR",
            StructRole::TableHeaderCell => "TH",
            StructRole::TableCell => "TD",
            StructRole::List => "L",
            StructRole::ListItem => "LI",
            StructRole::ListItemLabel => "Lbl",
            StructRole::ListItemBody => "LBody",
            StructRole::Toc => "TOC",
            StructRole::TocItem => "TOCI",
            StructRole::Artifact => "Artifact",
        }
    }
}

impl RenderNode {
    /// Wraps `self` in a clipping group bound to `area` — the render-pass
    /// safety net required from every element, not only containers
    /// (Grundprinzip 4/6).
    pub fn clipped(area: Rect, inner: RenderNode) -> RenderNode {
        RenderNode::Group {
            area,
            clip: true,
            background: None,
            border: None,
            corner_radius: 0.0,
            children: vec![inner],
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            RenderNode::Empty => 0.0,
            RenderNode::TextLines { area, .. }
            | RenderNode::RichTextLines { area, .. }
            | RenderNode::Rect { area, .. }
            | RenderNode::Group { area, .. }
            | RenderNode::Image { area, .. } => area.height,
            RenderNode::Line { .. } => 0.0,
            RenderNode::Tagged { inner, .. } => inner.height(),
        }
    }

    /// Peels away `Tagged` wrappers — for tests that assert on the
    /// underlying `Group`/`TextLines`/etc. shape without caring about
    /// structure-tree tagging (issue #27) specifically.
    #[cfg(test)]
    pub fn untagged(&self) -> &RenderNode {
        match self {
            RenderNode::Tagged { inner, .. } => inner.untagged(),
            other => other,
        }
    }

    /// Wraps `self` with a structure-tree role (issue #27) — see
    /// `RenderNode::Tagged`'s doc comment.
    pub fn tagged(role: StructRole, inner: RenderNode) -> RenderNode {
        RenderNode::Tagged {
            role,
            inner: Box::new(inner),
        }
    }
}

/// Just used inside `Row`/`Column` cross-axis alignment.
pub fn align_offset(align: Align, available: f32, used: f32) -> f32 {
    match align {
        Align::Start | Align::Justify => 0.0,
        Align::Center => ((available - used) / 2.0).max(0.0),
        Align::End => (available - used).max(0.0),
    }
}
