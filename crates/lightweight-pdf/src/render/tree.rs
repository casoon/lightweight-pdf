//! Walks a `RenderNode` subtree and emits the non-text content-stream
//! operations for it — rects (background/border fills), lines, and images.
//! `super::text` owns `RenderNode::TextLines`; `render_node` here just
//! dispatches to it for that variant.

use super::{pdf_rect_y, text, to_rgb, RenderCtx, RenderError};
use crate::images;
use lightweight_pdf_core::{Border, BorderStyle, Color, ImageFormat};
#[cfg(feature = "tagged-pdf")]
use lightweight_pdf_layout::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_layout::{Rect, RenderNode, StructRole};
use lightweight_pdf_writer::PdfDocument;

/// Fills `area`'s background, if any — shared by `RenderNode::Group` (under
/// its children) and `RenderNode::Rect` (its whole body).
fn paint_background(area: &Rect, background: &Option<Color>, corner_radius: f32, ctx: &mut RenderCtx) {
    if let Some(bg) = background {
        let pdf_y = pdf_rect_y(ctx.page_height, area.y, area.height);
        if corner_radius > 0.0 {
            ctx.cb
                .draw_rounded_rect(area.x, pdf_y, area.width, area.height, corner_radius, Some(to_rgb(*bg)), None, None);
        } else {
            ctx.cb.fill_rect(area.x, pdf_y, area.width, area.height, to_rgb(*bg));
        }
    }
}

/// Strokes `area`'s border, if any — shared by `RenderNode::Group` (over
/// its children) and `RenderNode::Rect` (its whole body).
fn paint_border(area: &Rect, border: &Option<Border>, corner_radius: f32, ctx: &mut RenderCtx) {
    if let Some(b) = border {
        let pdf_y = pdf_rect_y(ctx.page_height, area.y, area.height);
        let dash_info = match b.style {
            BorderStyle::Solid => None,
            BorderStyle::Dashed { dash, gap } => Some((dash, gap)),
        };
        if corner_radius > 0.0 {
            ctx.cb.draw_rounded_rect(
                area.x,
                pdf_y,
                area.width,
                area.height,
                corner_radius,
                None,
                Some((b.width, to_rgb(b.color))),
                dash_info,
            );
        } else if let Some((dash, gap)) = dash_info {
            ctx.cb.set_dash(dash, gap);
            ctx.cb.stroke_rect(area.x, pdf_y, area.width, area.height, b.width, to_rgb(b.color));
            ctx.cb.reset_dash();
        } else {
            ctx.cb.stroke_rect(area.x, pdf_y, area.width, area.height, b.width, to_rgb(b.color));
        }
    }
}

fn render_group(
    area: &Rect,
    clip: bool,
    background: &Option<Color>,
    border: &Option<Border>,
    corner_radius: f32,
    children: &[RenderNode],
    ctx: &mut RenderCtx,
) -> Result<(), RenderError> {
    ctx.cb.save();
    if clip {
        ctx.cb
            .clip_rect(area.x, pdf_rect_y(ctx.page_height, area.y, area.height), area.width, area.height);
    }
    paint_background(area, background, corner_radius, ctx);
    for child in children {
        render_node(child, ctx)?;
    }
    paint_border(area, border, corner_radius, ctx);
    ctx.cb.restore();
    Ok(())
}

fn render_rect(area: &Rect, background: &Option<Color>, border: &Option<Border>, corner_radius: f32, ctx: &mut RenderCtx) {
    paint_background(area, background, corner_radius, ctx);
    paint_border(area, border, corner_radius, ctx);
}

fn render_line(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Color, ctx: &mut RenderCtx) {
    ctx.cb
        .line(x1, ctx.page_height - y1, x2, ctx.page_height - y2, thickness, to_rgb(color));
}

fn render_image(
    area: &Rect,
    bytes: &[u8],
    format: ImageFormat,
    width_px: u32,
    height_px: u32,
    components: u8,
    ctx: &mut RenderCtx,
) -> Result<(), RenderError> {
    let mut pdf_image = images::build_pdf_image(bytes, format, components)?;
    pdf_image.width_px = width_px;
    pdf_image.height_px = height_px;
    let index = ctx.pdf.add_image(pdf_image);
    let resource = PdfDocument::image_resource_name(index);
    ctx.cb.draw_image(
        &resource,
        area.x,
        pdf_rect_y(ctx.page_height, area.y, area.height),
        area.width,
        area.height,
    );
    Ok(())
}

pub(super) fn render_node(node: &RenderNode, ctx: &mut RenderCtx) -> Result<(), RenderError> {
    match node {
        RenderNode::Empty => Ok(()),
        RenderNode::Group {
            area,
            clip,
            background,
            border,
            corner_radius,
            children,
        } => render_group(area, *clip, background, border, *corner_radius, children, ctx),
        RenderNode::Rect {
            area,
            background,
            border,
            corner_radius,
        } => {
            render_rect(area, background, border, *corner_radius, ctx);
            Ok(())
        }
        RenderNode::Line {
            x1,
            y1,
            x2,
            y2,
            thickness,
            color,
        } => {
            render_line(*x1, *y1, *x2, *y2, *thickness, *color, ctx);
            Ok(())
        }
        RenderNode::TextLines {
            area,
            style,
            lines,
            paragraph_end,
            line_height_pt,
            url,
            link_to,
            anchor: _,
            outline_level: _,
        } => {
            let target = text::LinkTarget::from_text(url.as_deref(), link_to.as_deref());
            text::render_text_lines(area, style, lines, paragraph_end, *line_height_pt, target, ctx);
            Ok(())
        }
        RenderNode::RichTextLines { area, align, lines } => {
            text::render_rich_text_lines(area, *align, lines, ctx);
            Ok(())
        }
        RenderNode::Image {
            area,
            bytes,
            format,
            width_px,
            height_px,
            components,
            alt: _,
        } => render_image(area, bytes, *format, *width_px, *height_px, *components, ctx),
        RenderNode::Tagged { role, inner } => render_tagged(*role, inner, ctx),
    }
}

/// Handles a `RenderNode::Tagged` wrapper (issue #27) — transparent
/// (`inner` renders exactly as if unwrapped) when structure-tree building
/// isn't active; otherwise dispatches on `role`: `Artifact` gets `BDC
/// /Artifact`/`EMC` with no structure-tree involvement at all, every
/// other role opens a new `StructElem` (`enter`), renders `inner` (a
/// grouping role's own children recurse and fill it; a leaf role emits
/// exactly one marked-content span via `next_content_ref`), then closes
/// it (`exit`) — see `StructTreeBuilder`'s doc comment for why both
/// shapes share this same enter/exit call.
#[cfg_attr(not(feature = "tagged-pdf"), allow(unused_variables))]
fn render_tagged(role: StructRole, inner: &RenderNode, ctx: &mut RenderCtx) -> Result<(), RenderError> {
    #[cfg(feature = "tagged-pdf")]
    {
        if ctx.struct_tree.is_none() {
            return render_node(inner, ctx);
        }
        if role == StructRole::Artifact || ctx.force_artifact {
            ctx.cb.begin_artifact();
            render_node(inner, ctx)?;
            ctx.cb.end_marked_content();
            return Ok(());
        }
        ctx.struct_tree.as_deref_mut().expect("checked Some above").enter();
        if role.is_grouping() {
            render_node(inner, ctx)?;
        } else {
            let page_index = ctx.page_index;
            let mcid = ctx
                .struct_tree
                .as_deref_mut()
                .expect("checked Some above")
                .next_content_ref(page_index);
            ctx.cb.begin_marked_content(role.tag_name(), mcid);
            render_node(inner, ctx)?;
            ctx.cb.end_marked_content();
        }
        // PDF/UA-1 requires every Figure to have *some* /Alt (a real
        // veraPDF run confirms it: omitting the key entirely fails
        // "Figure structure element neither has an alternate description
        // nor a replacement text", not just a quality nit) — an empty
        // string satisfies that structural requirement while still
        // being honest that no real description was given; inventing
        // placeholder text would be actively misleading to a screen
        // reader user, so this crate doesn't.
        let alt = if role == StructRole::Figure {
            let found = super::struct_tree::find_image_alt(inner);
            if found.is_none() {
                ctx.warnings.push(LayoutWarning {
                    kind: LayoutWarningKind::MissingAltText,
                    page: ctx.page_index + 1,
                    element_hint: "Image without alt text".to_string(),
                });
            }
            Some(found.unwrap_or_default())
        } else {
            None
        };
        ctx.struct_tree
            .as_deref_mut()
            .expect("checked Some above")
            .exit(role.tag_name(), alt);
        Ok(())
    }
    #[cfg(not(feature = "tagged-pdf"))]
    render_node(inner, ctx)
}
