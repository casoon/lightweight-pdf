//! Named style roles (`Document::theme(..)`), resolved exactly once —
//! when an element is added via `Document::add()`, walking the whole
//! subtree that was just built. Not a cascade: no parent-to-child
//! inheritance, no re-evaluation at layout/render time, resolution reads
//! from exactly one source (the `Theme` on the `Document` the element is
//! being added to).
//!
//! `Text::role` is what makes an element theme-eligible: `Text::new()`
//! defaults it to `Some(ThemeRole::Body)`, and every style-mutating
//! builder method (`.size()`, `.bold()`, `.color()`, ...) clears it back
//! to `None` — the caller just took over styling manually, so the theme
//! must leave it alone. The `.heading1()`/`.heading2()`/`.heading3()`
//! presets (and the new `.caption()`/`.muted()`/`.table_header()` ones)
//! re-set a specific role *after* their own style-mutating calls, which
//! is why they stay theme-eligible despite calling `.size()`/`.bold()`
//! internally.
//!
//! A `Document` with no `.theme(..)` call resolves nothing — `apply_theme`
//! is only ever invoked when `self.theme` is `Some`, so unthemed output is
//! byte-for-byte what it was before `Theme` existed.

use crate::element::Element;
use crate::style::{Color, FontKey, TextStyle};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeRole {
    Body,
    Caption,
    Heading1,
    Heading2,
    Heading3,
    TableHeader,
    Muted,
}

/// Named `TextStyle` roles. `Theme::default()` reproduces exactly the
/// hardcoded values `TextStyle::default()`/`.heading1()`/`.heading2()`/
/// `.heading3()` already used before `Theme` existed (`caption`/
/// `table_header`/`muted` are new roles with no prior hardcoded
/// equivalent, so they can't break existing output either way).
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub body: TextStyle,
    pub caption: TextStyle,
    pub heading1: TextStyle,
    pub heading2: TextStyle,
    pub heading3: TextStyle,
    pub table_header: TextStyle,
    pub muted: TextStyle,
}

impl Theme {
    pub fn role(&self, role: ThemeRole) -> TextStyle {
        match role {
            ThemeRole::Body => self.body,
            ThemeRole::Caption => self.caption,
            ThemeRole::Heading1 => self.heading1,
            ThemeRole::Heading2 => self.heading2,
            ThemeRole::Heading3 => self.heading3,
            ThemeRole::TableHeader => self.table_header,
            ThemeRole::Muted => self.muted,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        let body = TextStyle::default();
        let muted_color = Color::rgb(0x66, 0x66, 0x66);
        Theme {
            body,
            caption: TextStyle {
                size: 9.0,
                color: muted_color,
                ..body
            },
            heading1: TextStyle {
                size: 24.0,
                font: FontKey::SANS_BOLD,
                ..body
            },
            heading2: TextStyle {
                size: 18.0,
                font: FontKey::SANS_BOLD,
                ..body
            },
            heading3: TextStyle {
                size: 14.0,
                font: FontKey::SANS_BOLD,
                ..body
            },
            table_header: TextStyle {
                font: FontKey::SANS_BOLD,
                ..body
            },
            muted: TextStyle {
                color: muted_color,
                ..body
            },
        }
    }
}

/// Recursively resolves every theme-eligible `Text::role` in `element`
/// (and, for containers, its whole subtree — already fully built by the
/// time `Document::add()` receives it, so one pass is enough) to its
/// `theme` role's style. Table header cells that are still at the
/// default `Body` role (i.e. never explicitly re-tagged) are upgraded to
/// `TableHeader` first — plain `&str` header cells (`Table::header(["A",
/// "B"])`) have no other way to signal "this is a header," and "look
/// like a table header automatically" is the whole point of the role
/// existing.
pub(crate) fn apply_theme(element: &mut Element, theme: &Theme) {
    match element {
        Element::Text(text) => {
            if let Some(role) = text.role {
                // `align` is always independently whatever `.align()` set
                // (or the `TextStyle::default()`/preset value if never
                // called) — not part of role resolution, see `Text::align`.
                let align = text.style.align;
                text.style = TextStyle { align, ..theme.role(role) };
            }
        }
        Element::Row(row) => {
            for child in &mut row.children {
                apply_theme(child, theme);
            }
        }
        Element::Column(col) => {
            for child in &mut col.children {
                apply_theme(child, theme);
            }
        }
        Element::Table(table) => {
            if let Some(header) = &mut table.header {
                for cell in header {
                    if let Element::Text(text) = &mut cell.element {
                        if text.role == Some(ThemeRole::Body) {
                            text.role = Some(ThemeRole::TableHeader);
                        }
                    }
                    apply_theme(&mut cell.element, theme);
                }
            }
            for row in &mut table.rows {
                for cell in row {
                    apply_theme(&mut cell.element, theme);
                }
            }
        }
        Element::List(list) => {
            for item in &mut list.items {
                apply_theme(&mut item.content, theme);
            }
        }
        Element::Spacer(_) | Element::Line(_) | Element::Rect(_) | Element::Image(_) | Element::PageBreak => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, PageFormat};
    use crate::element::Text;
    use crate::table::{Table, TableColumn};

    fn test_theme() -> Theme {
        let mut theme = Theme::default();
        theme.body.size = 30.0;
        theme.heading1.size = 99.0;
        theme.table_header.size = 55.0;
        theme
    }

    #[test]
    fn plain_text_resolves_from_the_theme() {
        let mut doc = Document::new(PageFormat::A4).theme(test_theme());
        doc.add(Text::new("hello"));
        let Element::Text(t) = &doc.children[0] else {
            panic!("expected Text")
        };
        assert_eq!(t.style.size, 30.0, "untouched Text should pick up theme.body");
    }

    #[test]
    fn explicitly_styled_text_is_not_overridden() {
        let mut doc = Document::new(PageFormat::A4).theme(test_theme());
        doc.add(Text::new("hello").size(12.5));
        let Element::Text(t) = &doc.children[0] else {
            panic!("expected Text")
        };
        assert_eq!(t.style.size, 12.5, "explicit .size() must survive theming");
    }

    #[test]
    fn heading_preset_resolves_from_its_own_theme_role() {
        let mut doc = Document::new(PageFormat::A4).theme(test_theme());
        doc.add(Text::new("Chapter").heading1());
        let Element::Text(t) = &doc.children[0] else {
            panic!("expected Text")
        };
        assert_eq!(t.style.size, 99.0, ".heading1() should pick up theme.heading1, not theme.body");
    }

    #[test]
    fn no_theme_leaves_text_at_its_own_defaults() {
        let mut doc = Document::new(PageFormat::A4);
        doc.add(Text::new("hello"));
        let Element::Text(t) = &doc.children[0] else {
            panic!("expected Text")
        };
        assert_eq!(t.style, TextStyle::default(), "no .theme(..) call must mean unchanged output");
    }

    #[test]
    fn align_after_a_preset_survives_theming_without_losing_the_role() {
        // A common real pattern (see examples/report.rs): centering a
        // heading. `.align()` must neither lose the Heading1 role nor
        // have its own value clobbered by the theme's role resolution.
        let mut doc = Document::new(PageFormat::A4).theme(test_theme());
        doc.add(Text::new("Chapter").heading1().align(crate::style::Align::Center));
        let Element::Text(t) = &doc.children[0] else {
            panic!("expected Text")
        };
        assert_eq!(t.style.size, 99.0, "still themed as Heading1 despite the trailing .align() call");
        assert_eq!(
            t.style.align,
            crate::style::Align::Center,
            ".align() must not be overwritten by the theme's role"
        );
    }

    #[test]
    fn plain_string_table_header_cells_theme_as_table_header_automatically() {
        let mut doc = Document::new(PageFormat::A4).theme(test_theme());
        doc.add(Table::new().columns([TableColumn::fixed(50.0)]).header(["Spalte"]));
        let Element::Table(table) = &doc.children[0] else {
            panic!("expected Table")
        };
        let header = table.header.as_ref().unwrap();
        let Element::Text(t) = &header[0].element else {
            panic!("expected Text")
        };
        assert_eq!(
            t.style.size, 55.0,
            "a plain-string table header cell should theme as TableHeader, not Body"
        );
    }
}
