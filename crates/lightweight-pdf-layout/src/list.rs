//! `List` layout (Phase 6): sugar over `Row`/`Column` — a fixed-width
//! marker column beside each item's content, stacked vertically. Reuses
//! Row/Column's existing measure/layout entirely rather than duplicating
//! that logic; see `lightweight_pdf_core::list` for why `List` is still its own
//! element (builder ergonomics) despite delegating like this.

use crate::geometry::{Constraints, Rect, Size};
use crate::layoutable::{coerce_to_fit_and_warn, LayoutCtx, LayoutResult, Layoutable};
use crate::warnings::LayoutWarning;
use lightweight_pdf_core::{Align, Column, List, Marker, Row, Text};

fn marker_text(marker: &Marker) -> String {
    match marker {
        Marker::Bullet => "\u{2022}".to_string(),
        Marker::Number(n) => format!("{n}."),
    }
}

/// Translates a `List` into an equivalent `Column` of `Row`s (marker +
/// content). Item content that has no explicit width gets `flex(1.0)` so
/// it fills the remaining row width instead of shrink-wrapping to its own
/// natural size (`Element::common_mut`, added for exactly this).
fn to_column(list: &List) -> Column {
    let mut col = Column::new().gap(list.gap);
    for item in &list.items {
        let mut content = item.content.clone();
        if let Some(common) = content.common_mut() {
            if common.width.is_none() && common.flex.is_none() {
                common.flex = Some(1.0);
            }
        }
        let row = Row::new()
            .gap(list.gap)
            .child(Text::new(marker_text(&item.marker)).width(list.marker_width).align(Align::End))
            .child(content);
        col = col.child(row);
    }
    col.common = list.common;
    col
}

impl Layoutable for List {
    fn measure(&self, ctx: &LayoutCtx, constraints: Constraints) -> Size {
        to_column(self).measure(ctx, constraints)
    }

    fn layout(&self, ctx: &LayoutCtx, area: Rect, warnings: &mut Vec<LayoutWarning>, page: usize) -> LayoutResult {
        // Lists don't paginate in V1 (same "atomic, no Split" contract as
        // Row) — if the translated Column would need to split, keep what
        // fits and clip+warn on the rest instead of silently dropping it.
        let result = to_column(self).layout(ctx, area, warnings, page);
        LayoutResult::Fit(coerce_to_fit_and_warn(
            result,
            warnings,
            page,
            "List content exceeds available space",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_resolver::{FontMetrics, FontResolver};
    use crate::render_node::RenderNode;
    use lightweight_pdf_core::{FontKey, Text as TextEl};

    struct FixedMetrics;
    impl FontMetrics for FixedMetrics {
        fn advance(&self, ch: char) -> f32 {
            if ch == ' ' {
                300.0
            } else {
                600.0
            }
        }
        fn ascent(&self) -> f32 {
            800.0
        }
        fn descent(&self) -> f32 {
            -200.0
        }
    }
    struct FixedResolver;
    impl FontResolver for FixedResolver {
        fn metrics(&self, _key: FontKey) -> &dyn FontMetrics {
            &FixedMetrics
        }
    }
    fn ctx() -> LayoutCtx<'static> {
        LayoutCtx::new(&FixedResolver)
    }

    #[test]
    fn bullet_and_numbered_items_get_distinct_markers() {
        let list = List::new()
            .bullet(TextEl::new("Erstens"))
            .numbered(TextEl::new("Zweitens"))
            .numbered(TextEl::new("Drittens"));
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 300.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: rows, .. }) = list.layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        assert_eq!(rows.len(), 3);

        fn marker_of(row: &RenderNode) -> String {
            let RenderNode::Group { children: cells, .. } = row else {
                panic!("expected row group")
            };
            let RenderNode::Group { children: wrap, .. } = &cells[0] else {
                panic!("expected clip wrapper")
            };
            let RenderNode::TextLines { lines, .. } = &wrap[0] else {
                panic!("expected TextLines")
            };
            lines.join("")
        }
        assert_eq!(marker_of(&rows[0]), "\u{2022}");
        assert_eq!(marker_of(&rows[1]), "1.");
        assert_eq!(marker_of(&rows[2]), "2.");
    }

    #[test]
    fn item_content_fills_remaining_row_width() {
        let list = List::new().bullet(TextEl::new("x"));
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children: rows, .. }) = list.layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit");
        };
        let RenderNode::Group { children: cells, .. } = &rows[0] else {
            panic!("expected row group")
        };
        let RenderNode::Group { area: content_area, .. } = &cells[1] else {
            panic!("expected clip wrapper for content cell")
        };
        // marker_width (16) + row gap (6) = 22; content should claim the
        // rest of the 200pt row, not shrink to "x"'s own tiny width.
        assert!(
            content_area.width > 150.0,
            "expected content to fill remaining width, got {}",
            content_area.width
        );
    }
}
