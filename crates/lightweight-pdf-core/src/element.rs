//! Element catalog (V1 subset through concept Phase 5, see
//! `plan/02-elementcatalog-and-features.md`).

use crate::image::Image;
use crate::list::List;
use crate::style::{Align, Border, Color, Common, FontKey, Overflow, TextStyle};
use crate::table::Table;
use crate::theme::ThemeRole;

/// One element in the document tree. Enum-based (not `Box<dyn Layoutable>`)
/// — a closed, small set of primitives.
///
/// `serde` (issue #17): internally tagged on a `type` field
/// (`{"type": "text", "content": "...", ...}`), `snake_case` variant
/// names. `Image`'s JSON shape is base64 (see its own doc comment), not
/// the same fields its Rust builder exposes.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug)]
pub enum Element {
    Text(Text),
    Row(Row),
    Column(Column),
    Spacer(Spacer),
    Line(Line),
    Rect(Rect),
    Table(Table),
    Image(Image),
    List(List),
    TableOfContents(TableOfContents),
    /// Forces a page break at this point in the enclosing flow, regardless
    /// of whether the remaining content would still fit (Phase 2).
    PageBreak,
}

/// Generates the match arms shared by `Element::common`/`common_mut`: both
/// are exact mirrors of each other, differing only in `&`/`&mut`. `$ref` is
/// spliced in front of each `.common` field access, so invoking this with
/// `&` vs `&mut` produces the two accessors from one written body.
macro_rules! common_accessor {
    ($self:expr, $($ref:tt)*) => {
        match $self {
            Element::Text(t) => Some($($ref)* t.common),
            Element::Row(r) => Some($($ref)* r.common),
            Element::Column(c) => Some($($ref)* c.common),
            Element::Line(l) => Some($($ref)* l.common),
            Element::Rect(r) => Some($($ref)* r.common),
            Element::Table(t) => Some($($ref)* t.common),
            Element::Image(i) => Some($($ref)* i.common),
            Element::List(l) => Some($($ref)* l.common),
            Element::TableOfContents(t) => Some($($ref)* t.common),
            Element::Spacer(_) | Element::PageBreak => None,
        }
    };
}

impl Element {
    /// Shared style properties, where applicable. `Spacer` and `PageBreak`
    /// carry no `Common` (nothing to size/clip/keep-with-next).
    pub fn common(&self) -> Option<&Common> {
        common_accessor!(self, &)
    }

    /// Mutable counterpart to [`Self::common`] — used by `List`'s layout
    /// translation to make item content fill the remaining row width
    /// (`flex(1.0)`) without needing a bespoke setter per element variant.
    pub fn common_mut(&mut self) -> Option<&mut Common> {
        common_accessor!(self, &mut)
    }
}

/// Generates the shared `Common`-backed builder methods for a wrapper type
/// that has a `pub common: Common` field. Avoids repeating five setters on
/// every element type (Text, Row, Column, Line, Rect).
macro_rules! common_builder_methods {
    () => {
        pub fn width(mut self, width: f32) -> Self {
            self.common.width = Some(width);
            self
        }

        pub fn height(mut self, height: f32) -> Self {
            self.common.height = Some(height);
            self
        }

        pub fn flex(mut self, factor: f32) -> Self {
            self.common.flex = Some(factor);
            self
        }

        pub fn padding(mut self, padding: f32) -> Self {
            self.common.padding = padding;
            self
        }

        pub fn corner_radius(mut self, radius: f32) -> Self {
            self.common.corner_radius = radius;
            self
        }

        pub fn overflow(mut self, overflow: Overflow) -> Self {
            self.common.overflow = overflow;
            self
        }

        pub fn background(mut self, color: Color) -> Self {
            self.common.background = Some(color);
            self
        }

        pub fn border(mut self, border: Border) -> Self {
            self.common.border = Some(border);
            self
        }

        /// See `plan/05-overflow-and-robustness.md` Grundprinzip 9: only
        /// placed on a page if the following sibling also still fits.
        pub fn keep_with_next(mut self) -> Self {
            self.common.keep_with_next = true;
            self
        }
    };
}

// ---------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default)]
pub struct Text {
    pub content: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub style: TextStyle,
    pub url: Option<String>,
    /// Registers this element as an internal jump target other `Text`
    /// elements can point at via `.link_to(name)` — analogous to an HTML
    /// `id`. Independent of `url`/`link_to`: an element can be a target,
    /// a source, both, or neither.
    pub anchor: Option<String>,
    /// Internal counterpart to `url`: jumps to whatever element in this
    /// document called `.anchor(name)` with the same name, instead of an
    /// external URI. If both `url` and `link_to` are set, `url` wins.
    pub link_to: Option<String>,
    /// Set by `.heading1()`/`.heading2()`/`.heading3()` (1/2/3), or
    /// explicitly via `.outline_level(n)` for text that should appear in
    /// the PDF bookmark sidebar without being an actual heading preset.
    /// `None` (the default for plain `Text`) means "not a bookmark".
    pub outline_level: Option<u8>,
    /// Theme eligibility (`Document::theme(..)`, ADR/issue #16): `Some`
    /// means "resolve this element's style from the theme's matching role
    /// the next time it's added to a themed `Document`." `Text::new()`
    /// defaults this to `Some(ThemeRole::Body)`; every style-mutating
    /// method below (`.size()`, `.bold()`, `.color()`, ...) clears it back
    /// to `None` since the caller has taken over styling by hand. The
    /// `.heading1()`/`.heading2()`/`.heading3()`/`.caption()`/`.muted()`/
    /// `.table_header()` presets re-set a specific role afterwards.
    pub role: Option<ThemeRole>,
    /// Set only by `Text::rich(..)` (issue #11) — a sequence of
    /// independently-styled runs instead of one `style` for the whole
    /// `content`. When `Some`, layout/render use this instead of
    /// `content`/`style` (`content` is still populated, as the spans'
    /// text concatenated, so anything that only reads `content` — e.g. a
    /// future plain-text export — degrades to unstyled text instead of
    /// seeing nothing). Rich text doesn't (yet) support
    /// `url`/`anchor`/`link_to`/`outline_level`/`Align::Justify` — plain
    /// `Text` remains the only way to get those.
    /// Boxed, not `Option<Vec<Span>>` directly: `Text` is the payload of
    /// `Element`'s largest variant (in turn embedded in `LayoutResult`
    /// and every `Row`/`Column`'s `children: Vec<Element>`), and a bare
    /// `Vec` here would cost every plain `Text` (the overwhelming
    /// majority, where this field is always `None`) the full 24 bytes;
    /// `Option<Box<Vec<Span>>>` costs 8.
    pub spans: Option<Box<Vec<Span>>>,
    /// Set by `.hyphenate(lang)` (issue #13, Stage 2): before wrapping,
    /// each word gets Knuth-Liang break points inserted as soft hyphens
    /// (U+00AD) for `lang`, on top of Stage 1's always-on soft-hyphen
    /// support (an author-inserted U+00AD works with or without this).
    /// `None` (the default) means "only break where the author put a
    /// soft hyphen, if anywhere." Only consulted for plain `Text`; a
    /// `Text::rich(..)` ignores it, same as `Align::Justify`.
    /// Requires the `hyphenation` cargo feature — with it disabled this
    /// silently has no effect, since skipping automatic hyphenation only
    /// changes where a line wraps, not what the text says.
    pub hyphenate: Option<HyphenationLanguage>,
    pub common: Common,
}

/// A language `.hyphenate(lang)` can insert Knuth-Liang break points for
/// (`lightweight-pdf-layout`'s `hyphenation` feature; see that crate's
/// `hyphenate` module for the dictionaries themselves).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HyphenationLanguage {
    EnglishUs,
    German,
}

/// One independently-styled run within `Text::rich(..)`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug)]
pub struct Span {
    pub text: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub style: TextStyle,
}

impl Span {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Span { text: text.into(), style }
    }
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Text {
            content: content.into(),
            style: TextStyle::default(),
            url: None,
            anchor: None,
            link_to: None,
            outline_level: None,
            role: Some(ThemeRole::Body),
            spans: None,
            hyphenate: None,
            common: Common::default(),
        }
    }

    /// A `Text` made of independently-styled `Span`s instead of one
    /// uniform style — the paragraph still wraps and paginates as a
    /// single unit, word boundaries and line breaks span across spans
    /// freely, and mixed sizes on the same line share one baseline (see
    /// `lightweight-pdf-layout::text::wrap_spans`).
    pub fn rich(spans: impl IntoIterator<Item = Span>) -> Self {
        let spans: Vec<Span> = spans.into_iter().collect();
        let content = spans.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().concat();
        let style = spans.first().map(|s| s.style).unwrap_or_default();
        Text {
            content,
            style,
            url: None,
            anchor: None,
            link_to: None,
            outline_level: None,
            role: None,
            spans: Some(Box::new(spans)),
            hyphenate: None,
            common: Common::default(),
        }
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn anchor(mut self, name: impl Into<String>) -> Self {
        self.anchor = Some(name.into());
        self
    }

    pub fn link_to(mut self, anchor: impl Into<String>) -> Self {
        self.link_to = Some(anchor.into());
        self
    }

    pub fn outline_level(mut self, level: u8) -> Self {
        self.outline_level = Some(level);
        self
    }

    /// Opts this `Text` into automatic (Knuth-Liang) hyphenation for
    /// `lang` — see the `hyphenate` field's doc comment for scope and the
    /// `hyphenation` cargo feature it requires.
    pub fn hyphenate(mut self, lang: HyphenationLanguage) -> Self {
        self.hyphenate = Some(lang);
        self
    }

    /// Opts a `Text` into theme resolution under `role` without going
    /// through one of the named presets — e.g. a custom role-like use
    /// that isn't `.heading1()`/`.caption()`/etc.
    pub fn role(mut self, role: ThemeRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.style.size = size;
        self.role = None;
        self
    }

    pub fn bold(mut self) -> Self {
        self.style.font = FontKey::SANS_BOLD;
        self.role = None;
        self
    }

    pub fn italic(mut self) -> Self {
        self.style.font = FontKey::SANS_ITALIC;
        self.role = None;
        self
    }

    pub fn bold_italic(mut self) -> Self {
        self.style.font = FontKey::SANS_BOLD_ITALIC;
        self.role = None;
        self
    }

    pub fn font(mut self, font: FontKey) -> Self {
        self.style.font = font;
        self.role = None;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self.role = None;
        self
    }

    /// Unlike the other style setters, `.align()` does *not* clear
    /// `role`: alignment is a positioning choice independent of which
    /// named style a `Text` resolves from (`.heading1().align(Center)`
    /// should stay theme-eligible as a heading, just centered) — see
    /// `theme::apply_theme`, which resolves every role field except
    /// `align` and always leaves whatever `.align()` set alone.
    pub fn align(mut self, align: Align) -> Self {
        self.style.align = align;
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.line_height = line_height;
        self.role = None;
        self
    }

    /// Heading presets (Phase 6, `plan/02-elementcatalog-and-features.md`):
    /// thin wrappers over `.size()`/`.bold()`, additionally setting
    /// `keep_with_next` so a heading never ends up alone at the bottom of
    /// a page without its following content
    /// (`plan/05-overflow-and-robustness.md` Grundprinzip 9), and
    /// `outline_level` so the PDF bookmark sidebar can be derived from the
    /// heading hierarchy without a separate API (`.outline_level(n)`
    /// overrides this for the rare case the derivation doesn't fit).
    pub fn heading1(self) -> Self {
        self.size(24.0).bold().keep_with_next().outline_level(1).role(ThemeRole::Heading1)
    }

    pub fn heading2(self) -> Self {
        self.size(18.0).bold().keep_with_next().outline_level(2).role(ThemeRole::Heading2)
    }

    pub fn heading3(self) -> Self {
        self.size(14.0).bold().keep_with_next().outline_level(3).role(ThemeRole::Heading3)
    }

    /// `Theme::caption` preset — a smaller, muted-gray label (e.g. under
    /// an image, or a secondary line under a heading).
    pub fn caption(self) -> Self {
        self.size(9.0).color(Color::rgb(0x66, 0x66, 0x66)).role(ThemeRole::Caption)
    }

    /// `Theme::muted` preset — body-sized text in the same muted gray as
    /// `.caption()`, for de-emphasized inline text rather than a label.
    pub fn muted(self) -> Self {
        self.color(Color::rgb(0x66, 0x66, 0x66)).role(ThemeRole::Muted)
    }

    /// `Theme::table_header` preset. `Table::header([...])` cells built
    /// from plain strings pick this role up automatically (see
    /// `theme::apply_theme`); use this directly for a `Text` header cell
    /// built by hand, or for header-like text outside a `Table`.
    pub fn table_header(self) -> Self {
        self.bold().role(ThemeRole::TableHeader)
    }

    common_builder_methods!();
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Text::new(value)
    }
}

impl From<String> for Text {
    fn from(value: String) -> Self {
        Text::new(value)
    }
}

// ---------------------------------------------------------------------
// Row / Column
// ---------------------------------------------------------------------

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub children: Vec<Element>,
    pub gap: f32,
    pub align: Align,
    pub common: Common,
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default)]
pub struct Column {
    pub children: Vec<Element>,
    pub gap: f32,
    pub align: Align,
    pub common: Common,
}

macro_rules! container_impl {
    ($ty:ident) => {
        impl $ty {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn child(mut self, child: impl Into<Element>) -> Self {
                self.children.push(child.into());
                self
            }

            pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Element>>) -> Self {
                self.children.extend(children.into_iter().map(Into::into));
                self
            }

            pub fn gap(mut self, gap: f32) -> Self {
                self.gap = gap;
                self
            }

            pub fn align(mut self, align: Align) -> Self {
                self.align = align;
                self
            }

            common_builder_methods!();
        }
    };
}

container_impl!(Row);
container_impl!(Column);

// ---------------------------------------------------------------------
// Spacer
// ---------------------------------------------------------------------

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug)]
pub struct Spacer {
    pub size: f32,
}

impl Spacer {
    pub fn new(size: f32) -> Self {
        Spacer { size }
    }
}

// ---------------------------------------------------------------------
// Line
// ---------------------------------------------------------------------

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug)]
pub struct Line {
    pub thickness: f32,
    pub color: Color,
    pub common: Common,
}

impl Default for Line {
    fn default() -> Self {
        Line {
            thickness: 1.0,
            color: Color::BLACK,
            common: Common::default(),
        }
    }
}

impl Line {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    common_builder_methods!();
}

// ---------------------------------------------------------------------
// Rect
// ---------------------------------------------------------------------

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default)]
pub struct Rect {
    pub common: Common,
}

impl Rect {
    pub fn new() -> Self {
        Self::default()
    }

    common_builder_methods!();
}

// ---------------------------------------------------------------------
// TableOfContents (issue #10)
// ---------------------------------------------------------------------

/// Self-populating from every `Text::outline_level`/`.heading1()`-etc.
/// heading in the document (the same source the PDF bookmark sidebar is
/// built from), with correct page numbers — the two-pass layout already
/// determines those in pass 1, this element just renders them in pass 2
/// (see `lightweight-pdf-layout::toc`). Entries are always left-aligned,
/// one per line, indented by heading depth, with a leader (`.leader()`)
/// filling the gap to a right-hand page number; `.style` controls
/// font/size/color for every entry uniformly.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug)]
pub struct TableOfContents {
    /// Only headings at this `outline_level` or shallower become entries
    /// (default `3`).
    pub max_depth: u8,
    pub style: TextStyle,
    /// Character repeated between an entry's title and its page number
    /// (default `.`); set to `' '` for no visible leader.
    pub leader: char,
    /// Internal: how many matching headings to skip before this
    /// instance's first entry. Set only by the layout crate when a
    /// `TableOfContents` itself splits across a page boundary — always
    /// `0` on one an author constructs.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub skip: usize,
    pub common: Common,
}

impl Default for TableOfContents {
    fn default() -> Self {
        TableOfContents {
            max_depth: 3,
            style: TextStyle::default(),
            leader: '.',
            skip: 0,
            common: Common::default(),
        }
    }
}

impl TableOfContents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_depth(mut self, depth: u8) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn leader(mut self, leader: char) -> Self {
        self.leader = leader;
        self
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    common_builder_methods!();
}

// ---------------------------------------------------------------------
// Element From-impls (ADR/03-builder-api-design.md point 3)
// ---------------------------------------------------------------------

macro_rules! element_from {
    ($ty:ident) => {
        impl From<$ty> for Element {
            fn from(value: $ty) -> Self {
                Element::$ty(value)
            }
        }
    };
}

element_from!(Text);
element_from!(Row);
element_from!(Column);
element_from!(Spacer);
element_from!(Line);
element_from!(Rect);
element_from!(Table);
element_from!(Image);
element_from!(List);
element_from!(TableOfContents);

impl From<&str> for Element {
    fn from(value: &str) -> Self {
        Element::Text(Text::new(value))
    }
}

impl From<String> for Element {
    fn from(value: String) -> Self {
        Element::Text(Text::new(value))
    }
}
