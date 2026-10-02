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
    /// The row's parse: the axis of a `rotate` value.
    pub fn parse(css: &str) -> Option<Self> {
        rotate(css).map(|(axis, _)| Self(axis))
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
        assert_eq!(translate_z("10px 20px 30px"), Some(30.0));
        assert_eq!(translate_z("10px"), Some(0.0));
        assert_eq!(perspective("none"), Some(0.0));
        assert_eq!(perspective("800px"), Some(800.0));
        assert_eq!(perspective("-1px"), None);
    }
}
