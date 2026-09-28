//! `Path2D` (LLP 1056 D9, §3 stage 2): a path built in its own coordinates,
//! painted under the matrix current at the paint.
//!
//! A `Path2D` lives in the recorder. At `fill(path)`, `stroke(path)` or
//! `clip(path)` its segments are transformed by the current matrix and cross
//! as the list's path-segment records, followed by the paint; the context's
//! own current path is untouched. So a replayer holds no live path objects:
//! a declared difference from D3's ids, with the same result (a path's later
//! mutations never reach an earlier paint).
//!
//! `new Path2D(d)` reads SVG path data: every command, relative and
//! absolute, with the reflections of `S` and `T` and arcs by the SVG
//! endpoint rules. Parsing stops at the first error and keeps what came
//! before it, as SVG and Chrome do. The parser is this crate's own, in f64
//! (the kernel's is f32; this crate depends on nothing).

use crate::context::{throw, CanvasWindingRule, Context2d, DomException};
use crate::geom::{self, Matrix, Radius, Seg};
use crate::list::Op;
use std::sync::Mutex;

/// One segment, in the path's coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PathSeg {
    Move(f64, f64),
    Line(f64, f64),
    Quad(f64, f64, f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Debug, Clone, Default)]
struct PathData {
    segs: Vec<PathSeg>,
    /// The current subpath's start and last point.
    subpath: Option<((f64, f64), (f64, f64))>,
}

impl PathData {
    fn move_to(&mut self, x: f64, y: f64) {
        self.segs.push(PathSeg::Move(x, y));
        self.subpath = Some(((x, y), (x, y)));
    }

    fn line_to(&mut self, x: f64, y: f64) {
        match self.subpath.as_mut() {
            Some(sp) => {
                sp.1 = (x, y);
                self.segs.push(PathSeg::Line(x, y));
            }
            None => self.move_to(x, y),
        }
    }

    fn ensure(&mut self, x: f64, y: f64) {
        if self.subpath.is_none() {
            self.move_to(x, y);
        }
    }

    fn last(&mut self, p: (f64, f64)) {
        if let Some(sp) = self.subpath.as_mut() {
            sp.1 = p;
        }
    }

    fn segments(&mut self, segs: &[Seg]) {
        for s in segs {
            match *s {
                Seg::Line(x, y) => self.line_to(x, y),
                Seg::Cubic(a, b, c, d, x, y) => {
                    self.segs.push(PathSeg::Cubic(a, b, c, d, x, y));
                    self.last((x, y));
                }
            }
        }
    }

    fn close(&mut self) {
        if let Some((start, _)) = self.subpath {
            self.subpath = Some((start, start));
            self.segs.push(PathSeg::Close);
        }
    }

    fn quad(&mut self, cx: f64, cy: f64, x: f64, y: f64) {
        self.ensure(cx, cy);
        self.segs.push(PathSeg::Quad(cx, cy, x, y));
        self.last((x, y));
    }

    fn cubic(&mut self, a: f64, b: f64, c: f64, d: f64, x: f64, y: f64) {
        self.ensure(a, b);
        self.segs.push(PathSeg::Cubic(a, b, c, d, x, y));
        self.last((x, y));
    }

    #[allow(clippy::too_many_arguments)]
    fn ellipse(&mut self, x: f64, y: f64, rx: f64, ry: f64, rot: f64, s: f64, e: f64, ac: bool) {
        let (first, segs) = geom::ellipse(x, y, rx, ry, rot, s, e, ac);
        self.line_to(first.0, first.1);
        self.segments(&segs);
    }
}

/// `Path2D`.
#[derive(Debug, Default)]
pub struct Path2d {
    data: Mutex<PathData>,
}

impl Clone for Path2d {
    fn clone(&self) -> Self {
        Path2d {
            data: Mutex::new(self.d().clone()),
        }
    }
}

macro_rules! finite {
    ($($v:expr),*) => { if !($($v.is_finite())&&*) { return Ok(()); } };
}

impl Path2d {
    fn d(&self) -> std::sync::MutexGuard<'_, PathData> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `new Path2D()`.
    pub fn new() -> Path2d {
        Path2d::default()
    }

    /// `new Path2D(path)`: a copy.
    pub fn new_with_path_2d(other: &Path2d) -> Path2d {
        other.clone()
    }

    /// `new Path2D(d)`: SVG path data, up to its first error.
    pub fn new_with_path_string(d: &str) -> Path2d {
        let p = Path2d::new();
        svg(d, &mut p.d());
        p
    }

    pub(crate) fn segs(&self) -> Vec<PathSeg> {
        self.d().segs.clone()
    }

    /// `addPath(path)`.
    pub fn add_path(&self, path: &Path2d) {
        let _ = self.add_path_with_transformation(path, &Matrix::IDENTITY);
    }

    /// `addPath(path, transform)`: its segments through `transform`; a
    /// non-finite matrix is a `TypeError`, as the dictionary's conversion is.
    pub fn add_path_with_transformation(
        &self,
        path: &Path2d,
        m: &Matrix,
    ) -> Result<(), DomException> {
        if !m.operands().iter().all(|v| v.is_finite()) {
            return throw("TypeError", "The matrix has non-finite entries.");
        }
        let segs = path.segs();
        let mut d = self.d();
        for s in segs {
            let t = transform(s, *m);
            match t {
                PathSeg::Move(x, y) => d.move_to(x, y),
                PathSeg::Close => d.close(),
                PathSeg::Line(x, y) => {
                    d.segs.push(t);
                    d.last((x, y))
                }
                PathSeg::Quad(_, _, x, y) | PathSeg::Cubic(_, _, _, _, x, y) => {
                    d.segs.push(t);
                    d.last((x, y))
                }
            }
        }
        Ok(())
    }

    /// `moveTo(x, y)`.
    pub fn move_to(&self, x: f64, y: f64) {
        if x.is_finite() && y.is_finite() {
            self.d().move_to(x, y)
        }
    }

    /// `lineTo(x, y)`.
    pub fn line_to(&self, x: f64, y: f64) {
        if x.is_finite() && y.is_finite() {
            self.d().line_to(x, y)
        }
    }

    /// `quadraticCurveTo(cpx, cpy, x, y)`.
    pub fn quadratic_curve_to(&self, cpx: f64, cpy: f64, x: f64, y: f64) {
        if [cpx, cpy, x, y].iter().all(|v| v.is_finite()) {
            self.d().quad(cpx, cpy, x, y)
        }
    }

    /// `bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y)`.
    pub fn bezier_curve_to(&self, a: f64, b: f64, c: f64, d: f64, x: f64, y: f64) {
        if [a, b, c, d, x, y].iter().all(|v| v.is_finite()) {
            self.d().cubic(a, b, c, d, x, y)
        }
    }

    /// `closePath()`.
    pub fn close_path(&self) {
        self.d().close()
    }

    /// `rect(x, y, w, h)`.
    pub fn rect(&self, x: f64, y: f64, w: f64, h: f64) {
        if ![x, y, w, h].iter().all(|v| v.is_finite()) {
            return;
        }
        let mut d = self.d();
        d.move_to(x, y);
        d.line_to(x + w, y);
        d.line_to(x + w, y + h);
        d.line_to(x, y + h);
        d.close();
        d.move_to(x, y);
    }

    /// `roundRect(x, y, w, h, radii)`, with the context's rules.
    pub fn round_rect_with_radii(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radii: &[Radius],
    ) -> Result<(), DomException> {
        finite!(x, y, w, h);
        if radii.is_empty() || radii.len() > 4 {
            return throw(
                "RangeError",
                format!(
                    "{} radii provided. Between one and four radii are necessary.",
                    radii.len()
                ),
            );
        }
        for r in radii {
            finite!(r.x, r.y);
            if r.x < 0.0 || r.y < 0.0 {
                let v = if r.x < 0.0 { r.x } else { r.y };
                return throw("RangeError", format!("Radius value {v} is negative."));
            }
        }
        let (start, segs) = geom::round_rect(x, y, w, h, radii);
        let mut d = self.d();
        d.move_to(start.0, start.1);
        d.segments(&segs);
        d.close();
        d.move_to(x, y);
        Ok(())
    }

    /// `arc(x, y, radius, startAngle, endAngle, anticlockwise)`.
    pub fn arc_with_anticlockwise(
        &self,
        x: f64,
        y: f64,
        r: f64,
        start: f64,
        end: f64,
        anticlockwise: bool,
    ) -> Result<(), DomException> {
        finite!(x, y, r, start, end);
        if r < 0.0 {
            return throw(
                "IndexSizeError",
                format!("The radius provided ({r}) is negative."),
            );
        }
        self.d().ellipse(x, y, r, r, 0.0, start, end, anticlockwise);
        Ok(())
    }

    /// `arc(x, y, radius, startAngle, endAngle)`.
    pub fn arc(&self, x: f64, y: f64, r: f64, start: f64, end: f64) -> Result<(), DomException> {
        self.arc_with_anticlockwise(x, y, r, start, end, false)
    }

    /// `ellipse(x, y, rx, ry, rotation, startAngle, endAngle, anticlockwise)`.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse_with_anticlockwise(
        &self,
        x: f64,
        y: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
        anticlockwise: bool,
    ) -> Result<(), DomException> {
        finite!(x, y, rx, ry, rotation, start, end);
        if rx < 0.0 || ry < 0.0 {
            let r = if rx < 0.0 { rx } else { ry };
            return throw(
                "IndexSizeError",
                format!("The radius provided ({r}) is negative."),
            );
        }
        self.d()
            .ellipse(x, y, rx, ry, rotation, start, end, anticlockwise);
        Ok(())
    }

    /// `arcTo(x1, y1, x2, y2, radius)`.
    pub fn arc_to(&self, x1: f64, y1: f64, x2: f64, y2: f64, r: f64) -> Result<(), DomException> {
        finite!(x1, y1, x2, y2, r);
        let mut d = self.d();
        d.ensure(x1, y1);
        if r < 0.0 {
            return throw(
                "IndexSizeError",
                format!("The radius provided ({r}) is negative."),
            );
        }
        let p0 = d.subpath.map(|s| s.1).unwrap_or((0.0, 0.0));
        match geom::arc_to(p0, (x1, y1), (x2, y2), r) {
            None => d.line_to(x1, y1),
            Some((t1, segs)) => {
                d.line_to(t1.0, t1.1);
                d.segments(&segs);
            }
        }
        Ok(())
    }
}

fn transform(s: PathSeg, m: Matrix) -> PathSeg {
    let p = |x, y| m.apply(x, y);
    match s {
        PathSeg::Move(x, y) => {
            let a = p(x, y);
            PathSeg::Move(a.0, a.1)
        }
        PathSeg::Line(x, y) => {
            let a = p(x, y);
            PathSeg::Line(a.0, a.1)
        }
        PathSeg::Quad(cx, cy, x, y) => {
            let (c, a) = (p(cx, cy), p(x, y));
            PathSeg::Quad(c.0, c.1, a.0, a.1)
        }
        PathSeg::Cubic(c1x, c1y, c2x, c2y, x, y) => {
            let (c1, c2, a) = (p(c1x, c1y), p(c2x, c2y), p(x, y));
            PathSeg::Cubic(c1.0, c1.1, c2.0, c2.1, a.0, a.1)
        }
        PathSeg::Close => PathSeg::Close,
    }
}

impl Context2d {
    fn paint_path(&self, path: &Path2d, op: Op, operands: &[f64], clip: bool) {
        let mut g = self.g();
        if !clip && !g.paints() {
            return;
        }
        let m = g.state.transform;
        for s in path.segs() {
            match transform(s, m) {
                PathSeg::Move(x, y) => g.op(Op::PathMoveTo, &[x, y]),
                PathSeg::Line(x, y) => g.op(Op::PathLineTo, &[x, y]),
                PathSeg::Quad(a, b, x, y) => g.op(Op::PathQuadTo, &[a, b, x, y]),
                PathSeg::Cubic(a, b, c, d, x, y) => g.op(Op::PathCubicTo, &[a, b, c, d, x, y]),
                PathSeg::Close => g.op(Op::PathClose, &[]),
            }
        }
        g.op(op, operands);
    }

    /// `fill(path)`.
    pub fn fill_with_path_2d(&self, path: &Path2d) {
        self.fill_with_path_2d_and_winding(path, CanvasWindingRule::Nonzero)
    }

    /// `fill(path, rule)`.
    pub fn fill_with_path_2d_and_winding(&self, path: &Path2d, rule: CanvasWindingRule) {
        self.paint_path(path, Op::FillPath, &[rule.code()], false)
    }

    /// `stroke(path)`.
    pub fn stroke_with_path(&self, path: &Path2d) {
        self.paint_path(path, Op::StrokePath, &[], false)
    }

    /// `clip(path)`.
    pub fn clip_with_path_2d(&self, path: &Path2d) {
        self.clip_with_path_2d_and_winding(path, CanvasWindingRule::Nonzero)
    }

    /// `clip(path, rule)`.
    pub fn clip_with_path_2d_and_winding(&self, path: &Path2d, rule: CanvasWindingRule) {
        self.paint_path(path, Op::ClipPath, &[rule.code()], true)
    }
}

// --- SVG path data ---------------------------------------------------------

struct Scan<'a> {
    s: &'a [u8],
    at: usize,
}

impl Scan<'_> {
    fn skip(&mut self, comma: bool) {
        let mut seen = false;
        while let Some(&c) = self.s.get(self.at) {
            if c.is_ascii_whitespace() || (comma && c == b',' && !seen) {
                seen |= c == b',';
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn number(&mut self) -> Option<f64> {
        self.skip(true);
        let start = self.at;
        let mut i = self.at;
        let s = self.s;
        if matches!(s.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let digits = |i: &mut usize| {
            let from = *i;
            while s.get(*i).is_some_and(u8::is_ascii_digit) {
                *i += 1;
            }
            *i > from
        };
        let int = digits(&mut i);
        let mut frac = false;
        if s.get(i) == Some(&b'.') {
            i += 1;
            frac = digits(&mut i);
        }
        if !int && !frac {
            return None;
        }
        if matches!(s.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(s.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            if digits(&mut j) {
                i = j;
            }
        }
        let v = exact_num::parse_f64(std::str::from_utf8(&s[start..i]).ok()?).ok()?;
        self.at = i;
        v.is_finite().then_some(v)
    }

    fn flag(&mut self) -> Option<bool> {
        self.skip(true);
        let c = *self.s.get(self.at)?;
        self.at += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }

    fn more_numbers(&mut self) -> bool {
        self.skip(true);
        matches!(self.s.get(self.at), Some(c) if c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'+'))
    }
}

/// SVG's elliptical arc from `p0` to `p` (SVG 2 §B.2.4, endpoint to centre).
#[allow(clippy::too_many_arguments)]
fn svg_arc(
    d: &mut PathData,
    p0: (f64, f64),
    rx: f64,
    ry: f64,
    phi_deg: f64,
    large: bool,
    sweep: bool,
    p: (f64, f64),
) {
    if p0 == p {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        d.line_to(p.0, p.1);
        return;
    }
    let phi = phi_deg.to_radians();
    let (sin, cos) = phi.sin_cos();
    let (hx, hy) = ((p0.0 - p.0) / 2.0, (p0.1 - p.1) / 2.0);
    let x1 = cos * hx + sin * hy;
    let y1 = -sin * hx + cos * hy;
    let lambda = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if large == sweep {
        co = -co;
    }
    let (cx1, cy1) = (co * rx * y1 / ry, -co * ry * x1 / rx);
    let cx = cos * cx1 - sin * cy1 + (p0.0 + p.0) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (p0.1 + p.1) / 2.0;
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angle(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    if !sweep && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let (_, segs) = geom::ellipse(cx, cy, rx, ry, phi, t1, t1 + dt, dt < 0.0);
    d.segments(&segs);
    // End exactly at the endpoint.
    if let Some(PathSeg::Cubic(.., x, y)) = d.segs.last_mut() {
        (*x, *y) = p;
    }
    d.last(p);
}

fn svg(text: &str, d: &mut PathData) {
    let mut s = Scan {
        s: text.as_bytes(),
        at: 0,
    };
    let mut cmd = 0u8;
    let mut cur = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    // The last control point, for S and T.
    let mut last_cubic: Option<(f64, f64)> = None;
    let mut last_quad: Option<(f64, f64)> = None;
    let mut first = true;
    loop {
        s.skip(false);
        let Some(&c) = s.s.get(s.at) else { return };
        if c.is_ascii_alphabetic() {
            cmd = c;
            s.at += 1;
        } else if cmd == 0 || !s.more_numbers() {
            return;
        }
        if first && !matches!(cmd, b'M' | b'm') {
            return;
        }
        first = false;
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { (0.0, 0.0) };
        let pt = |s: &mut Scan<'_>| -> Option<(f64, f64)> {
            let x = s.number()?;
            let y = s.number()?;
            Some((base.0 + x, base.1 + y))
        };
        let (mut cubic, mut quad) = (None, None);
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let Some(p) = pt(&mut s) else { return };
                d.move_to(p.0, p.1);
                cur = p;
                start = p;
                // Further pairs are lines.
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let Some(p) = pt(&mut s) else { return };
                d.line_to(p.0, p.1);
                cur = p;
            }
            b'H' => {
                let Some(x) = s.number() else { return };
                cur = (if rel { cur.0 + x } else { x }, cur.1);
                d.line_to(cur.0, cur.1);
            }
            b'V' => {
                let Some(y) = s.number() else { return };
                cur = (cur.0, if rel { cur.1 + y } else { y });
                d.line_to(cur.0, cur.1);
            }
            b'C' => {
                let (Some(c1), Some(c2), Some(p)) = (pt(&mut s), pt(&mut s), pt(&mut s)) else {
                    return;
                };
                d.cubic(c1.0, c1.1, c2.0, c2.1, p.0, p.1);
                cubic = Some(c2);
                cur = p;
            }
            b'S' => {
                let (Some(c2), Some(p)) = (pt(&mut s), pt(&mut s)) else {
                    return;
                };
                let c1 = last_cubic.map_or(cur, |c| (2.0 * cur.0 - c.0, 2.0 * cur.1 - c.1));
                d.cubic(c1.0, c1.1, c2.0, c2.1, p.0, p.1);
                cubic = Some(c2);
                cur = p;
            }
            b'Q' => {
                let (Some(c), Some(p)) = (pt(&mut s), pt(&mut s)) else {
                    return;
                };
                d.quad(c.0, c.1, p.0, p.1);
                quad = Some(c);
                cur = p;
            }
            b'T' => {
                let Some(p) = pt(&mut s) else { return };
                let c = last_quad.map_or(cur, |c| (2.0 * cur.0 - c.0, 2.0 * cur.1 - c.1));
                d.quad(c.0, c.1, p.0, p.1);
                quad = Some(c);
                cur = p;
            }
            b'A' => {
                let (Some(rx), Some(ry), Some(rot), Some(large), Some(sweep)) =
                    (s.number(), s.number(), s.number(), s.flag(), s.flag())
                else {
                    return;
                };
                let Some(p) = pt(&mut s) else { return };
                d.ensure(cur.0, cur.1);
                svg_arc(d, cur, rx, ry, rot, large, sweep, p);
                cur = p;
            }
            b'Z' => {
                d.close();
                cur = start;
                // A command after Z starts at the subpath's start.
                d.subpath = Some((start, start));
            }
            _ => return,
        }
        last_cubic = cubic;
        last_quad = quad;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_data_reads_every_command_and_stops_at_an_error() {
        let p = Path2d::new_with_path_string("M10 10 h 5 v5 H0 V0 l1,1 z m 2 2 C1 1 2 2 3 3 S 4 4 5 5 Q 6 6 7 7 T 8 8 A 5 5 0 0 1 20 20 L x 9");
        let segs = p.segs();
        assert_eq!(segs[0], PathSeg::Move(10.0, 10.0));
        assert_eq!(segs[1], PathSeg::Line(15.0, 10.0));
        assert_eq!(segs[2], PathSeg::Line(15.0, 15.0));
        assert!(segs.contains(&PathSeg::Close));
        assert_eq!(segs[7], PathSeg::Move(12.0, 12.0));
        // S reflects C's second control point (2,2) about (3,3).
        assert_eq!(segs[9], PathSeg::Cubic(4.0, 4.0, 4.0, 4.0, 5.0, 5.0));
        // The arc ends at (20,20); `L x` stops the parse.
        assert!(matches!(segs.last(), Some(PathSeg::Cubic(.., 20.0, 20.0))));
        assert!(Path2d::new_with_path_string("L 1 1").segs().is_empty());
    }
}
