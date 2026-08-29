use crate::writer::{fmt_num, PdfWriter, Ref};

/// `/Producer` is always set (unlike the other `/Info` fields, which are
/// opt-in) — it identifies the generator, not the document, so there's no
/// caller-supplied value to opt out of.
const PRODUCER: &str = concat!("lightweight-pdf ", env!("CARGO_PKG_VERSION"));

/// A subset, embedded TrueType font written as `/Subtype /Type0` with a
/// `/CIDFontType2` descendant (ADR-012: Identity-H, `CIDToGIDMap`,
/// `ToUnicode`). CID space equals the subset's own glyph-index space (the
/// facade assigns CIDs that way), so `CIDToGIDMap` is always `/Identity`
/// and no separate CID-to-GID stream is needed.
pub struct CidFont {
    pub base_font: String,
    /// Already-subset sfnt bytes (`lightweight-pdf-fonts::subset_font`).
    pub subset_bytes: Vec<u8>,
    /// Advance width per CID, `widths[cid]` — CIDs `0..widths.len()` are
    /// assumed consecutive (true by construction: CID == subset GID).
    pub widths: Vec<f32>,
    pub ascent: f32,
    pub descent: f32,
    pub cap_height: f32,
    pub italic_angle: f32,
    pub bbox: (f32, f32, f32, f32),
    pub is_italic: bool,
    pub is_bold: bool,
    /// `(CID, Unicode scalar)` pairs for the `ToUnicode` CMap — what makes
    /// the text copyable/searchable despite going through Identity-H CIDs.
    pub to_unicode: Vec<(u16, char)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorSpace {
    DeviceGray,
    DeviceRgb,
}

impl ColorSpace {
    fn as_pdf_name(self) -> &'static str {
        match self {
            ColorSpace::DeviceGray => "DeviceGray",
            ColorSpace::DeviceRgb => "DeviceRGB",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageDataFilter {
    /// Raw samples — Flate-compressed like any other stream by default
    /// (ADR-016), unlike `DctDecode` below.
    None,
    /// The original JPEG bytes, embedded byte-for-byte (`phases/
    /// phase-5-images.md` step 2: "kein Neukodieren").
    DctDecode,
}

/// One embeddable `/Subtype /Image` XObject (ISO 32000-1 8.9.5), named for
/// that specific PDF construct the same way `CidFont` is named for its
/// (`Type0`/`CIDFontType2`) construct, rather than for the generic Pdf-noun
/// pattern used by [`PdfPage`]/[`PdfDocument`]/[`PdfWriter`] (those name the
/// crate's document-structure types; this and `CidFont` name embeddable
/// resource types). `smask`, if present, is itself an `ImageXObject`
/// (always `DeviceGray`, `filter: None`, no further `smask`) — PNG alpha,
/// per ADR-013.
pub struct ImageXObject {
    pub width_px: u32,
    pub height_px: u32,
    pub color_space: ColorSpace,
    pub bits_per_component: u8,
    pub filter: ImageDataFilter,
    pub bytes: Vec<u8>,
    pub smask: Option<Box<ImageXObject>>,
}

#[derive(Clone, Debug)]
pub enum PdfLinkAction {
    /// `/A << /S /URI /URI (...) >>` — an external link.
    Uri(String),
    /// `/Dest [pageRef /XYZ null y null]` — an internal jump target.
    /// `page_index` is resolved against the writer's own `page_refs`
    /// (built before any page is written, so a forward reference to a
    /// later page is fine) at write time, not by the caller.
    GoTo { page_index: usize, y: f32 },
}

#[derive(Clone, Debug)]
pub struct PdfLinkAnnotation {
    pub rect: (f32, f32, f32, f32),
    pub action: PdfLinkAction,
}

#[derive(Default)]
pub struct PdfPage {
    pub width: f32,
    pub height: f32,
    pub content: Vec<u8>,
    pub annotations: Vec<PdfLinkAnnotation>,
}

/// One entry in the `/Outlines` bookmark tree (`Text::outline_level`,
/// resolved). `page_index`/`y` mean the same thing as `PdfLinkAction::GoTo`
/// — resolved against `page_refs` at write time, not by the caller.
#[derive(Clone, Debug)]
pub struct PdfOutlineNode {
    pub title: String,
    pub page_index: usize,
    pub y: f32,
    pub children: Vec<PdfOutlineNode>,
}

#[derive(Clone, Debug, Default)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    /// Already-formatted PDF date strings (`D:YYYYMMDDHHmmSSZ`) — this
    /// crate has no date logic of its own, the facade formats
    /// `lightweight_pdf_core::PdfDate` before handing it over.
    pub creation_date: Option<String>,
    pub mod_date: Option<String>,
}

#[derive(Default)]
pub struct PdfDocument {
    fonts: Vec<CidFont>,
    images: Vec<ImageXObject>,
    pages: Vec<PdfPage>,
    pub metadata: PdfMetadata,
    /// Top-level bookmark entries; empty means no `/Outlines` object at
    /// all (not an empty one — a reader shouldn't see a bookmark panel
    /// with nothing in it for a document with no headings).
    pub outline: Vec<PdfOutlineNode>,
}

impl PdfDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a font, returning its index (used to build the resource
    /// name `F{index + 1}` referenced from content streams via
    /// [`Self::font_resource_name`]).
    pub fn add_font(&mut self, font: CidFont) -> usize {
        self.fonts.push(font);
        self.fonts.len() - 1
    }

    pub fn font_resource_name(index: usize) -> String {
        format!("F{}", index + 1)
    }

    /// Registers an image, returning its index (used to build the
    /// resource name `Im{index + 1}`).
    pub fn add_image(&mut self, image: ImageXObject) -> usize {
        self.images.push(image);
        self.images.len() - 1
    }

    pub fn image_resource_name(index: usize) -> String {
        format!("Im{}", index + 1)
    }

    pub fn add_page(&mut self, page: PdfPage) {
        self.pages.push(page);
    }

    /// FontDescriptor `/Flags`: bit 6 (32) = Nonsymbolic, bit 7 (64) =
    /// Italic when applicable.
    fn descriptor_flags(font: &CidFont) -> u32 {
        let mut flags = 32u32;
        if font.is_italic {
            flags |= 64;
        }
        flags
    }

    /// `/ToUnicode` CMap program body: maps each CID back to its Unicode
    /// scalar so text stays copyable/searchable despite Identity-H
    /// encoding. Chunked into groups of <=100 `bfchar` entries, the
    /// conventional safe limit for CMap resources.
    fn to_unicode_cmap(font: &CidFont) -> Vec<u8> {
        let mut body = String::new();
        body.push_str("/CIDInit /ProcSet findresource begin\n");
        body.push_str("12 dict begin\n");
        body.push_str("begincmap\n");
        body.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
        body.push_str("/CMapName /Adobe-Identity-UCS def\n");
        body.push_str("/CMapType 2 def\n");
        body.push_str("1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
        for chunk in font.to_unicode.chunks(100) {
            body.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for &(cid, ch) in chunk {
                let utf16: Vec<u16> = ch.encode_utf16(&mut [0u16; 2]).to_vec();
                let hex: String = utf16.iter().map(|u| format!("{u:04X}")).collect();
                body.push_str(&format!("<{cid:04X}> <{hex}>\n"));
            }
            body.push_str("endbfchar\n");
        }
        body.push_str("endcmap\n");
        body.push_str("CMapType findresource /CMap defineresource pop\n");
        body.push_str("end\n");
        body.push_str("end");
        body.into_bytes()
    }

    /// Writes one image XObject (recursing once for `smask`, PNG alpha)
    /// and returns its object reference.
    fn write_image(w: &mut PdfWriter, image: &ImageXObject) -> Ref {
        let smask_ref = image.smask.as_deref().map(|m| Self::write_image(w, m));
        let image_ref = w.alloc();
        let filter = match image.filter {
            ImageDataFilter::None => String::new(),
            ImageDataFilter::DctDecode => " /Filter /DCTDecode".to_string(),
        };
        // `smask` is a genuinely optional field (most images carry no alpha
        // mask) — omitting `/SMask` when absent is not a swallowed error.
        let smask_entry = match smask_ref {
            Some(r) => format!(" /SMask {}", r.write()),
            None => String::new(),
        };
        let dict = format!(
            "/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /{cs} /BitsPerComponent {bpc}{filter}{smask}",
            w = image.width_px,
            h = image.height_px,
            cs = image.color_space.as_pdf_name(),
            bpc = image.bits_per_component,
            filter = filter,
            smask = smask_entry,
        );
        // JPEG samples (DctDecode) are already compressed — re-deflating
        // near-random bytes wastes CPU for ~0 size benefit, so only raw
        // (None) samples go through the compressing path.
        match image.filter {
            ImageDataFilter::None => w.compressed_stream(image_ref, &dict, &image.bytes),
            ImageDataFilter::DctDecode => w.stream(image_ref, &dict, &image.bytes),
        }
        image_ref
    }

    /// Maps each item through `f` and joins the results with a single
    /// space — shared by [`Self::write_fonts`]'s glyph-width array and
    /// [`Self::write`]'s `/Kids` array.
    fn join_with_space<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
        items.iter().map(f).collect::<Vec<_>>().join(" ")
    }

    /// Formats `/Name Ref` resource-dictionary entries (space-joined) for a
    /// sequence of object refs — shared by [`Self::write_fonts`]'s `/Font`
    /// entries and [`Self::write`]'s `/XObject` entries.
    fn resource_entries(refs: &[Ref], name_fn: impl Fn(usize) -> String) -> String {
        refs.iter()
            .enumerate()
            .map(|(i, r)| format!("/{} {}", name_fn(i), r.write()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Writes all font objects (Type0 + CIDFontType2 + FontDescriptor +
    /// embedded subset FontFile2 + ToUnicode) and returns the `/Font`
    /// resource-dictionary entries for the page objects. Zips `fonts` with
    /// their pre-allocated refs rather than indexing by position, so the
    /// pairing can't panic even if the two ever fell out of step.
    fn write_fonts(w: &mut PdfWriter, fonts: &[CidFont]) -> String {
        let font_refs: Vec<(Ref, Ref, Ref, Ref)> = fonts.iter().map(|_| (w.alloc(), w.alloc(), w.alloc(), w.alloc())).collect();

        for (font, &(type0_ref, cid_ref, descriptor_ref, file_ref)) in fonts.iter().zip(&font_refs) {
            let to_unicode_ref = w.alloc();

            let widths_str = Self::join_with_space(&font.widths, |w| fmt_num(*w));

            w.object(
                type0_ref,
                &format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /{base} /Encoding /Identity-H /DescendantFonts [{cid}] /ToUnicode {tu} >>",
                    base = font.base_font,
                    cid = cid_ref.write(),
                    tu = to_unicode_ref.write(),
                ),
            );
            w.object(
                cid_ref,
                &format!(
                    "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc} /DW 1000 /W [0 [{widths}]] /CIDToGIDMap /Identity >>",
                    base = font.base_font,
                    desc = descriptor_ref.write(),
                    widths = widths_str,
                ),
            );
            w.object(
                descriptor_ref,
                &format!(
                    "<< /Type /FontDescriptor /FontName /{base} /Flags {flags} /FontBBox [{bx0} {by0} {bx1} {by1}] /ItalicAngle {italic} /Ascent {ascent} /Descent {descent} /CapHeight {cap} /StemV {stemv} /FontFile2 {file} >>",
                    base = font.base_font,
                    flags = Self::descriptor_flags(font),
                    bx0 = fmt_num(font.bbox.0),
                    by0 = fmt_num(font.bbox.1),
                    bx1 = fmt_num(font.bbox.2),
                    by1 = fmt_num(font.bbox.3),
                    italic = fmt_num(font.italic_angle),
                    ascent = fmt_num(font.ascent),
                    descent = fmt_num(font.descent),
                    cap = fmt_num(font.cap_height),
                    stemv = if font.is_bold { 120 } else { 80 },
                    file = file_ref.write(),
                ),
            );
            w.compressed_stream(file_ref, &format!("/Length1 {}", font.subset_bytes.len()), &font.subset_bytes);
            w.compressed_stream(to_unicode_ref, "", &Self::to_unicode_cmap(font));
        }

        let type0_refs: Vec<Ref> = font_refs.iter().map(|&(t, ..)| t).collect();
        Self::resource_entries(&type0_refs, Self::font_resource_name)
    }

    /// Writes each page's `/Page` object and content stream and returns the
    /// page object refs (used by the caller to build the `/Pages /Kids`
    /// array). Zips `pages` with their pre-allocated refs rather than
    /// indexing by position, so the pairing can't panic even if the two
    /// ever fell out of step.
    fn write_pages(w: &mut PdfWriter, pages: &[PdfPage], pages_ref: Ref, font_resources: &str, image_resources: &str) -> Vec<Ref> {
        let page_refs: Vec<Ref> = (0..pages.len()).map(|_| w.alloc()).collect();
        let content_refs: Vec<Ref> = (0..pages.len()).map(|_| w.alloc()).collect();

        for ((page, &page_ref), &content_ref) in pages.iter().zip(&page_refs).zip(&content_refs) {
            let mut annot_refs = Vec::new();
            for annot in &page.annotations {
                let id = w.alloc();
                let action = match &annot.action {
                    PdfLinkAction::Uri(uri) => format!("/A << /S /URI /URI {} >>", format_pdf_string(uri)),
                    PdfLinkAction::GoTo { page_index, y } => {
                        // Falls back to this annotation's own page if
                        // `page_index` is somehow out of range — a link to
                        // itself is a harmless no-op, not a broken PDF.
                        let target = page_refs.get(*page_index).copied().unwrap_or(page_ref);
                        format!("/Dest [{} /XYZ null {} null]", target.write(), fmt_num(*y))
                    }
                };
                w.object(
                    id,
                    &format!(
                        "<< /Type /Annot /Subtype /Link /Rect [{x0} {y0} {x1} {y1}] /Border [0 0 0] {action} >>",
                        x0 = fmt_num(annot.rect.0),
                        y0 = fmt_num(annot.rect.1),
                        x1 = fmt_num(annot.rect.2),
                        y1 = fmt_num(annot.rect.3),
                    ),
                );
                annot_refs.push(id);
            }

            let annots_entry = if !annot_refs.is_empty() {
                let refs = Self::join_with_space(&annot_refs, |r| r.write());
                format!(" /Annots [{refs}]")
            } else {
                String::new()
            };

            w.object(
                page_ref,
                &format!(
                    "<< /Type /Page /Parent {parent} /MediaBox [0 0 {w} {h}] /Resources << /Font << {fonts} >> /XObject << {images} >> >> /Contents {content}{annots} >>",
                    parent = pages_ref.write(),
                    w = fmt_num(page.width),
                    h = fmt_num(page.height),
                    fonts = font_resources,
                    images = image_resources,
                    content = content_ref.write(),
                    annots = annots_entry,
                ),
            );
            w.compressed_stream(content_ref, "", &page.content);
        }

        page_refs
    }

    /// Writes the `/Outlines` bookmark tree and returns its object ref, or
    /// `None` if `outline` is empty — a document with no headings gets no
    /// `/Outlines` entry at all, not an empty bookmark panel. Two passes:
    /// [`alloc_outline_refs`] allocates one `Ref` per node first (so
    /// siblings/parents can reference each other regardless of write
    /// order), [`write_outline_siblings`] then writes every node's dict.
    fn write_outline(w: &mut PdfWriter, outline: &[PdfOutlineNode], page_refs: &[Ref]) -> Option<Ref> {
        if outline.is_empty() {
            return None;
        }
        let outlines_ref = w.alloc();
        let ref_tree = alloc_outline_refs(w, outline);
        write_outline_siblings(w, outline, &ref_tree, outlines_ref, page_refs);

        let total_count: i64 = outline.iter().map(|n| 1 + count_descendants(n)).sum();
        let first = ref_tree.first().map(|t| t.r);
        let last = ref_tree.last().map(|t| t.r);
        let mut entries = vec!["/Type /Outlines".to_string(), format!("/Count {total_count}")];
        if let Some(f) = first {
            entries.push(format!("/First {}", f.write()));
        }
        if let Some(l) = last {
            entries.push(format!("/Last {}", l.write()));
        }
        w.object(outlines_ref, &format!("<< {} >>", entries.join(" ")));
        Some(outlines_ref)
    }

    /// Assembles the full PDF byte stream: Catalog, Pages, Page objects,
    /// content streams, fonts (Type0 + CIDFontType2 + FontDescriptor +
    /// embedded subset FontFile2 + ToUnicode), images (XObjects + optional
    /// SMask), xref and trailer.
    pub fn write(&self) -> Vec<u8> {
        let mut w = PdfWriter::new();

        let catalog_ref = w.alloc();
        let pages_ref = w.alloc();

        let image_refs: Vec<Ref> = self.images.iter().map(|img| Self::write_image(&mut w, img)).collect();
        let image_resources = Self::resource_entries(&image_refs, Self::image_resource_name);

        let font_resources = Self::write_fonts(&mut w, &self.fonts);
        let page_refs = Self::write_pages(&mut w, &self.pages, pages_ref, &font_resources, &image_resources);

        let kids = Self::join_with_space(&page_refs, |r| r.write());
        w.object(pages_ref, &format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", self.pages.len()));

        let outlines_entry = match Self::write_outline(&mut w, &self.outline, &page_refs) {
            Some(outlines_ref) => format!(" /Outlines {}", outlines_ref.write()),
            None => String::new(),
        };
        w.object(
            catalog_ref,
            &format!("<< /Type /Catalog /Pages {}{outlines_entry} >>", pages_ref.write()),
        );

        let mut info_entries = Vec::new();
        if let Some(ref title) = self.metadata.title {
            info_entries.push(format!("/Title {}", format_pdf_string(title)));
        }
        if let Some(ref author) = self.metadata.author {
            info_entries.push(format!("/Author {}", format_pdf_string(author)));
        }
        if let Some(ref subject) = self.metadata.subject {
            info_entries.push(format!("/Subject {}", format_pdf_string(subject)));
        }
        if let Some(ref keywords) = self.metadata.keywords {
            info_entries.push(format!("/Keywords {}", format_pdf_string(keywords)));
        }
        if let Some(ref creator) = self.metadata.creator {
            info_entries.push(format!("/Creator {}", format_pdf_string(creator)));
        }
        if let Some(ref creation_date) = self.metadata.creation_date {
            info_entries.push(format!("/CreationDate {}", format_pdf_string(creation_date)));
        }
        if let Some(ref mod_date) = self.metadata.mod_date {
            info_entries.push(format!("/ModDate {}", format_pdf_string(mod_date)));
        }
        info_entries.push(format!("/Producer {}", format_pdf_string(PRODUCER)));

        let info_ref = {
            let id = w.alloc();
            w.object(id, &format!("<< {} >>", info_entries.join(" ")));
            Some(id)
        };

        w.finish(catalog_ref, info_ref)
    }
}

/// [`PdfOutlineNode`]'s shape, mirrored with an allocated [`Ref`] per node
/// instead of the node data — lets [`write_outline_siblings`] look up any
/// node's own/children's refs without re-allocating or borrowing `w`.
struct RefTree {
    r: Ref,
    children: Vec<RefTree>,
}

fn alloc_outline_refs(w: &mut PdfWriter, nodes: &[PdfOutlineNode]) -> Vec<RefTree> {
    nodes
        .iter()
        .map(|n| RefTree {
            r: w.alloc(),
            children: alloc_outline_refs(w, &n.children),
        })
        .collect()
}

/// Total number of descendants (not just direct children) — the PDF
/// `/Count` an always-expanded outline entry needs.
fn count_descendants(node: &PdfOutlineNode) -> i64 {
    node.children.len() as i64 + node.children.iter().map(count_descendants).sum::<i64>()
}

/// Writes every node in `nodes` (a sibling list — top-level entries or one
/// node's children) as its own indirect object: `/Title`, `/Parent`,
/// `/Prev`/`/Next` (siblings), `/First`/`/Last`/`/Count` (children), and
/// `/Dest` resolved from `page_index`/`y` against `page_refs` (falls back
/// to the entry's own object if `page_index` is somehow out of range — a
/// self-link is a harmless no-op, not a broken PDF). Recurses into each
/// node's own children afterwards.
fn write_outline_siblings(w: &mut PdfWriter, nodes: &[PdfOutlineNode], ref_nodes: &[RefTree], parent_ref: Ref, page_refs: &[Ref]) {
    for (i, (node, ref_node)) in nodes.iter().zip(ref_nodes).enumerate() {
        let prev = (i > 0).then(|| ref_nodes[i - 1].r);
        let next = (i + 1 < nodes.len()).then(|| ref_nodes[i + 1].r);
        let first = ref_node.children.first().map(|c| c.r);
        let last = ref_node.children.last().map(|c| c.r);
        let count = count_descendants(node);
        let target_page = page_refs.get(node.page_index).copied().unwrap_or(ref_node.r);

        let mut entries = vec![
            format!("/Title {}", format_pdf_string(&node.title)),
            format!("/Parent {}", parent_ref.write()),
            format!("/Dest [{} /XYZ null {} null]", target_page.write(), fmt_num(node.y)),
        ];
        if let Some(p) = prev {
            entries.push(format!("/Prev {}", p.write()));
        }
        if let Some(n) = next {
            entries.push(format!("/Next {}", n.write()));
        }
        if let Some(f) = first {
            entries.push(format!("/First {}", f.write()));
        }
        if let Some(l) = last {
            entries.push(format!("/Last {}", l.write()));
        }
        if count > 0 {
            entries.push(format!("/Count {count}"));
        }
        w.object(ref_node.r, &format!("<< {} >>", entries.join(" ")));

        write_outline_siblings(w, &node.children, &ref_node.children, ref_node.r, page_refs);
    }
}

fn format_pdf_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
    format!("({escaped})")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_font() -> CidFont {
        CidFont {
            base_font: "Test".to_string(),
            subset_bytes: vec![0u8; 16],
            widths: vec![0.0, 600.0],
            ascent: 800.0,
            descent: -200.0,
            cap_height: 700.0,
            italic_angle: 0.0,
            bbox: (-100.0, -200.0, 900.0, 900.0),
            is_italic: false,
            is_bold: false,
            to_unicode: vec![(1, 'H')],
        }
    }

    #[test]
    fn writes_a_single_empty_page() {
        let mut doc = PdfDocument::new();
        doc.add_page(PdfPage {
            width: 595.0,
            height: 842.0,
            content: Vec::new(),
            annotations: Vec::new(),
        });
        let bytes = doc.write();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Type /Page"));
        assert!(text.contains("/MediaBox [0 0 595 842]"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn writes_a_goto_destination_for_an_internal_link_annotation() {
        let mut doc = PdfDocument::new();
        doc.add_page(PdfPage {
            width: 595.0,
            height: 842.0,
            content: Vec::new(),
            annotations: vec![PdfLinkAnnotation {
                rect: (10.0, 20.0, 100.0, 40.0),
                action: PdfLinkAction::GoTo { page_index: 1, y: 700.0 },
            }],
        });
        doc.add_page(PdfPage {
            width: 595.0,
            height: 842.0,
            content: Vec::new(),
            annotations: Vec::new(),
        });
        let bytes = doc.write();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Subtype /Link"));
        assert!(text.contains("/Dest ["));
        assert!(text.contains("/XYZ null 700 null"));
        assert!(!text.contains("/S /URI"), "a GoTo annotation must not also emit a URI action");
    }

    #[test]
    fn writes_type0_cid_font_structure() {
        let mut doc = PdfDocument::new();
        doc.add_font(tiny_font());
        doc.add_page(PdfPage {
            width: 595.0,
            height: 842.0,
            content: Vec::new(),
            annotations: Vec::new(),
        });
        let bytes = doc.write();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Subtype /Type0"));
        assert!(text.contains("/Encoding /Identity-H"));
        assert!(text.contains("/Subtype /CIDFontType2"));
        assert!(text.contains("/CIDToGIDMap /Identity"));
        assert!(text.contains("/ToUnicode"));
        // The ToUnicode CMap body itself lives inside a stream, which is
        // `/FlateDecode`-compressed by default (ADR-016) — inflate every
        // stream body before checking, rather than the raw dict text.
        let decoded = stream_bodies_decoded(&bytes);
        assert!(decoded.contains("beginbfchar"));
        assert!(decoded.contains("<0001> <0048>")); // CID 1 -> U+0048 'H'
    }

    /// Every `stream\n...\nendstream` payload in `bytes`, inflated (when
    /// `compress` is enabled — a no-op passthrough otherwise, since
    /// nothing is compressed then) and concatenated, for tests that need
    /// to read stream content rather than just check the surrounding
    /// dict.
    fn stream_bodies_decoded(bytes: &[u8]) -> String {
        const START: &[u8] = b"stream\n";
        const END: &[u8] = b"\nendstream";
        let mut bodies = Vec::new();
        let mut i = 0;
        while let Some(start_rel) = bytes[i..].windows(START.len()).position(|w| w == START) {
            let start = i + start_rel + START.len();
            let Some(end_rel) = bytes[start..].windows(END.len()).position(|w| w == END) else {
                break;
            };
            let end = start + end_rel;
            bodies.push(&bytes[start..end]);
            i = end + END.len();
        }
        bodies.into_iter().map(decode_one_stream_body).collect::<Vec<_>>().join("\n")
    }

    #[cfg(feature = "compress")]
    fn decode_one_stream_body(body: &[u8]) -> String {
        match miniz_oxide::inflate::decompress_to_vec_zlib(body) {
            Ok(v) => String::from_utf8_lossy(&v).into_owned(),
            Err(_) => String::new(), // not a zlib stream (shouldn't happen for our own output) — skip, don't panic
        }
    }

    #[cfg(not(feature = "compress"))]
    fn decode_one_stream_body(body: &[u8]) -> String {
        String::from_utf8_lossy(body).into_owned()
    }

    #[test]
    fn writes_image_xobject_with_smask() {
        let mut doc = PdfDocument::new();
        doc.add_image(ImageXObject {
            width_px: 4,
            height_px: 4,
            color_space: ColorSpace::DeviceRgb,
            bits_per_component: 8,
            filter: ImageDataFilter::None,
            bytes: vec![0u8; 4 * 4 * 3],
            smask: Some(Box::new(ImageXObject {
                width_px: 4,
                height_px: 4,
                color_space: ColorSpace::DeviceGray,
                bits_per_component: 8,
                filter: ImageDataFilter::None,
                bytes: vec![255u8; 4 * 4],
                smask: None,
            })),
        });
        doc.add_page(PdfPage {
            width: 200.0,
            height: 200.0,
            content: Vec::new(),
            annotations: Vec::new(),
        });
        let bytes = doc.write();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Subtype /Image"));
        assert!(text.contains("/ColorSpace /DeviceRGB"));
        assert!(text.contains("/ColorSpace /DeviceGray"));
        assert!(text.contains("/SMask"));
        assert!(text.contains("/XObject << /Im1"));
    }

    #[test]
    fn writes_jpeg_image_with_dct_decode_filter() {
        let mut doc = PdfDocument::new();
        doc.add_image(ImageXObject {
            width_px: 10,
            height_px: 10,
            color_space: ColorSpace::DeviceRgb,
            bits_per_component: 8,
            filter: ImageDataFilter::DctDecode,
            bytes: vec![0xFF, 0xD8, 0xFF, 0xD9], // stand-in bytes, structure only
            smask: None,
        });
        doc.add_page(PdfPage {
            width: 200.0,
            height: 200.0,
            content: Vec::new(),
            annotations: Vec::new(),
        });
        let bytes = doc.write();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Filter /DCTDecode"));
    }
}
