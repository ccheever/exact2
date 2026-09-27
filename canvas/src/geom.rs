//! f64 path geometry (LLP 1056 §1, §7): the author matrix, and `arc`,
//! `ellipse`, `arcTo` and `roundRect` as line and cubic segments. The
//! recorder resolves these at the call, so every replayer draws the same
//! segments. Angles and their canonicalisation are Chrome's
//! (`CanonicalizeAngle`, `AdjustEndAngle` in Blink's `canvas_path.cc`).

use std::f64::consts::{FRAC_PI_2, TAU};

/// A 2D affine matrix, as `DOMMatrix` names it: `x' = a x + c y + e`,
/// `y' = b x + d y + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    /// Scale x.
    pub a: f64,
    /// Skew y.
    pub b: f64,
    /// Skew x.
    pub c: f64,
    /// Scale y.
    pub d: f64,
    /// Translate x.
    pub e: f64,
    /// Translate y.
    pub f: f64,
}

impl Default for Matrix {
    fn default() -> Self {
        Matrix::IDENTITY
    }
}

impl Matrix {
    /// The identity.
    pub const IDENTITY: Matrix = Matrix {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// From six numbers.
    pub fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Matrix {
        Matrix { a, b, c, d, e, f }
    }

    /// `self × m`: `m` applied first, as `ctx.transform(m)` post-multiplies.
    pub fn then(self, m: Matrix) -> Matrix {
        Matrix {
            a: self.a * m.a + self.c * m.b,
            b: self.b * m.a + self.d * m.b,
            c: self.a * m.c + self.c * m.d,
            d: self.b * m.c + self.d * m.d,
            e: self.a * m.e + self.c * m.f + self.e,
            f: self.b * m.e + self.d * m.f + self.f,
        }
    }

    /// A point.
    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// The determinant.
    pub fn det(self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// Whether it can be inverted: every entry finite and the determinant
    /// non-zero.
    pub fn invertible(self) -> bool {
        self.operands().iter().all(|v| v.is_finite()) && self.det() != 0.0 && self.det().is_finite()
    }

    /// The inverse, when it has one.
    pub fn invert(self) -> Option<Matrix> {
        if !self.invertible() {
            return None;
        }
        let k = 1.0 / self.det();
        Some(Matrix {
            a: self.d * k,
            b: -self.b * k,
            c: -self.c * k,
            d: self.a * k,
            e: (self.c * self.f - self.d * self.e) * k,
            f: (self.b * self.e - self.a * self.f) * k,
        })
    }

    /// The six list operands.
    pub fn operands(self) -> [f64; 6] {
        [self.a, self.b, self.c, self.d, self.e, self.f]
    }
}

/// A path segment in user space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    /// A line to a point.
    Line(f64, f64),
    /// A cubic through two control points to a point.
    Cubic(f64, f64, f64, f64, f64, f64),
}

/// The end angle Chrome draws to, after canonicalising the start to
/// [0, 2π): returns `(start, end)`.
pub fn angles(start: f64, end: f64, anticlockwise: bool) -> (f64, f64) {
    let mut s = start % TAU;
    if s < 0.0 {
        s += TAU;
    }
    let e = end + (s - start);
    let e = if !anticlockwise && e - s >= TAU {
        s + TAU
    } else if anticlockwise && s - e >= TAU {
        s - TAU
    } else if !anticlockwise && s > e {
        s + (TAU - (s - e) % TAU)
    } else if anticlockwise && s < e {
        s - (TAU - (e - s) % TAU)
    } else {
        e
    };
    (s, e)
}

/// An ellipse arc as segments: the start point (joined to the path by the
/// caller) and what follows it. Radii are non-negative; a zero radius is the
/// degenerate case, lines through the start and end points.
#[allow(clippy::too_many_arguments)]
pub fn ellipse(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    rotation: f64,
    start: f64,
    end: f64,
    anticlockwise: bool,
) -> ((f64, f64), Vec<Seg>) {
    let (s, e) = angles(start, end, anticlockwise);
    let (sin_r, cos_r) = rotation.sin_cos();
    let at = |t: f64| {
        let (x, y) = (rx * t.cos(), ry * t.sin());
        (cx + x * cos_r - y * sin_r, cy + x * sin_r + y * cos_r)
    };
    let tangent = |t: f64| {
        let (x, y) = (-rx * t.sin(), ry * t.cos());
        (x * cos_r - y * sin_r, x * sin_r + y * cos_r)
    };
    let first = at(s);
    let mut out = Vec::new();
    if rx == 0.0 || ry == 0.0 {
        let last = at(e);
        out.push(Seg::Line(last.0, last.1));
        return (first, out);
    }
    let sweep = e - s;
    if sweep == 0.0 {
        return (first, out);
    }
    let n = ((sweep.abs() / FRAC_PI_2) - 1e-9).ceil().max(1.0) as usize;
    let step = sweep / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    for i in 0..n {
        let (t0, t1) = (s + step * i as f64, s + step * (i + 1) as f64);
        let (p0, p1) = (at(t0), if i + 1 == n { at(e) } else { at(t1) });
        let (d0, d1) = (tangent(t0), tangent(t1));
        out.push(Seg::Cubic(
            p0.0 + k * d0.0,
            p0.1 + k * d0.1,
            p1.0 - k * d1.0,
            p1.1 - k * d1.1,
            p1.0,
            p1.1,
        ));
    }
    (first, out)
}

/// `arcTo`'s geometry from the last point `p0` (user space): `None` when the
/// spec's straight-line cases apply (coincident points, zero radius,
/// collinear), else the tangent point to join with a line and the arc.
pub fn arc_to(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    r: f64,
) -> Option<((f64, f64), Vec<Seg>)> {
    if p0 == p1 || p1 == p2 || r == 0.0 {
        return None;
    }
    let (v1, v2) = ((p0.0 - p1.0, p0.1 - p1.1), (p2.0 - p1.0, p2.1 - p1.1));
    let cross = v1.0 * v2.1 - v1.1 * v2.0;
    let (l1, l2) = (v1.0.hypot(v1.1), v2.0.hypot(v2.1));
    if cross.abs() <= 1e-12 * l1 * l2 {
        return None;
    }
    let (u1, u2) = ((v1.0 / l1, v1.1 / l1), (v2.0 / l2, v2.1 / l2));
    let cos = (u1.0 * u2.0 + u1.1 * u2.1).clamp(-1.0, 1.0);
    let half = cos.acos() / 2.0;
    let dist = r / half.tan();
    let t1 = (p1.0 + u1.0 * dist, p1.1 + u1.1 * dist);
    let t2 = (p1.0 + u2.0 * dist, p1.1 + u2.1 * dist);
    // The centre is along the bisector, r / sin(half) from p1.
    let (bx, by) = (u1.0 + u2.0, u1.1 + u2.1);
    let bl = bx.hypot(by);
    let h = r / half.sin();
    let c = (p1.0 + bx / bl * h, p1.1 + by / bl * h);
    let a0 = (t1.1 - c.1).atan2(t1.0 - c.0);
    let a1 = (t2.1 - c.1).atan2(t2.0 - c.0);
    // Clockwise in canvas space (y down) when the turn p0→p1→p2 is.
    let anticlockwise = cross > 0.0;
    let (_, segs) = ellipse(c.0, c.1, r, r, 0.0, a0, a1, anticlockwise);
    Some((t1, segs))
}

/// One `roundRect` corner radius: horizontal and vertical.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius {
    /// Horizontal.
    pub x: f64,
    /// Vertical.
    pub y: f64,
}

/// `roundRect`'s subpath after the spec's normalisation, as segments from a
/// start point, closed by the caller. `radii` has already been checked
/// (1–4 entries, finite, non-negative).
pub fn round_rect(x: f64, y: f64, w: f64, h: f64, radii: &[Radius]) -> ((f64, f64), Vec<Seg>) {
    let r = |i: usize| radii[i];
    let (mut ul, mut ur, mut lr, mut ll) = match radii.len() {
        1 => (r(0), r(0), r(0), r(0)),
        2 => (r(0), r(1), r(0), r(1)),
        3 => (r(0), r(1), r(2), r(1)),
        _ => (r(0), r(1), r(2), r(3)),
    };
    // A negative width or height mirrors the rectangle: the corners swap,
    // and the path runs the other way round.
    let (mut x0, mut y0, mut ww, mut hh) = (x, y, w, h);
    if w < 0.0 {
        x0 += w;
        ww = -w;
        std::mem::swap(&mut ul, &mut ur);
        std::mem::swap(&mut ll, &mut lr);
    }
    if h < 0.0 {
        y0 += h;
        hh = -h;
        std::mem::swap(&mut ul, &mut ll);
        std::mem::swap(&mut ur, &mut lr);
    }
    let top = ul.x + ur.x;
    let right = ur.y + lr.y;
    let bottom = lr.x + ll.x;
    let left = ul.y + ll.y;
    let mut scale = 1.0f64;
    for (side, sum) in [(ww, top), (hh, right), (ww, bottom), (hh, left)] {
        if sum > 0.0 {
            scale = scale.min(side / sum);
        }
    }
    if scale < 1.0 {
        for c in [&mut ul, &mut ur, &mut lr, &mut ll] {
            c.x *= scale;
            c.y *= scale;
        }
    }
    let k = 0.552_284_749_830_793_4;
    let (x1, y1) = (x0 + ww, y0 + hh);
    // Clockwise from the top edge's start, in normalised space.
    let mut pts: Vec<Seg> = Vec::with_capacity(8);
    let corner = |pts: &mut Vec<Seg>,
                  from: (f64, f64),
                  to: (f64, f64),
                  c: Radius,
                  horizontal_first: bool| {
        if c.x == 0.0 || c.y == 0.0 {
            pts.push(Seg::Line(to.0, to.1));
            return;
        }
        let (c1, c2) = if horizontal_first {
            (
                (from.0 + (to.0 - from.0) * k, from.1),
                (to.0, to.1 - (to.1 - from.1) * k),
            )
        } else {
            (
                (from.0, from.1 + (to.1 - from.1) * k),
                (to.0 - (to.0 - from.0) * k, to.1),
            )
        };
        pts.push(Seg::Cubic(c1.0, c1.1, c2.0, c2.1, to.0, to.1));
    };
    let start = (x0 + ul.x, y0);
    pts.push(Seg::Line(x1 - ur.x, y0));
    corner(&mut pts, (x1 - ur.x, y0), (x1, y0 + ur.y), ur, true);
    pts.push(Seg::Line(x1, y1 - lr.y));
    corner(&mut pts, (x1, y1 - lr.y), (x1 - lr.x, y1), lr, false);
    pts.push(Seg::Line(x0 + ll.x, y1));
    corner(&mut pts, (x0 + ll.x, y1), (x0, y1 - ll.y), ll, true);
    pts.push(Seg::Line(x0, y0 + ul.y));
    corner(&mut pts, (x0, y0 + ul.y), start, ul, false);
    if (w < 0.0) != (h < 0.0) {
        return reverse(start, &pts);
    }
    (start, pts)
}

/// The same closed contour traversed the other way.
fn reverse(start: (f64, f64), segs: &[Seg]) -> ((f64, f64), Vec<Seg>) {
    let mut ends = vec![start];
    for s in segs {
        ends.push(match *s {
            Seg::Line(x, y) | Seg::Cubic(_, _, _, _, x, y) => (x, y),
        });
    }
    let mut out = Vec::with_capacity(segs.len());
    for i in (0..segs.len()).rev() {
        let from = ends[i];
        out.push(match segs[i] {
            Seg::Line(..) => Seg::Line(from.0, from.1),
            Seg::Cubic(a, b, c, d, _, _) => Seg::Cubic(c, d, a, b, from.0, from.1),
        });
    }
    (*ends.last().unwrap(), out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(segs: &[Seg]) -> (f64, f64) {
        match *segs.last().unwrap() {
            Seg::Line(x, y) | Seg::Cubic(_, _, _, _, x, y) => (x, y),
        }
    }

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn angles_follow_chrome() {
        assert_eq!(angles(0.0, TAU, false), (0.0, TAU));
        // Chrome draws the whole circle for (0, 2π, anticlockwise).
        assert_eq!(angles(0.0, TAU, true), (0.0, -TAU));
        let (s, e) = angles(0.0, FRAC_PI_2, true);
        assert!((s - 0.0).abs() < 1e-12 && (e + 3.0 * FRAC_PI_2).abs() < 1e-12);
        let (s, e) = angles(-FRAC_PI_2, 0.0, false);
        assert!((s - 3.0 * FRAC_PI_2).abs() < 1e-12 && (e - TAU).abs() < 1e-12);
    }

    #[test]
    fn a_quarter_arc_is_one_cubic_on_the_circle() {
        let (start, segs) = ellipse(0.0, 0.0, 10.0, 10.0, 0.0, 0.0, FRAC_PI_2, false);
        assert!(near(start, (10.0, 0.0)));
        assert_eq!(segs.len(), 1);
        // Clockwise in canvas space: from +x toward +y (down).
        assert!(near(end(&segs), (0.0, 10.0)));
        let (_, full) = ellipse(0.0, 0.0, 10.0, 10.0, 0.0, 0.0, TAU, false);
        assert_eq!(full.len(), 4);
    }

    #[test]
    fn arc_to_rounds_a_right_angle() {
        let (t1, segs) = arc_to((0.0, 0.0), (10.0, 0.0), (10.0, 10.0), 5.0).unwrap();
        assert!(near(t1, (5.0, 0.0)));
        assert!(near(end(&segs), (10.0, 5.0)));
        assert!(arc_to((0.0, 0.0), (5.0, 0.0), (10.0, 0.0), 5.0).is_none());
        assert!(arc_to((0.0, 0.0), (0.0, 0.0), (10.0, 0.0), 5.0).is_none());
    }

    #[test]
    fn round_rect_scales_radii_that_do_not_fit() {
        let r = Radius { x: 40.0, y: 40.0 };
        let (start, segs) = round_rect(0.0, 0.0, 40.0, 20.0, &[r]);
        // Scaled by 20 / 80: 10 px corners.
        assert!(near(start, (10.0, 0.0)));
        assert_eq!(segs.len(), 8);
        assert!(near(end(&segs), start));
        let (s2, flipped) = round_rect(40.0, 0.0, -40.0, 20.0, &[r]);
        assert!(near(end(&flipped), s2));
    }

    #[test]
    fn matrices_compose_as_the_canvas_does() {
        let m = Matrix::IDENTITY
            .then(Matrix::new(1.0, 0.0, 0.0, 1.0, 10.0, 0.0))
            .then(Matrix::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0));
        assert_eq!(m.apply(1.0, 1.0), (12.0, 2.0));
        let i = m.invert().unwrap();
        assert!(near(i.apply(12.0, 2.0), (1.0, 1.0)));
        assert!(Matrix::new(0.0, 0.0, 0.0, 1.0, 0.0, 0.0).invert().is_none());
    }
}
