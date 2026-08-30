//! `Watermark` (Phase 6, `plan/phases/phase-6-business-polish.md` step 2):
//! a rotated diagonal stamp ("ENTWURF", "STORNIERT"). Deliberately **not**
//! a normal flow `Element` — it's a document-level, independent layer,
//! always drawn first (bottom) and clipped to the body content box, per
//! `05-overflow-and-robustness.md`'s explicit requirement that it must
//! never make normal content unreadable or bleed into the header/footer
//! bands. No general rotation/transform API is introduced for other
//! elements (ADR/plan: "kein allgemeine Transform-API").

use crate::style::{Color, FontKey};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
#[derive(Clone, Debug)]
pub struct Watermark {
    pub text: String,
    /// Counter-clockwise rotation in degrees, applied around the body
    /// box's center. 45° (bottom-left to top-right diagonal) matches the
    /// conventional "ENTWURF"/"DRAFT" stamp look.
    pub rotation_deg: f32,
    pub size: f32,
    pub color: Color,
    pub font: FontKey,
}

impl Watermark {
    pub fn new(text: impl Into<String>) -> Self {
        Watermark {
            text: text.into(),
            rotation_deg: 45.0,
            size: 72.0,
            // Light gray: legible-by-construction since normal content
            // always draws *after* (on top of) the watermark, but a light
            // color keeps the page from looking visually "shouted at".
            color: Color::rgb(210, 210, 210),
            font: FontKey::SANS_BOLD,
        }
    }

    pub fn rotation(mut self, degrees: f32) -> Self {
        self.rotation_deg = degrees;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}
