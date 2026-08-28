use lightweight_pdf_core::FontKey;

/// Per-font metrics, normalized to 1000 units-per-em (PDF/Type1
/// convention). `lightweight-pdf-layout` never sees font bytes — only this.
pub trait FontMetrics {
    /// Advance width for one character, in 1/1000 em. Implementors must
    /// return *some* usable width even for unrepresentable characters
    /// (e.g. a notdef/fallback width) so wrapping code never has to
    /// special-case "no metric available".
    fn advance(&self, ch: char) -> f32;
    fn ascent(&self) -> f32;
    fn descent(&self) -> f32;

    /// Whether the font has an actual glyph for `ch` (as opposed to
    /// `advance`'s notdef fallback width). Defaults to `true` so existing
    /// implementors (test doubles, anything that doesn't track glyph
    /// coverage) don't need to change; `lightweight-pdf`'s embedded-font
    /// adapter overrides this to drive `LayoutWarningKind::MissingGlyph`.
    fn has_glyph(&self, _ch: char) -> bool {
        true
    }
}

/// `lightweight-pdf-layout`'s only dependency on fonts: given a [`FontKey`], hand
/// back metrics. Implemented by the facade crate, which bridges to
/// `lightweight-pdf-fonts::FontData` (ADR-010).
pub trait FontResolver {
    fn metrics(&self, key: FontKey) -> &dyn FontMetrics;
}
