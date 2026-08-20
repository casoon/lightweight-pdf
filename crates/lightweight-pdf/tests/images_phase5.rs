//! Phase 5 DoD (`plan/phases/phase-5-images.md`): a JPEG logo and an RGBA
//! PNG logo (with real transparency) both embed and render correctly, at
//! their *layout* target size (not their natural pixel size).

mod support;

use lightweight_pdf::*;
use std::sync::Arc;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-fixtures/images/").to_string() + name).expect("test fixture present")
}

#[test]
fn baseline_jpeg_logo_renders_at_its_target_size() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    let logo = Image::new(Arc::<[u8]>::from(fixture("logo_baseline.jpg"))).expect("valid baseline JPEG");
    doc.add(Element::from(logo.width(120.0).height(90.0)));
    doc.add(Text::new("Rechnung mit JPEG-Logo"));

    let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
    assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");

    let (ok, log) = support::qpdf_check(&bytes);
    assert!(ok, "qpdf --check failed:\n{log}");

    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Subtype /Image"));
    assert!(
        text.contains("/Filter /DCTDecode"),
        "JPEG must be embedded byte-for-byte, not re-encoded"
    );
    assert!(
        text.contains("120 0 0 90"),
        "expected the cm matrix to use the 120x90 target size, not the natural 80x60 pixel size"
    );
}

#[cfg(feature = "png")]
mod with_png_feature {
    use super::*;

    #[test]
    fn rgba_png_logo_with_transparency_renders_with_smask() {
        let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
        let logo = Image::new(Arc::<[u8]>::from(fixture("logo_rgba.png"))).expect("valid RGBA PNG");
        doc.add(Element::from(logo.width(64.0).height(48.0)));
        doc.add(Text::new("Rechnung mit PNG-Logo (transparent)"));

        let (bytes, warnings) = doc.render_with_diagnostics().expect("render should succeed");
        assert!(warnings.is_empty(), "unexpected layout warnings: {warnings:?}");

        let (ok, log) = support::qpdf_check(&bytes);
        assert!(ok, "qpdf --check failed:\n{log}");

        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Subtype /Image"));
        assert!(text.contains("/SMask"), "RGBA PNG must produce a separate alpha SMask");
        assert!(text.contains("/ColorSpace /DeviceGray"), "the SMask must be DeviceGray");
        assert!(text.contains("64 0 0 48"), "expected the cm matrix to use the 64x48 target size");
    }

    #[test]
    fn opaque_rgb_png_logo_renders_without_smask() {
        let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
        let logo = Image::new(Arc::<[u8]>::from(fixture("logo_rgb.png"))).expect("valid RGB PNG");
        doc.add(Element::from(logo.width(40.0).height(30.0)));

        let bytes = doc.render().expect("render should succeed");
        let (ok, log) = support::qpdf_check(&bytes);
        assert!(ok, "qpdf --check failed:\n{log}");
        assert!(
            !String::from_utf8_lossy(&bytes).contains("/SMask"),
            "an opaque RGB PNG must not get an alpha SMask"
        );
    }

    #[test]
    fn image_without_explicit_size_scales_down_to_fit_the_page_width() {
        // logo_baseline.jpg is 80x60px; force a very narrow body so Contain
        // must shrink it well below its 96dpi-converted natural size.
        let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(280.0, 56.0));
        let logo = Image::new(Arc::<[u8]>::from(fixture("logo_baseline.jpg"))).expect("valid JPEG");
        doc.add(Element::from(logo));

        let bytes = doc.render().expect("render should succeed");
        let (ok, log) = support::qpdf_check(&bytes);
        assert!(ok, "qpdf --check failed:\n{log}");
    }
}

#[cfg(not(feature = "png"))]
#[test]
fn png_image_fails_render_with_a_clear_error_when_the_png_feature_is_disabled() {
    let mut doc = Document::new(PageFormat::A4).margin(Margin::symmetric(56.0, 56.0));
    let logo = Image::new(Arc::<[u8]>::from(fixture("logo_rgba.png"))).expect("header-only validation doesn't need the png feature");
    doc.add(Element::from(logo.width(64.0).height(48.0)));

    let err = doc.render().expect_err("must fail without a decoder available");
    assert!(matches!(err, RenderError::Image(ImageEmbedError::PngFeatureDisabled)));
}
