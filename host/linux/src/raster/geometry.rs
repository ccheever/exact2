use super::{tiny_path, Path, PathBuilder, Rect, Shape};

/// A shape as a path: a rectangle, or rounded corners as cubic arcs.
pub fn rounded_rect(shape: &Shape) -> Option<Path> {
    let (x, y, w, h) = shape.rect;
    if ![x, y, w, h].iter().all(|v| v.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    // Direct internal Shapes can bypass the constructors' CSS reduction.
    // Sanitize before either the cubic arcs or corner-shape outline is built.
    let radius = |r: f32| if r.is_finite() { r.max(0.0) } else { 0.0 };
    let shape = Shape {
        radii: crate::paint::border::reduced(
            shape.radii.map(|(x, y)| (radius(x), radius(y))),
            w,
            h,
        ),
        ..*shape
    };
    if !shape.rounded() {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let mut ops = Vec::new();
    crate::paint::border::shape_path(&mut ops, &shape);
    tiny_path(&ops)
}
