use super::{tiny_path, Path, PathBuilder, Rect, Shape};

/// A shape as a path: a rectangle, or rounded corners as cubic arcs.
pub fn rounded_rect(shape: &Shape) -> Option<Path> {
    let (x, y, w, h) = shape.rect;
    if ![x, y, w, h].iter().all(|v| v.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    // Direct internal Shapes can bypass the constructors' CSS reduction.
    // Sanitize before either the cubic arcs or corner-shape outline is built.
    let radius = |r: f32| if r.is_finite() && r >= 0.0 { r } else { 0.0 };
    let mut radii = shape.radii.map(|(x, y)| (radius(x), radius(y)));
    let sums = [
        (radii[0].0 + radii[1].0, w),
        (radii[3].0 + radii[2].0, w),
        (radii[0].1 + radii[3].1, h),
        (radii[1].1 + radii[2].1, h),
    ];
    // The constructor's sum, division, products and final sum each round.
    // Repeating its reduction can change corner-shape's sample count.
    if sums.iter().any(|&(sum, edge)| {
        let rounding =
            (f64::from(edge) * f64::from(f32::EPSILON)).max(f64::from(f32::from_bits(1)));
        f64::from(sum) > f64::from(edge) + 4.0 * rounding
    }) {
        radii = crate::paint::border::reduced(radii, w, h);
    }
    let shape = Shape { radii, ..*shape };
    if !shape.rounded() {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let mut ops = Vec::new();
    crate::paint::border::shape_path(&mut ops, &shape);
    tiny_path(&ops)
}
