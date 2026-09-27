//! SVG path data and point lists, to one absolute path of moves, lines,
//! cubics and closes.
//!
//! @ref LLP 1055 D1; SVG 2 §9.3 (the grammar), §9.5.4 (error handling:
//! render up to the last good segment), Implementation Notes §B.2.4 (arcs
//! to centre form)
//!
//! Quadratics and arcs become cubics here, so a painter needs four verbs.

/// One absolute path segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    /// Start a subpath.
    Move(f32, f32),
    /// A straight line to a point.
    Line(f32, f32),
    /// A cubic Bézier: two control points, then the end.
    Cubic(f32, f32, f32, f32, f32, f32),
    /// Close the subpath back to its start.
    Close,
}

/// An absolute path.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path(pub Vec<Seg>);

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len()
            && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
        {
            self.i += 1;
        }
    }
    fn skip_sep(&mut self) {
        self.skip();
        if self.i < self.s.len() && self.s[self.i] == b',' {
            self.i += 1;
            self.skip();
        }
    }
    fn number(&mut self) -> Option<f32> {
        self.skip_sep();
        let start = self.i;
        let s = self.s;
        let mut i = self.i;
        if i < s.len() && matches!(s[i], b'+' | b'-') {
            i += 1;
        }
        let digits = |i: &mut usize| {
            let from = *i;
            while *i < s.len() && s[*i].is_ascii_digit() {
                *i += 1;
            }
            *i > from
        };
        let int = digits(&mut i);
        let mut frac = false;
        if i < s.len() && s[i] == b'.' {
            i += 1;
            frac = digits(&mut i);
        }
        if !int && !frac {
            return None;
        }
        if i < s.len() && matches!(s[i], b'e' | b'E') {
            let mut j = i + 1;
            if j < s.len() && matches!(s[j], b'+' | b'-') {
                j += 1;
            }
            if digits(&mut j) {
                i = j;
            }
        }
        let text = std::str::from_utf8(&s[start..i]).ok()?;
        let v = exact_num::parse_f64(text).ok()? as f32;
        self.i = i;
        v.is_finite().then_some(v)
    }
    fn flag(&mut self) -> Option<bool> {
        self.skip_sep();
        let f = match self.s.get(self.i)? {
            b'0' => false,
            b'1' => true,
            _ => return None,
        };
        self.i += 1;
        Some(f)
    }
    fn command(&mut self) -> Option<u8> {
        self.skip_sep();
        let c = *self.s.get(self.i)?;
        c.is_ascii_alphabetic().then(|| {
            self.i += 1;
            c
        })
    }
    fn at_number(&mut self) -> bool {
        self.skip_sep();
        self.s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.'))
    }
}

/// Parse `d`. A syntax error ends the path at the last complete segment, as
/// browsers render it.
pub fn parse_d(d: &str) -> Path {
    parse_d_commands(d).0
}

/// [`parse_d`], with where each authored command's segments end: a marker
/// sits at the end of each command, so an arc drawn as several cubics is
/// one vertex (LLP 1055.000 D9).
pub fn parse_d_commands(d: &str) -> (Path, Vec<usize>) {
    let (path, ends, _) = parse(d);
    (path, ends)
}

/// `d` with no error in it, or `None`: CSS's `path()` refuses the whole
/// declaration where an SVG `path` renders up to the error.
pub fn parse_d_whole(d: &str) -> Option<Path> {
    let (path, _, whole) = parse(d);
    (whole && !path.0.is_empty()).then_some(path)
}

/// The path, its commands' ends, and whether every byte of `d` was read.
fn parse(d: &str) -> (Path, Vec<usize>, bool) {
    let mut ends = Vec::new();
    let mut lx = Lexer {
        s: d.as_bytes(),
        i: 0,
    };
    let mut out = Vec::new();
    let (mut cx, mut cy) = (0f32, 0f32); // current point
    let (mut sx, mut sy) = (0f32, 0f32); // subpath start
    let mut last_ctrl: Option<(u8, f32, f32)> = None; // for S and T
    let mut open = false; // a subpath is started
    let mut cmd: Option<u8> = None;
    let mut clean = false;
    loop {
        let c = if let Some(c) = lx.command() {
            c
        } else if let Some(previous) = cmd.filter(|_| lx.at_number()) {
            // An implicit repeat; a repeated move is a line.
            match previous {
                b'M' => b'L',
                b'm' => b'l',
                c => c,
            }
        } else {
            clean = true; // between commands: every other `break` is an error
            break;
        };
        if out.is_empty() && !matches!(c, b'M' | b'm') {
            break;
        }
        let rel = c.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        let upper = c.to_ascii_uppercase();
        if upper != b'M' && upper != b'Z' && !open {
            // Drawing after a close continues from the subpath's start.
            out.push(Seg::Move(cx, cy));
            open = true;
        }
        macro_rules! num {
            () => {
                match lx.number() {
                    Some(v) => v,
                    None => break,
                }
            };
        }
        let mut ctrl = None;
        match upper {
            b'M' => {
                let (x, y) = (num!() + ox, num!() + oy);
                out.push(Seg::Move(x, y));
                (cx, cy, sx, sy) = (x, y, x, y);
                open = true;
            }
            b'Z' => {
                if open {
                    out.push(Seg::Close);
                }
                (cx, cy) = (sx, sy);
                open = false;
            }
            b'L' => {
                let (x, y) = (num!() + ox, num!() + oy);
                out.push(Seg::Line(x, y));
                (cx, cy) = (x, y);
            }
            b'H' => {
                let x = num!() + ox;
                out.push(Seg::Line(x, cy));
                cx = x;
            }
            b'V' => {
                let y = num!() + oy;
                out.push(Seg::Line(cx, y));
                cy = y;
            }
            b'C' | b'S' => {
                let (x1, y1) = if upper == b'C' {
                    (num!() + ox, num!() + oy)
                } else {
                    match last_ctrl {
                        Some((b'C', px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                        _ => (cx, cy),
                    }
                };
                let (x2, y2, x, y) = (num!() + ox, num!() + oy, num!() + ox, num!() + oy);
                out.push(Seg::Cubic(x1, y1, x2, y2, x, y));
                ctrl = Some((b'C', x2, y2));
                (cx, cy) = (x, y);
            }
            b'Q' | b'T' => {
                let (qx, qy) = if upper == b'Q' {
                    (num!() + ox, num!() + oy)
                } else {
                    match last_ctrl {
                        Some((b'Q', px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                        _ => (cx, cy),
                    }
                };
                let (x, y) = (num!() + ox, num!() + oy);
                out.push(quad(cx, cy, qx, qy, x, y));
                ctrl = Some((b'Q', qx, qy));
                (cx, cy) = (x, y);
            }
            b'A' => {
                let (rx, ry, rot) = (num!(), num!(), num!());
                let Some(large) = lx.flag() else { break };
                let Some(sweep) = lx.flag() else { break };
                let (x, y) = (num!() + ox, num!() + oy);
                arc(&mut out, (cx, cy), rx, ry, rot, large, sweep, (x, y));
                (cx, cy) = (x, y);
            }
            _ => break,
        }
        last_ctrl = ctrl;
        cmd = Some(c);
        ends.push(out.len());
    }
    let whole = clean && lx.i == lx.s.len() && !d.trim_end().ends_with(',');
    (Path(out), ends, whole)
}

fn quad(x0: f32, y0: f32, qx: f32, qy: f32, x: f32, y: f32) -> Seg {
    Seg::Cubic(
        x0 + 2.0 / 3.0 * (qx - x0),
        y0 + 2.0 / 3.0 * (qy - y0),
        x + 2.0 / 3.0 * (qx - x),
        y + 2.0 / 3.0 * (qy - y),
        x,
        y,
    )
}

/// An elliptical arc as cubics (SVG 2 Implementation Notes §B.2.4–B.2.5).
#[allow(clippy::too_many_arguments)]
fn arc(
    out: &mut Vec<Seg>,
    (x1, y1): (f32, f32),
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
    (x2, y2): (f32, f32),
) {
    if x1 == x2 && y1 == y2 {
        return;
    }
    let (mut rx, mut ry) = (rx.abs() as f64, ry.abs() as f64);
    if rx == 0.0 || ry == 0.0 {
        out.push(Seg::Line(x2, y2));
        return;
    }
    let phi = (rotation as f64).to_radians();
    let (sin, cos) = phi.sin_cos();
    let (x1d, y1d, x2d, y2d) = (x1 as f64, y1 as f64, x2 as f64, y2 as f64);
    let dx = (x1d - x2d) / 2.0;
    let dy = (y1d - y2d) / 2.0;
    let x1p = cos * dx + sin * dy;
    let y1p = -sin * dx + cos * dy;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = (rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p).max(0.0);
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let cx = cos * cxp - sin * cyp + (x1d + x2d) / 2.0;
    let cy = sin * cxp + cos * cyp + (y1d + y2d) / 2.0;
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            -a
        } else {
            a
        }
    };
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = angle(1.0, 0.0, ux, uy);
    let mut delta = angle(ux, uy, vx, vy) % std::f64::consts::TAU;
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let pieces = (delta.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = delta / pieces as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |t: f64| {
        let (s, c) = t.sin_cos();
        (
            cx + rx * c * cos - ry * s * sin,
            cy + rx * c * sin + ry * s * cos,
        )
    };
    let deriv = |t: f64| {
        let (s, c) = t.sin_cos();
        (-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
    };
    let mut t = theta1;
    for i in 0..pieces {
        let t2 = t + step;
        let (p0, d0) = (point(t), deriv(t));
        let (p3, d3) = (point(t2), deriv(t2));
        let end = if i + 1 == pieces { (x2d, y2d) } else { p3 };
        out.push(Seg::Cubic(
            (p0.0 + k * d0.0) as f32,
            (p0.1 + k * d0.1) as f32,
            (p3.0 - k * d3.0) as f32,
            (p3.1 - k * d3.1) as f32,
            end.0 as f32,
            end.1 as f32,
        ));
        t = t2;
    }
}

/// Parse `points`: pairs of numbers; an odd trailing coordinate is dropped,
/// and a syntax error ends the list, as browsers do.
pub fn parse_points(text: &str) -> Vec<(f32, f32)> {
    let mut lx = Lexer {
        s: text.as_bytes(),
        i: 0,
    };
    let mut out = Vec::new();
    while let Some(x) = lx.number() {
        let Some(y) = lx.number() else { break };
        out.push((x, y));
    }
    out
}

impl Path {
    /// A polyline, or a closed polygon.
    pub fn polyline(points: &[(f32, f32)], close: bool) -> Path {
        let mut segs = Vec::with_capacity(points.len() + 1);
        for (i, (x, y)) in points.iter().enumerate() {
            segs.push(if i == 0 {
                Seg::Move(*x, *y)
            } else {
                Seg::Line(*x, *y)
            });
        }
        if close && !segs.is_empty() {
            segs.push(Seg::Close);
        }
        Path(segs)
    }

    /// The path's total length (every subpath, closes included), the number
    /// `pathLength` and dashes are measured against.
    pub fn length(&self) -> f64 {
        let (mut total, mut cx, mut cy, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let dist =
            |ax: f64, ay: f64, bx: f64, by: f64| ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        for seg in &self.0 {
            match *seg {
                Seg::Move(x, y) => {
                    (cx, cy, sx, sy) = (x as f64, y as f64, x as f64, y as f64);
                }
                Seg::Line(x, y) => {
                    total += dist(cx, cy, x as f64, y as f64);
                    (cx, cy) = (x as f64, y as f64);
                }
                Seg::Cubic(x1, y1, x2, y2, x, y) => {
                    let p = [
                        (cx, cy),
                        (x1 as f64, y1 as f64),
                        (x2 as f64, y2 as f64),
                        (x as f64, y as f64),
                    ];
                    total += cubic_length(p, 0);
                    (cx, cy) = (x as f64, y as f64);
                }
                Seg::Close => {
                    total += dist(cx, cy, sx, sy);
                    (cx, cy) = (sx, sy);
                }
            }
        }
        total
    }

    /// The exact bounding box of the geometry (SVG 2 §8.10's fill box):
    /// `(x, y, width, height)`, or `None` for an empty path. A cubic's
    /// extremes are found where its derivative is zero, not at its control
    /// points.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let mut b: Option<(f64, f64, f64, f64)> = None;
        let mut add = |x: f64, y: f64| {
            b = Some(match b {
                None => (x, y, x, y),
                Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
            });
        };
        let (mut cx, mut cy) = (0.0f64, 0.0f64);
        for seg in &self.0 {
            match *seg {
                Seg::Move(x, y) | Seg::Line(x, y) => {
                    (cx, cy) = (x as f64, y as f64);
                    add(cx, cy);
                }
                Seg::Cubic(x1, y1, x2, y2, x, y) => {
                    let (x1, y1, x2, y2, x, y) = (
                        x1 as f64, y1 as f64, x2 as f64, y2 as f64, x as f64, y as f64,
                    );
                    add(x, y);
                    for t in cubic_extrema(cx, x1, x2, x)
                        .into_iter()
                        .chain(cubic_extrema(cy, y1, y2, y))
                        .flatten()
                    {
                        let at = |a: f64, b: f64, c: f64, d: f64| {
                            let u = 1.0 - t;
                            u * u * u * a
                                + 3.0 * u * u * t * b
                                + 3.0 * u * t * t * c
                                + t * t * t * d
                        };
                        add(at(cx, x1, x2, x), at(cy, y1, y2, y));
                    }
                    (cx, cy) = (x, y);
                }
                Seg::Close => {}
            }
        }
        b.map(|(x0, y0, x1, y1)| (x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32))
    }

    /// Every point through an affine `[a, b, c, d, e, f]`.
    pub fn transformed(&self, m: [f32; 6]) -> Path {
        let p = |x: f32, y: f32| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        Path(
            self.0
                .iter()
                .map(|s| match *s {
                    Seg::Move(x, y) => {
                        let (x, y) = p(x, y);
                        Seg::Move(x, y)
                    }
                    Seg::Line(x, y) => {
                        let (x, y) = p(x, y);
                        Seg::Line(x, y)
                    }
                    Seg::Cubic(a, b, c, d, x, y) => {
                        let (a, b) = p(a, b);
                        let (c, d) = p(c, d);
                        let (x, y) = p(x, y);
                        Seg::Cubic(a, b, c, d, x, y)
                    }
                    Seg::Close => Seg::Close,
                })
                .collect(),
        )
    }
}

/// The parameters in (0, 1) where one coordinate of a cubic is extreme.
fn cubic_extrema(p0: f64, p1: f64, p2: f64, p3: f64) -> [Option<f64>; 2] {
    // B'(t)/3 = a t² + b t + c
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    let inside = |t: f64| (t > 0.0 && t < 1.0).then_some(t);
    if a.abs() < 1e-12 {
        if b.abs() < 1e-12 {
            return [None, None];
        }
        return [inside(-c / b), None];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return [None, None];
    }
    let r = disc.sqrt();
    [inside((-b + r) / (2.0 * a)), inside((-b - r) / (2.0 * a))]
}

fn cubic_length(p: [(f64, f64); 4], depth: u32) -> f64 {
    let d = |a: (f64, f64), b: (f64, f64)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let chord = d(p[0], p[3]);
    let poly = d(p[0], p[1]) + d(p[1], p[2]) + d(p[2], p[3]);
    if depth >= 16 || poly - chord <= 1e-6 * poly.max(1.0) {
        return (chord + poly) / 2.0;
    }
    let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let ab = mid(p[0], p[1]);
    let bc = mid(p[1], p[2]);
    let cd = mid(p[2], p[3]);
    let abc = mid(ab, bc);
    let bcd = mid(bc, cd);
    let m = mid(abc, bcd);
    cubic_length([p[0], ab, abc, m], depth + 1) + cubic_length([m, bcd, cd, p[3]], depth + 1)
}
