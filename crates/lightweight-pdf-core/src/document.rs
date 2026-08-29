use crate::element::Element;
use std::rc::Rc;

/// Page formats supported for documents. Dimensions in PDF points (1/72 inch).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PageFormat {
    A3,
    A4,
    A5,
    Letter,
    Legal,
    Custom(f32, f32),
}

impl PageFormat {
    /// (width, height) in points, portrait.
    pub fn size(&self) -> (f32, f32) {
        match self {
            PageFormat::A3 => (841.8898, 1190.5512),
            PageFormat::A4 => (595.2756, 841.8898),
            PageFormat::A5 => (419.5276, 595.2756),
            PageFormat::Letter => (612.0, 792.0),
            PageFormat::Legal => (612.0, 1008.0),
            PageFormat::Custom(w, h) => (*w, *h),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Clone, Debug, Default)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub creation_date: Option<PdfDate>,
    pub mod_date: Option<PdfDate>,
}

/// A UTC timestamp for `/CreationDate`/`/ModDate`. Always an explicit
/// caller-supplied value, never read from the system clock: `wasm32-unknown-unknown`
/// has none, and reproducible output (same `Document` -> byte-identical
/// PDF) is a feature, not an accident.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PdfDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl PdfDate {
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        PdfDate {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// `D:YYYYMMDDHHmmSSZ` — the PDF date string format (ISO/IEC 32000-1
    /// 7.9.4), UTC only (no offset support needed here).
    pub fn to_pdf_string(self) -> String {
        format!(
            "D:{:04}{:02}{:02}{:02}{:02}{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
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
    pub orientation: Orientation,
    pub margin: Margin,
    pub header: Option<Header>,
    pub footer: Option<Footer>,
    pub header_visible_from: usize,
    pub footer_visible_from: usize,
    pub watermark: Option<crate::watermark::Watermark>,
    pub metadata: DocumentMetadata,
    /// `None` (the default) means every element renders exactly as it
    /// always did — `Document::theme(..)` opts in per-document, resolved
    /// once per element as it's `.add()`-ed (see `theme::apply_theme`).
    pub theme: Option<crate::theme::Theme>,
    pub children: Vec<Element>,
}

impl Document {
    pub fn new(page_format: PageFormat) -> Self {
        Document {
            page_format,
            orientation: Orientation::default(),
            margin: Margin::default(),
            header: None,
            footer: None,
            header_visible_from: 1,
            footer_visible_from: 1,
            watermark: None,
            metadata: DocumentMetadata::default(),
            theme: None,
            children: Vec::new(),
        }
    }

    pub fn theme(mut self, theme: crate::theme::Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Effective page dimensions (width, height) in PDF points, accounting for orientation.
    pub fn page_size(&self) -> (f32, f32) {
        let (w, h) = self.page_format.size();
        match self.orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn landscape(mut self) -> Self {
        self.orientation = Orientation::Landscape;
        self
    }

    pub fn portrait(mut self) -> Self {
        self.orientation = Orientation::Portrait;
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.metadata.title = Some(title.into());
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.metadata.author = Some(author.into());
        self
    }

    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.metadata.subject = Some(subject.into());
        self
    }

    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.metadata.keywords = Some(keywords.into());
        self
    }

    pub fn creator(mut self, creator: impl Into<String>) -> Self {
        self.metadata.creator = Some(creator.into());
        self
    }

    pub fn creation_date(mut self, date: PdfDate) -> Self {
        self.metadata.creation_date = Some(date);
        self
    }

    pub fn mod_date(mut self, date: PdfDate) -> Self {
        self.metadata.mod_date = Some(date);
        self
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
        let mut element = element.into();
        if let Some(theme) = &self.theme {
            crate::theme::apply_theme(&mut element, theme);
        }
        self.children.push(element);
        self
    }
}
