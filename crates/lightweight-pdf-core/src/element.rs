//! Element catalog (V1 subset through concept Phase 5, see
//! `plan/02-elementcatalog-and-features.md`).

use crate::image::Image;
use crate::list::List;
use crate::style::{Align, Border, Color, Common, FontKey, Overflow, TextStyle};
use crate::table::Table;

/// One element in the document tree. Enum-based (not `Box<dyn Layoutable>`)
/// — a closed, small set of primitives.
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

#[derive(Clone, Debug, Default)]
pub struct Text {
    pub content: String,
    pub style: TextStyle,
    pub url: Option<String>,
    pub common: Common,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Text {
            content: content.into(),
            style: TextStyle::default(),
            url: None,
            common: Common::default(),
        }
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.style.size = size;
        self
    }

    pub fn bold(mut self) -> Self {
        self.style.font = FontKey::SANS_BOLD;
        self
    }

    pub fn italic(mut self) -> Self {
        self.style.font = FontKey::SANS_ITALIC;
        self
    }

    pub fn bold_italic(mut self) -> Self {
        self.style.font = FontKey::SANS_BOLD_ITALIC;
        self
    }

    pub fn font(mut self, font: FontKey) -> Self {
        self.style.font = font;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.style.align = align;
        self
    }

    pub fn line_height(mut self, line_height: f32) -> Self {
        self.style.line_height = line_height;
        self
    }

    /// Heading presets (Phase 6, `plan/02-elementcatalog-and-features.md`):
    /// thin wrappers over `.size()`/`.bold()`, additionally setting
    /// `keep_with_next` so a heading never ends up alone at the bottom of
    /// a page without its following content
    /// (`plan/05-overflow-and-robustness.md` Grundprinzip 9).
    pub fn heading1(self) -> Self {
        self.size(24.0).bold().keep_with_next()
    }

    pub fn heading2(self) -> Self {
        self.size(18.0).bold().keep_with_next()
    }

    pub fn heading3(self) -> Self {
        self.size(14.0).bold().keep_with_next()
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

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub children: Vec<Element>,
    pub gap: f32,
    pub align: Align,
    pub common: Common,
}

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
