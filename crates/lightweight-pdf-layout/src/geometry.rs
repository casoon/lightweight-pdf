#[derive(Clone, Copy, Debug)]
pub struct Constraints {
    pub max_width: f32,
    pub max_height: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Coordinates are page-relative, top-down (x grows right, y grows down
/// from the top of the body/header/footer band) — a purely internal layout
/// convention. The facade converts to PDF's bottom-left origin only at the
/// very end, when translating a `RenderNode` tree into content-stream ops.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn shrink(&self, amount: f32) -> Rect {
        Rect {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - 2.0 * amount).max(0.0),
            height: (self.height - 2.0 * amount).max(0.0),
        }
    }
}
