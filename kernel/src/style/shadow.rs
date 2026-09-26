//! CSS `box-shadow`, one outer shadow, as the four shadow rows hold it.
//!
//! @ref LLP 1055 D1 — Contract's `box-shadow` binds its one value to all four
//! rows and each row takes its part of the parse, so a literal, a style block
//! and a computed string all set them alike, and a refusal is the same text
//! at compile time and at run time.

use super::{parse_pixel_length, Color, ColorValue, Vec2};

/// One outer shadow: what `shadow_color`, `shadow_offset`, `shadow_radius`
/// and `shadow_opacity` hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    /// The colour, scheme-aware as any colour row.
    pub color: ColorValue,
    /// `<offset-x> <offset-y>`, points.
    pub offset: Vec2,
    /// The blur radius, points (a Gaussian of standard deviation half of it).
    pub blur: f32,
    /// 1 for a shadow, 0 for `none`: CSS carries opacity in the colour.
    pub opacity: f32,
}

impl BoxShadow {
    /// `none`, or `<color>? <length>{2,4} <color>?` with exactly one colour
    /// and lengths in `px` (unitless zero). Refused, by name: a shadow list,
    /// `inset`, a non-zero spread, and a missing colour (CSS's default is
    /// `currentcolor`, which no shadow row can hold).
    pub fn parse(text: &str) -> Result<BoxShadow, &'static str> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("none") {
            return Ok(BoxShadow {
                color: ColorValue::Fixed(Color::TRANSPARENT),
                offset: Vec2 { x: 0.0, y: 0.0 },
                blur: 0.0,
                opacity: 0.0,
            });
        }
        let (mut lengths, mut color, mut closed) = (Vec::new(), None, false);
        for token in tokens(text)? {
            if token.eq_ignore_ascii_case("inset") {
                return Err("`inset` is CSS, but exact2 draws only outer shadows");
            }
            if let Some(n) = parse_pixel_length(token) {
                if closed {
                    return Err("the lengths are written together: <offset-x> <offset-y> [<blur>], the colour before or after them");
                }
                lengths.push(n);
                continue;
            }
            if color.is_some() {
                return Err("one colour, and lengths in px (unitless zero)");
            }
            closed = !lengths.is_empty();
            color = Some(
                ColorValue::parse_light_dark(token)
                    .or_else(|| Color::parse(token).map(ColorValue::Fixed))
                    .ok_or("a word is neither a length in px (unitless zero) nor a colour")?,
            );
        }
        let (x, y, blur) = match lengths[..] {
            [x, y] => (x, y, 0.0),
            [x, y, blur] | [x, y, blur, 0.0] => (x, y, blur),
            [_, _, _, _] => return Err("a non-zero spread is CSS, but exact2 has no spread row"),
            _ => return Err("two to four lengths: <offset-x> <offset-y> [<blur> [<spread>]]"),
        };
        if blur < 0.0 {
            return Err("a blur radius is never negative");
        }
        let color = color.ok_or(
            "write the colour: CSS's default, currentcolor, is not one a shadow row holds",
        )?;
        Ok(BoxShadow {
            color,
            offset: Vec2 { x, y },
            blur,
            opacity: 1.0,
        })
    }
}

/// The value's tokens: split at CSS white space outside parentheses, so
/// `rgba(0, 0, 0, 0.2)` is one. A comma outside them is a second shadow.
fn tokens(text: &str) -> Result<Vec<&str>, &'static str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, None);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                return Err("one shadow: CSS's comma-separated list is not implemented")
            }
            '\t' | '\n' | '\u{c}' | '\r' | ' ' if depth == 0 => {
                if let Some(s) = start.take() {
                    out.push(&text[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        out.push(&text[s..]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_outer_shadow_with_the_colour_either_side() {
        let black = ColorValue::Fixed(Color::parse("rgba(0,0,0,0.2)").unwrap());
        for text in [
            "0 2px 12px rgba(0, 0, 0, 0.2)",
            "rgba(0,0,0,0.2) 0 2px 12px 0",
        ] {
            let s = BoxShadow::parse(text).unwrap();
            assert_eq!(s.color, black, "{text}");
            assert_eq!(
                (s.offset.x, s.offset.y, s.blur, s.opacity),
                (0.0, 2.0, 12.0, 1.0)
            );
        }
        let s = BoxShadow::parse("-1px 3px light-dark(#000, #fff)").unwrap();
        assert!(matches!(s.color, ColorValue::LightDark(..)));
        assert_eq!((s.offset.x, s.offset.y, s.blur), (-1.0, 3.0, 0.0));
        assert_eq!(BoxShadow::parse(" None ").unwrap().opacity, 0.0);
    }

    #[test]
    fn what_is_refused_is_named() {
        for (text, says) in [
            ("0 1px 2px #f00, 0 2px 4px #00f", "one shadow"),
            ("inset 0 1px 2px #f00", "inset"),
            ("0 1px 2px 3px #f00", "spread"),
            ("0 1px 2px", "currentcolor"),
            ("0 1px #f00 2px", "together"),
            ("0 1px 2px #f00 #00f", "one colour"),
            ("0 1 2 #f00", "neither"),
            ("0 1px -2px #f00", "negative"),
            ("0 #f00", "two to four"),
        ] {
            let e = BoxShadow::parse(text).unwrap_err();
            assert!(e.contains(says), "{text}: {e}");
        }
    }
}
