use crate::element::Element;
use std::rc::Rc;

/// Page formats supported for documents. Dimensions in PDF points (1/72 inch).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

    /// ISO 8601, as XMP (`xmp:CreateDate`/`xmp:ModifyDate`) wants it — the
    /// same fields as `to_pdf_string`, just reordered/repunctuated, not a
    /// second date representation (issue #25).
    pub fn to_xmp_string(self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone)]
pub struct Document {
    pub page_format: PageFormat,
    #[cfg_attr(feature = "serde", serde(default))]
    pub orientation: Orientation,
    #[cfg_attr(feature = "serde", serde(default))]
    pub margin: Margin,
    /// Not representable in the JSON schema (issue #17 V1 scope): the
    /// content is a Rust closure, re-evaluated per page. Always `None` on
    /// a JSON-loaded `Document`; `Document::to_json` refuses to serialize
    /// a `Document` that has one set rather than silently dropping it.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub header: Option<Header>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub footer: Option<Footer>,
    #[cfg_attr(feature = "serde", serde(skip, default = "default_visible_from"))]
    pub header_visible_from: usize,
    #[cfg_attr(feature = "serde", serde(skip, default = "default_visible_from"))]
    pub footer_visible_from: usize,
    #[cfg_attr(feature = "serde", serde(default))]
    pub watermark: Option<crate::watermark::Watermark>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub metadata: DocumentMetadata,
    /// `None` (the default) means every element renders exactly as it
    /// always did — `Document::theme(..)` opts in per-document, resolved
    /// once per element as it's `.add()`-ed (see `theme::apply_theme`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub theme: Option<crate::theme::Theme>,
    /// Set by `.pdf_a3b()` (issue #25): asks the facade to write a
    /// PDF/A-3b-conformant document (XMP metadata, `/OutputIntent` with an
    /// embedded sRGB ICC profile, transparency-group colour space) instead
    /// of the default output. Always present on `Document` regardless of
    /// the facade's `pdf-a` Cargo feature (this flag itself costs
    /// nothing) — `render()` returns `RenderError::PdfAFeatureDisabled` if
    /// this is `true` but that feature isn't compiled in, rather than
    /// silently rendering a non-conformant PDF.
    #[cfg_attr(feature = "serde", serde(default))]
    pub pdf_a3b: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub children: Vec<Element>,
}

#[cfg(feature = "serde")]
fn default_visible_from() -> usize {
    1
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
            pdf_a3b: false,
            children: Vec::new(),
        }
    }

    pub fn theme(mut self, theme: crate::theme::Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Opt in to PDF/A-3b-conformant output (issue #25) — see
    /// `Document::pdf_a3b`'s field doc comment. Needs the facade's `pdf-a`
    /// Cargo feature; without it, `render()` returns
    /// `RenderError::PdfAFeatureDisabled` rather than silently ignoring
    /// this.
    pub fn pdf_a3b(mut self) -> Self {
        self.pdf_a3b = true;
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

// ---------------------------------------------------------------------
// JSON (issue #17): `Document` ↔ JSON, behind the `serde` feature.
// Header/Footer aren't representable (Rust closures) — excluded from the
// wire format entirely rather than silently dropped; `to_json` refuses
// outright if either is set.
// ---------------------------------------------------------------------

#[cfg(feature = "serde")]
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The versioned envelope every JSON document is wrapped in (ADR-009: an
/// external entry point needs a schema version from day one to stay
/// extensible). Deliberately not `#[serde(flatten)]`ed into `Document` —
/// `flatten` and `deny_unknown_fields` don't compose in serde, and
/// "unknown fields are a clear error, not silent loss" is an explicit
/// acceptance criterion.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct DocumentSchema {
    pub schema_version: u32,
    pub document: Document,
}

#[cfg(feature = "serde")]
#[derive(Debug)]
pub enum DocumentJsonError {
    /// `schema_version` isn't one this version of the crate understands.
    UnsupportedSchemaVersion(u32),
    /// `Document::to_json` on a `Document` with a `header`/`footer` set —
    /// neither is representable in JSON, so refusing beats silently
    /// dropping them.
    HeaderOrFooterNotSupported,
    Json(serde_json::Error),
    /// From `Document::from_template` (issue #18): placeholder/`$each`
    /// resolution against the data tree failed before JSON parsing of
    /// the resolved document even started.
    Template(crate::template::TemplateError),
}

#[cfg(feature = "serde")]
impl std::fmt::Display for DocumentJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentJsonError::UnsupportedSchemaVersion(v) => {
                write!(
                    f,
                    "unsupported schema_version {v} (this crate understands {CURRENT_SCHEMA_VERSION})"
                )
            }
            DocumentJsonError::HeaderOrFooterNotSupported => {
                write!(
                    f,
                    "Document::to_json: header/footer aren't representable in the JSON schema (issue #17 V1 scope)"
                )
            }
            DocumentJsonError::Json(e) => write!(f, "{e}"),
            DocumentJsonError::Template(e) => write!(f, "{e}"),
        }
    }
}

#[cfg(feature = "serde")]
impl std::error::Error for DocumentJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DocumentJsonError::Json(e) => Some(e),
            DocumentJsonError::Template(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(feature = "serde")]
impl Document {
    /// Parses `{"schema_version": N, "document": { .. }}`. Unknown fields
    /// anywhere in the tree are a clear error, never silently dropped.
    pub fn from_json(json: &str) -> Result<Document, DocumentJsonError> {
        let schema: DocumentSchema = serde_json::from_str(json).map_err(DocumentJsonError::Json)?;
        if schema.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(DocumentJsonError::UnsupportedSchemaVersion(schema.schema_version));
        }
        Ok(schema.document)
    }

    /// `crate::template::render_template` + `from_json` in one call — a
    /// template document (with `{{path}}` placeholders and/or `$each`
    /// repetition, see the `template` module) plus a separate data
    /// document, no Rust code needed (issue #18).
    pub fn from_template(
        template_json: &str,
        data_json: &str,
        on_missing: crate::template::MissingPlaceholder,
    ) -> Result<Document, DocumentJsonError> {
        let resolved = crate::template::render_template(template_json, data_json, on_missing).map_err(DocumentJsonError::Template)?;
        Document::from_json(&resolved)
    }

    /// The inverse of `from_json` — round-trips to a byte-identical
    /// rendered PDF as long as neither `header` nor `footer` is set.
    pub fn to_json(&self) -> Result<String, DocumentJsonError> {
        if self.header.is_some() || self.footer.is_some() {
            return Err(DocumentJsonError::HeaderOrFooterNotSupported);
        }
        let schema = DocumentSchema {
            schema_version: CURRENT_SCHEMA_VERSION,
            document: self.clone(),
        };
        serde_json::to_string(&schema).map_err(DocumentJsonError::Json)
    }
}

#[cfg(all(test, feature = "serde"))]
mod json_tests {
    use super::*;
    use crate::element::Text;
    use crate::style::{Align, Color};

    fn sample_document() -> Document {
        let mut doc = Document::new(PageFormat::A4).margin(Margin::all(30.0)).title("Rechnung");
        doc.add(Text::new("Hello").size(18.0).color(Color::rgb(200, 0, 0)).align(Align::Center));
        doc
    }

    #[test]
    fn round_trip_preserves_page_format_and_children() {
        let json = sample_document().to_json().expect("to_json should succeed");
        assert!(
            json.contains("\"schema_version\":1"),
            "expected a versioned root field, got: {json}"
        );
        let doc = Document::from_json(&json).expect("from_json should succeed");
        assert_eq!(doc.page_format, PageFormat::A4);
        assert_eq!(doc.metadata.title.as_deref(), Some("Rechnung"));
        assert_eq!(doc.children.len(), 1);
        let Element::Text(t) = &doc.children[0] else {
            panic!("expected a Text child");
        };
        assert_eq!(t.content, "Hello");
        assert_eq!(t.style.size, 18.0);
        assert_eq!(t.style.color, Color::rgb(200, 0, 0));
        assert_eq!(t.style.align, Align::Center);
    }

    #[test]
    fn unknown_field_is_a_clear_error_not_silent_loss() {
        let json = r#"{"schema_version":1,"document":{"page_format":"A4","typo_field":true}}"#;
        let Err(err) = Document::from_json(json) else {
            panic!("an unknown field must be rejected");
        };
        let message = err.to_string();
        assert!(
            message.contains("typo_field") || message.contains("unknown field"),
            "expected the error to mention the unknown field, got: {message}"
        );
    }

    #[test]
    fn to_json_refuses_a_document_with_a_header() {
        let mut doc = sample_document();
        doc = doc.header(Header::new(20.0, |_| Element::Text(Text::new("Header"))));
        assert!(matches!(doc.to_json(), Err(DocumentJsonError::HeaderOrFooterNotSupported)));
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let json = r#"{"schema_version":99,"document":{"page_format":"A4"}}"#;
        assert!(matches!(
            Document::from_json(json),
            Err(DocumentJsonError::UnsupportedSchemaVersion(99))
        ));
    }
}
