//! Flat style properties shared across elements (no cascading, see
//! `plan/03-builder-api-design.md`).

/// Opaque handle for a font. `lightweight-pdf-core` never sees font bytes, only this
/// key (see `plan/00a-contracts-and-artifacts.md`, point 3).
///
/// `serde` (issue #17): deserializes from a plain JSON string. Since the
/// wrapped `&'static str` can't borrow from a transient JSON buffer, each
/// distinct name deserialized is leaked (`Box::leak`) to get a `'static`
/// reference — one small, permanent allocation per distinct font-key name
/// ever seen in a document, acceptable for "parse a document, render it,
/// done" but not for a long-running process parsing unbounded distinct
/// names in a hot loop.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
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

#[cfg(feature = "serde")]
impl serde::Serialize for FontKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FontKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        Ok(FontKey(Box::leak(name.into_boxed_str())))
    }
}

/// Hand-written to match the custom `Serialize`/`Deserialize` above
/// (`#[derive(JsonSchema)]` only works from a real derive, not a custom
/// serde impl).
#[cfg(feature = "schemars")]
impl schemars::JsonSchema for FontKey {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FontKey".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// Text only (`TextStyle::align`): word gaps stretch so every line but
    /// the last of a paragraph is flush with both edges. Meaningless for
    /// block/container alignment (`Row`/`Column`/`TableColumn`), which
    /// treats it the same as `Start`.
    Justify,
}

/// Overflow policy for explicitly, fixed-size elements. See
/// `plan/05-overflow-and-robustness.md`, Grundprinzip 3. `Visible` is
/// intentionally not part of V1 (ADR-011).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Overflow {
    /// Default: clip hard at the element's box.
    #[default]
    Clip,
    /// Single-line text only: truncate with a trailing "…".
    Ellipsis,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed {
        dash: f32,
        gap: f32,
    },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields, default)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
