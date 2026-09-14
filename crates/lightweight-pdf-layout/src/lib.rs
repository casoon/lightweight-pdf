//! `Layoutable` trait, pagination and text wrapping for `lightweight-pdf`. Knows
//! `lightweight-pdf-core`'s element types, but nothing about `lightweight-pdf-writer` or fonts
//! beyond the [`FontResolver`] contract (ADR-010).

mod font_resolver;
mod geometry;
#[cfg(feature = "hyphenation")]
mod hyphenate;
mod image;
mod layoutable;
mod list;
mod pagination;
mod render_node;
mod table;
mod text;
mod toc;
mod warnings;

pub use font_resolver::*;
pub use geometry::*;
pub use layoutable::*;
pub use pagination::*;
pub use render_node::*;
pub use text::{text_width_pt, wrap_text, RichLine, StyledWord};
pub use toc::TocHeading;
pub use warnings::*;

use lightweight_pdf_core::{Element, ElementMeasurement, TextMeasurement, TextStyle};

/// Measures a single element (or subtree) against `max_width`.
pub fn measure_element(resolver: &dyn FontResolver, element: &Element, max_width: f32) -> ElementMeasurement {
    let ctx = LayoutCtx::new(resolver);
    let size = measure_at_width(&ctx, element, max_width);
    ElementMeasurement {
        width: size.width,
        height: size.height,
    }
}

/// Measures text wrapped against `max_width`, returning its actual width, total height and line count.
pub fn measure_text(resolver: &dyn FontResolver, text: &str, style: &TextStyle, max_width: f32) -> TextMeasurement {
    let lines = wrap_text(resolver, style, text, max_width);
    let line_count = lines.len();
    let actual_width = lines
        .iter()
        .map(|l| text_width_pt(resolver, style.font, style.size, l))
        .fold(0.0f32, f32::max);
    let lh = style.size * style.line_height;
    let height = line_count as f32 * lh;
    TextMeasurement {
        width: actual_width,
        height,
        lines: line_count,
    }
}
