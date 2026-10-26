use super::{tiny_path, Path, PathBuilder, Rect, Shape};

/// A shape as a path: a rectangle, or rounded corners as cubic arcs.
pub fn rounded_rect(shape: &Shape) -> Option<Path> {
    let (x, y, w, h) = shape.rect;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    if !shape.rounded() {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let mut ops = Vec::new();
    crate::paint::border::shape_path(&mut ops, shape);
    tiny_path(&ops)
}
