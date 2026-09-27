//! SVG path data and `viewBox` for the `path` node (LLP 1065).
//!
//! `d` is SVG 2 path data (§9.3) in full: every command, absolute and
//! relative, implicit repeats, `S`/`T` reflection, and elliptical arcs. It is
//! parsed once here and normalized to absolute `M`, `L`, `C` and `Z` — the
//! vocabulary Core Graphics, tiny-skia and an SVG `<path>` all draw — so no
//! host parses path data and every host draws the same curves. A quadratic is
//! its exact cubic; an arc is cubics of at most 90° each.
//!
//! Errors follow SVG: a path renders up to, not including, the command that
//! holds the first error, and data that does not begin with a moveto renders
//! nothing. The compiler refuses a literal that has an error at all.
//!
//! Each subpath's length is measured here too, so a host that trims a stroke
//! by length along the whole path (`stroke-start`/`stroke-end`) knows where
//! one subpath ends and the next begins.

use std::fmt::Write as _;

/// A point, in the path's own (view-box) units.
pub type Point = [f32; 2];

/// One normalized drawing command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Start a subpath.
    Move(Point),
    /// A straight line to a point.
    Line(Point),
    /// A cubic Bézier: two control points, then the end point.
    Cubic(Point, Point, Point),
    /// Close the subpath with a line back to its start.
    Close,
}

/// Why path data stopped where it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathError {
    /// Byte offset of the command holding the first error.
    pub at: usize,
}

/// Parsed, normalized path data: commands, and each subpath's length.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PathData {
    commands: Vec<Command>,
    /// For each subpath in order: the index of its `Move`, and its length.
    subpaths: Vec<(usize, f64)>,
    error: Option<PathError>,
}

impl PathData {
    /// Parse SVG path data. Never fails: an error keeps what came before it
    /// (SVG 2 §9.5.4) and is reported by [`PathData::error`].
    pub fn parse(d: &str) -> PathData {
        let mut commands = Vec::new();
        let error = Parser::new(d).run(&mut commands).err();
        let mut data = PathData {
            commands,
            subpaths: Vec::new(),
            error,
        };
        data.measure();
        data
    }

    /// The normalized commands.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// The first error, if the data had one.
    pub fn error(&self) -> Option<PathError> {
        self.error
    }

    /// Each subpath's length, in order.
    pub fn lengths(&self) -> impl Iterator<Item = f64> + '_ {
        self.subpaths.iter().map(|(_, length)| *length)
    }

    /// The whole path's length: every subpath's, summed.
    pub fn length(&self) -> f64 {
        self.lengths().sum()
    }

    /// Each subpath's commands, in order.
    pub fn subpaths(&self) -> impl Iterator<Item = &[Command]> + '_ {
        let ends = self
            .subpaths
            .iter()
            .skip(1)
            .map(|(start, _)| *start)
            .chain([self.commands.len()]);
        self.subpaths
            .iter()
            .zip(ends)
            .map(|((start, _), end)| &self.commands[*start..end])
    }

    /// Canonical absolute text: `M x y C … Z`. What crosses to a native
    /// host, and a valid `d` itself.
    pub fn css(&self) -> String {
        commands_css(&self.commands)
    }

    /// The stroke `stroke-start`…`stroke-end` shows, for a host that trims
    /// geometry itself: fractions of the whole path's length, measured
    /// through the subpaths in order (LLP 1065 D4). A subpath the window cuts
    /// is an open piece; one it shows whole keeps its closepath, so its last
    /// join is the untrimmed one.
    pub fn trimmed(&self, start: f64, end: f64) -> Vec<Command> {
        let total = self.length();
        let from = start.clamp(0.0, 1.0) * total;
        let to = end.clamp(0.0, 1.0) * total;
        if from <= 0.0 && to >= total {
            return self.commands.clone();
        }
        let mut out = Vec::new();
        if to <= from {
            return out;
        }
        let (mut walked, mut first, mut at) = (0.0, [0.0; 2], [0.0; 2]);
        // `down`: the output ends where this subpath's pen is. `whole`: the
        // subpath began inside the window, so it is drawn from its start.
        let (mut down, mut whole) = (false, false);
        for command in &self.commands {
            let segment = match *command {
                Command::Move(p) => {
                    (first, at, down, whole) = (p, p, false, walked >= from);
                    continue;
                }
                Command::Line(p) => Segment::Line(at, p),
                Command::Cubic(a, b, p) => Segment::Cubic([at, a, b, p]),
                Command::Close => Segment::Line(at, first),
            };
            let length = segment.length();
            let (lo, hi) = (walked, walked + length);
            walked = hi;
            at = segment.end();
            if hi <= from {
                continue;
            }
            if lo >= to {
                break;
            }
            let t0 = if from > lo {
                segment.at_length(from - lo)
            } else {
                0.0
            };
            let t1 = if to < hi {
                segment.at_length(to - lo)
            } else {
                1.0
            };
            let piece = segment.between(t0, t1);
            if !down {
                out.push(Command::Move(piece.start()));
            }
            out.push(match piece {
                _ if *command == Command::Close && whole && t1 >= 1.0 => Command::Close,
                Segment::Line(_, p) => Command::Line(p),
                Segment::Cubic([_, a, b, p]) => Command::Cubic(a, b, p),
            });
            down = t1 >= 1.0;
            if !down {
                break;
            }
        }
        out
    }

    fn measure(&mut self) {
        let mut start = [0.0; 2];
        let mut at = [0.0; 2];
        for (i, command) in self.commands.iter().enumerate() {
            let length = match *command {
                Command::Move(p) => {
                    self.subpaths.push((i, 0.0));
                    start = p;
                    at = p;
                    continue;
                }
                Command::Line(p) => distance(at, p),
                Command::Cubic(c1, c2, p) => cubic_length([at, c1, c2, p], 0),
                Command::Close => distance(at, start),
            };
            at = match *command {
                Command::Line(p) | Command::Cubic(_, _, p) => p,
                _ => start,
            };
            if let Some(last) = self.subpaths.last_mut() {
                last.1 += length;
            }
        }
    }
}

/// Normalized commands as canonical path text.
pub fn commands_css(commands: &[Command]) -> String {
    let mut out = String::new();
    let point = |out: &mut String, p: Point| {
        let _ = write!(
            out,
            " {} {}",
            exact_num::Shortest32(p[0]),
            exact_num::Shortest32(p[1])
        );
    };
    for command in commands {
        if !out.is_empty() {
            out.push(' ');
        }
        match *command {
            Command::Move(p) => {
                out.push('M');
                point(&mut out, p);
            }
            Command::Line(p) => {
                out.push('L');
                point(&mut out, p);
            }
            Command::Cubic(a, b, p) => {
                out.push('C');
                point(&mut out, a);
                point(&mut out, b);
                point(&mut out, p);
            }
            Command::Close => out.push('Z'),
        }
    }
    out
}

/// SVG `viewBox`: `min-x min-y width height`, separated by whitespace
/// and/or a comma. A negative or zero width or height is refused (SVG
/// disables rendering for zero; negative is an error).
pub fn parse_view_box(text: &str) -> Option<[f32; 4]> {
    let mut parser = Parser::new(text);
    parser.whitespace();
    let mut out = [0.0; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        if i > 0 {
            parser.separator();
        }
        *slot = parser.number()? as f32;
    }
    parser.whitespace();
    (parser.at == parser.text.len()
        && out[2] > 0.0
        && out[3] > 0.0
        && out.iter().all(|n| n.is_finite()))
    .then_some(out)
}

/// The view box fitted into a box `width` × `height` as SVG's default
/// `preserveAspectRatio="xMidYMid meet"` does: `(scale, dx, dy)`, mapping a
/// path point `p` to `p * scale + (dx, dy)` in the box. No view box is the
/// identity: path units are CSS pixels from the box's origin.
pub fn fit(view_box: Option<[f32; 4]>, width: f32, height: f32) -> (f32, f32, f32) {
    let Some([x, y, w, h]) = view_box else {
        return (1.0, 0.0, 0.0);
    };
    let scale = (width / w).min(height / h).max(0.0);
    (
        scale,
        (width - w * scale) / 2.0 - x * scale,
        (height - h * scale) / 2.0 - y * scale,
    )
}

/// One drawn segment, for trimming.
#[derive(Clone, Copy)]
enum Segment {
    Line(Point, Point),
    Cubic([Point; 4]),
}

impl Segment {
    fn start(self) -> Point {
        match self {
            Segment::Line(a, _) => a,
            Segment::Cubic(p) => p[0],
        }
    }

    fn end(self) -> Point {
        match self {
            Segment::Line(_, b) => b,
            Segment::Cubic(p) => p[3],
        }
    }

    fn length(self) -> f64 {
        match self {
            Segment::Line(a, b) => distance(a, b),
            Segment::Cubic(p) => cubic_length(p, 0),
        }
    }

    /// The parameter `length` along the segment reaches, by bisection on a
    /// cubic (arc length has no closed form).
    fn at_length(self, length: f64) -> f64 {
        let whole = self.length();
        if whole <= 0.0 {
            return 0.0;
        }
        let Segment::Cubic(p) = self else {
            return (length / whole).clamp(0.0, 1.0);
        };
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if cubic_length(split(p, mid).0, 0) < length {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    }

    /// The part between two parameters.
    fn between(self, t0: f64, t1: f64) -> Segment {
        match self {
            Segment::Line(a, b) => {
                let at = |t: f64| {
                    [
                        (f64::from(a[0]) + (f64::from(b[0]) - f64::from(a[0])) * t) as f32,
                        (f64::from(a[1]) + (f64::from(b[1]) - f64::from(a[1])) * t) as f32,
                    ]
                };
                Segment::Line(at(t0), if t1 >= 1.0 { b } else { at(t1) })
            }
            Segment::Cubic(p) => {
                let head = if t1 >= 1.0 { p } else { split(p, t1).0 };
                let tail = if t0 <= 0.0 || t1 <= 0.0 {
                    head
                } else {
                    split(head, t0 / t1).1
                };
                Segment::Cubic(tail)
            }
        }
    }
}

/// A cubic split at `t` by de Casteljau: the part before, the part after.
fn split(p: [Point; 4], t: f64) -> ([Point; 4], [Point; 4]) {
    let lerp = |a: [f64; 2], b: [f64; 2]| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    let q = p.map(|p| [f64::from(p[0]), f64::from(p[1])]);
    let (ab, bc, cd) = (lerp(q[0], q[1]), lerp(q[1], q[2]), lerp(q[2], q[3]));
    let (abc, bcd) = (lerp(ab, bc), lerp(bc, cd));
    let m = lerp(abc, bcd);
    let f = |v: [f64; 2]| [v[0] as f32, v[1] as f32];
    ([p[0], f(ab), f(abc), f(m)], [f(m), f(bcd), f(cd), p[3]])
}

fn distance(a: Point, b: Point) -> f64 {
    let dx = f64::from(b[0]) - f64::from(a[0]);
    let dy = f64::from(b[1]) - f64::from(a[1]);
    (dx * dx + dy * dy).sqrt()
}

/// A cubic's length by subdivision until the control polygon and the chord
/// agree (their mean is Gravesen's estimate), to about a millionth of a unit.
fn cubic_length(p: [Point; 4], depth: u32) -> f64 {
    let chord = distance(p[0], p[3]);
    let polygon = distance(p[0], p[1]) + distance(p[1], p[2]) + distance(p[2], p[3]);
    if depth >= 16 || polygon - chord <= 1e-6 * polygon.max(1.0) {
        return (chord + polygon) / 2.0;
    }
    let mid = |a: Point, b: Point| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let (ab, bc, cd) = (mid(p[0], p[1]), mid(p[1], p[2]), mid(p[2], p[3]));
    let (abc, bcd) = (mid(ab, bc), mid(bc, cd));
    let m = mid(abc, bcd);
    cubic_length([p[0], ab, abc, m], depth + 1) + cubic_length([m, bcd, cd, p[3]], depth + 1)
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

/// The state SVG's command rules carry from one segment to the next.
struct Pen {
    at: [f64; 2],
    start: [f64; 2],
    /// The last cubic's second control point, for `S`.
    cubic: Option<[f64; 2]>,
    /// The last quadratic's control point, for `T`.
    quad: Option<[f64; 2]>,
    /// A closepath ended the last subpath: a drawing command starts a new
    /// one at the same point (SVG 2 §9.3.3).
    closed: bool,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Parser { text, at: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')) {
            self.at += 1;
        }
    }

    /// `wsp* ,? wsp*`
    fn separator(&mut self) {
        self.whitespace();
        if self.peek() == Some(b',') {
            self.at += 1;
            self.whitespace();
        }
    }

    /// SVG's number: sign, digits with an optional fraction (or a bare
    /// fraction), optional exponent. `1.5.5` is two numbers; `-1-2` too.
    fn number(&mut self) -> Option<f64> {
        let bytes = self.text.as_bytes();
        let begin = self.at;
        let mut i = self.at;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let digits = |i: &mut usize| {
            let from = *i;
            while bytes.get(*i).is_some_and(u8::is_ascii_digit) {
                *i += 1;
            }
            *i > from
        };
        let whole = digits(&mut i);
        let mut fraction = false;
        if bytes.get(i) == Some(&b'.') {
            let mut j = i + 1;
            fraction = digits(&mut j);
            if fraction || whole {
                i = j;
            }
        }
        if !whole && !fraction {
            return None;
        }
        if matches!(bytes.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(bytes.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            if digits(&mut j) {
                i = j;
            }
        }
        let n = exact_num::parse_f64(&self.text[begin..i]).ok()?;
        if !n.is_finite() || n.abs() > f64::from(f32::MAX) {
            return None;
        }
        self.at = i;
        Some(n)
    }

    /// An arc flag: exactly `0` or `1`, which needs no separator after it.
    fn flag(&mut self) -> Option<bool> {
        let flag = match self.peek()? {
            b'0' => false,
            b'1' => true,
            _ => return None,
        };
        self.at += 1;
        Some(flag)
    }

    /// Whether another argument set follows (an implicit repeat).
    fn more(&mut self) -> bool {
        self.separator();
        matches!(self.peek(), Some(b'0'..=b'9' | b'+' | b'-' | b'.'))
    }

    fn numbers<const N: usize>(&mut self) -> Option<[f64; N]> {
        let mut out = [0.0; N];
        for (i, slot) in out.iter_mut().enumerate() {
            if i > 0 {
                self.separator();
            }
            *slot = self.number()?;
        }
        Some(out)
    }

    fn run(&mut self, out: &mut Vec<Command>) -> Result<(), PathError> {
        let mut pen = Pen {
            at: [0.0; 2],
            start: [0.0; 2],
            cubic: None,
            quad: None,
            closed: false,
        };
        self.whitespace();
        let mut first = true;
        while let Some(letter) = self.peek() {
            let command_at = self.at;
            let fail = PathError { at: command_at };
            if first && !matches!(letter, b'M' | b'm') {
                return Err(fail);
            }
            first = false;
            self.at += 1;
            let relative = letter.is_ascii_lowercase();
            let mut segment = Vec::new();
            let mut sets = 0;
            loop {
                self.whitespace();
                let mark = segment.len();
                if !self.segment(letter, relative, sets, &mut pen, &mut segment) {
                    // A malformed set draws nothing, not even the subpath
                    // start a closepath would have implied.
                    segment.truncate(mark);
                    out.extend(segment);
                    return Err(fail);
                }
                sets += 1;
                if matches!(letter, b'Z' | b'z') || !self.more() {
                    break;
                }
            }
            out.extend(segment);
            self.whitespace();
        }
        Ok(())
    }

    /// One argument set of one command; `false` on a malformed one.
    fn segment(
        &mut self,
        letter: u8,
        relative: bool,
        set: usize,
        pen: &mut Pen,
        out: &mut Vec<Command>,
    ) -> bool {
        let base = if relative { pen.at } else { [0.0; 2] };
        let abs = |p: [f64; 2]| [p[0] + base[0], p[1] + base[1]];
        let point = |p: [f64; 2]| [p[0] as f32, p[1] as f32];
        // A drawing command after a closepath starts a subpath of its own.
        if pen.closed && !matches!(letter, b'M' | b'm' | b'Z' | b'z') {
            out.push(Command::Move(point(pen.start)));
            pen.closed = false;
        }
        let mut cubic = None;
        let mut quad = None;
        match letter.to_ascii_uppercase() {
            b'M' => {
                let Some(p) = self.numbers::<2>() else {
                    return false;
                };
                let p = abs(p);
                // Pairs after the first are implicit linetos.
                if set == 0 {
                    out.push(Command::Move(point(p)));
                    pen.start = p;
                    pen.closed = false;
                } else {
                    out.push(Command::Line(point(p)));
                }
                pen.at = p;
            }
            b'L' => {
                let Some(p) = self.numbers::<2>() else {
                    return false;
                };
                let p = abs(p);
                out.push(Command::Line(point(p)));
                pen.at = p;
            }
            b'H' | b'V' => {
                let Some([n]) = self.numbers::<1>() else {
                    return false;
                };
                let horizontal = letter.eq_ignore_ascii_case(&b'H');
                let axis = usize::from(!horizontal);
                let mut p = pen.at;
                p[axis] = if relative { pen.at[axis] + n } else { n };
                out.push(Command::Line(point(p)));
                pen.at = p;
            }
            b'C' => {
                let Some([x1, y1, x2, y2, x, y]) = self.numbers::<6>() else {
                    return false;
                };
                let (c1, c2, p) = (abs([x1, y1]), abs([x2, y2]), abs([x, y]));
                out.push(Command::Cubic(point(c1), point(c2), point(p)));
                cubic = Some(c2);
                pen.at = p;
            }
            b'S' => {
                let Some([x2, y2, x, y]) = self.numbers::<4>() else {
                    return false;
                };
                let c1 = reflect(pen.cubic, pen.at);
                let (c2, p) = (abs([x2, y2]), abs([x, y]));
                out.push(Command::Cubic(point(c1), point(c2), point(p)));
                cubic = Some(c2);
                pen.at = p;
            }
            b'Q' | b'T' => {
                let (q, p) = if letter.eq_ignore_ascii_case(&b'Q') {
                    let Some([x1, y1, x, y]) = self.numbers::<4>() else {
                        return false;
                    };
                    (abs([x1, y1]), abs([x, y]))
                } else {
                    let Some(p) = self.numbers::<2>() else {
                        return false;
                    };
                    (reflect(pen.quad, pen.at), abs(p))
                };
                let (c1, c2) = quad_controls(pen.at, q, p);
                out.push(Command::Cubic(point(c1), point(c2), point(p)));
                quad = Some(q);
                pen.at = p;
            }
            b'A' => {
                let Some([rx, ry, angle]) = self.numbers::<3>() else {
                    return false;
                };
                self.separator();
                let Some(large) = self.flag() else {
                    return false;
                };
                self.separator();
                let Some(sweep) = self.flag() else {
                    return false;
                };
                self.separator();
                let Some(p) = self.numbers::<2>() else {
                    return false;
                };
                let p = abs(p);
                arc(pen.at, rx, ry, angle, large, sweep, p, &mut |c1, c2, e| {
                    out.push(match (c1, c2) {
                        (Some(c1), Some(c2)) => Command::Cubic(point(c1), point(c2), point(e)),
                        _ => Command::Line(point(e)),
                    })
                });
                pen.at = p;
            }
            b'Z' => {
                out.push(Command::Close);
                pen.at = pen.start;
                pen.closed = true;
            }
            _ => return false,
        }
        pen.cubic = cubic;
        pen.quad = quad;
        true
    }
}

/// The reflection of a previous control point about the current point, or
/// the current point when the previous command was not the same kind.
fn reflect(control: Option<[f64; 2]>, at: [f64; 2]) -> [f64; 2] {
    control.map_or(at, |c| [2.0 * at[0] - c[0], 2.0 * at[1] - c[1]])
}

/// A quadratic's exact cubic control points.
fn quad_controls(p0: [f64; 2], q: [f64; 2], p: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    (
        [
            p0[0] + 2.0 / 3.0 * (q[0] - p0[0]),
            p0[1] + 2.0 / 3.0 * (q[1] - p0[1]),
        ],
        [
            p[0] + 2.0 / 3.0 * (q[0] - p[0]),
            p[1] + 2.0 / 3.0 * (q[1] - p[1]),
        ],
    )
}

type Emit<'a> = dyn FnMut(Option<[f64; 2]>, Option<[f64; 2]>, [f64; 2]) + 'a;

/// An elliptical arc as cubics of at most 90°, by SVG 2's implementation
/// notes (§B.2.4 endpoint to center, §B.2.5 out-of-range radii). A zero
/// radius is a straight line; identical endpoints draw nothing.
#[allow(clippy::too_many_arguments)]
fn arc(
    from: [f64; 2],
    rx: f64,
    ry: f64,
    angle: f64,
    large: bool,
    sweep: bool,
    to: [f64; 2],
    emit: &mut Emit<'_>,
) {
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        emit(None, None, to);
        return;
    }
    let (sin, cos) = angle.to_radians().sin_cos();
    let dx = (from[0] - to[0]) / 2.0;
    let dy = (from[1] - to[1]) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2.0;
    let angle_of = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            -a
        } else {
            a
        }
    };
    let (ux, uy) = ((x1 - cx1) / rx, (y1 - cy1) / ry);
    let (vx, vy) = ((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let theta = angle_of(1.0, 0.0, ux, uy);
    let mut delta = angle_of(ux, uy, vx, vy) % (2.0 * std::f64::consts::PI);
    if !sweep && delta > 0.0 {
        delta -= 2.0 * std::f64::consts::PI;
    } else if sweep && delta < 0.0 {
        delta += 2.0 * std::f64::consts::PI;
    }
    let pieces = (delta.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = delta / pieces as f64;
    let t = 4.0 / 3.0 * (step / 4.0).tan();
    let on = |a: f64| {
        let (s, c) = a.sin_cos();
        (
            [
                cx + rx * c * cos - ry * s * sin,
                cy + rx * c * sin + ry * s * cos,
            ],
            [-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos],
        )
    };
    let mut a = theta;
    for i in 0..pieces {
        let b = a + step;
        let (p0, d0) = on(a);
        let (p1, d1) = on(b);
        let end = if i + 1 == pieces { to } else { p1 };
        emit(
            Some([p0[0] + t * d0[0], p0[1] + t * d0[1]]),
            Some([p1[0] - t * d1[0], p1[1] - t * d1[1]]),
            end,
        );
        a = b;
    }
}

#[cfg(test)]
mod tests;
