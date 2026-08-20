use crate::element::Element;
use std::rc::Rc;

/// V1 supports a single fixed page size (Phase 0 spike scope: "fixe
/// A4-Größe"). Dimensions in PDF points (1/72 inch).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PageFormat {
    A4,
}

impl PageFormat {
    /// (width, height) in points, portrait.
    pub fn size(&self) -> (f32, f32) {
        match self {
            // 210mm x 297mm at 72pt/25.4mm.
            PageFormat::A4 => (595.2756, 841.8898),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Margin {
    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Margin {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    pub fn all(value: f32) -> Self {
        Margin {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Passed to `Header`/`Footer` closures on every (re-)evaluation. Plain data
/// only, so it can live in `lightweight-pdf-core` without pulling in layout/font
/// knowledge (ADR-010).
#[derive(Clone, Copy, Debug)]
pub struct PageContext {
    pub page: usize,
    pub total_pages: usize,
}

type HeaderFooterFn = Rc<dyn Fn(&PageContext) -> Element>;

/// A header band with a fixed, document-creation-time height (ADR-011): the
/// closure may vary its content per page but never the reserved band size.
#[derive(Clone)]
pub struct Header {
    pub height: f32,
    pub content: HeaderFooterFn,
}

impl Header {
    pub fn new(height: f32, content: impl Fn(&PageContext) -> Element + 'static) -> Self {
        Header {
            height,
            content: Rc::new(content),
        }
    }
}

#[derive(Clone)]
pub struct Footer {
    pub height: f32,
    pub content: HeaderFooterFn,
}

impl Footer {
    pub fn new(height: f32, content: impl Fn(&PageContext) -> Element + 'static) -> Self {
        Footer {
            height,
            content: Rc::new(content),
        }
    }
}

#[derive(Clone)]
pub struct Document {
    pub page_format: PageFormat,
    pub margin: Margin,
    pub header: Option<Header>,
    pub footer: Option<Footer>,
    pub header_visible_from: usize,
    pub footer_visible_from: usize,
    pub watermark: Option<crate::watermark::Watermark>,
    pub children: Vec<Element>,
}

impl Document {
    pub fn new(page_format: PageFormat) -> Self {
        Document {
            page_format,
            margin: Margin::default(),
            header: None,
            footer: None,
            header_visible_from: 1,
            footer_visible_from: 1,
            watermark: None,
            children: Vec::new(),
        }
    }

    pub fn margin(mut self, margin: Margin) -> Self {
        self.margin = margin;
        self
    }

    pub fn header(mut self, header: Header) -> Self {
        self.header = Some(header);
        self
    }

    pub fn footer(mut self, footer: Footer) -> Self {
        self.footer = Some(footer);
        self
    }

    /// First page number (1-based) on which the header is drawn. Cover-page
    /// convenience, see `plan/02-elementcatalog-and-features.md` ("Deckblatt
    /// / Titelseite").
    pub fn header_visible_from(mut self, page: usize) -> Self {
        self.header_visible_from = page;
        self
    }

    pub fn footer_visible_from(mut self, page: usize) -> Self {
        self.footer_visible_from = page;
        self
    }

    /// Sets a document-wide diagonal stamp ("ENTWURF", "STORNIERT") — an
    /// independent layer, not a normal flow element (Phase 6).
    pub fn watermark(mut self, watermark: crate::watermark::Watermark) -> Self {
        self.watermark = Some(watermark);
        self
    }

    pub fn add(&mut self, element: impl Into<Element>) -> &mut Self {
        self.children.push(element.into());
        self
    }
}
