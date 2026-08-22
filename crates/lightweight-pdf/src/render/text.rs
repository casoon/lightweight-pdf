//! Font subsetting/embedding and text-line rendering — the `render`
//! module's font-facing half; `super::tree` handles non-text drawing
//! (rects/lines/images).

use super::{pdf_rect_y, to_rgb, RenderCtx, RenderError};
use crate::fonts::{FontRegistry, RegisteredFont};
use lightweight_pdf_core::{FontKey, TextStyle, Watermark};
use lightweight_pdf_fonts::{EmbeddedFontMetrics, FontSubset};
use lightweight_pdf_layout::{align_offset, PageRender, Rect, RenderNode};
use lightweight_pdf_writer::{CidFont, ContentBuilder, PdfDocument, TextRotation};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

pub(super) fn collect_chars_in_page(page: &PageRender, used: &mut HashMap<FontKey, BTreeSet<char>>) {
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
pub(super) struct EmbeddedFont {
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
            // `gid` is a `u16`; widening to `usize` is lossless.
            font.widths_1000_by_gid.get(usize::from(gid)).copied().unwrap_or(0.0)
        })
        .sum();
    sum_1000 / 1000.0 * size
}

/// Looks up the embedded font for `key` and its PDF resource name together
/// (`None` means that weight had nothing usable subset — callers treat that
/// as a silent no-op rather than an error).
fn font_resource(embedded: &HashMap<FontKey, EmbeddedFont>, key: FontKey) -> Option<(&EmbeddedFont, String)> {
    let font = embedded.get(&key)?;
    Some((font, PdfDocument::font_resource_name(font.index)))
}

pub(super) fn render_text_lines(area: &Rect, style: &TextStyle, lines: &[String], line_height_pt: f32, ctx: &mut RenderCtx) {
    let Some((font, resource)) = font_resource(ctx.embedded, style.font) else {
        return; // font had nothing usable subset (shouldn't happen for a font that produced text, defensive only)
    };
    let ascent_pt = font.ascent_1000 / 1000.0 * style.size;
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let line_top = area.y + i as f32 * line_height_pt;
        let baseline_pdf_y = ctx.page_height - (line_top + ascent_pt);
        let line_width = line_width_pt(font, style.size, line);
        let x = area.x + align_offset(style.align, area.width, line_width);
        let bytes = encode_cid(line, &font.char_to_gid);
        ctx.cb.text(&resource, style.size, x, baseline_pdf_y, to_rgb(style.color), &bytes);
    }
}

/// Draws the document-wide watermark centered on `body_area`, clipped to
/// it — never the header/footer bands (`plan/phases/phase-6-business-
/// polish.md` step 2's explicit requirement). A missing font entry (the
/// watermark's chars somehow weren't subset) is a silent no-op rather than
/// an error: a decorative stamp failing to draw must not fail the whole
/// render.
pub(super) fn draw_watermark(
    watermark: &Watermark,
    body_area: Rect,
    page_height: f32,
    embedded: &HashMap<FontKey, EmbeddedFont>,
    cb: &mut ContentBuilder,
) {
    let Some((font, resource)) = font_resource(embedded, watermark.font) else {
        return;
    };
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
        TextRotation {
            cx,
            cy: cy_pdf,
            angle_deg: watermark.rotation_deg,
            half_width,
        },
        to_rgb(watermark.color),
        &bytes,
    );
    cb.restore();
}

/// Builds the `CidFont` `pdf.add_font` needs from one subsetted weight —
/// factored out of `embed_fonts` so that function's own complexity reflects
/// the per-weight loop, not this struct's field count too.
fn cid_font(entry: &RegisteredFont, metrics: &EmbeddedFontMetrics, subset: FontSubset) -> CidFont {
    CidFont {
        base_font: entry.base_font_name.to_string(),
        to_unicode: subset.char_to_gid.iter().map(|(&ch, &gid)| (gid, ch)).collect(),
        subset_bytes: subset.font_data,
        widths: subset.widths_1000,
        ascent: metrics.ascent,
        descent: metrics.descent,
        cap_height: metrics.cap_height,
        italic_angle: metrics.italic_angle,
        bbox: metrics.bbox,
        is_italic: metrics.is_italic,
        is_bold: metrics.is_bold,
    }
}

/// Subsets and embeds each font weight actually used in the document
/// (`used_chars`), registering it with `pdf` and returning the lookup
/// `render_node`/`draw_watermark` need to translate `RenderNode::TextLines`
/// into content-stream text ops.
pub(super) fn embed_fonts(
    pdf: &mut PdfDocument,
    fonts: &FontRegistry,
    used_chars: &HashMap<FontKey, BTreeSet<char>>,
) -> Result<HashMap<FontKey, EmbeddedFont>, RenderError> {
    let mut embedded: HashMap<FontKey, EmbeddedFont> = HashMap::new();
    for (key, entry) in fonts.font_entries() {
        let Some(chars) = used_chars.get(&key) else {
            continue; // this weight was never referenced in the document
        };
        let subset = lightweight_pdf_fonts::subset_font(&entry.font_data, chars)?;
        let metrics = entry.metrics();
        let char_to_gid = subset.char_to_gid.clone();
        let widths_1000_by_gid = subset.widths_1000.clone();
        let index = pdf.add_font(cid_font(entry, metrics, subset));
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
    Ok(embedded)
}
