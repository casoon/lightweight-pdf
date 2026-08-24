//! Flat content-stream operations (`plan/00a-contracts-and-artifacts.md`
//! point 3: `PdfTextRun`, path resources — never `Element`/`RenderNode`).
//! The facade crate translates layout output into calls on this builder.

use crate::writer::fmt_num;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The position/orientation operands for [`ContentBuilder::text_rotated`],
/// grouped into one argument so the method stays under clippy's
/// too-many-arguments threshold without merging it into [`ContentBuilder::text`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextRotation {
    /// Rotation center, x.
    pub cx: f32,
    /// Rotation center, y.
    pub cy: f32,
    /// Counter-clockwise rotation angle in degrees.
    pub angle_deg: f32,
    /// Half the text run's width, used to horizontally center it on `cx`.
    pub half_width: f32,
}

fn color_component(c: u8) -> String {
    fmt_num(c as f32 / 255.0)
}

/// Formats a color-setting operator, e.g. `"0 0 0 rg"` (fill) or
/// `"0.5 0.5 0.5 RG"` (stroke) — shared by every drawing/text primitive
/// below that sets a fill or stroke color.
fn color_op(color: Rgb, op: &str) -> String {
    format!(
        "{} {} {} {}",
        color_component(color.0),
        color_component(color.1),
        color_component(color.2),
        op
    )
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

    /// Formats `<x> <y> <w> <h>` — the rectangle-operand pair shared by
    /// every rectangle-drawing primitive ([`Self::clip_rect`],
    /// [`Self::rect_op`], and transitively [`Self::fill_rect`]/
    /// [`Self::stroke_rect`]) ahead of their `re` operator.
    fn rect_operands(x: f32, y: f32, w: f32, h: f32) -> String {
        format!("{} {} {} {}", fmt_num(x), fmt_num(y), fmt_num(w), fmt_num(h))
    }

    /// Intersects the clip path with a rectangle: `re W n`. Must be called
    /// right after `save()` and before any drawing in that scope
    /// (Grundprinzip 4: a clip set at the end protects nothing).
    pub fn clip_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.op(&format!("{} re W n", Self::rect_operands(x, y, w, h)));
    }

    /// Emits `<rect_prefix> <x> <y> <w> <h> re <op>` — the rectangle-operand
    /// skeleton shared by [`Self::fill_rect`] (`rect_prefix` is just the
    /// fill color operator, `op` is `"f"`) and [`Self::stroke_rect`]
    /// (`rect_prefix` also carries the line width, `op` is `"S"`).
    fn rect_op(&mut self, rect_prefix: &str, x: f32, y: f32, w: f32, h: f32, op: &str) {
        self.op(&format!("{} {} re {}", rect_prefix, Self::rect_operands(x, y, w, h), op));
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgb) {
        self.rect_op(&color_op(color, "rg"), x, y, w, h, "f");
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, line_width: f32, color: Rgb) {
        let prefix = format!("{} {} w", color_op(color, "RG"), fmt_num(line_width));
        self.rect_op(&prefix, x, y, w, h, "S");
    }

    pub fn set_dash(&mut self, dash: f32, gap: f32) {
        self.op(&format!("[{} {}] 0 d", fmt_num(dash), fmt_num(gap)));
    }

    pub fn reset_dash(&mut self) {
        self.op("[] 0 d");
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        fill: Option<Rgb>,
        stroke: Option<(f32, Rgb)>,
        dash: Option<(f32, f32)>,
    ) {
        let r = radius.min(w / 2.0).min(h / 2.0);
        let k = r * 0.552_284_8;
        let mut path = String::new();

        path.push_str(&format!("{} {} m\n", fmt_num(x + r), fmt_num(y)));
        path.push_str(&format!("{} {} l\n", fmt_num(x + w - r), fmt_num(y)));
        path.push_str(&format!(
            "{} {} {} {} {} {} c\n",
            fmt_num(x + w - r + k),
            fmt_num(y),
            fmt_num(x + w),
            fmt_num(y + r - k),
            fmt_num(x + w),
            fmt_num(y + r)
        ));
        path.push_str(&format!("{} {} l\n", fmt_num(x + w), fmt_num(y + h - r)));
        path.push_str(&format!(
            "{} {} {} {} {} {} c\n",
            fmt_num(x + w),
            fmt_num(y + h - r + k),
            fmt_num(x + w - r + k),
            fmt_num(y + h),
            fmt_num(x + w - r),
            fmt_num(y + h)
        ));
        path.push_str(&format!("{} {} l\n", fmt_num(x + r), fmt_num(y + h)));
        path.push_str(&format!(
            "{} {} {} {} {} {} c\n",
            fmt_num(x + r - k),
            fmt_num(y + h),
            fmt_num(x),
            fmt_num(y + h - r + k),
            fmt_num(x),
            fmt_num(y + h - r)
        ));
        path.push_str(&format!("{} {} l\n", fmt_num(x), fmt_num(y + r)));
        path.push_str(&format!(
            "{} {} {} {} {} {} c\nh",
            fmt_num(x),
            fmt_num(y + r - k),
            fmt_num(x + r - k),
            fmt_num(y),
            fmt_num(x + r),
            fmt_num(y)
        ));

        if let Some((dash_len, gap_len)) = dash {
            self.set_dash(dash_len, gap_len);
        }

        match (fill, stroke) {
            (Some(f), Some((sw, s))) => {
                let fill_str = color_op(f, "rg");
                let stroke_str = color_op(s, "RG");
                self.op(&format!("{} {} {} w\n{} b", fill_str, stroke_str, fmt_num(sw), path));
            }
            (Some(f), None) => {
                let fill_str = color_op(f, "rg");
                self.op(&format!("{}\n{} f", fill_str, path));
            }
            (None, Some((sw, s))) => {
                let stroke_str = color_op(s, "RG");
                self.op(&format!("{} {} w\n{} S", stroke_str, fmt_num(sw), path));
            }
            (None, None) => {}
        }

        if dash.is_some() {
            self.reset_dash();
        }
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, line_width: f32, color: Rgb) {
        self.op(&format!(
            "{} {} w {} {} m {} {} l S",
            color_op(color, "RG"),
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
        self.text_block(font_resource, size, color, x, y, encoded_bytes);
        self.buf.push(b'\n');
    }

    /// Writes `encoded_bytes` as a PDF hex string body (without the
    /// surrounding `<`/`>` delimiters) — shared by [`Self::text`] and
    /// [`Self::text_rotated`], the two text-showing primitives.
    fn write_hex_string(&mut self, encoded_bytes: &[u8]) {
        for b in encoded_bytes {
            self.buf.extend_from_slice(format!("{b:02X}").as_bytes());
        }
    }

    /// Emits the `BT /{font} {size} Tf {color} {tx} {ty} Td <hex> Tj ET`
    /// text-showing skeleton — shared by [`Self::text`] (called with no
    /// surrounding matrix) and [`Self::text_rotated`] (called inside a
    /// `q <matrix> cm` / `Q` rotation scope). `tx`/`ty` are the raw `Td`
    /// operands, formatted here so neither caller repeats the formatting.
    fn text_block(&mut self, font_resource: &str, size: f32, color: Rgb, tx: f32, ty: f32, encoded_bytes: &[u8]) {
        self.buf.extend_from_slice(
            format!(
                "BT /{} {} Tf {} {} {} Td <",
                font_resource,
                fmt_num(size),
                color_op(color, "rg"),
                fmt_num(tx),
                fmt_num(ty),
            )
            .as_bytes(),
        );
        self.write_hex_string(encoded_bytes);
        self.buf.extend_from_slice(b"> Tj ET");
    }

    /// Writes one text-showing run rotated around `(cx, cy)` (Phase 6
    /// watermark support, `05-overflow-and-robustness.md`: "keine
    /// allgemeine Transform-API" — this is the one narrow, watermark-
    /// specific rotation primitive, not a general element transform).
    /// `angle_deg` is counter-clockwise; `half_width` horizontally centers
    /// the text on `cx` (the baseline sits exactly on `cy` in the rotated
    /// frame — a documented, deliberately simple approximation of vertical
    /// centering, adequate for a decorative diagonal stamp).
    pub fn text_rotated(&mut self, font_resource: &str, size: f32, rotation: TextRotation, color: Rgb, encoded_bytes: &[u8]) {
        let rad = rotation.angle_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        self.buf.extend_from_slice(
            format!(
                "q {} {} {} {} {} {} cm ",
                fmt_num(cos),
                fmt_num(sin),
                fmt_num(-sin),
                fmt_num(cos),
                fmt_num(rotation.cx),
                fmt_num(rotation.cy),
            )
            .as_bytes(),
        );
        self.text_block(font_resource, size, color, -rotation.half_width, 0.0, encoded_bytes);
        self.buf.extend_from_slice(b" Q\n");
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
        c.text_rotated(
            "F1",
            72.0,
            TextRotation {
                cx: 300.0,
                cy: 400.0,
                angle_deg: 45.0,
                half_width: 50.0,
            },
            Rgb(210, 210, 210),
            &[0x00, 0x01],
        );
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
