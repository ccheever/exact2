//! @ref LLP 1043.000 §3 D6 — subtract the union of filled projections in each band.
use crate::FlowShape;

/// Clear `out` and return sorted free intervals in `[0,width)` for `[top,bottom)`.
///
/// Only positive intervals at least `min_width` wide survive. Invalid bands or
/// nonpositive/nonfinite widths give no intervals; invalid minimums become zero.
/// Uses `out` plus stack scratch for CSS polygons (at most 64 vertices).
/// Nonpolygon work is O(E + S log S). A V-vertex polygon adds O(V² + K*V²),
/// K <= V²+V+1 topology slabs; projected components merge as they are emitted.
/// Larger direct programmatic polygons use O(V²) temporary scratch.
pub fn intervals(
    shapes: &[FlowShape],
    top: f32,
    bottom: f32,
    width: f32,
    min_width: f32,
    out: &mut Vec<(f32, f32)>,
) {
    out.clear();
    append_intervals(shapes, top, bottom, width, min_width, out);
}

// A caller can keep band scratch in its final output allocation, without a
// second Vec allocated anew on every frame.
pub(crate) trait Slot: Copy {
    fn interval(a: f32, b: f32) -> Self;
    fn edges(self) -> (f32, f32);
}
impl Slot for (f32, f32) {
    fn interval(a: f32, b: f32) -> Self {
        (a, b)
    }
    fn edges(self) -> (f32, f32) {
        self
    }
}
pub(crate) fn append_intervals<T: Slot>(
    shapes: &[FlowShape],
    top: f32,
    bottom: f32,
    width: f32,
    min_width: f32,
    out: &mut Vec<T>,
) {
    let start = out.len();
    if !top.is_finite()
        || !bottom.is_finite()
        || bottom <= top
        || !width.is_finite()
        || width <= 0.0
    {
        return;
    }
    let min_width = if min_width.is_finite() {
        min_width.max(0.0)
    } else {
        0.0
    };
    for s in shapes {
        let shape_start = out.len();
        let mut add = |a: f64, b: f64| {
            let (mut a, mut b) = (crate::finite(a).max(0.0), crate::finite(b).min(width));
            if a >= b {
                return;
            }
            // Merge each shape as we go: projected components, not one entry
            // per slab, bound the output scratch even for self intersections.
            let mut i = shape_start;
            while i < out.len() {
                let (x, y) = out[i].edges();
                if a <= y && b >= x {
                    a = a.min(x);
                    b = b.max(y);
                    out.swap_remove(i);
                } else {
                    i += 1;
                }
            }
            out.push(T::interval(a, b));
        };
        match s {
            FlowShape::Polygon(p) | FlowShape::EvenOddPolygon(p) => crate::geometry::polygon_bands(
                p,
                matches!(s, FlowShape::EvenOddPolygon(_)),
                top as f64,
                bottom as f64,
                &mut add,
            ),
            _ => {
                if let Some((a, b)) = s.extent(top as f64, bottom as f64) {
                    add(a as f64, b as f64);
                }
            }
        }
    }
    crate::sort_by(&mut out[start..], |a, b| {
        a.edges().0.total_cmp(&b.edges().0)
    });
    let end = out.len();
    let mut write = start;
    let mut x = 0.0;
    for read in start..end {
        let (a, b) = out[read].edges();
        if a > x && a - x >= min_width {
            out[write] = T::interval(x, a);
            write += 1;
        }
        x = x.max(b);
    }
    out.truncate(write);
    if width > x && width - x >= min_width {
        out.push(T::interval(x, width));
    }
}
