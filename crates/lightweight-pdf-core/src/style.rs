//! Flat style properties shared across elements (no cascading, see
//! `plan/03-builder-api-design.md`).

/// Opaque handle for a font. `lightweight-pdf-core` never sees font bytes, only this
/// key (see `plan/00a-contracts-and-artifacts.md`, point 3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FontKey(pub &'static str);

impl FontKey {
    pub const SANS_REGULAR: FontKey = FontKey("sans-regular");
    pub const SANS_BOLD: FontKey = FontKey("sans-bold");
    pub const SANS_ITALIC: FontKey = FontKey("sans-italic");
    pub const SANS_BOLD_ITALIC: FontKey = FontKey("sans-bold-italic");

    pub const fn custom(name: &'static str) -> Self {
        FontKey(name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// Overflow policy for explicitly, fixed-size elements. See
/// `plan/05-overflow-and-robustness.md`, Grundprinzip 3. `Visible` is
/// intentionally not part of V1 (ADR-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Overflow {
    /// Default: clip hard at the element's box.
    #[default]
    Clip,
    /// Single-line text only: truncate with a trailing "…".
    Ellipsis,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub const WHITE: Color = Color(255, 255, 255);

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(r, g, b)
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed {
        dash: f32,
        gap: f32,
    },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Border {
    pub width: f32,
    pub color: Color,
    pub style: BorderStyle,
}

impl Border {
    pub fn solid(width: f32, color: Color) -> Self {
        Border {
            width,
            color,
            style: BorderStyle::Solid,
        }
    }

    pub fn dashed(width: f32, color: Color, dash: f32, gap: f32) -> Self {
        Border {
            width,
            color,
            style: BorderStyle::Dashed { dash, gap },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub font: FontKey,
    pub size: f32,
    pub color: Color,
    pub align: Align,
    /// Multiple of `size`, e.g. 1.2 for 20% leading.
    pub line_height: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            font: FontKey::SANS_REGULAR,
            size: 12.0,
            color: Color::BLACK,
            align: Align::Start,
            line_height: 1.2,
        }
    }
}

/// Properties shared by every container/block element: `padding, width,
/// height, overflow, background, border`, plus
/// `flex` (taffy-vocabulary, ADR-004) and `keep_with_next` (ADR-007 /
/// Grundprinzip 9). Deliberately no `margin` on elements (ADR/03: only
/// `padding` + `Row`/`Column` `gap`, margin is a `Document`-level property).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Common {
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Growth factor along the parent's main axis; `None` means "auto",
    /// i.e. sized from measured content. Only has an effect when the
    /// parent's main-axis size is bounded (see lightweight-pdf-layout Row/Column).
    pub flex: Option<f32>,
    pub padding: f32,
    pub corner_radius: f32,
    pub overflow: Overflow,
    pub background: Option<Color>,
    pub border: Option<Border>,
    /// See `plan/05-overflow-and-robustness.md` Grundprinzip 9 / ADR-007:
    /// only placed if the immediately following sibling also still fits.
    pub keep_with_next: bool,
}
