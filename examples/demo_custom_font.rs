//! Example: rendering with a caller-supplied font instead of the bundled
//! Source Sans 3 default — demonstrates `FontRegistry::with_fonts()` /
//! `Document::render_with_fonts()` (see `crates/lightweight-pdf/src/fonts.rs`;
//! tracking issue for the broader arbitrary-weight/arbitrary-`FontKey`
//! scope: <https://github.com/casoon/lightweight-pdf/issues/1>). Unlike
//! every other `demo_*` example, this one works with `--no-default-features`
//! too — it never touches the bundled Source Sans 3 assets.
//!
//! The "custom" font here is Source Serif 4 Regular, already vendored as a
//! test fixture (`crates/lightweight-pdf-fonts/tests/fixtures/`, SIL OFL
//! 1.1) so this demo doesn't need a new font asset of its own. It's reused
//! for both the regular and bold slot since only one weight is vendored —
//! this demo intentionally avoids `.bold()` so the unused "bold" entry
//! (same glyphs, not visually bold) is never actually embedded in the
//! output (only referenced weights get embedded, see `render.rs`).
//!
//! Run: `cargo run -p lightweight-pdf --example demo_custom_font`
//! Also works: `cargo run -p lightweight-pdf --example demo_custom_font --no-default-features`

use lightweight_pdf::*;

const CUSTOM_FONT: &[u8] = include_bytes!("../crates/lightweight-pdf-fonts/tests/fixtures/custom-test-font.ttf");

fn main() {
    let fonts = FontRegistry::with_fonts(CUSTOM_FONT, CUSTOM_FONT).expect("valid demo font (Source Serif 4)");

    let mut doc = Document::new(PageFormat::A4).margin(Margin::all(20.0 * 72.0 / 25.4));

    doc.add(Text::new("Custom Font Demo").heading1());
    doc.add(Spacer::new(10.0));
    doc.add(Text::new(
        "This document is rendered with a caller-supplied font (Source Serif 4) instead of \
         the bundled Source Sans 3 default, via FontRegistry::with_fonts() and \
         Document::render_with_fonts() \u{2014} no default-fonts feature required.",
    ));
    doc.add(Spacer::new(14.0));
    doc.add(Text::new(
        "Any static TrueType font with glyf outlines works here (see FontData::load / ADR-012 \
         for the exact constraints) \u{2014} variable fonts and CFF/OTF fonts are rejected.",
    ));
    doc.add(Spacer::new(14.0));
    doc.add(
        Text::new("Sample document \u{2014} demonstrates the API, not a real document.")
            .size(9.0)
            .color(Color::rgb(0x88, 0x88, 0x88)),
    );

    let bytes = doc.render_with_fonts(&fonts).expect("render should succeed");
    std::fs::write("examples/demo_custom_font.pdf", &bytes).expect("write examples/demo_custom_font.pdf");
    println!("wrote examples/demo_custom_font.pdf ({} bytes)", bytes.len());
}
