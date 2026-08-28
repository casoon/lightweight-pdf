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
    /// Raw, uncompressed samples (V1 doesn't implement a Flate encoder —
    /// consistent with content streams/embedded fonts also being
    /// uncompressed, see `progress.md`).
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
pub struct PdfLinkAnnotation {
    pub rect: (f32, f32, f32, f32),
    pub uri: String,
}

#[derive(Default)]
pub struct PdfPage {
    pub width: f32,
    pub height: f32,
    pub content: Vec<u8>,
    pub annotations: Vec<PdfLinkAnnotation>,
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
        w.stream(image_ref, &dict, &image.bytes);
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
            w.stream(file_ref, &format!("/Length1 {}", font.subset_bytes.len()), &font.subset_bytes);
            w.stream(to_unicode_ref, "", &Self::to_unicode_cmap(font));
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
                w.object(
                    id,
                    &format!(
                        "<< /Type /Annot /Subtype /Link /Rect [{x0} {y0} {x1} {y1}] /Border [0 0 0] /A << /S /URI /URI {uri} >> >>",
                        x0 = fmt_num(annot.rect.0),
                        y0 = fmt_num(annot.rect.1),
                        x1 = fmt_num(annot.rect.2),
                        y1 = fmt_num(annot.rect.3),
                        uri = format_pdf_string(&annot.uri),
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
            w.stream(content_ref, "", &page.content);
        }

        page_refs
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
        w.object(catalog_ref, &format!("<< /Type /Catalog /Pages {} >>", pages_ref.write()));

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
        assert!(text.contains("beginbfchar"));
        assert!(text.contains("<0001> <0048>")); // CID 1 -> U+0048 'H'
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
