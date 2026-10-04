//! CSS colours as motion values: any colour a row admits, premultiplied.
//!
//! @ref LLP 1055.000 D6; CSS Color 4 §12.2 (legacy colours interpolate in
//! premultiplied sRGB)
//!
//! [`css`] is the one CSS colour parser: the kernel's colour rows read with
//! it, and so does a keyframe, so a colour a node can hold is a colour it can
//! animate to. Motion sits below the kernel, so the parser lives here.

pub mod css;

use crate::property::Value;

/// A CSS colour [`css::parse`] admits, as a motion value. `None` for
/// anything else, and for `currentColor`, which is the caller's to resolve.
pub fn parse(text: &str) -> Option<Value> {
    let c = match css::parse_exact(text)? {
        css::Parsed::Color(c) | css::Parsed::Wide(c, _) => c,
        css::Parsed::Current => return None,
    };
    let byte = |b: u8| b as f64 / 255.0;
    Some(Value::rgba(byte(c.r), byte(c.g), byte(c.b), c.a))
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
        assert_eq!(parse("red").unwrap().to_rgba8(), [255, 0, 0, 255]);
        assert_eq!(parse("hsl(0, 0%, 0%)").unwrap().to_rgba8(), [0, 0, 0, 255]);
        for bad in ["blurple", "#12", "rgb(1, 2)", "currentcolor", "#ggg"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        assert_eq!(css(half), "rgba(255, 0, 0, 0.5)");
    }
}
