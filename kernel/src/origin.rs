//! CSS `transform-origin` (CSS Transforms 1 §6), in two dimensions.
//!
//! The point `translate`, `rotate` and `scale` (and a host's press feedback,
//! which folds into `scale`) turn about, from the border box's top-left
//! corner. The initial value is `50% 50%`, the centre. A third, `z` length is
//! CSS and is accepted, but it moves nothing a plane's transforms can show,
//! so it is not kept.

use crate::Dimension;
use std::fmt::Write;

/// A computed `transform-origin`: each axis points or a percentage of the
/// border box on that axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformOrigin {
    /// From the left edge.
    pub x: Dimension,
    /// From the top edge.
    pub y: Dimension,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        Self {
            x: Dimension::Percent(50.0),
            y: Dimension::Percent(50.0),
        }
    }
}

impl TransformOrigin {
    /// CSS's grammar: one value (a keyword or `<length-percentage>`, the
    /// other axis `center`), or two — `<x> <y>` in that order when either is
    /// a length, keywords in either order (`top left`) — then an optional
    /// `z` `<length>`. Lengths are `px` or a unitless zero.
    pub fn parse(css: &str) -> Option<Self> {
        let mut parts: Vec<&str> = css.split_ascii_whitespace().collect();
        if parts.len() == 3 {
            length(parts.pop()?)?; // z: a length, never a percentage
        }
        let values: Vec<Value> = parts.iter().map(|p| value(p)).collect::<Option<_>>()?;
        let (x, y) = match values[..] {
            [Value::Y(y)] => (Dimension::Percent(50.0), y),
            [Value::X(x) | Value::Center(x) | Value::Length(x)] => (x, Dimension::Percent(50.0)),
            // Keywords name their axis, so they may come in either order.
            [Value::Y(y), Value::X(x) | Value::Center(x)] | [Value::Center(y), Value::X(x)] => {
                (x, y)
            }
            [Value::X(x) | Value::Center(x) | Value::Length(x), Value::Y(y) | Value::Center(y) | Value::Length(y)] => {
                (x, y)
            }
            _ => return None,
        };
        Some(Self { x, y })
    }

    /// The declaration's CSS value, each axis as a percentage or `px`.
    pub fn css(&self) -> String {
        let mut out = String::new();
        for (i, d) in [self.x, self.y].into_iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let _ = match d {
                Dimension::Percent(p) => write!(out, "{}%", exact_num::Shortest32(p)),
                Dimension::Points(p) => write!(out, "{}px", exact_num::Shortest32(p)),
                _ => unreachable!("an origin is points or a percentage"),
            };
        }
        out
    }

    /// The point in a `width` × `height` border box, from its top-left.
    pub fn resolve(&self, width: f32, height: f32) -> (f32, f32) {
        let axis = |d: Dimension, size: f32| match d {
            Dimension::Percent(p) => size * p / 100.0,
            Dimension::Points(p) => p,
            _ => size / 2.0,
        };
        (axis(self.x, width), axis(self.y, height))
    }

    /// Whether it is the centre, the initial value.
    pub fn centred(&self) -> bool {
        *self == Self::default()
    }
}

/// One component: a keyword names its axis (`center` either), a length
/// neither.
enum Value {
    X(Dimension),
    Y(Dimension),
    Center(Dimension),
    Length(Dimension),
}

fn value(token: &str) -> Option<Value> {
    let pct = Dimension::Percent;
    Some(match token.to_ascii_lowercase().as_str() {
        "left" => Value::X(pct(0.0)),
        "right" => Value::X(pct(100.0)),
        "top" => Value::Y(pct(0.0)),
        "bottom" => Value::Y(pct(100.0)),
        "center" => Value::Center(pct(50.0)),
        other => Value::Length(match other.strip_suffix('%') {
            Some(p) => pct(finite(p)?),
            None => Dimension::Points(length(other)?),
        }),
    })
}

fn length(token: &str) -> Option<f32> {
    match token.to_ascii_lowercase().strip_suffix("px") {
        Some(px) => finite(px),
        None => (finite(token)? == 0.0).then_some(0.0),
    }
}

fn finite(number: &str) -> Option<f32> {
    exact_num::parse_f32(number).ok().filter(|n| n.is_finite())
}

#[cfg(test)]
mod tests {
    use super::TransformOrigin;
    use crate::Dimension::{Percent as P, Points as Px};

    #[test]
    fn the_css_grammar() {
        let o = |css: &str| TransformOrigin::parse(css).map(|o| (o.x, o.y));
        assert_eq!(o("center"), Some((P(50.0), P(50.0))));
        assert_eq!(o("left"), Some((P(0.0), P(50.0))));
        assert_eq!(o("top"), Some((P(50.0), P(0.0))));
        assert_eq!(o("bottom"), Some((P(50.0), P(100.0))));
        assert_eq!(o("10px"), Some((Px(10.0), P(50.0))));
        assert_eq!(o("left top"), Some((P(0.0), P(0.0))));
        assert_eq!(o("top left"), Some((P(0.0), P(0.0))));
        assert_eq!(o("bottom right"), Some((P(100.0), P(100.0))));
        assert_eq!(o("center top"), Some((P(50.0), P(0.0))));
        assert_eq!(o("top center"), Some((P(50.0), P(0.0))));
        assert_eq!(o("center left"), Some((P(0.0), P(50.0))));
        assert_eq!(o("25% 75%"), Some((P(25.0), P(75.0))));
        assert_eq!(o("right 8px"), Some((P(100.0), Px(8.0))));
        assert_eq!(o("-4px bottom"), Some((Px(-4.0), P(100.0))));
        assert_eq!(o("0 0"), Some((Px(0.0), Px(0.0))));
        assert_eq!(o("LEFT 10PX 3px"), Some((P(0.0), Px(10.0))));
        for bad in [
            "",
            "left right",
            "top bottom",
            "10px left",
            "top 10px",
            "8",
            "1em",
            "50% 50% 10%",
            "left top 0 0",
            "middle",
            "NaN%",
            "infpx",
        ] {
            assert_eq!(o(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn round_trips_and_resolves() {
        for css in ["50% 50%", "0% 100%", "10px -2.5px", "33.5% 4px"] {
            let parsed = TransformOrigin::parse(css).unwrap();
            assert_eq!(parsed.css(), css);
        }
        let p = |css: &str| TransformOrigin::parse(css).unwrap();
        assert_eq!(p("left top").resolve(200.0, 100.0), (0.0, 0.0));
        assert_eq!(p("center").resolve(200.0, 100.0), (100.0, 50.0));
        assert_eq!(p("right 10px").resolve(200.0, 100.0), (200.0, 10.0));
        assert!(p("50% 50%").centred() && TransformOrigin::default().centred());
        assert!(!p("left").centred());
    }
}
