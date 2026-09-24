//! @ref LLP 1043.000 §3 D5, D6 — resolved, unscaled exclusion geometry.

use crate::finite;
use std::sync::Arc;

/// An exclusion in paragraph coordinates; zero-area or invalid geometry is empty.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowShape {
    /// A circle.
    Circle {
        /// Center x.
        cx: f32,
        /// Center y.
        cy: f32,
        /// Nonnegative radius.
        r: f32,
    },
    /// An axis-aligned ellipse.
    Ellipse {
        /// Center x.
        cx: f32,
        /// Center y.
        cy: f32,
        /// Horizontal radius.
        rx: f32,
        /// Vertical radius.
        ry: f32,
    },
    /// An axis-aligned rectangle with one circular corner radius.
    RoundRect {
        /// Left edge.
        x: f32,
        /// Top edge.
        y: f32,
        /// Width.
        width: f32,
        /// Height.
        height: f32,
        /// Corner radius, clamped to half the smaller dimension.
        radius: f32,
    },
    /// Closed polygon filled by the CSS nonzero rule.
    Polygon(Arc<[(f32, f32)]>),
    /// Closed polygon filled by the CSS evenodd rule.
    EvenOddPolygon(Arc<[(f32, f32)]>),
    /// A precomputed silhouette; row i covers `[y+i*h, y+(i+1)*h)`.
    Spans {
        /// Horizontal origin added to both row endpoints.
        x: f32,
        /// Vertical origin.
        y: f32,
        /// Positive height of each row.
        row_height: f32,
        /// Excluded half-open x intervals; `(0,0)` is an empty row.
        rows: Arc<[(f32, f32)]>,
    },
}

impl FlowShape {
    /// Return a translated shape; polygon storage is copied, spans remain shared.
    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        let dx = finite(dx as f64);
        let dy = finite(dy as f64);
        let add = |a: f32, b: f32| finite(a as f64 + b as f64);
        match self {
            Self::Circle { cx, cy, r } => Self::Circle {
                cx: add(*cx, dx),
                cy: add(*cy, dy),
                r: *r,
            },
            Self::Ellipse { cx, cy, rx, ry } => Self::Ellipse {
                cx: add(*cx, dx),
                cy: add(*cy, dy),
                rx: *rx,
                ry: *ry,
            },
            Self::RoundRect {
                x,
                y,
                width,
                height,
                radius,
            } => Self::RoundRect {
                x: add(*x, dx),
                y: add(*y, dy),
                width: *width,
                height: *height,
                radius: *radius,
            },
            Self::Polygon(p) | Self::EvenOddPolygon(p) => {
                let points = p.iter().map(|(x, y)| (add(*x, dx), add(*y, dy))).collect();
                if matches!(self, Self::EvenOddPolygon(_)) {
                    Self::EvenOddPolygon(points)
                } else {
                    Self::Polygon(points)
                }
            }
            Self::Spans {
                x,
                y,
                row_height,
                rows,
            } => Self::Spans {
                x: add(*x, dx),
                y: add(*y, dy),
                row_height: *row_height,
                rows: rows.clone(),
            },
        }
    }

    /// Grow by a nonnegative CSS `shape-margin`; invalid/negative margins do nothing.
    ///
    /// Circles/ellipses grow their radii; rectangles grow their edges and corners.
    /// Polygon/spans use a conservative 64-row envelope: each output row encloses
    /// the original geometry in that row extended vertically by the margin, then
    /// expands horizontally by the margin (a box dilation, not a circular offset).
    /// This may close holes and narrow gaps. Growth allocates only for these shapes.
    pub fn grow(&self, margin: f32) -> Self {
        if !margin.is_finite() || margin <= 0.0 {
            return self.clone();
        }
        let add = |a: f32, b: f32| finite(a as f64 + b as f64);
        match self {
            Self::Circle { cx, cy, r } => Self::Circle {
                cx: *cx,
                cy: *cy,
                r: add(*r, margin),
            },
            Self::Ellipse { cx, cy, rx, ry } => Self::Ellipse {
                cx: *cx,
                cy: *cy,
                rx: add(*rx, margin),
                ry: add(*ry, margin),
            },
            Self::RoundRect {
                x,
                y,
                width,
                height,
                radius,
            } => Self::RoundRect {
                x: add(*x, -margin),
                y: add(*y, -margin),
                width: finite(*width as f64 + 2.0 * margin as f64),
                height: finite(*height as f64 + 2.0 * margin as f64),
                radius: add(*radius, margin),
            },
            Self::Polygon(_) | Self::EvenOddPolygon(_) | Self::Spans { .. } => {
                let (x0, y0, x1, y1) = self.bounds();
                if x0 >= x1 || y0 >= y1 {
                    return self.clone();
                }
                let y = add(y0, -margin);
                let row_height = finite((y1 as f64 - y0 as f64 + 2.0 * margin as f64) / 64.0);
                let rows = (0..64)
                    .map(|i| {
                        let top = y as f64 + i as f64 * row_height as f64;
                        self.extent(top - margin as f64, top + row_height as f64 + margin as f64)
                            .map_or((0.0, 0.0), |(a, b)| (add(a, -margin), add(b, margin)))
                    })
                    .collect();
                Self::Spans {
                    x: 0.0,
                    y,
                    row_height,
                    rows,
                }
            }
        }
    }

    /// Return finite `(left, top, right, bottom)` bounds; empty shapes return zeros.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let raw = match self {
            Self::Circle { cx, cy, r } => ellipse_bounds(*cx, *cy, *r, *r),
            Self::Ellipse { cx, cy, rx, ry } => ellipse_bounds(*cx, *cy, *rx, *ry),
            Self::RoundRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                if !valid(&[*x, *y, *width, *height, *radius])
                    || *width <= 0.0
                    || *height <= 0.0
                    || *radius < 0.0
                {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                (
                    *x as f64,
                    *y as f64,
                    *x as f64 + *width as f64,
                    *y as f64 + *height as f64,
                )
            }
            Self::Polygon(p) | Self::EvenOddPolygon(p) => {
                if p.len() < 3 || p.iter().any(|(x, y)| !valid(&[*x, *y])) {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                p.iter().fold(
                    (
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    ),
                    |(a, b, c, d), (x, y)| {
                        (
                            a.min(*x as f64),
                            b.min(*y as f64),
                            c.max(*x as f64),
                            d.max(*y as f64),
                        )
                    },
                )
            }
            Self::Spans {
                x,
                y,
                row_height,
                rows,
            } => {
                if !valid(&[*x, *y, *row_height]) || *row_height <= 0.0 {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                let mut b = (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
                for (i, (a, c)) in rows.iter().enumerate() {
                    if valid(&[*a, *c]) && a < c {
                        b.0 = b.0.min(*x as f64 + *a as f64);
                        b.1 = b.1.min(*y as f64 + i as f64 * *row_height as f64);
                        b.2 = b.2.max(*x as f64 + *c as f64);
                        b.3 = b.3.max(*y as f64 + (i + 1) as f64 * *row_height as f64);
                    }
                }
                b
            }
        };
        if raw.0 >= raw.2 || raw.1 >= raw.3 {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            (finite(raw.0), finite(raw.1), finite(raw.2), finite(raw.3))
        }
    }

    // The envelope of all occupied scanlines in this band. Endpoints use the
    // supremum at bottom: excluding a little extra at an open edge is safe.
    pub(crate) fn extent(&self, top: f64, bottom: f64) -> Option<(f32, f32)> {
        if bottom <= top {
            return None;
        }
        let e = match self {
            Self::Circle { cx, cy, r } => ellipse_extent(*cx, *cy, *r, *r, top, bottom),
            Self::Ellipse { cx, cy, rx, ry } => ellipse_extent(*cx, *cy, *rx, *ry, top, bottom),
            Self::RoundRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                if !valid(&[*x, *y, *width, *height, *radius])
                    || *width <= 0.0
                    || *height <= 0.0
                    || *radius < 0.0
                {
                    return None;
                }
                let (x, y, w, h) = (*x as f64, *y as f64, *width as f64, *height as f64);
                if bottom <= y || top >= y + h {
                    return None;
                }
                let r = (*radius as f64).min(w.min(h) / 2.0);
                let near = (y + h / 2.0).clamp(top, bottom);
                let dy = (y + r - near).max(near - (y + h - r)).max(0.0).min(r);
                let inset = r - (r * r - dy * dy).max(0.0).sqrt();
                Some((x + inset, x + w - inset))
            }
            Self::Polygon(p) | Self::EvenOddPolygon(p) => {
                let mut low = f64::INFINITY;
                let mut high = f64::NEG_INFINITY;
                polygon_bands(
                    p,
                    matches!(self, Self::EvenOddPolygon(_)),
                    top,
                    bottom,
                    &mut |a, b| {
                        low = low.min(a);
                        high = high.max(b);
                    },
                );
                (low < high).then_some((low, high))
            }
            Self::Spans {
                x,
                y,
                row_height,
                rows,
            } => {
                if !valid(&[*x, *y, *row_height]) || *row_height <= 0.0 {
                    return None;
                }
                let first = ((top - *y as f64) / *row_height as f64).floor().max(0.0) as usize;
                let last = ((bottom - *y as f64) / *row_height as f64).ceil().max(0.0) as usize;
                let mut low = f64::INFINITY;
                let mut high = f64::NEG_INFINITY;
                for &(a, b) in &rows[first.min(rows.len())..last.min(rows.len())] {
                    if valid(&[a, b]) && a < b {
                        low = low.min(a as f64);
                        high = high.max(b as f64);
                    }
                }
                (low < high).then_some((*x as f64 + low, *x as f64 + high))
            }
        }?;
        (e.0 < e.1).then_some((finite(e.0), finite(e.1)))
    }
}
fn valid(values: &[f32]) -> bool {
    values.iter().all(|n| n.is_finite())
}
fn ellipse_bounds(cx: f32, cy: f32, rx: f32, ry: f32) -> (f64, f64, f64, f64) {
    if !valid(&[cx, cy, rx, ry]) || rx <= 0.0 || ry <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (
        cx as f64 - rx as f64,
        cy as f64 - ry as f64,
        cx as f64 + rx as f64,
        cy as f64 + ry as f64,
    )
}
fn ellipse_extent(cx: f32, cy: f32, rx: f32, ry: f32, top: f64, bottom: f64) -> Option<(f64, f64)> {
    if !valid(&[cx, cy, rx, ry]) || rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let (cx, cy, rx, ry) = (cx as f64, cy as f64, rx as f64, ry as f64);
    let dy = (cy - cy.clamp(top, bottom)).abs();
    if dy >= ry {
        return None;
    }
    let half = rx * (1.0 - (dy / ry).powi(2)).max(0.0).sqrt();
    Some((cx - half, cx + half))
}

/// Whether any nonempty shape's bounds intersect the half-open query rectangle.
pub fn meets(shapes: &[FlowShape], x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    valid(&[x0, y0, x1, y1])
        && x0 < x1
        && y0 < y1
        && shapes.iter().any(|s| {
            let (a, b, c, d) = s.bounds();
            a < c && b < d && a < x1 && c > x0 && b < y1 && d > y0
        })
}

// @ref LLP 1043.000 §3 D5/D6 — fill-rule crossings between every topology
// event. Inside one slab edges are linear and their ordering cannot change,
// so the union projection is exactly the extrema at its two endpoints.
// V vertices: O(V² + K*V log V), K <= V²+V+1 slabs, plus caller union work.
// CSS caps V at 64: both scratch arrays stay on the stack. Programmatic larger
// polygons use O(V²) temporary storage; no arbitrary vertex truncation.
pub(crate) fn polygon_bands(
    p: &[(f32, f32)],
    evenodd: bool,
    top: f64,
    bottom: f64,
    emit: &mut dyn FnMut(f64, f64),
) {
    if p.len() < 3 || p.iter().any(|(x, y)| !valid(&[*x, *y])) || bottom <= top {
        return;
    }
    let edge = |i: usize| {
        let a = p[i];
        let b = p[(i + 1) % p.len()];
        (a.0 as f64, a.1 as f64, b.0 as f64, b.1 as f64)
    };
    let mut heap_y = Vec::new();
    let capacity = p.len().saturating_mul(p.len() - 1) / 2 + p.len() + 2;
    // Initialize the 16-vertex scratch only for small shapes; both CSS
    // capacities remain on the stack, with heap scratch above 64 vertices.
    let ys = if capacity <= 138 {
        &mut [0f64; 138][..capacity]
    } else if capacity <= 2082 {
        &mut [0f64; 2082][..capacity]
    } else {
        heap_y.resize(capacity, 0.);
        &mut heap_y
    };
    ys[0] = top;
    ys[1] = bottom;
    let mut count = 2;
    for i in 0..p.len() {
        let (ax, ay, bx, by) = edge(i);
        if ay > top && ay < bottom {
            ys[count] = ay;
            count += 1;
        }
        let (ux, uy) = (bx - ax, by - ay);
        // For 0 < t < 1, ay + t*uy lies between these evaluated endpoints.
        // Use ay+uy rather than by to preserve that bound under cancellation.
        let end_y = ay + uy;
        if ay.max(end_y) <= top || ay.min(end_y) >= bottom {
            continue;
        }
        for j in i + 1..p.len() {
            let (cx, cy, dx, dy) = edge(j);
            let (vx, vy) = (dx - cx, dy - cy);
            let det = ux * vy - uy * vx;
            if det == 0. {
                continue;
            }
            let t = ((cx - ax) * vy - (cy - ay) * vx) / det;
            let u = ((cx - ax) * uy - (cy - ay) * ux) / det;
            let y = ay + t * uy;
            if t > 0. && t < 1. && u > 0. && u < 1. && y > top && y < bottom {
                ys[count] = y;
                count += 1;
            }
        }
    }
    crate::sort_by(&mut ys[..count], |a, b| a.total_cmp(b));
    #[derive(Clone, Copy, Default)]
    struct Crossing {
        mid: f64,
        low: f64,
        high: f64,
        winding: i32,
    }
    let mut heap = Vec::new();
    let crossings = if p.len() <= 16 {
        &mut [Crossing::default(); 16][..p.len()]
    } else if p.len() <= 64 {
        &mut [Crossing::default(); 64][..p.len()]
    } else {
        heap.resize(p.len(), Crossing::default());
        &mut heap
    };
    for band in ys[..count].windows(2) {
        let (lo, hi) = (band[0], band[1]);
        if lo == hi {
            continue;
        }
        let mid = lo + (hi - lo) / 2.;
        let mut n = 0;
        for i in 0..p.len() {
            let (ax, ay, bx, by) = edge(i);
            if ay == by || mid <= ay.min(by) || mid >= ay.max(by) {
                continue;
            }
            let x = |y| ax + (bx - ax) * ((y - ay) / (by - ay));
            crossings[n] = Crossing {
                mid: x(mid),
                low: x(lo),
                high: x(hi),
                winding: if by > ay { 1 } else { -1 },
            };
            n += 1;
        }
        crate::sort_by(&mut crossings[..n], |a, b| a.mid.total_cmp(&b.mid));
        let mut winding = 0i32;
        let mut start = (0., 0.);
        let mut i = 0;
        let inside = |w: i32| if evenodd { w % 2 != 0 } else { w != 0 };
        while i < n {
            let c = crossings[i];
            let was = inside(winding);
            while i < n && crossings[i].mid == c.mid {
                winding += crossings[i].winding;
                i += 1;
            }
            let now = inside(winding);
            if !was && now {
                start = (c.low, c.high);
            }
            if was && !now {
                emit(start.0.min(start.1), c.low.max(c.high));
            }
        }
    }
}

impl FlowShape {
    /// Shared native batch/agent geometry encoding.
    pub fn write_json(&self, out: &mut String) {
        use std::fmt::Write as _;
        use FlowShape::*;
        let shape = self;
        let pairs = |out: &mut String, rows: &[(f32, f32)]| {
            out.push('[');
            for (i, (x, y)) in rows.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(
                    out,
                    "[{},{}]",
                    json_number(*x as f64),
                    json_number(*y as f64)
                );
            }
            out.push(']');
        };
        let (kind, fields): (_, Vec<(&str, f32)>) = match shape {
            Circle { cx, cy, r } => ("Circle", vec![("cx", *cx), ("cy", *cy), ("r", *r)]),
            Ellipse { cx, cy, rx, ry } => (
                "Ellipse",
                vec![("cx", *cx), ("cy", *cy), ("rx", *rx), ("ry", *ry)],
            ),
            RoundRect {
                x,
                y,
                width,
                height,
                radius,
            } => (
                "RoundRect",
                vec![
                    ("x", *x),
                    ("y", *y),
                    ("width", *width),
                    ("height", *height),
                    ("radius", *radius),
                ],
            ),
            Polygon(points) | EvenOddPolygon(points) => {
                out.push_str(if matches!(shape, EvenOddPolygon(_)) {
                    "{\"kind\":\"Polygon\",\"fill_rule\":\"evenodd\",\"points\":"
                } else {
                    "{\"kind\":\"Polygon\",\"fill_rule\":\"nonzero\",\"points\":"
                });
                pairs(out, points);
                out.push('}');
                return;
            }
            Spans {
                x,
                y,
                row_height,
                rows,
            } => {
                let _ = write!(
                    out,
                    "{{\"kind\":\"Spans\",\"x\":{},\"y\":{},\"row_height\":{},\"rows\":",
                    json_number(*x as f64),
                    json_number(*y as f64),
                    json_number(*row_height as f64)
                );
                pairs(out, rows);
                out.push('}');
                return;
            }
        };
        let _ = write!(out, "{{\"kind\":\"{kind}\"");
        for (name, value) in fields {
            let _ = write!(out, ",\"{name}\":{}", json_number(value as f64));
        }
        out.push('}');
    }
}

fn json_number(n: f64) -> impl std::fmt::Display {
    struct Number(f64);
    impl std::fmt::Display for Number {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let n = self.0;
            if !n.is_finite() {
                f.write_str("null")
            } else if n == n.trunc() && n.abs() < 1e15 {
                write!(f, "{}", n as i64)
            } else {
                write!(f, "{}", exact_num::Shortest(n))
            }
        }
    }
    Number(n)
}
