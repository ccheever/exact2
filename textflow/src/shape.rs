//! @ref LLP 1043.000 §3 D1 — the restricted CSS Shapes value vocabulary.

use crate::{finite, FlowShape};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Length {
    Px(f32),
    Percent(f32),
}
impl Length {
    fn parse(s: &str) -> Option<Self> {
        let (s, percent) = s.strip_suffix('%').map_or((s, false), |n| (n, true));
        let s = if percent {
            s
        } else {
            s.strip_suffix("px").unwrap_or(s)
        };
        // Rust's float parser also accepts inf/NaN; CSS numbers do not.
        if s.is_empty()
            || !s
                .bytes()
                .all(|c| c.is_ascii_digit() || b"+-.eE".contains(&c))
        {
            return None;
        }
        let n = exact_num::parse_f32(s).ok()?;
        n.is_finite().then_some(if percent {
            Self::Percent(n)
        } else {
            Self::Px(n)
        })
    }
    fn value(self) -> f32 {
        match self {
            Self::Px(n) | Self::Percent(n) => n,
        }
    }
    fn resolve(self, extent: f32) -> f32 {
        match self {
            Self::Px(n) => n,
            Self::Percent(n) => finite(n as f64 * extent as f64 / 100.0),
        }
    }
    fn css(self) -> String {
        match self {
            Self::Px(n) => format!("{}px", exact_num::Shortest32(n)),
            Self::Percent(n) => format!("{}%", exact_num::Shortest32(n)),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Radius {
    Length(Length),
    Closest,
    Farthest,
}
impl Radius {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "closest-side" => Some(Self::Closest),
            "farthest-side" => Some(Self::Farthest),
            _ => Length::parse(s)
                .filter(|n| n.value() >= 0.0)
                .map(Self::Length),
        }
    }
    fn css(self) -> String {
        match self {
            Self::Length(n) => n.css(),
            Self::Closest => "closest-side".into(),
            Self::Farthest => "farthest-side".into(),
        }
    }
    fn axis(self, center: f32, extent: f32) -> f32 {
        let a = center.abs();
        let b = finite(extent as f64 - center as f64).abs();
        match self {
            Self::Length(n) => n.resolve(extent),
            Self::Closest => a.min(b),
            Self::Farthest => a.max(b),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Default)]
enum Shape {
    #[default]
    None,
    Circle(Radius, [Length; 2]),
    Ellipse([Radius; 2], [Length; 2]),
    Inset([Length; 4], Length),
    Polygon(Arc<[(Length, Length)]>, bool),
}

/// A validated `shape-outside` value; default is `none` (the exclusion's box).
///
/// Accepts circle, ellipse, inset with one round radius, and up to 64 polygon
/// vertices. Polygon fill rules and disjoint band intervals are preserved.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ShapeOutside(Shape);

impl ShapeOutside {
    /// Parse the supported CSS subset; unsupported syntax and nonfinite numbers fail.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim().to_ascii_lowercase();
        if input == "none" {
            return Some(Self::default());
        }
        let (name, body) = input.split_once('(')?;
        let body = body.strip_suffix(')')?;
        if body.contains(['(', ')']) {
            return None;
        }
        let tokens: Vec<_> = body.split_ascii_whitespace().collect();
        let shape = match name {
            "circle" | "ellipse" => {
                let at = tokens
                    .iter()
                    .position(|t| *t == "at")
                    .unwrap_or(tokens.len());
                let position = if at == tokens.len() {
                    center()
                } else {
                    match &tokens[at + 1..] {
                        ["center"] => center(),
                        [x, y] => [Length::parse(x)?, Length::parse(y)?],
                        _ => return None,
                    }
                };
                let radii = &tokens[..at];
                if name == "circle" {
                    let r = match radii {
                        [] => Radius::Closest,
                        [r] => Radius::parse(r)?,
                        _ => return None,
                    };
                    Shape::Circle(r, position)
                } else {
                    let r = match radii {
                        [] => [Radius::Closest; 2],
                        [x, y] => [Radius::parse(x)?, Radius::parse(y)?],
                        _ => return None,
                    };
                    Shape::Ellipse(r, position)
                }
            }
            "inset" => {
                let round = tokens
                    .iter()
                    .position(|t| *t == "round")
                    .unwrap_or(tokens.len());
                let radius = if round == tokens.len() {
                    Length::Px(0.0)
                } else {
                    match &tokens[round + 1..] {
                        [r] => Length::parse(r).filter(|r| r.value() >= 0.0)?,
                        _ => return None,
                    }
                };
                let values: Option<Vec<_>> =
                    tokens[..round].iter().map(|v| Length::parse(v)).collect();
                let values = values?;
                let edges = match values.as_slice() {
                    [a] => [*a; 4],
                    [a, b] => [*a, *b, *a, *b],
                    [a, b, c] => [*a, *b, *c, *b],
                    [a, b, c, d] => [*a, *b, *c, *d],
                    _ => return None,
                };
                Shape::Inset(edges, radius)
            }
            "polygon" => {
                let mut points = Vec::new();
                let mut evenodd = false;
                for (i, part) in body.split(',').enumerate() {
                    let part = part.trim();
                    if i == 0 && matches!(part, "nonzero" | "evenodd") {
                        evenodd = part == "evenodd";
                        continue;
                    }
                    let mut xy = part.split_ascii_whitespace();
                    let x = Length::parse(xy.next()?)?;
                    let y = Length::parse(xy.next()?)?;
                    if xy.next().is_some() || points.len() == 64 {
                        return None;
                    }
                    points.push((x, y));
                }
                if points.is_empty() {
                    return None;
                }
                Shape::Polygon(points.into(), evenodd)
            }
            _ => return None,
        };
        Some(Self(shape))
    }

    /// Serialize a canonical value that parses back to this exact value.
    pub fn css(&self) -> String {
        match &self.0 {
            Shape::None => "none".into(),
            Shape::Circle(r, p) => format!("circle({} at {} {})", r.css(), p[0].css(), p[1].css()),
            Shape::Ellipse(r, p) => format!(
                "ellipse({} {} at {} {})",
                r[0].css(),
                r[1].css(),
                p[0].css(),
                p[1].css()
            ),
            Shape::Inset(e, r) => format!(
                "inset({} {} {} {} round {})",
                e[0].css(),
                e[1].css(),
                e[2].css(),
                e[3].css(),
                r.css()
            ),
            Shape::Polygon(p, evenodd) => {
                let mut text = String::from(if *evenodd {
                    "polygon(evenodd, "
                } else {
                    "polygon("
                });
                for (i, (x, y)) in p.iter().enumerate() {
                    if i > 0 {
                        text.push_str(", ");
                    }
                    text.push_str(&x.css());
                    text.push(' ');
                    text.push_str(&y.css());
                }
                text.push(')');
                text
            }
        }
    }

    /// Whether this value uses the entire reference box.
    pub fn is_none(&self) -> bool {
        matches!(self.0, Shape::None)
    }

    /// Resolve lengths and percentages against `(0, 0, width, height)`.
    ///
    /// Negative/nonfinite box dimensions become zero. Circle percentages use
    /// the normalized diagonal. Overlapping inset edges produce zero area.
    /// A percentage uniform corner radius uses the smaller axis (the circular
    /// approximation to CSS's elliptical percentage corners).
    pub fn resolve(&self, width: f32, height: f32) -> FlowShape {
        let w = dimension(width);
        let h = dimension(height);
        match &self.0 {
            Shape::None => rect(0.0, 0.0, w, h, 0.0),
            Shape::Circle(r, p) => {
                let cx = p[0].resolve(w);
                let cy = p[1].resolve(h);
                let radius = match r {
                    Radius::Length(n) => {
                        n.resolve(finite((w as f64).hypot(h as f64) / 2.0_f64.sqrt()))
                    }
                    Radius::Closest => r.axis(cx, w).min(r.axis(cy, h)),
                    Radius::Farthest => r.axis(cx, w).max(r.axis(cy, h)),
                };
                FlowShape::Circle { cx, cy, r: radius }
            }
            Shape::Ellipse(r, p) => {
                let cx = p[0].resolve(w);
                let cy = p[1].resolve(h);
                FlowShape::Ellipse {
                    cx,
                    cy,
                    rx: r[0].axis(cx, w),
                    ry: r[1].axis(cy, h),
                }
            }
            Shape::Inset(e, r) => {
                let top = e[0].resolve(h);
                let right = e[1].resolve(w);
                let bottom = e[2].resolve(h);
                let left = e[3].resolve(w);
                let iw = finite(w as f64 - left as f64 - right as f64).max(0.0);
                let ih = finite(h as f64 - top as f64 - bottom as f64).max(0.0);
                rect(left, top, iw, ih, r.resolve(w.min(h)).min(iw.min(ih) / 2.0))
            }
            Shape::Polygon(p, evenodd) => {
                let points = p
                    .iter()
                    .map(|(x, y)| (x.resolve(w), y.resolve(h)))
                    .collect();
                if *evenodd {
                    FlowShape::EvenOddPolygon(points)
                } else {
                    FlowShape::Polygon(points)
                }
            }
        }
    }
}
fn center() -> [Length; 2] {
    [Length::Percent(50.0); 2]
}
fn dimension(n: f32) -> f32 {
    if n.is_finite() {
        n.max(0.0)
    } else {
        0.0
    }
}
fn rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> FlowShape {
    FlowShape::RoundRect {
        x,
        y,
        width,
        height,
        radius,
    }
}

#[cfg(test)]
mod css_tests {
    use super::ShapeOutside;

    #[test]
    fn polygons_write_their_points_comma_separated() {
        for (text, css) in [
            (
                "polygon(0% 0%, 100% 50%, 0% 100%)",
                "polygon(0% 0%, 100% 50%, 0% 100%)",
            ),
            (
                "polygon(evenodd, 0px 0px, 10px 0px, 5px 8px)",
                "polygon(evenodd, 0px 0px, 10px 0px, 5px 8px)",
            ),
        ] {
            let shape = ShapeOutside::parse(text).unwrap();
            assert_eq!(shape.css(), css);
            assert_eq!(ShapeOutside::parse(&shape.css()), Some(shape));
        }
    }
}
