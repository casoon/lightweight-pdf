//! `plan/05-overflow-and-robustness.md` Grundprinzip 8: diagnostics instead
//! of silent clipping. Collected during the layout pass and returned
//! alongside the finished PDF bytes by the facade's
//! `render_with_diagnostics()`, never separately (InDesign "Overset Text"
//! anti-pattern).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutWarningKind {
    /// Text was cut off because its box was too small (`Overflow::Clip`).
    TextClipped,
    /// A container's content overflowed its box and was clipped.
    ContentOverflow,
    /// An atomic element bigger than a whole page was forced onto its own
    /// page and clipped there (Grundprinzip 7).
    ForcedPageBreak,
    /// Header/Footer content was taller than its reserved band and got
    /// clipped (band size is fixed, never grows, ADR-011).
    HeaderFooterOverflow,
}

#[derive(Clone, Debug)]
pub struct LayoutWarning {
    pub kind: LayoutWarningKind,
    pub page: usize,
    pub element_hint: String,
}
