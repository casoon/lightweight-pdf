//! Document model, elements and builder API for `lightweight-pdf`. No font bytes, no
//! PDF types (`plan/00a-contracts-and-artifacts.md`, point 3) — this crate
//! knows nothing but the document tree, styling and `FontKey`.

mod currency;
mod document;
mod element;
mod image;
mod list;
mod style;
mod table;
mod theme;
mod watermark;

pub use currency::*;
pub use document::*;
pub use element::*;
pub use image::*;
pub use list::*;
pub use style::*;
pub use table::*;
pub use theme::{Theme, ThemeRole};
pub use watermark::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_minimal_document() {
        let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(20.0, 18.0));
        doc.add(
            Row::new()
                .gap(8.0)
                .child(Text::new("Rechnung").size(22.0))
                .child(Text::new("RE-2026-0042")),
        );
        assert_eq!(doc.children.len(), 1);
        match &doc.children[0] {
            Element::Row(row) => assert_eq!(row.children.len(), 2),
            _ => panic!("expected Row"),
        }
    }

    #[test]
    fn str_converts_into_text_element() {
        let el: Element = "Hallo".into();
        match el {
            Element::Text(t) => assert_eq!(t.content, "Hallo"),
            _ => panic!("expected Text"),
        }
    }
}
