//! `Layoutable` trait, pagination and text wrapping for `lightweight-pdf`. Knows
//! `lightweight-pdf-core`'s element types, but nothing about `lightweight-pdf-writer` or fonts
//! beyond the [`FontResolver`] contract (ADR-010).

mod font_resolver;
mod geometry;
mod image;
mod layoutable;
mod list;
mod pagination;
mod render_node;
mod table;
mod text;
mod warnings;

pub use font_resolver::*;
pub use geometry::*;
pub use layoutable::*;
pub use pagination::*;
pub use render_node::*;
pub use text::{text_width_pt, wrap_text, RichLine, StyledWord};
pub use warnings::*;
