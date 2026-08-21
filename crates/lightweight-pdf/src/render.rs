//! Translates `lightweight-pdf-layout`'s `RenderNode` tree into `lightweight-pdf-writer`
//! content-stream operations — the facade is where layout output meets the
//! PDF writer (`plan/00a-contracts-and-artifacts.md` point 3: `lightweight-pdf-writer`
//! never sees `RenderNode` itself). Also converts the internal top-down
//! layout coordinate system to PDF's bottom-left origin, and (Phase 4)
//! drives font subsetting: layout first (needs advances for whatever
//! Unicode text the document contains), then walk the finished render tree
//! to learn exactly which characters were used, then subset each font
//! down to just those glyphs before writing the PDF.
//!
//! V1 has no custom-font API yet (see `fonts.rs`): `default-fonts` is
//! currently the only font source, so `DocumentExt` below is `#[cfg]`-gated
//! on it and everything in this module is unreachable without it — building
//! `--no-default-features` is not a functional configuration for calling
//! `.render()` in V1 (only useful for consumers who want the `Document`
//! builder API without pulling in font bytes). `allow(dead_code)` in that
//! case is therefore intentional, not a suppressed bug.

#![cfg_attr(not(feature = "default-fonts"), allow(dead_code))]

use crate::fonts::FontRegistry;
use crate::images::{self, ImageEmbedError};
use lightweight_pdf_core::{Align, Color, Document, FontKey, Watermark};
use lightweight_pdf_layout::{paginate, LayoutCtx, LayoutWarning, PageRender, Rect, RenderNode};
use lightweight_pdf_writer::{CidFont, ContentBuilder, PdfDocument, PdfPage, Rgb};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug)]
pub enum RenderError {
    Font(lightweight_pdf_fonts::FontError),
    Image(ImageEmbedError),
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RenderError::Font(e) => write!(f, "font error: {e}"),
            RenderError::Image(e) => write!(f, "image error: {e}"),
        }
    }
}

impl From<lightweight_pdf_fonts::FontError> for RenderError {
    fn from(e: lightweight_pdf_fonts::FontError) -> Self {
        RenderError::Font(e)
    }
}

impl From<ImageEmbedError> for RenderError {
    fn from(e: ImageEmbedError) -> Self {
        RenderError::Image(e)
    }
}

fn to_rgb(c: Color) -> Rgb {
    Rgb(c.0, c.1, c.2)
}

fn align_offset(align: Align, available: f32, used: f32) -> f32 {
    match align {
        Align::Start => 0.0,
        Align::Center => ((available - used) / 2.0).max(0.0),
        Align::End => (available - used).max(0.0),
    }
}

/// `page_height - top_left_y - height` — converts a layout-space box's
/// top-left/height into the bottom-left `y` PDF rectangles expect.
fn pdf_rect_y(page_height: f32, y_top: f32, height: f32) -> f32 {
    page_height - y_top - height
}

fn collect_chars_in_node(node: &RenderNode, used: &mut HashMap<FontKey, BTreeSet<char>>) {
    match node {
        RenderNode::Empty | RenderNode::Rect { .. } | RenderNode::Line { .. } | RenderNode::Image { .. } => {}
        RenderNode::Group { children, .. } => {
            for child in children {
                collect_chars_in_node(child, used);
            }
        }
        RenderNode::TextLines { style, lines, .. } => {
            let set = used.entry(style.font).or_default();
            for line in lines {
                set.extend(line.chars());
            }
        }
    }
}

fn collect_chars_in_page(page: &PageRender, used: &mut HashMap<FontKey, BTreeSet<char>>) {
    if let Some(header) = &page.header {
        collect_chars_in_node(header, used);
    }
    collect_chars_in_node(&page.body, used);
    if let Some(footer) = &page.footer {
        collect_chars_in_node(footer, used);
    }
}

/// A font actually embedded in the output: its PDF resource/font index,
/// the character-to-CID mapping content streams encode text with (CID ==
/// the subset's own glyph ID, `CIDToGIDMap /Identity`), and the per-GID
/// widths/ascent needed to position text — taken from the *subset* itself
/// so positioning always matches exactly what got embedded.
struct EmbeddedFont {
    index: usize,
    char_to_gid: BTreeMap<char, u16>,
    ascent_1000: f32,
    widths_1000_by_gid: Vec<f32>,
}

fn encode_cid(line: &str, char_to_gid: &BTreeMap<char, u16>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(line.chars().count() * 2);
    for ch in line.chars() {
        let gid = char_to_gid.get(&ch).copied().unwrap_or(0); // .notdef fallback
        bytes.extend_from_slice(&gid.to_be_bytes());
    }
    bytes
}

fn line_width_pt(font: &EmbeddedFont, size: f32, line: &str) -> f32 {
    let sum_1000: f32 = line
        .chars()
        .map(|ch| {
            let gid = font.char_to_gid.get(&ch).copied().unwrap_or(0);
            font.widths_1000_by_gid.get(gid as usize).copied().unwrap_or(0.0)
        })
        .sum();
    sum_1000 / 1000.0 * size
}

fn render_node(
    node: &RenderNode,
    page_height: f32,
    embedded: &HashMap<FontKey, EmbeddedFont>,
    pdf: &mut PdfDocument,
    cb: &mut ContentBuilder,
) -> Result<(), RenderError> {
    match node {
        RenderNode::Empty => {}
        RenderNode::Group {
            area,
            clip,
            background,
            border,
            children,
        } => {
            cb.save();
            if *clip {
                cb.clip_rect(area.x, pdf_rect_y(page_height, area.y, area.height), area.width, area.height);
            }
            if let Some(bg) = background {
                cb.fill_rect(
                    area.x,
                    pdf_rect_y(page_height, area.y, area.height),
                    area.width,
                    area.height,
                    to_rgb(*bg),
                );
            }
            for child in children {
                render_node(child, page_height, embedded, pdf, cb)?;
            }
            if let Some(b) = border {
                cb.stroke_rect(
                    area.x,
                    pdf_rect_y(page_height, area.y, area.height),
                    area.width,
                    area.height,
                    b.width,
                    to_rgb(b.color),
                );
            }
            cb.restore();
        }
        RenderNode::Rect { area, background, border } => {
            if let Some(bg) = background {
                cb.fill_rect(
                    area.x,
                    pdf_rect_y(page_height, area.y, area.height),
                    area.width,
                    area.height,
                    to_rgb(*bg),
                );
            }
            if let Some(b) = border {
                cb.stroke_rect(
                    area.x,
                    pdf_rect_y(page_height, area.y, area.height),
                    area.width,
                    area.height,
                    b.width,
                    to_rgb(b.color),
                );
            }
        }
        RenderNode::Line {
            x1,
            y1,
            x2,
            y2,
            thickness,
            color,
        } => {
            cb.line(*x1, page_height - *y1, *x2, page_height - *y2, *thickness, to_rgb(*color));
        }
        RenderNode::TextLines {
            area,
            style,
            lines,
            line_height_pt,
        } => {
            let Some(font) = embedded.get(&style.font) else {
                return Ok(()); // font had nothing usable subset (shouldn't happen for a font that produced text, defensive only)
            };
            let resource = PdfDocument::font_resource_name(font.index);
            let ascent_pt = font.ascent_1000 / 1000.0 * style.size;
            for (i, line) in lines.iter().enumerate() {
                if line.is_empty() {
                    continue;
                }
                let line_top = area.y + i as f32 * line_height_pt;
                let baseline_pdf_y = page_height - (line_top + ascent_pt);
                let line_width = line_width_pt(font, style.size, line);
                let x = area.x + align_offset(style.align, area.width, line_width);
                let bytes = encode_cid(line, &font.char_to_gid);
                cb.text(&resource, style.size, x, baseline_pdf_y, to_rgb(style.color), &bytes);
            }
        }
        RenderNode::Image {
            area,
            bytes,
            format,
            width_px,
            height_px,
            components,
        } => {
            let mut pdf_image = images::build_pdf_image(bytes, *format, *components)?;
            pdf_image.width_px = *width_px;
            pdf_image.height_px = *height_px;
            let index = pdf.add_image(pdf_image);
            let resource = PdfDocument::image_resource_name(index);
            cb.draw_image(
                &resource,
                area.x,
                pdf_rect_y(page_height, area.y, area.height),
                area.width,
                area.height,
            );
        }
    }
    Ok(())
}

/// Draws the document-wide watermark centered on `body_area`, clipped to
/// it — never the header/footer bands (`plan/phases/phase-6-business-
/// polish.md` step 2's explicit requirement). A missing font entry (the
/// watermark's chars somehow weren't subset) is a silent no-op rather than
/// an error: a decorative stamp failing to draw must not fail the whole
/// render.
fn draw_watermark(
    watermark: &Watermark,
    body_area: Rect,
    page_height: f32,
    embedded: &HashMap<FontKey, EmbeddedFont>,
    cb: &mut ContentBuilder,
) {
    let Some(font) = embedded.get(&watermark.font) else {
        return;
    };
    let resource = PdfDocument::font_resource_name(font.index);
    let bytes = encode_cid(&watermark.text, &font.char_to_gid);
    let half_width = line_width_pt(font, watermark.size, &watermark.text) / 2.0;
    let cx = body_area.x + body_area.width / 2.0;
    let cy_layout = body_area.y + body_area.height / 2.0;
    let cy_pdf = page_height - cy_layout;

    cb.save();
    cb.clip_rect(
        body_area.x,
        pdf_rect_y(page_height, body_area.y, body_area.height),
        body_area.width,
        body_area.height,
    );
    cb.text_rotated(
        &resource,
        watermark.size,
        cx,
        cy_pdf,
        watermark.rotation_deg,
        half_width,
        to_rgb(watermark.color),
        &bytes,
    );
    cb.restore();
}

fn render_document(doc: &Document, fonts: &FontRegistry) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError> {
    let ctx = LayoutCtx { resolver: fonts };
    let paginated = paginate(doc, &ctx);

    let mut used_chars: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    for page in &paginated.pages {
        collect_chars_in_page(page, &mut used_chars);
    }
    if let Some(watermark) = &doc.watermark {
        used_chars.entry(watermark.font).or_default().extend(watermark.text.chars());
    }

    let mut pdf = PdfDocument::new();
    let mut embedded: HashMap<FontKey, EmbeddedFont> = HashMap::new();
    for (key, entry) in fonts.font_entries() {
        let Some(chars) = used_chars.get(&key) else {
            continue; // this weight was never referenced in the document
        };
        let subset = lightweight_pdf_fonts::subset_font(&entry.data, chars)?;
        let metrics = entry.metrics();
        let char_to_gid = subset.char_to_gid.clone();
        let widths_1000_by_gid = subset.widths_1000.clone();
        let index = pdf.add_font(CidFont {
            base_font: entry.base_font_name.to_string(),
            subset_bytes: subset.font_data,
            widths: subset.widths_1000,
            ascent: metrics.ascent,
            descent: metrics.descent,
            cap_height: metrics.cap_height,
            italic_angle: metrics.italic_angle,
            bbox: metrics.bbox,
            is_italic: metrics.is_italic,
            is_bold: metrics.is_bold,
            to_unicode: subset.char_to_gid.iter().map(|(&ch, &gid)| (gid, ch)).collect(),
        });
        embedded.insert(
            key,
            EmbeddedFont {
                index,
                char_to_gid,
                ascent_1000: metrics.ascent,
                widths_1000_by_gid,
            },
        );
    }

    for page in &paginated.pages {
        let mut cb = ContentBuilder::new();
        cb.save();
        cb.clip_rect(0.0, 0.0, paginated.page_width, paginated.page_height);
        // Watermark first (bottom layer, `05-overflow-and-robustness.md`):
        // normal content always draws on top of it afterwards, which is
        // what guarantees it never makes text unreadable.
        if let Some(watermark) = &doc.watermark {
            draw_watermark(watermark, paginated.body_area, paginated.page_height, &embedded, &mut cb);
        }
        if let Some(header) = &page.header {
            render_node(header, paginated.page_height, &embedded, &mut pdf, &mut cb)?;
        }
        render_node(&page.body, paginated.page_height, &embedded, &mut pdf, &mut cb)?;
        if let Some(footer) = &page.footer {
            render_node(footer, paginated.page_height, &embedded, &mut pdf, &mut cb)?;
        }
        cb.restore();
        pdf.add_page(PdfPage {
            width: paginated.page_width,
            height: paginated.page_height,
            content: cb.into_bytes(),
        });
    }

    Ok((pdf.write(), paginated.warnings))
}

/// Extension trait adding `render()`/`render_with_diagnostics()` to
/// `lightweight_pdf_core::Document`. Lives here (not in `lightweight-pdf-core`) because
/// rendering needs layout, fonts and the PDF writer — `lightweight-pdf-core` must
/// not depend on any of them (ADR-002). This is the point at which
/// `render()` becomes public (ADR-002).
pub trait DocumentExt {
    fn render(&self) -> Result<Vec<u8>, RenderError>;
    fn render_with_diagnostics(&self) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError>;
}

// Custom (non-default) font sources are a later follow-up (a general
// registry beyond the two bundled weights). Without `default-fonts` there
// is currently no font source at all, so the extension methods are simply
// not available rather than failing at runtime.
#[cfg(feature = "default-fonts")]
impl DocumentExt for Document {
    fn render(&self) -> Result<Vec<u8>, RenderError> {
        let (bytes, _warnings) = self.render_with_diagnostics()?;
        Ok(bytes)
    }

    fn render_with_diagnostics(&self) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError> {
        let fonts = FontRegistry::with_defaults()?;
        render_document(self, &fonts)
    }
}
