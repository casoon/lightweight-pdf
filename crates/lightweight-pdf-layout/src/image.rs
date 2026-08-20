//! `Image` layout (Phase 5, `plan/phases/phase-5-images.md` step 1):
//! constraint-based scaling with `Contain` as the only/default fit — the
//! image is scaled proportionally to fit entirely inside its target box,
//! never cropped, never stretched, never drawn past the box
//! (`05-overflow-and-robustness.md`'s element table: "Default `Contain`-Fit").

use crate::geometry::{Constraints, Rect, Size};
use crate::layoutable::{LayoutCtx, LayoutResult, Layoutable};
use crate::render_node::RenderNode;
use crate::warnings::LayoutWarning;
use lightweight_pdf_core::Image;

/// Assumed density when neither explicit `.width()`/`.height()` is set and
/// the image carries no density metadata (V1 doesn't parse JFIF/`pHYs`
/// density — a documented simplification, see `plan/progress.md`): 96 CSS
/// pixels per inch, the common web/document default.
const PX_TO_PT: f32 = 72.0 / 96.0;

fn natural_size_pt(image: &Image) -> (f32, f32) {
    (image.width_px as f32 * PX_TO_PT, image.height_px as f32 * PX_TO_PT)
}

/// Resolves the actual drawn size for a given bound. `Contain`: scale the
/// natural aspect ratio down (or up) to fit entirely within the bound,
/// never exceeding it on either axis.
fn contain_size(image: &Image, bound_w: f32, bound_h: f32) -> (f32, f32) {
    let (nw, nh) = natural_size_pt(image);
    if nw <= 0.0 || nh <= 0.0 {
        return (0.0, 0.0);
    }
    match (image.common.width, image.common.height) {
        (Some(w), Some(h)) => {
            let scale = (w / nw).min(h / nh);
            (nw * scale, nh * scale)
        }
        (Some(w), None) => (w, nh * (w / nw)),
        (None, Some(h)) => (nw * (h / nh), h),
        (None, None) => {
            if bound_w.is_finite() && nw > bound_w {
                let scale = (bound_w / nw).min(if bound_h.is_finite() { bound_h / nh } else { f32::INFINITY });
                (nw * scale, nh * scale)
            } else if bound_h.is_finite() && nh > bound_h {
                let scale = bound_h / nh;
                (nw * scale, nh * scale)
            } else {
                (nw, nh)
            }
        }
    }
}

impl Layoutable for Image {
    fn measure(&self, _ctx: &LayoutCtx, constraints: Constraints) -> Size {
        let (w, h) = contain_size(self, constraints.max_width, constraints.max_height);
        Size { width: w, height: h }
    }

    fn layout(&self, _ctx: &LayoutCtx, area: Rect, _warnings: &mut Vec<LayoutWarning>, _page: usize) -> LayoutResult {
        let (w, h) = contain_size(self, area.width, area.height);
        let x = area.x + ((area.width - w).max(0.0)) / 2.0;
        let y = area.y + ((area.height - h).max(0.0)) / 2.0;
        let node = RenderNode::Image {
            area: Rect { x, y, width: w, height: h },
            bytes: self.bytes.clone(),
            format: self.format,
            width_px: self.width_px,
            height_px: self.height_px,
            components: self.components,
        };
        LayoutResult::Fit(RenderNode::clipped(area, node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_resolver::{FontMetrics, FontResolver};
    use lightweight_pdf_core::FontKey;

    struct NoopMetrics;
    impl FontMetrics for NoopMetrics {
        fn advance(&self, _ch: char) -> f32 {
            500.0
        }
        fn ascent(&self) -> f32 {
            800.0
        }
        fn descent(&self) -> f32 {
            -200.0
        }
    }
    struct NoopResolver;
    impl FontResolver for NoopResolver {
        fn metrics(&self, _key: FontKey) -> &dyn FontMetrics {
            &NoopMetrics
        }
    }
    fn ctx() -> LayoutCtx<'static> {
        LayoutCtx { resolver: &NoopResolver }
    }

    fn image(width_px: u32, height_px: u32) -> Image {
        // A 1x1 minimal PNG is enough to construct a valid `Image` for
        // layout math tests — this module never touches pixel data.
        let png: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
            0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
            0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
        ];
        let mut img = Image::new(png.to_vec()).expect("valid minimal PNG");
        img.width_px = width_px;
        img.height_px = height_px;
        img
    }

    #[test]
    fn fills_target_box_when_both_dimensions_are_explicit_and_aspect_matches() {
        let img = image(100, 100).width(50.0).height(50.0);
        let c = ctx();
        let size = img.measure(
            &c,
            Constraints {
                max_width: 500.0,
                max_height: 500.0,
            },
        );
        assert_eq!((size.width, size.height), (50.0, 50.0));
    }

    #[test]
    fn contain_leaves_slack_on_the_shorter_axis_when_aspect_does_not_match() {
        // 2:1 natural aspect into a 1:1 box -> width-limited, height gets slack.
        let img = image(200, 100).width(50.0).height(50.0);
        let c = ctx();
        let size = img.measure(
            &c,
            Constraints {
                max_width: 500.0,
                max_height: 500.0,
            },
        );
        assert_eq!(size.width, 50.0);
        assert_eq!(size.height, 25.0, "must preserve aspect ratio, not stretch to fill the box");
    }

    #[test]
    fn single_dimension_preserves_aspect_exactly() {
        let img = image(200, 100).width(80.0);
        let c = ctx();
        let size = img.measure(
            &c,
            Constraints {
                max_width: 500.0,
                max_height: 500.0,
            },
        );
        assert_eq!(size.width, 80.0);
        assert!((size.height - 40.0).abs() < 0.01);
    }

    #[test]
    fn shrinks_to_fit_available_width_when_natural_size_overflows() {
        // Natural width at 96 DPI: 2000px * 0.75 = 1500pt, way over a
        // typical page's available width.
        let img = image(2000, 1000);
        let c = ctx();
        let size = img.measure(
            &c,
            Constraints {
                max_width: 300.0,
                max_height: f32::INFINITY,
            },
        );
        assert!(size.width <= 300.0 + 0.01);
        assert!(
            (size.width / size.height - 2.0).abs() < 0.01,
            "aspect ratio must be preserved while shrinking"
        );
    }

    #[test]
    fn never_exceeds_an_explicit_box_even_when_natural_size_is_smaller() {
        // object-fit: contain also scales *up* to fill the box, matching
        // "proportional eingepasst" wording (not "never upscale").
        let img = image(10, 10).width(200.0).height(200.0);
        let c = ctx();
        let size = img.measure(
            &c,
            Constraints {
                max_width: 500.0,
                max_height: 500.0,
            },
        );
        assert_eq!((size.width, size.height), (200.0, 200.0));
    }

    #[test]
    fn layout_centers_the_contained_image_within_a_larger_box() {
        let img = image(200, 100).width(50.0).height(50.0); // -> 50x25, centered in a 50x50 box
        let c = ctx();
        let mut warnings = Vec::new();
        let area = Rect {
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 50.0,
        };
        let LayoutResult::Fit(RenderNode::Group { children, .. }) = img.layout(&c, area, &mut warnings, 1) else {
            panic!("expected Fit Group (clip wrapper)");
        };
        let RenderNode::Image { area: img_area, .. } = &children[0] else {
            panic!("expected Image node");
        };
        assert_eq!(img_area.width, 50.0);
        assert_eq!(img_area.height, 25.0);
        assert_eq!(
            img_area.y,
            20.0 + (50.0 - 25.0) / 2.0,
            "must be vertically centered in the slack axis"
        );
    }
}
