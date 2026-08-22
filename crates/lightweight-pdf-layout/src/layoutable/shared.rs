//! Small helpers shared across this crate's `Layoutable` impls (`Row`,
//! `Column`, `Table`, ...) — the same handful of box-model computations
//! and the `LayoutWarning` construction otherwise repeat almost verbatim
//! at every call site.

use super::{LayoutCtx, LayoutResult, Layoutable};
use crate::geometry::{Constraints, Rect, Size};
use crate::render_node::RenderNode;
use crate::warnings::{LayoutWarning, LayoutWarningKind};
use lightweight_pdf_core::{Common, Element, TextStyle};

pub(crate) const EPS: f32 = 0.01;

pub(super) fn line_height_pt(style: &TextStyle) -> f32 {
    style.size * style.line_height
}

/// Builds and pushes a [`LayoutWarning`]; the `{ kind, page, element_hint }`
/// literal otherwise repeats at every clip/overflow site in this crate.
pub(crate) fn push_warning(warnings: &mut Vec<LayoutWarning>, kind: LayoutWarningKind, page: usize, element_hint: impl Into<String>) {
    warnings.push(LayoutWarning {
        kind,
        page,
        element_hint: element_hint.into(),
    });
}

/// Measures `child` at a fixed width with an unconstrained height — the
/// standard "how tall would this be at this width" query used throughout
/// `Row`/`Column`/`Table` layout.
pub(crate) fn measure_at_width(ctx: &LayoutCtx, child: &Element, width: f32) -> Size {
    child.measure(
        ctx,
        Constraints {
            max_width: width,
            max_height: f32::INFINITY,
        },
    )
}

/// Resolves an explicit-or-fallback outer bound (`width`/`height`) plus
/// the padding-shrunk inner size, for elements that only carry a scalar
/// bound (not a full `Rect`) at `measure` time.
pub(crate) fn resolve_bound(explicit: Option<f32>, max: f32, padding: f32) -> (f32, f32) {
    let bound = explicit.unwrap_or(max);
    let inner = (bound - 2.0 * padding).max(0.0);
    (bound, inner)
}

/// The common "auto size grows with content" fallback: an explicit
/// `.height()`/`.width()` wins, otherwise `content + 2 * padding`.
pub(crate) fn resolve_auto_size(explicit: Option<f32>, content: f32, padding: f32) -> f32 {
    explicit.unwrap_or(content + 2.0 * padding)
}

/// Clamps an explicit fixed height down by padding for the inner content
/// budget, or falls back to the already-inner `default` (used by
/// `Column`/`Table` layout, which start from a padding-shrunk `Rect`).
fn resolve_bound_height(explicit: Option<f32>, padding: f32, default: f32) -> f32 {
    explicit.map(|h| h - 2.0 * padding).unwrap_or(default)
}

/// Shrinks `area` by `padding` and resolves the bound height in one step —
/// the `Column`/`Table::layout` entry sequence (`area.shrink(padding)` then
/// `resolve_bound_height`) otherwise repeats verbatim at both call sites.
pub(crate) fn shrink_and_bound_height(area: Rect, height: Option<f32>, padding: f32) -> (Rect, f32) {
    let inner = area.shrink(padding);
    let bound_height = resolve_bound_height(height, padding, inner.height);
    (inner, bound_height)
}

/// `Size { width, height }` from `Common`'s explicit overrides, falling
/// back to `constraints.max_width` for width and `default_height` for
/// height — shared by leaf elements (`Line`, `Rect`) whose natural size
/// has no content of its own to measure.
pub(super) fn size_with_defaults(common: &Common, constraints: Constraints, default_height: f32) -> Size {
    Size {
        width: common.width.unwrap_or(constraints.max_width),
        height: common.height.unwrap_or(default_height),
    }
}

/// Wraps placed `children` into the clipped `RenderNode::Group` that
/// `Row`/`Column`/`Table::layout` all build as their result: same `area`
/// as the incoming box except for `outer_height`, using the container's
/// own background/border. Also used by `finish_page`/`Table::layout` for
/// the `current` half of a `Split`.
pub(crate) fn wrap_children(area: Rect, outer_height: f32, common: &Common, children: Vec<RenderNode>) -> RenderNode {
    RenderNode::Group {
        area: Rect {
            height: outer_height,
            ..area
        },
        clip: true,
        background: common.background,
        border: common.border,
        children,
    }
}

/// `Row`/`Table::layout`'s shared final step once all children are
/// placed: auto-size the outer height from `content_extent` (the
/// tallest/summed child extent) and wrap `children` into a `Fit`. `Column`
/// doesn't use this — its outer height follows a different, pagination-
/// aware formula (see its own `layout`).
pub(crate) fn finish_fit(common: &Common, area: Rect, content_extent: f32, children: Vec<RenderNode>) -> LayoutResult {
    let outer_height = resolve_auto_size(common.height, content_extent, common.padding);
    LayoutResult::Fit(wrap_children(area, outer_height, common, children))
}

/// Coerces a child's `LayoutResult` into a plain `RenderNode`, for callers
/// that don't propagate `Split` themselves: keep whatever fit, discard the
/// remainder. Used by `Row`'s per-child placement (no Row-level Split in
/// V1) as well as `List::layout` and pagination's header/footer band,
/// which both translate/delegate into a `Layoutable` that can split but
/// must present as atomic to their own caller.
pub(crate) fn coerce_to_fit(result: LayoutResult) -> RenderNode {
    match result {
        LayoutResult::Fit(node) => node,
        LayoutResult::Split { current, .. } => current,
    }
}

/// `coerce_to_fit`, plus a `ContentOverflow` warning when `result` actually
/// split — the common case at `Row`'s per-child placement and
/// `List::layout`, both of which can only warn-and-clip, never propagate a
/// `Split` of their own.
pub(crate) fn coerce_to_fit_and_warn(result: LayoutResult, warnings: &mut Vec<LayoutWarning>, page: usize, hint: &str) -> RenderNode {
    if matches!(result, LayoutResult::Split { .. }) {
        push_warning(warnings, LayoutWarningKind::ContentOverflow, page, hint);
    }
    coerce_to_fit(result)
}

/// Shared "fixed-size container clips instead of splitting" behavior
/// (Grundprinzip 3): when a `Column`/`Table` carries an explicit
/// `.height()`, content that doesn't fit is clipped and warned about right
/// here rather than producing a `Split` — used by `finish_page` (Column)
/// and `Table::layout`. `overflow_hint` is `Some` (and a warning pushed)
/// only when there's actually leftover content being clipped away.
pub(crate) fn clip_to_fixed_height(
    area: Rect,
    fixed_height: f32,
    common: &Common,
    rendered: Vec<RenderNode>,
    warnings: &mut Vec<LayoutWarning>,
    page: usize,
    overflow_hint: Option<&str>,
) -> LayoutResult {
    if let Some(hint) = overflow_hint {
        push_warning(warnings, LayoutWarningKind::ContentOverflow, page, hint);
    }
    LayoutResult::Fit(wrap_children(area, fixed_height, common, rendered))
}
