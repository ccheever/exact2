//! CSS colours as motion values: the legacy syntax, premultiplied.
//!
//! @ref LLP 1055.000 D6; CSS Color 4 §12.2 (legacy colours interpolate in
//! premultiplied sRGB)
//!
//! The kernel's colour rows admit hex, `rgb()`/`rgba()`, `transparent` and
//! the named colours;
//! a keyframe takes the same, so a colour a node can hold is a colour it can
//! animate to. Motion sits below the kernel, so it parses them itself.

use crate::property::Value;

/// A CSS colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`
/// (numbers 0–255 or percentages, alpha 0–1 or a percentage, commas or the
/// space-and-slash form), `transparent`, or a named colour. `None` for
/// anything else.
pub fn parse(text: &str) -> Option<Value> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("transparent") {
        return Some(Value::ZERO);
    }
    if let Some([r, g, b]) = crate::named::named(t) {
        return Some(Value::rgba8(r, g, b, 255));
    }
    if let Some(hex) = t.strip_prefix('#') {
        let digit = |i: usize| u8::from_str_radix(hex.get(i..i + 1)?, 16).ok();
        let pair = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        let (r, g, b, a) = match hex.len() {
            3 | 4 => (
                digit(0)? * 17,
                digit(1)? * 17,
                digit(2)? * 17,
                if hex.len() == 4 { digit(3)? * 17 } else { 255 },
            ),
            6 | 8 => (
                pair(0)?,
                pair(2)?,
                pair(4)?,
                if hex.len() == 8 { pair(6)? } else { 255 },
            ),
            _ => return None,
        };
        return Some(Value::rgba8(r, g, b, a));
    }
    let lower = t.to_ascii_lowercase();
    let body = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let (rgb, alpha) = match body.split_once('/') {
        Some((c, a)) => (c.to_string(), Some(a.trim().to_string())),
        None => (body.to_string(), None),
    };
    let mut parts: Vec<&str> = rgb
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    let alpha = match (alpha, parts.len()) {
        (Some(a), 3) => Some(a),
        (None, 4) => parts.pop().map(str::to_string),
        (None, 3) => None,
        _ => return None,
    };
    let channel = |p: &str| -> Option<f64> {
        let v = match p.strip_suffix('%') {
            Some(n) => exact_num::parse_f64(n).ok()? / 100.0,
            None => exact_num::parse_f64(p).ok()? / 255.0,
        };
        v.is_finite().then(|| v.clamp(0.0, 1.0))
    };
    let a = match alpha {
        Some(a) => {
            let v = match a.strip_suffix('%') {
                Some(n) => exact_num::parse_f64(n).ok()? / 100.0,
                None => exact_num::parse_f64(&a).ok()?,
            };
            if !v.is_finite() {
                return None;
            }
            v.clamp(0.0, 1.0)
        }
        None => 1.0,
    };
    Some(Value::rgba(
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?,
        a,
    ))
}

/// A colour value as CSS reads it: `rgba(r, g, b, a)`, straight alpha.
pub fn css(v: Value) -> String {
    let [r, g, b, _] = v.to_rgba8();
    let a = v.w.clamp(0.0, 1.0);
    format!("rgba({r}, {g}, {b}, {})", exact_num::Shortest(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_colours_parse_premultiplied() {
        assert_eq!(
            parse("#16a34a").unwrap().to_rgba8(),
            [0x16, 0xa3, 0x4a, 255]
        );
        assert_eq!(parse("#f00").unwrap(), Value::rgba(1.0, 0.0, 0.0, 1.0));
        assert_eq!(parse("transparent"), Some(Value::ZERO));
        let half = parse("rgba(255, 0, 0, 0.5)").unwrap();
        assert_eq!((half.x, half.w), (0.5, 0.5), "premultiplied");
        assert_eq!(parse("rgb(255 0 0 / 50%)"), Some(half));
        assert_eq!(parse("#ff000080").unwrap().to_rgba8(), [255, 0, 0, 128]);
        // A named colour, as the kernel's rows take it (one table, `named`).
        assert_eq!(parse(" Gray ").unwrap().to_rgba8(), [128, 128, 128, 255]);
        for bad in ["reddish", "#12", "rgb(1, 2)", "hsl(0, 0%, 0%)", "#ggg"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        assert_eq!(css(half), "rgba(255, 0, 0, 0.5)");
    }
}
