//! Walks a `RenderNode` subtree and emits the non-text content-stream
//! operations for it — rects (background/border fills), lines, and images.
//! `super::text` owns `RenderNode::TextLines`; `render_node` here just
//! dispatches to it for that variant.

use super::{pdf_rect_y, text, to_rgb, RenderCtx, RenderError};
use crate::images;
use lightweight_pdf_core::{Border, Color, ImageFormat};
use lightweight_pdf_layout::{Rect, RenderNode};
use lightweight_pdf_writer::PdfDocument;

/// Fills `area`'s background, if any — shared by `RenderNode::Group` (under
/// its children) and `RenderNode::Rect` (its whole body).
fn paint_background(area: &Rect, background: &Option<Color>, ctx: &mut RenderCtx) {
    if let Some(bg) = background {
        ctx.cb.fill_rect(
            area.x,
            pdf_rect_y(ctx.page_height, area.y, area.height),
            area.width,
            area.height,
            to_rgb(*bg),
        );
    }
}

/// Strokes `area`'s border, if any — shared by `RenderNode::Group` (over
/// its children) and `RenderNode::Rect` (its whole body).
fn paint_border(area: &Rect, border: &Option<Border>, ctx: &mut RenderCtx) {
    if let Some(b) = border {
        ctx.cb.stroke_rect(
            area.x,
            pdf_rect_y(ctx.page_height, area.y, area.height),
            area.width,
            area.height,
            b.width,
            to_rgb(b.color),
        );
    }
}

fn render_group(
    area: &Rect,
    clip: bool,
    background: &Option<Color>,
    border: &Option<Border>,
    children: &[RenderNode],
    ctx: &mut RenderCtx,
) -> Result<(), RenderError> {
    ctx.cb.save();
    if clip {
        ctx.cb
            .clip_rect(area.x, pdf_rect_y(ctx.page_height, area.y, area.height), area.width, area.height);
    }
    paint_background(area, background, ctx);
    for child in children {
        render_node(child, ctx)?;
    }
    paint_border(area, border, ctx);
    ctx.cb.restore();
    Ok(())
}

fn render_rect(area: &Rect, background: &Option<Color>, border: &Option<Border>, ctx: &mut RenderCtx) {
    paint_background(area, background, ctx);
    paint_border(area, border, ctx);
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
            children,
        } => render_group(area, *clip, background, border, children, ctx),
        RenderNode::Rect { area, background, border } => {
            render_rect(area, background, border, ctx);
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
            line_height_pt,
        } => {
            text::render_text_lines(area, style, lines, *line_height_pt, ctx);
            Ok(())
        }
        RenderNode::Image {
            area,
            bytes,
            format,
            width_px,
            height_px,
            components,
        } => render_image(area, bytes, *format, *width_px, *height_px, *components, ctx),
    }
}
