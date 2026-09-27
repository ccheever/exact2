//! SVG markers (LLP 1065 D11): `marker-start`, `marker-mid` and
//! `marker-end`, each `none` or a marker — a file-level `marker` declaration
//! the compiler writes into the row whole, as `keyframes` are, so the plan,
//! the runner and every host carry the marker, never a name to look up.
//!
//! Where markers go is SVG 2's (§11.6): `marker-start` on the path's first
//! vertex, `marker-end` on its last, `marker-mid` on every other; a closed
//! subpath's closing point is a vertex, an arc's pieces are one segment.
//! `orient="auto"` turns a marker to the path's direction there — the
//! bisector of the incoming and outgoing directions where both exist, a
//! closed subpath's start taking its closing segment as incoming — and
//! `auto-start-reverse` turns the start marker round. Its content is its
//! view box fitted (`preserveAspectRatio`) into `markerWidth` ×
//! `markerHeight`, scaled by the stroke's width under
//! `markerUnits="strokeWidth"`, `refX`/`refY` on the vertex, and clipped to
//! that viewport (SVG's `overflow: hidden` for markers).

use super::{fit, parse_view_box, Command, PathData, Point, PreserveAspectRatio};
use crate::generated::{FillRule, StrokeLinecap, StrokeLinejoin};
use crate::style::{Color, ColorValue};
use exact_num::Shortest32;
use std::fmt::Write as _;

/// `orient`: `auto`, `auto-start-reverse`, or a fixed angle in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orient {
    /// The path's direction at the vertex.
    Auto,
    /// As `auto`, but the start marker points back along the path.
    AutoStartReverse,
    /// A fixed angle, degrees clockwise.
    Angle(f32),
}

/// A marker shape's paint: SVG's `<paint>` with SVG 2's context paints, the
/// referencing path's own `fill` or `stroke`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapePaint {
    /// Nothing.
    None,
    /// The referencing path's `color`.
    CurrentColor,
    /// The referencing path's fill.
    ContextFill,
    /// The referencing path's stroke.
    ContextStroke,
    /// A colour.
    Color(ColorValue),
}

impl ShapePaint {
    /// `none`, `currentcolor`, `context-fill`, `context-stroke`, or a
    /// colour (a `light-dark()` pair included).
    pub fn parse(text: &str) -> Option<ShapePaint> {
        let text = text.trim();
        Some(match text.to_ascii_lowercase().as_str() {
            "none" => ShapePaint::None,
            "currentcolor" => ShapePaint::CurrentColor,
            "context-fill" => ShapePaint::ContextFill,
            "context-stroke" => ShapePaint::ContextStroke,
            _ => ShapePaint::Color(
                ColorValue::parse_light_dark(text)
                    .or_else(|| Color::parse(text).map(ColorValue::Fixed))?,
            ),
        })
    }

    /// Canonical text, which [`ShapePaint`]'s parser reads back.
    pub fn css(self) -> String {
        match self {
            ShapePaint::None => "none".into(),
            ShapePaint::CurrentColor => "currentcolor".into(),
            ShapePaint::ContextFill => "context-fill".into(),
            ShapePaint::ContextStroke => "context-stroke".into(),
            ShapePaint::Color(ColorValue::Fixed(c)) => hex(c),
            ShapePaint::Color(ColorValue::LightDark(l, d)) => {
                format!("light-dark({}, {})", hex(l), hex(d))
            }
        }
    }
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
}

/// One `path` in a marker: its data and SVG's painting properties, whose
/// initial values it takes where it names none (the declaration has no
/// ancestors to inherit from).
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerShape {
    /// The path, parsed.
    pub data: PathData,
    /// `fill` (initially black).
    pub fill: ShapePaint,
    /// `stroke` (initially `none`).
    pub stroke: ShapePaint,
    /// `stroke-width`, in the marker's content units.
    pub width: f32,
    /// `stroke-linecap`.
    pub cap: StrokeLinecap,
    /// `stroke-linejoin`.
    pub join: StrokeLinejoin,
    /// `stroke-miterlimit`.
    pub miter: f32,
    /// `fill-rule`.
    pub rule: FillRule,
}

impl Default for MarkerShape {
    fn default() -> Self {
        MarkerShape {
            data: PathData::default(),
            fill: ShapePaint::Color(ColorValue::Fixed(Color::rgba(0, 0, 0, 255))),
            stroke: ShapePaint::None,
            width: 1.0,
            cap: StrokeLinecap::Butt,
            join: StrokeLinejoin::Miter,
            miter: 4.0,
            rule: FillRule::Nonzero,
        }
    }
}

/// A `marker` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerDef {
    /// Its name, for the web's `url(#…)` and a reader.
    pub name: String,
    /// `viewBox`, if any.
    pub view_box: Option<[f32; 4]>,
    /// `preserveAspectRatio`.
    pub aspect: PreserveAspectRatio,
    /// `refX`, `refY`: the content point on the vertex.
    pub reference: [f32; 2],
    /// `markerWidth`, `markerHeight` (initially 3 × 3).
    pub size: [f32; 2],
    /// `markerUnits="strokeWidth"` (the initial value), else
    /// `userSpaceOnUse`.
    pub stroke_units: bool,
    /// `orient` (initially the angle 0).
    pub orient: Orient,
    /// Its paths, painted in order.
    pub shapes: Vec<MarkerShape>,
}

impl Default for MarkerDef {
    fn default() -> Self {
        MarkerDef {
            name: String::new(),
            view_box: None,
            aspect: PreserveAspectRatio::default(),
            reference: [0.0; 2],
            size: [3.0; 2],
            stroke_units: true,
            orient: Orient::Angle(0.0),
            shapes: Vec::new(),
        }
    }
}

/// A `marker-*` row: `none`, or a marker.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Marker(pub Option<Box<MarkerDef>>);

impl Marker {
    /// `none`, or the compiler's text: `marker(name;viewBox;aspect;refX
    /// refY;width height;units;orient;shape;…)`, each shape
    /// `d|fill|stroke|width|cap|join|miterlimit|rule`. Only the kernel and
    /// the compiler write it.
    pub fn parse(text: &str) -> Option<Marker> {
        let text = text.trim();
        if text == "none" {
            return Some(Marker(None));
        }
        let inner = text.strip_prefix("marker(")?.strip_suffix(')')?;
        let mut fields = inner.split(';');
        let mut def = MarkerDef {
            name: fields.next()?.trim().into(),
            ..MarkerDef::default()
        };
        if def.name.is_empty() {
            return None;
        }
        def.view_box = match fields.next()?.trim() {
            "none" => None,
            v => Some(parse_view_box(v)?),
        };
        def.aspect = PreserveAspectRatio::parse(fields.next()?)?;
        def.reference = pair(fields.next()?)?;
        def.size = pair(fields.next()?)?;
        if def.size.iter().any(|n| *n < 0.0) {
            return None;
        }
        def.stroke_units = match fields.next()?.trim() {
            "strokeWidth" => true,
            "userSpaceOnUse" => false,
            _ => return None,
        };
        def.orient = match fields.next()?.trim() {
            "auto" => Orient::Auto,
            "auto-start-reverse" => Orient::AutoStartReverse,
            angle => Orient::Angle(number(angle.strip_suffix("deg").unwrap_or(angle))?),
        };
        for shape in fields {
            def.shapes.push(parse_shape(shape)?);
        }
        Some(Marker(Some(Box::new(def))))
    }

    /// Canonical text, which [`Marker::parse`] reads back.
    pub fn css(&self) -> String {
        let Some(def) = &self.0 else {
            return "none".into();
        };
        let n = |v: f32| Shortest32(v).to_string();
        let mut out = format!("marker({};", def.name);
        match def.view_box {
            Some(v) => out.push_str(&v.map(n).join(" ")),
            None => out.push_str("none"),
        }
        let _ = write!(
            out,
            ";{};{} {};{} {};{};{}",
            def.aspect.css(),
            n(def.reference[0]),
            n(def.reference[1]),
            n(def.size[0]),
            n(def.size[1]),
            if def.stroke_units {
                "strokeWidth"
            } else {
                "userSpaceOnUse"
            },
            match def.orient {
                Orient::Auto => "auto".into(),
                Orient::AutoStartReverse => "auto-start-reverse".into(),
                Orient::Angle(a) => n(a),
            }
        );
        for s in &def.shapes {
            let _ = write!(
                out,
                ";{}|{}|{}|{}|{}|{}|{}|{}",
                s.data.css(),
                s.fill.css(),
                s.stroke.css(),
                n(s.width),
                s.cap.name(),
                s.join.name(),
                n(s.miter),
                s.rule.name()
            );
        }
        out.push(')');
        out
    }

    /// The marker, unless `none`.
    pub fn def(&self) -> Option<&MarkerDef> {
        self.0.as_deref()
    }
}

fn number(text: &str) -> Option<f32> {
    let n = exact_num::parse_f32(text.trim()).ok()?;
    n.is_finite().then_some(n)
}

fn pair(text: &str) -> Option<[f32; 2]> {
    let mut words = text.split_ascii_whitespace();
    let out = [number(words.next()?)?, number(words.next()?)?];
    words.next().is_none().then_some(out)
}

fn parse_shape(text: &str) -> Option<MarkerShape> {
    let f: Vec<&str> = text.split('|').collect();
    let [d, fill, stroke, width, cap, join, miter, rule] = f[..] else {
        return None;
    };
    let data = PathData::parse(d);
    let shape = MarkerShape {
        fill: ShapePaint::parse(fill)?,
        stroke: ShapePaint::parse(stroke)?,
        width: number(width).filter(|w| *w >= 0.0)?,
        cap: StrokeLinecap::from_name(cap.trim())?,
        join: StrokeLinejoin::from_name(join.trim())?,
        miter: number(miter).filter(|m| *m >= 1.0)?,
        rule: FillRule::from_name(rule.trim())?,
        data,
    };
    (shape.data.error().is_none()).then_some(shape)
}

/// Which `marker-*` property a vertex takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// `marker-start`: the path's first vertex.
    Start,
    /// `marker-mid`: every vertex but the first and last.
    Mid,
    /// `marker-end`: the path's last vertex.
    End,
}

/// A vertex of a path: where it is, the path's direction there, and how far
/// along the whole path (subpaths in order) it lies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// The point, in path units.
    pub point: Point,
    /// The direction `orient="auto"` takes, degrees clockwise from +x.
    pub angle: f64,
    /// The length along the whole path at the vertex.
    pub at: f64,
    /// Which marker property it takes.
    pub slot: Slot,
}

/// One marker drawn: the transform from its viewport to path units
/// (`[a, b, c, d, e, f]`, `x' = a x + c y + e`, `y' = b x + d y + f`), the
/// viewport it clips to (`[x, y, width, height]` in that space), the view
/// box's fit into it (`[sx, sy, dx, dy]`), and the vertex's length along the
/// path, which a trim shows or hides it by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Viewport space to path units.
    pub outer: [f32; 6],
    /// The viewport, in viewport space.
    pub clip: [f32; 4],
    /// The content's view box into viewport space.
    pub fit: [f32; 4],
    /// The vertex's length along the whole path.
    pub at: f64,
}

impl MarkerDef {
    /// The marker on `vertex`, for a path stroked `stroke_width` wide;
    /// `None` when its viewport is empty (SVG draws nothing).
    pub fn place(&self, vertex: &Vertex, stroke_width: f32) -> Option<Placed> {
        let [w, h] = self.size;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let k = if self.stroke_units { stroke_width } else { 1.0 };
        let [sx, sy, dx, dy] = fit(self.view_box, self.aspect, w, h);
        let r = [self.reference[0] * sx + dx, self.reference[1] * sy + dy];
        let angle = match self.orient {
            Orient::Auto => vertex.angle,
            Orient::AutoStartReverse if vertex.slot == Slot::Start => vertex.angle + 180.0,
            Orient::AutoStartReverse => vertex.angle,
            Orient::Angle(a) => f64::from(a),
        };
        let (sin, cos) = angle.to_radians().sin_cos();
        let (sin, cos, k) = (sin as f32, cos as f32, k);
        let [a, b, c, d] = [k * cos, k * sin, -k * sin, k * cos];
        let [px, py] = vertex.point;
        Some(Placed {
            outer: [
                a,
                b,
                c,
                d,
                px - a * r[0] - c * r[1],
                py - b * r[0] - d * r[1],
            ],
            clip: [0.0, 0.0, w, h],
            fit: [sx, sy, dx, dy],
            at: vertex.at,
        })
    }
}

/// Every marker a path draws, in SVG's order (start, the mids along the
/// path, end), each with its definition.
pub fn markers<'m>(
    data: &PathData,
    [start, mid, end]: [&'m Marker; 3],
    stroke_width: f32,
) -> Vec<(&'m MarkerDef, Placed)> {
    if start.0.is_none() && mid.0.is_none() && end.0.is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for vertex in data.vertices() {
        let marker = match vertex.slot {
            Slot::Start => start,
            Slot::Mid => mid,
            Slot::End => end,
        };
        if let Some(def) = marker.def() {
            if let Some(placed) = def.place(&vertex, stroke_width) {
                out.push((def, placed));
            }
        }
    }
    out
}

/// A segment's directions (degrees) at its start and end; `None` for a
/// segment of no length.
fn directions(from: Point, command: Command, first: Point) -> (Option<f64>, Option<f64>, Point) {
    let dir = |a: Point, b: Point| {
        let (x, y) = (f64::from(b[0] - a[0]), f64::from(b[1] - a[1]));
        (x != 0.0 || y != 0.0).then(|| y.atan2(x).to_degrees())
    };
    match command {
        Command::Line(p) => (dir(from, p), dir(from, p), p),
        Command::Close => (dir(from, first), dir(from, first), first),
        Command::Cubic(c1, c2, p) => (
            dir(from, c1).or(dir(from, c2)).or(dir(from, p)),
            dir(c2, p).or(dir(c1, p)).or(dir(from, p)),
            p,
        ),
        Command::Move(p) => (None, None, p),
    }
}

impl PathData {
    /// The path's vertices, in order, each with its slot: the first
    /// `Start`, the last `End`, the rest `Mid`. A lone moveto has none; a
    /// path with one vertex has it twice, `Start` then `End`.
    pub fn vertices(&self) -> Vec<Vertex> {
        let mut out = Vec::new();
        let mut walked = 0.0;
        for ((commands, length), (first_index, _)) in
            self.subpaths().zip(self.lengths()).zip(&self.subpaths)
        {
            let [Command::Move(first), rest @ ..] = commands else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            // Logical segments: an arc's pieces are one.
            struct Seg {
                end: Point,
                d0: Option<f64>,
                d1: Option<f64>,
                length: f64,
            }
            let mut segs: Vec<Seg> = Vec::new();
            let mut at = *first;
            for (i, command) in rest.iter().enumerate() {
                let (d0, d1, end) = directions(at, *command, *first);
                let length = super::Segment::of(at, *command, *first).length();
                at = end;
                let seam = self.seams.contains(&(first_index + 1 + i));
                match segs.last_mut() {
                    Some(last) if seam => {
                        last.end = end;
                        last.d0 = last.d0.or(d0);
                        last.d1 = d1.or(last.d1);
                        last.length += length;
                    }
                    _ => segs.push(Seg {
                        end,
                        d0,
                        d1,
                        length,
                    }),
                }
            }
            let closed = matches!(rest.last(), Some(Command::Close));
            let n = segs.len();
            let incoming = |k: usize| segs[..k].iter().rev().find_map(|s| s.d1);
            let outgoing = |k: usize| segs[k..].iter().find_map(|s| s.d0);
            let mut position = walked;
            for k in 0..=n {
                let point = if k == 0 { *first } else { segs[k - 1].end };
                if k > 0 {
                    position += segs[k - 1].length;
                }
                let inward = match k {
                    0 if closed => incoming(n),
                    0 => None,
                    _ => incoming(k),
                };
                let outward = match k {
                    _ if k == n && closed => outgoing(0),
                    _ if k == n => None,
                    _ => outgoing(k),
                };
                let angle = match (inward, outward) {
                    (Some(a), Some(b)) => {
                        let diff = (b - a + 540.0).rem_euclid(360.0) - 180.0;
                        a + diff / 2.0
                    }
                    (a, b) => a.or(b).unwrap_or(0.0),
                };
                out.push(Vertex {
                    point,
                    angle,
                    at: position.min(walked + length),
                    slot: Slot::Mid,
                });
            }
            walked += length;
        }
        if let Some(first) = out.first_mut() {
            first.slot = Slot::Start;
        }
        if out.len() == 1 {
            let mut end = out[0];
            end.slot = Slot::End;
            out.push(end);
        } else if let Some(last) = out.last_mut() {
            last.slot = Slot::End;
        }
        out
    }
}

#[cfg(test)]
mod tests;
