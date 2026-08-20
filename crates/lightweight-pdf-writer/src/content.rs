//! Flat content-stream operations (`plan/00a-contracts-and-artifacts.md`
//! point 3: `PdfTextRun`, path resources — never `Element`/`RenderNode`).
//! The facade crate translates layout output into calls on this builder.

use crate::writer::fmt_num;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

fn color_component(c: u8) -> String {
    fmt_num(c as f32 / 255.0)
}

pub struct ContentBuilder {
    buf: Vec<u8>,
}

impl Default for ContentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentBuilder {
    pub fn new() -> Self {
        ContentBuilder { buf: Vec::new() }
    }

    fn op(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'\n');
    }

    /// `q` — push graphics state (used before every clip scope, Grundprinzip 4).
    pub fn save(&mut self) {
        self.op("q");
    }

    /// `Q` — pop graphics state, closing the matching clip scope.
    pub fn restore(&mut self) {
        self.op("Q");
    }

    /// Intersects the clip path with a rectangle: `re W n`. Must be called
    /// right after `save()` and before any drawing in that scope
    /// (Grundprinzip 4: a clip set at the end protects nothing).
    pub fn clip_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.op(&format!("{} {} {} {} re W n", fmt_num(x), fmt_num(y), fmt_num(w), fmt_num(h)));
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
        self.op(&format!(
            "{} {} {} rg {} {} {} {} re f",
            color_component(color.0),
            color_component(color.1),
            color_component(color.2),
            fmt_num(x),
            fmt_num(y),
            fmt_num(w),
            fmt_num(h)
        ));
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, line_width: f32, color: Rgb) {
        self.op(&format!(
            "{} {} {} RG {} w {} {} {} {} re S",
            color_component(color.0),
            color_component(color.1),
            color_component(color.2),
            fmt_num(line_width),
            fmt_num(x),
            fmt_num(y),
            fmt_num(w),
            fmt_num(h)
        ));
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, line_width: f32, color: Rgb) {
        self.op(&format!(
            "{} {} {} RG {} w {} {} m {} {} l S",
            color_component(color.0),
            color_component(color.1),
            color_component(color.2),
            fmt_num(line_width),
            fmt_num(x1),
            fmt_num(y1),
            fmt_num(x2),
            fmt_num(y2)
        ));
    }

    /// Writes one text-showing run: `BT /F1 12 Tf 1 0 0 rg 100 700 Td
    /// <0001 0002> Tj ET`. `encoded_bytes` are the already-encoded character
    /// codes for the target font's encoding (Identity-H: big-endian 2-byte
    /// CIDs). Written as a PDF hex string, not a literal `(...)` string —
    /// binary-safe by construction, no escaping and no UTF-8 involved (CIDs
    /// are not Unicode and must never be routed through `str`/`String`).
    pub fn text(&mut self, font_resource: &str, size: f32, x: f32, y: f32, color: Rgb, encoded_bytes: &[u8]) {
        self.buf.extend_from_slice(
            format!(
                "BT /{} {} Tf {} {} {} rg {} {} Td <",
                font_resource,
                fmt_num(size),
                color_component(color.0),
                color_component(color.1),
                color_component(color.2),
                fmt_num(x),
                fmt_num(y),
            )
            .as_bytes(),
        );
        for b in encoded_bytes {
            self.buf.extend_from_slice(format!("{b:02X}").as_bytes());
        }
        self.buf.extend_from_slice(b"> Tj ET\n");
    }

    /// Writes one text-showing run rotated around `(cx, cy)` (Phase 6
    /// watermark support, `05-overflow-and-robustness.md`: "keine
    /// allgemeine Transform-API" — this is the one narrow, watermark-
    /// specific rotation primitive, not a general element transform).
    /// `angle_deg` is counter-clockwise; `half_width` horizontally centers
    /// the text on `cx` (the baseline sits exactly on `cy` in the rotated
    /// frame — a documented, deliberately simple approximation of vertical
    /// centering, adequate for a decorative diagonal stamp).
    #[allow(clippy::too_many_arguments)]
    pub fn text_rotated(
        &mut self,
        font_resource: &str,
        size: f32,
        cx: f32,
        cy: f32,
        angle_deg: f32,
        half_width: f32,
        color: Rgb,
        encoded_bytes: &[u8],
    ) {
        let rad = angle_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        self.buf.extend_from_slice(
            format!(
                "q {} {} {} {} {} {} cm BT /{} {} Tf {} {} {} rg {} 0 Td <",
                fmt_num(cos),
                fmt_num(sin),
                fmt_num(-sin),
                fmt_num(cos),
                fmt_num(cx),
                fmt_num(cy),
                font_resource,
                fmt_num(size),
                color_component(color.0),
                color_component(color.1),
                color_component(color.2),
                fmt_num(-half_width),
            )
            .as_bytes(),
        );
        for b in encoded_bytes {
            self.buf.extend_from_slice(format!("{b:02X}").as_bytes());
        }
        self.buf.extend_from_slice(b"> Tj ET Q\n");
    }

    /// Draws a registered image XObject into the unit square, scaled to
    /// `w`x`h` at `(x, y)`: `q <w> 0 0 <h> <x> <y> cm /Im1 Do Q` — the
    /// standard PDF idiom for placing an image (an XObject is always
    /// defined over the 1x1 unit square, `cm` maps that to the target box).
    pub fn draw_image(&mut self, image_resource: &str, x: f32, y: f32, w: f32, h: f32) {
        self.op(&format!(
            "q {} 0 0 {} {} {} cm /{} Do Q",
            fmt_num(w),
            fmt_num(h),
            fmt_num(x),
            fmt_num(y),
            image_resource
        ));
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_operators() {
        let mut c = ContentBuilder::new();
        c.save();
        c.clip_rect(0.0, 0.0, 100.0, 50.0);
        // CIDs 0x0001 0x0002 0x0003, big-endian 2-byte codes (Identity-H).
        c.text("F1", 12.0, 10.0, 20.0, Rgb(0, 0, 0), &[0x00, 0x01, 0x00, 0x02, 0x00, 0x03]);
        c.restore();
        let bytes = c.into_bytes();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains("q\n"));
        assert!(s.contains("0 0 100 50 re W n"));
        assert!(s.contains("<000100020003> Tj"));
        assert!(s.trim_end().ends_with('Q'));
    }

    #[test]
    fn draw_image_emits_the_cm_do_idiom() {
        let mut c = ContentBuilder::new();
        c.draw_image("Im1", 10.0, 20.0, 100.0, 50.0);
        let s = String::from_utf8(c.into_bytes()).unwrap();
        assert!(s.contains("100 0 0 50 10 20 cm /Im1 Do"));
    }

    #[test]
    fn text_rotated_emits_a_rotation_matrix_and_centers_horizontally() {
        let mut c = ContentBuilder::new();
        c.text_rotated("F1", 72.0, 300.0, 400.0, 45.0, 50.0, Rgb(210, 210, 210), &[0x00, 0x01]);
        let s = String::from_utf8(c.into_bytes()).unwrap();
        // cos(45)=sin(45)=0.707
        assert!(s.contains("0.707 0.707 -0.707 0.707 300 400 cm"));
        assert!(
            s.contains("-50 0 Td"),
            "text must be horizontally centered via a -half_width offset"
        );
        assert!(s.trim_end().ends_with("Q"));
    }
}
