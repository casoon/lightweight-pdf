//! `List` element (Phase 6, `plan/02-elementcatalog-and-features.md`):
//! bullet/numbered items, one flat level, no nesting. Layout-wise it's
//! sugar over `Row`/`Column` (a fixed-width marker column beside the
//! content) — see `lightweight-pdf-layout`'s `list` module — but exists as its own
//! element so callers don't have to hand-build that boilerplate per item.

use crate::element::Element;
use crate::style::Common;

#[derive(Clone, Debug)]
pub enum Marker {
    Bullet,
    /// Explicit, caller-assigned number (not auto-renumbered on removal —
    /// `.numbered()` assigns these sequentially as items are added).
    Number(u32),
}

#[derive(Clone, Debug)]
pub struct ListItem {
    pub marker: Marker,
    pub content: Element,
}

#[derive(Clone, Debug)]
pub struct List {
    pub items: Vec<ListItem>,
    pub marker_width: f32,
    pub gap: f32,
    next_number: u32,
    pub common: Common,
}

impl Default for List {
    fn default() -> Self {
        List {
            items: Vec::new(),
            marker_width: 16.0,
            gap: 6.0,
            next_number: 1,
            common: Common::default(),
        }
    }
}

impl List {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bullet(mut self, content: impl Into<Element>) -> Self {
        self.items.push(ListItem {
            marker: Marker::Bullet,
            content: content.into(),
        });
        self
    }

    /// Appends a numbered item, auto-incrementing from 1.
    pub fn numbered(mut self, content: impl Into<Element>) -> Self {
        let n = self.next_number;
        self.next_number += 1;
        self.items.push(ListItem {
            marker: Marker::Number(n),
            content: content.into(),
        });
        self
    }

    pub fn marker_width(mut self, width: f32) -> Self {
        self.marker_width = width;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

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

    pub fn keep_with_next(mut self) -> Self {
        self.common.keep_with_next = true;
        self
    }
}
