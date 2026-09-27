//! Transforms on SVG elements: the `transform` list (CSS's grammar and
//! SVG's attribute grammar), `transform-origin`, and 2-D affine arithmetic.
//!
//! @ref LLP 1055.000 D5; CSS Transforms 1 §7 (functions), §6 (the
//! transform-origin sandwich), CSS Transforms 2 §6.1 (the individual
//! properties first); SVG 2 §8.5 (the attribute grammar: unitless numbers,
//! commas, `rotate(a cx cy)`)

use crate::style::Dimension;

/// An affine `[a, b, c, d, e, f]`: x' = a·x + c·y + e, y' = b·x + d·y + f,
/// as SVG's `matrix()` and Core Graphics' `CGAffineTransform` write it.
pub type Affine = [f32; 6];

/// The identity.
pub const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a · b`: apply `b`, then `a`.
pub fn mul(a: Affine, b: Affine) -> Affine {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

/// The inverse, or `None` when singular.
pub fn invert(m: Affine) -> Option<Affine> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let i = 1.0 / det;
    Some([
        m[3] * i,
        -m[1] * i,
        -m[2] * i,
        m[0] * i,
        (m[2] * m[5] - m[3] * m[4]) * i,
        (m[1] * m[4] - m[0] * m[5]) * i,
    ])
}

/// A point through `m`.
pub fn apply(m: Affine, (x, y): (f32, f32)) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// A translation.
pub fn translate(x: f32, y: f32) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

/// A scale.
pub fn scale(x: f32, y: f32) -> Affine {
    [x, 0.0, 0.0, y, 0.0, 0.0]
}

/// A rotation by `deg` degrees, clockwise in SVG's y-down space.
pub fn rotate(deg: f32) -> Affine {
    let (s, c) = (deg as f64).to_radians().sin_cos();
    [c as f32, s as f32, -s as f32, c as f32, 0.0, 0.0]
}

/// Whether `m` is the identity.
pub fn is_identity(m: Affine) -> bool {
    m == IDENTITY
}

/// One transform function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformFn {
    /// `matrix(a, b, c, d, e, f)`.
    Matrix(Affine),
    /// `translate(x, y)`, `translateX`, `translateY`.
    Translate(f32, f32),
    /// `scale(x, y)`, `scaleX`, `scaleY`.
    Scale(f32, f32),
    /// `rotate(a)` in degrees, or SVG's `rotate(a cx cy)`.
    Rotate(f32, f32, f32),
    /// `skewX(a)`, `skewY(a)`, `skew(ax, ay)`, in degrees.
    Skew(f32, f32),
}

impl TransformFn {
    /// The function's matrix.
    pub fn matrix(&self) -> Affine {
        match *self {
            TransformFn::Matrix(m) => m,
            TransformFn::Translate(x, y) => translate(x, y),
            TransformFn::Scale(x, y) => scale(x, y),
            TransformFn::Rotate(a, cx, cy) => {
                mul(translate(cx, cy), mul(rotate(a), translate(-cx, -cy)))
            }
            TransformFn::Skew(ax, ay) => {
                let t = |d: f32| (d as f64).to_radians().tan() as f32;
                [1.0, t(ay), t(ax), 1.0, 0.0, 0.0]
            }
        }
    }
}

/// CSS `transform` on an SVG element (`none` is empty).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransformList(pub Vec<TransformFn>);

impl TransformList {
    /// Most functions a list may carry.
    pub const MAX: usize = 32;

    /// CSS's grammar (`rotate(45deg) translate(10px, 5px)`) or SVG's
    /// attribute grammar (`rotate(45 10 10), translate(10 5)`): numbers
    /// with or without units, separated by commas and/or whitespace;
    /// functions separated by whitespace and/or commas. Translations take
    /// user units or `px`; angles `deg` (or unitless), `rad`, `grad`,
    /// `turn`. Percentages in `translate` are refused.
    pub fn parse(css: &str) -> Option<TransformList> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") || t.is_empty() {
            return Some(TransformList::default());
        }
        let mut out = Vec::new();
        let mut rest = t;
        loop {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
            if rest.is_empty() {
                break;
            }
            let open = rest.find('(')?;
            let name = rest[..open].trim();
            let close = rest[open..].find(')')? + open;
            let args: Vec<&str> = rest[open + 1..close]
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|a| !a.is_empty())
                .collect();
            out.push(function(name, &args)?);
            rest = &rest[close + 1..];
            if out.len() > Self::MAX {
                return None;
            }
        }
        Some(TransformList(out))
    }

    /// The list in CSS's grammar, which the web host emits as the CSS
    /// property (SVG's `rotate(a cx cy)` becomes its three CSS functions).
    pub fn css(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let n = |v: f32| exact_num::Shortest(v as f64).to_string();
        let parts: Vec<String> = self
            .0
            .iter()
            .map(|f| match *f {
                TransformFn::Matrix(m) => format!(
                    "matrix({}, {}, {}, {}, {}, {})",
                    n(m[0]),
                    n(m[1]),
                    n(m[2]),
                    n(m[3]),
                    n(m[4]),
                    n(m[5])
                ),
                TransformFn::Translate(x, y) => format!("translate({}px, {}px)", n(x), n(y)),
                TransformFn::Scale(x, y) => format!("scale({}, {})", n(x), n(y)),
                TransformFn::Rotate(a, 0.0, 0.0) => format!("rotate({}deg)", n(a)),
                TransformFn::Rotate(a, cx, cy) => format!(
                    "translate({}px, {}px) rotate({}deg) translate({}px, {}px)",
                    n(cx),
                    n(cy),
                    n(a),
                    n(-cx),
                    n(-cy)
                ),
                TransformFn::Skew(ax, 0.0) => format!("skewX({}deg)", n(ax)),
                TransformFn::Skew(0.0, ay) => format!("skewY({}deg)", n(ay)),
                TransformFn::Skew(ax, ay) => format!("skew({}deg, {}deg)", n(ax), n(ay)),
            })
            .collect();
        parts.join(" ")
    }

    /// The list's matrix: the functions multiplied left to right.
    pub fn matrix(&self) -> Affine {
        self.0.iter().fold(IDENTITY, |m, f| mul(m, f.matrix()))
    }

    /// Whether every number is finite.
    pub fn is_finite(&self) -> bool {
        self.matrix().iter().all(|v| v.is_finite())
    }
}

fn function(name: &str, args: &[&str]) -> Option<TransformFn> {
    let len = |a: &str| length(a);
    let num = |a: &str| number(a);
    let ang = |a: &str| angle(a);
    Some(match (name.to_ascii_lowercase().as_str(), args.len()) {
        ("matrix", 6) => {
            let mut m = [0.0; 6];
            for (i, a) in args.iter().enumerate() {
                m[i] = num(a)?;
            }
            TransformFn::Matrix(m)
        }
        ("translate", 1) => TransformFn::Translate(len(args[0])?, 0.0),
        ("translate", 2) => TransformFn::Translate(len(args[0])?, len(args[1])?),
        ("translatex", 1) => TransformFn::Translate(len(args[0])?, 0.0),
        ("translatey", 1) => TransformFn::Translate(0.0, len(args[0])?),
        ("scale", 1) => {
            let s = num(args[0])?;
            TransformFn::Scale(s, s)
        }
        ("scale", 2) => TransformFn::Scale(num(args[0])?, num(args[1])?),
        ("scalex", 1) => TransformFn::Scale(num(args[0])?, 1.0),
        ("scaley", 1) => TransformFn::Scale(1.0, num(args[0])?),
        ("rotate", 1) => TransformFn::Rotate(ang(args[0])?, 0.0, 0.0),
        ("rotate", 3) => TransformFn::Rotate(ang(args[0])?, num(args[1])?, num(args[2])?),
        ("skewx", 1) => TransformFn::Skew(ang(args[0])?, 0.0),
        ("skewy", 1) => TransformFn::Skew(0.0, ang(args[0])?),
        ("skew", 1) => TransformFn::Skew(ang(args[0])?, 0.0),
        ("skew", 2) => TransformFn::Skew(ang(args[0])?, ang(args[1])?),
        _ => return None,
    })
}

fn number(text: &str) -> Option<f32> {
    let v = exact_num::parse_f64(text.trim()).ok()? as f32;
    v.is_finite().then_some(v)
}

fn length(text: &str) -> Option<f32> {
    number(text.strip_suffix("px").unwrap_or(text))
}

fn angle(text: &str) -> Option<f32> {
    let (n, scale) = if let Some(n) = text.strip_suffix("deg") {
        (n, 1.0)
    } else if let Some(n) = text.strip_suffix("grad") {
        (n, 0.9)
    } else if let Some(n) = text.strip_suffix("rad") {
        (n, 180.0 / std::f32::consts::PI)
    } else if let Some(n) = text.strip_suffix("turn") {
        (n, 360.0)
    } else {
        (text, 1.0)
    };
    number(n).map(|v| v * scale)
}

/// CSS `transform-origin`: two positions (a third, `z`, must be zero).
/// Its CSS initial value is `50% 50%`; an SVG element without the row
/// takes the UA stylesheet's `0 0` ([`TransformOrigin::SVG`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformOrigin {
    /// Horizontal: user units, or a percentage of the reference box.
    pub x: Dimension,
    /// Vertical.
    pub y: Dimension,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        TransformOrigin {
            x: Dimension::Percent(50.0),
            y: Dimension::Percent(50.0),
        }
    }
}

impl TransformOrigin {
    /// The UA's origin for SVG elements.
    pub const SVG: TransformOrigin = TransformOrigin {
        x: Dimension::Points(0.0),
        y: Dimension::Points(0.0),
    };

    /// `left`/`center`/`right`, `top`/`center`/`bottom`, lengths and
    /// percentages; one value sets x (or y for `top`/`bottom`), the other
    /// centres. Keywords may come in either order.
    pub fn parse(css: &str) -> Option<TransformOrigin> {
        let words: Vec<&str> = css.split_whitespace().collect();
        let part = |w: &str| -> Option<(Dimension, Option<bool>)> {
            // (value, Some(true) = horizontal keyword, Some(false) = vertical)
            Some(match w.to_ascii_lowercase().as_str() {
                "left" => (Dimension::Percent(0.0), Some(true)),
                "right" => (Dimension::Percent(100.0), Some(true)),
                "top" => (Dimension::Percent(0.0), Some(false)),
                "bottom" => (Dimension::Percent(100.0), Some(false)),
                "center" => (Dimension::Percent(50.0), None),
                _ => (super::length::Length::parse(w)?.dimension(), None),
            })
        };
        let center = Dimension::Percent(50.0);
        match words.as_slice() {
            [a] => {
                let (v, axis) = part(a)?;
                Some(if axis == Some(false) {
                    TransformOrigin { x: center, y: v }
                } else {
                    TransformOrigin { x: v, y: center }
                })
            }
            [a, b] | [a, b, _] => {
                if let [_, _, z] = words.as_slice() {
                    if length(z)? != 0.0 {
                        return None;
                    }
                }
                let (va, aa) = part(a)?;
                let (vb, ab) = part(b)?;
                if aa == Some(false) || ab == Some(true) {
                    // `top left`: swapped keywords.
                    (aa != Some(true) && ab != Some(false))
                        .then_some(TransformOrigin { x: vb, y: va })
                } else {
                    Some(TransformOrigin { x: va, y: vb })
                }
            }
            _ => None,
        }
    }

    /// The value as CSS reads it.
    pub fn css(&self) -> String {
        let d = |d: Dimension| match d {
            Dimension::Percent(p) => format!("{}%", exact_num::Shortest(p as f64)),
            Dimension::Points(v) => format!("{}px", exact_num::Shortest(v as f64)),
            _ => "0px".into(),
        };
        format!("{} {}", d(self.x), d(self.y))
    }

    /// Whether both numbers are finite.
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// The origin in user units, against a reference box `(x, y, w, h)`.
    pub fn point(&self, reference: (f32, f32, f32, f32)) -> (f32, f32) {
        (
            reference.0 + super::length::resolve(self.x, reference.2),
            reference.1 + super::length::resolve(self.y, reference.3),
        )
    }
}
