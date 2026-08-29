use crate::geometry::Rect;
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
            | RenderNode::Rect { area, .. }
            | RenderNode::Group { area, .. }
            | RenderNode::Image { area, .. } => area.height,
            RenderNode::Line { .. } => 0.0,
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
