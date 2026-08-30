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
//! Split across three files (round-3 `cargo judge` maintainability-index
//! cleanup: splitting `render_node` into per-variant functions lowered each
//! function's own complexity but raised the *whole-file* score, since MI
//! sums LOC/cyclomatic across every function in one file regardless of
//! their individual size — the fix is smaller files, not smaller
//! functions). This file owns page/document-level orchestration and the
//! state (`RenderCtx`) both submodules share; `tree` walks the `RenderNode`
//! tree (rects/lines/images), `text` owns font subsetting/embedding and
//! text-line rendering.
//!
//! `render()`/`render_with_diagnostics()` need the bundled default fonts
//! (`default-fonts` feature) and are `#[cfg]`-gated on it accordingly;
//! `render_with_fonts()`/`render_with_fonts_and_diagnostics()` take an
//! already-built `FontRegistry` (e.g. `FontRegistry::with_fonts(...)`, see
//! `fonts.rs`) and work regardless of that feature — so without
//! `default-fonts`, this module's private render pipeline is still
//! reachable through those two, not dead code.

#[cfg(feature = "tagged-pdf")]
mod struct_tree;
mod text;
mod tree;

use crate::fonts::FontRegistry;
use crate::images::ImageEmbedError;
use lightweight_pdf_core::{Color, Document, FontKey, Watermark};
use lightweight_pdf_layout::{paginate, LayoutCtx, LayoutWarning, PageRender, Rect};
use lightweight_pdf_writer::{ContentBuilder, PdfDocument, PdfPage, Rgb};
use std::collections::{BTreeSet, HashMap};
use text::EmbeddedFont;

#[derive(Debug)]
pub enum RenderError {
    Font(lightweight_pdf_fonts::FontError),
    Image(ImageEmbedError),
    /// A `Text` used a `FontKey` (e.g. via `.font(key)`, or `.italic()`
    /// when no italic was registered) that `FontRegistry` has nothing
    /// registered under — a typed error instead of silently substituting
    /// the registry's default font.
    MissingFont(FontKey),
    /// `Document::pdf_a3b()` was set but this crate wasn't compiled with
    /// the `pdf-a` feature — a clear error instead of silently rendering
    /// a non-conformant PDF the caller believes is PDF/A-3b (issue #25).
    PdfAFeatureDisabled,
    /// `Document::zugferd_xml()` was set but this crate wasn't compiled
    /// with the `zugferd` feature (issue #26) — same reasoning as
    /// `PdfAFeatureDisabled`.
    ZugferdFeatureDisabled,
    /// `Document::pdf_ua()` was set but this crate wasn't compiled with
    /// the `tagged-pdf` feature (issue #27) — same reasoning as
    /// `PdfAFeatureDisabled`.
    TaggedPdfFeatureDisabled,
    /// `render_with_fonts`/`render_with_fonts_and_diagnostics` were called
    /// with a `FontRegistry` nothing was ever registered on — layout has
    /// no error channel of its own (every width/wrap lookup just falls
    /// back to whatever `FontRegistry::entry` finds), so this is checked
    /// up front instead of surfacing as a panic once layout starts.
    NoFontsRegistered,
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RenderError::Font(e) => write!(f, "font error: {e}"),
            RenderError::Image(e) => write!(f, "image error: {e}"),
            RenderError::MissingFont(key) => write!(f, "no font registered for key {key:?}"),
            RenderError::PdfAFeatureDisabled => {
                write!(
                    f,
                    "Document::pdf_a3b() was set but this crate wasn't built with the `pdf-a` feature"
                )
            }
            RenderError::ZugferdFeatureDisabled => {
                write!(
                    f,
                    "Document::zugferd_xml() was set but this crate wasn't built with the `zugferd` feature"
                )
            }
            RenderError::TaggedPdfFeatureDisabled => {
                write!(
                    f,
                    "Document::pdf_ua() was set but this crate wasn't built with the `tagged-pdf` feature"
                )
            }
            RenderError::NoFontsRegistered => {
                write!(
                    f,
                    "FontRegistry has no fonts registered — call register()/register_named() (or with_defaults()/with_fonts()) first"
                )
            }
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

/// `page_height - top_left_y - height` — converts a layout-space box's
/// top-left/height into the bottom-left `y` PDF rectangles expect.
fn pdf_rect_y(page_height: f32, y_top: f32, height: f32) -> f32 {
    page_height - y_top - height
}

/// Shared state every `render_*` helper in `tree`/`text` needs: the
/// page-space-to-PDF-space conversion input, the fonts embedded for this
/// page, the resolved `Text::anchor` targets for internal links, and the
/// two sinks (`pdf` for images/font resources, `cb` for content-stream
/// ops) content actually gets written to. Bundled into one `&mut` so
/// per-variant helpers stay under clippy's argument-count limit without
/// losing any of them.
struct RenderCtx<'a> {
    page_height: f32,
    embedded: &'a HashMap<FontKey, EmbeddedFont>,
    anchors: &'a HashMap<String, (usize, f32)>,
    pdf: &'a mut PdfDocument,
    cb: &'a mut ContentBuilder,
    annotations: &'a mut Vec<lightweight_pdf_writer::PdfLinkAnnotation>,
    /// This page's 0-based index — a `ContentRef`'s `/Pg` (issue #27).
    #[cfg(feature = "tagged-pdf")]
    page_index: usize,
    /// `None` when `tagged-pdf` isn't compiled in, or when it is but
    /// `Document::pdf_ua()` wasn't called — `tree::render_node`'s
    /// `RenderNode::Tagged` handling checks this to decide between
    /// emitting `BDC`/`EMC`+building structure or rendering `inner`
    /// completely transparently.
    #[cfg(feature = "tagged-pdf")]
    struct_tree: Option<&'a mut struct_tree::StructTreeBuilder>,
    #[cfg(feature = "tagged-pdf")]
    warnings: &'a mut Vec<LayoutWarning>,
    /// Set for the duration of header/footer rendering (issue #27):
    /// makes `tree::render_tagged` treat *every* descendant as an
    /// artifact regardless of its own role, overriding whatever tag the
    /// header/footer closure's own content (built from ordinary
    /// `Element`s, individually tagged like any other content) would
    /// otherwise get — running headers/footers are pagination decoration
    /// project-wide, never real structure. Always present (not cfg-gated
    /// on `tagged-pdf`): a plain `bool`, and `render_tagged` already
    /// no-ops entirely when `struct_tree` is `None`.
    force_artifact: bool,
}

/// The two whole-document, read-only lookups every page's render pass
/// needs — bundled into one parameter (alongside `pdf`, which stays
/// separate since it's `&mut`) so `render_page` doesn't grow past
/// clippy's argument-count limit.
struct DocumentLookups<'a> {
    embedded: &'a HashMap<FontKey, EmbeddedFont>,
    anchors: &'a HashMap<String, (usize, f32)>,
}

/// Renders one page's header/watermark/body/footer into a fresh content
/// stream and returns the finished `PdfPage`.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(feature = "tagged-pdf"), allow(unused_variables))]
fn render_page(
    page: &PageRender,
    watermark: Option<&Watermark>,
    body_area: Rect,
    page_width: f32,
    page_height: f32,
    page_index: usize,
    lookups: &DocumentLookups,
    pdf: &mut PdfDocument,
    #[cfg(feature = "tagged-pdf")] mut struct_tree: Option<&mut struct_tree::StructTreeBuilder>,
    #[cfg(feature = "tagged-pdf")] warnings: &mut Vec<LayoutWarning>,
) -> Result<PdfPage, RenderError> {
    let mut cb = ContentBuilder::new();
    let mut annotations = Vec::new();
    cb.save();
    cb.clip_rect(0.0, 0.0, page_width, page_height);
    #[cfg(feature = "tagged-pdf")]
    if let Some(st) = struct_tree.as_deref_mut() {
        st.start_page();
    }
    // Watermark first (bottom layer, `05-overflow-and-robustness.md`):
    // normal content always draws on top of it afterwards, which is
    // what guarantees it never makes text unreadable. Marked as an
    // artifact (issue #27) when tagging is active — it's pagination
    // decoration, not real content, and must never enter reading order.
    if let Some(watermark) = watermark {
        #[cfg(feature = "tagged-pdf")]
        let is_tagged = struct_tree.is_some();
        #[cfg(not(feature = "tagged-pdf"))]
        let is_tagged = false;
        if is_tagged {
            cb.begin_artifact();
        }
        text::draw_watermark(watermark, body_area, page_height, lookups.embedded, &mut cb);
        if is_tagged {
            cb.end_marked_content();
        }
    }
    let mut ctx = RenderCtx {
        page_height,
        embedded: lookups.embedded,
        anchors: lookups.anchors,
        pdf,
        cb: &mut cb,
        annotations: &mut annotations,
        #[cfg(feature = "tagged-pdf")]
        page_index,
        #[cfg(feature = "tagged-pdf")]
        struct_tree,
        #[cfg(feature = "tagged-pdf")]
        warnings,
        force_artifact: false,
    };
    if let Some(header) = &page.header {
        ctx.force_artifact = true;
        tree::render_node(header, &mut ctx)?;
        ctx.force_artifact = false;
    }
    tree::render_node(&page.body, &mut ctx)?;
    if let Some(footer) = &page.footer {
        ctx.force_artifact = true;
        tree::render_node(footer, &mut ctx)?;
    }
    cb.restore();
    Ok(PdfPage {
        width: page_width,
        height: page_height,
        content: cb.into_bytes(),
        annotations,
    })
}

fn render_document(doc: &Document, fonts: &FontRegistry) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError> {
    #[cfg(not(feature = "pdf-a"))]
    if doc.pdf_a3b {
        return Err(RenderError::PdfAFeatureDisabled);
    }
    #[cfg(not(feature = "zugferd"))]
    if doc.zugferd_xml.is_some() {
        return Err(RenderError::ZugferdFeatureDisabled);
    }
    #[cfg(not(feature = "tagged-pdf"))]
    if doc.pdf_ua {
        return Err(RenderError::TaggedPdfFeatureDisabled);
    }
    if fonts.is_empty() {
        return Err(RenderError::NoFontsRegistered);
    }

    let ctx = LayoutCtx::new(fonts);
    #[cfg_attr(not(feature = "tagged-pdf"), allow(unused_mut))]
    let mut paginated = paginate(doc, &ctx);

    let mut used_chars: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    for page in &paginated.pages {
        text::collect_chars_in_page(page, &mut used_chars);
    }
    if let Some(watermark) = &doc.watermark {
        used_chars.entry(watermark.font).or_default().extend(watermark.text.chars());
    }

    // `Text::anchor` targets need their final page/position, which only
    // exists once the whole document is paginated — resolved here, once,
    // before the per-page render loop below (which needs it to turn
    // `Text::link_to` into a `/Dest`).
    let mut anchors: HashMap<String, (usize, f32)> = HashMap::new();
    for (page_index, page) in paginated.pages.iter().enumerate() {
        text::collect_anchors_in_page(page, page_index, paginated.page_height, &mut anchors);
    }

    let mut pdf = PdfDocument::new();
    pdf.outline = text::build_outline(&paginated.pages, paginated.page_height);
    pdf.metadata.title = doc.metadata.title.clone();
    pdf.metadata.author = doc.metadata.author.clone();
    pdf.metadata.subject = doc.metadata.subject.clone();
    pdf.metadata.keywords = doc.metadata.keywords.clone();
    pdf.metadata.creator = doc.metadata.creator.clone();
    pdf.metadata.creation_date = doc.metadata.creation_date.map(|d| d.to_pdf_string());
    pdf.metadata.mod_date = doc.metadata.mod_date.map(|d| d.to_pdf_string());
    #[cfg(feature = "pdf-a")]
    {
        pdf.metadata.xmp_creation_date = doc.metadata.creation_date.map(|d| d.to_xmp_string());
        pdf.metadata.xmp_mod_date = doc.metadata.mod_date.map(|d| d.to_xmp_string());
        pdf.pdf_a3b = doc.pdf_a3b;
    }
    #[cfg(feature = "zugferd")]
    {
        pdf.zugferd_xml = doc.zugferd_xml.clone();
    }
    pdf.lang = doc.lang.clone();
    #[cfg(feature = "tagged-pdf")]
    {
        pdf.pdf_ua = doc.pdf_ua;
    }

    let embedded = text::embed_fonts(&mut pdf, fonts, &used_chars)?;
    let lookups = DocumentLookups {
        embedded: &embedded,
        anchors: &anchors,
    };

    #[cfg(feature = "tagged-pdf")]
    let mut struct_builder = doc.pdf_ua.then(struct_tree::StructTreeBuilder::new);

    for (page_index, page) in paginated.pages.iter().enumerate() {
        let pdf_page = render_page(
            page,
            doc.watermark.as_ref(),
            paginated.body_area,
            paginated.page_width,
            paginated.page_height,
            page_index,
            &lookups,
            &mut pdf,
            #[cfg(feature = "tagged-pdf")]
            struct_builder.as_mut(),
            #[cfg(feature = "tagged-pdf")]
            &mut paginated.warnings,
        )?;
        pdf.add_page(pdf_page);
    }

    #[cfg(feature = "tagged-pdf")]
    if let Some(builder) = struct_builder {
        pdf.struct_tree = Some(builder.finish());
    }

    Ok((pdf.write(), paginated.warnings))
}

/// Extension trait adding `render()`/`render_with_diagnostics()` (bundled
/// default fonts) and `render_with_fonts()`/`render_with_fonts_and_diagnostics()`
/// (caller-supplied fonts, see `fonts.rs::FontRegistry::with_fonts()`) to
/// `lightweight_pdf_core::Document`. Lives here (not in `lightweight-pdf-core`)
/// because rendering needs layout, fonts and the PDF writer —
/// `lightweight-pdf-core` must not depend on any of them (ADR-002). This is
/// the point at which `render()` becomes public (ADR-002).
pub trait DocumentExt {
    // Without `default-fonts` there is no bundled font source, so these two
    // are simply not part of the trait rather than failing at runtime.
    #[cfg(feature = "default-fonts")]
    fn render(&self) -> Result<Vec<u8>, RenderError>;
    #[cfg(feature = "default-fonts")]
    fn render_with_diagnostics(&self) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError>;

    fn render_with_fonts(&self, fonts: &FontRegistry) -> Result<Vec<u8>, RenderError>;
    fn render_with_fonts_and_diagnostics(&self, fonts: &FontRegistry) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError>;
}

impl DocumentExt for Document {
    #[cfg(feature = "default-fonts")]
    fn render(&self) -> Result<Vec<u8>, RenderError> {
        let (bytes, _warnings) = self.render_with_diagnostics()?;
        Ok(bytes)
    }

    #[cfg(feature = "default-fonts")]
    fn render_with_diagnostics(&self) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError> {
        let fonts = FontRegistry::with_defaults()?;
        render_document(self, &fonts)
    }

    fn render_with_fonts(&self, fonts: &FontRegistry) -> Result<Vec<u8>, RenderError> {
        let (bytes, _warnings) = self.render_with_fonts_and_diagnostics(fonts)?;
        Ok(bytes)
    }

    fn render_with_fonts_and_diagnostics(&self, fonts: &FontRegistry) -> Result<(Vec<u8>, Vec<LayoutWarning>), RenderError> {
        render_document(self, fonts)
    }
}
