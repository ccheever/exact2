//! CSS's 3D transform properties (LLP 1076 D8): `rotate`'s axis forms,
//! `translate`'s z, and `perspective`. `rotate` stays the angle row the
//! engine animates; its axis is a row of its own, which the `rotate`
//! attribute sets beside it, each row taking its part of one value.

use super::parse_pixel_length;

/// The axis a `rotate` turns about, as authored (not normalised): `z` for
/// a bare angle, CSS's initial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateAxis(pub [f32; 3]);

impl Default for RotateAxis {
    fn default() -> Self {
        Self([0.0, 0.0, 1.0])
    }
}

/// An angle in degrees: `deg`, `rad`, `grad`, `turn`, or a bare number.
fn angle(word: &str) -> Option<f32> {
    let lower = word.to_ascii_lowercase();
    let (number, per_degree) = [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f32::consts::PI),
        ("turn", 360.0),
    ]
    .into_iter()
    .find_map(|(unit, k)| lower.strip_suffix(unit).map(|n| (n.to_string(), k)))
    .unwrap_or((lower.clone(), 1.0));
    let n = exact_num::parse_f32(&number).ok()? * per_degree;
    n.is_finite().then_some(n)
}

/// `rotate`'s value: `none`, `<angle>`, `x|y|z <angle>`, or
/// `<number>{3} <angle>`, as its axis and its angle in degrees.
pub fn rotate(text: &str) -> Option<([f32; 3], f32)> {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    match words[..] {
        [none] if none.eq_ignore_ascii_case("none") => Some(([0.0, 0.0, 1.0], 0.0)),
        [a] => Some(([0.0, 0.0, 1.0], angle(a)?)),
        [axis, a] | [a, axis] if angle(a).is_some() && axis.len() == 1 => {
            let axis = match axis.to_ascii_lowercase().as_str() {
                "x" => [1.0, 0.0, 0.0],
                "y" => [0.0, 1.0, 0.0],
                "z" => [0.0, 0.0, 1.0],
                _ => return None,
            };
            Some((axis, angle(a)?))
        }
        [x, y, z, a] => {
            let v = [x, y, z].map(|n| exact_num::parse_f32(n).ok().filter(|n| n.is_finite()));
            let [Some(x), Some(y), Some(z)] = v else {
                return None;
            };
            (x != 0.0 || y != 0.0 || z != 0.0).then_some(([x, y, z], angle(a)?))
        }
        _ => None,
    }
}

impl RotateAxis {
    /// The row's parse: the axis of a `rotate` value, or the axis alone
    /// (its wire form, `css()`). An axis along z is z: a `-z` one turns the
    /// angle the other way instead (`f32_row`), so the 2D path draws it.
    pub fn parse(css: &str) -> Option<Self> {
        let axis = rotate(css)
            .map(|(axis, _)| axis)
            .or_else(|| axis_only(css))?;
        Some(Self(if axis[0] == 0.0 && axis[1] == 0.0 {
            [0.0, 0.0, 1.0]
        } else {
            axis
        }))
    }

    /// Canonical CSS: `x`, `y`, `z`, or three numbers.
    pub fn css(&self) -> String {
        match self.0 {
            [1.0, 0.0, 0.0] => "x".into(),
            [0.0, 1.0, 0.0] => "y".into(),
            [0.0, 0.0, 1.0] => "z".into(),
            [x, y, z] => exact_num::text!(
                "{} {} {}",
                exact_num::Shortest32(x),
                exact_num::Shortest32(y),
                exact_num::Shortest32(z)
            ),
        }
    }

    /// Whether it turns out of the screen's plane.
    pub fn is_3d(&self) -> bool {
        self.0[0] != 0.0 || self.0[1] != 0.0
    }
}

/// An axis on its own: `x`, `y`, `z` or three numbers.
fn axis_only(text: &str) -> Option<[f32; 3]> {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    match words[..] {
        [w] => match w.to_ascii_lowercase().as_str() {
            "x" => Some([1.0, 0.0, 0.0]),
            "y" => Some([0.0, 1.0, 0.0]),
            "z" => Some([0.0, 0.0, 1.0]),
            _ => None,
        },
        [x, y, z] => {
            let v = [x, y, z].map(|n| exact_num::parse_f32(n).ok().filter(|n| n.is_finite()));
            let [Some(x), Some(y), Some(z)] = v else {
                return None;
            };
            (x != 0.0 || y != 0.0 || z != 0.0).then_some([x, y, z])
        }
        _ => None,
    }
}

/// `translate`'s z: its third length, 0 when it has two or fewer.
pub fn translate_z(text: &str) -> Option<f32> {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    match words[..] {
        [_] | [_, _] => Some(0.0),
        [_, _, z] => parse_pixel_length(z),
        _ => None,
    }
}

/// `perspective`: `none` (0) or a nonnegative length in px.
pub fn perspective(text: &str) -> Option<f32> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    parse_pixel_length(t).filter(|n| *n >= 0.0)
}

/// The `f32` rows that take CSS text or a range of their own (LLP 1076):
/// `rotate`'s angle, `translate`'s z and `perspective` (D8), a symbol's
/// value (D11) and `-webkit-text-stroke`'s width (D7). `None` for any other
/// row, which the plain number parse takes.
pub(crate) fn f32_row(
    value: &super::StyleValue,
    style: crate::StyleId,
) -> Option<Result<f32, crate::error::StyleValueError>> {
    use super::StyleValue;
    use crate::error::StyleValueError;
    use crate::StyleId;
    let wrong = |expected| Err(StyleValueError::WrongKind { style, expected });
    Some(match (style, value) {
        // A -z axis is z with the angle turned the other way (`RotateAxis`).
        (StyleId::Rotate, StyleValue::Text(t)) => rotate(t)
            .map(|(a, deg)| {
                if a[0] == 0.0 && a[1] == 0.0 && a[2] < 0.0 {
                    -deg
                } else {
                    deg
                }
            })
            .map_or_else(
                || wrong("an angle, and optionally an axis: `x`, `y`, `z` or three numbers"),
                Ok,
            ),
        (StyleId::TranslateZ, StyleValue::Text(t)) => {
            translate_z(t).map_or_else(|| wrong("up to three lengths in px"), Ok)
        }
        (StyleId::Perspective, StyleValue::Text(t)) => {
            perspective(t).map_or_else(|| wrong("`none` or a nonnegative length"), Ok)
        }
        (StyleId::Perspective, StyleValue::Number(n)) if n.is_nan() || *n < 0.0 => {
            wrong("`none` or a nonnegative length")
        }
        (StyleId::SymbolValue, StyleValue::Text(t)) if t.trim().eq_ignore_ascii_case("none") => {
            Ok(-1.0)
        }
        (StyleId::SymbolValue, StyleValue::Number(n)) if (0.0..=1.0).contains(n) || *n == -1.0 => {
            Ok(*n as f32)
        }
        (StyleId::SymbolValue, _) => wrong("`none` or a number from 0 to 1"),
        (StyleId::TextStrokeWidth, StyleValue::Number(n))
            if (*n as f32).is_finite() && *n >= 0.0 =>
        {
            Ok(*n as f32)
        }
        (StyleId::TextStrokeWidth, StyleValue::Text(t)) => super::stroke::parse(t)
            .map(|(w, _)| w)
            .map_err(|reason| StyleValueError::BadTextStroke { style, reason }),
        (StyleId::TextStrokeWidth, _) => wrong("a nonnegative width in px"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_takes_css_forms_and_splits_into_axis_and_angle() {
        assert_eq!(rotate("45deg"), Some(([0.0, 0.0, 1.0], 45.0)));
        assert_eq!(rotate("x 0.25turn"), Some(([1.0, 0.0, 0.0], 90.0)));
        assert_eq!(rotate("30deg y"), Some(([0.0, 1.0, 0.0], 30.0)));
        assert_eq!(rotate("1 1 0 10deg"), Some(([1.0, 1.0, 0.0], 10.0)));
        assert_eq!(rotate("none"), Some(([0.0, 0.0, 1.0], 0.0)));
        assert_eq!(rotate("12"), Some(([0.0, 0.0, 1.0], 12.0)));
        assert_eq!(rotate("w 10deg"), None);
        assert_eq!(rotate("0 0 0 10deg"), None);
        assert_eq!(RotateAxis::parse("y 30deg").unwrap().css(), "y");
        assert!(RotateAxis::parse("y 30deg").unwrap().is_3d());
        // The wire form round-trips, and a -z axis is z (its angle turns).
        for text in ["y 30deg", "1 1 0 10deg", "x 1turn"] {
            let a = RotateAxis::parse(text).unwrap();
            assert_eq!(RotateAxis::parse(&a.css()), Some(a), "{text}");
        }
        assert_eq!(
            RotateAxis::parse("0 0 -1 30deg").unwrap().0,
            [0.0, 0.0, 1.0]
        );
        let mut s = crate::StyleProps::default();
        s.set_dynamic(
            crate::StyleId::Rotate,
            &crate::StyleValue::Text("0 0 -1 30deg".into()),
        )
        .unwrap();
        assert_eq!(s.rotate, -30.0);
        assert_eq!(translate_z("10px 20px 30px"), Some(30.0));
        assert_eq!(translate_z("10px"), Some(0.0));
        assert_eq!(perspective("none"), Some(0.0));
        assert_eq!(perspective("800px"), Some(800.0));
        assert_eq!(perspective("-1px"), None);
    }
}
