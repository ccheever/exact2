//! `-webkit-text-stroke`, the Compat Standard's shorthand (LLP 1076 D7): one
//! value to its width and colour rows, each row taking its part, as
//! `box-shadow` once did for its four (LLP 1064 D1).

use super::{parse_pixel_length, Color, ColorValue};

/// `<length> <color>?` in either order, the length in `px` (unitless zero);
/// no colour is `currentcolor` (`None`).
pub(super) fn parse(text: &str) -> Result<(f32, Option<ColorValue>), &'static str> {
    let (mut width, mut color) = (None, None);
    let mut depth = 0usize;
    let mut words = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c.is_ascii_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    words.push(&text[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        words.push(&text[s..]);
    }
    for word in words {
        if let Some(n) = parse_pixel_length(word) {
            if width.replace(n).is_some() {
                return Err("one width");
            }
            continue;
        }
        if color.is_some() {
            return Err("one colour");
        }
        color = Some(if word.eq_ignore_ascii_case("currentcolor") {
            None
        } else {
            Some(
                ColorValue::parse_light_dark(word)
                    .or_else(|| Color::parse(word).map(ColorValue::Fixed))
                    .ok_or("a word is neither a width in px (unitless zero) nor a colour")?,
            )
        });
    }
    let width = width.ok_or("write the width: `-webkit-text-stroke: <width> <colour>`")?;
    if width < 0.0 || !width.is_finite() {
        return Err("a stroke width is never negative");
    }
    Ok((width, color.flatten()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shorthand_takes_a_width_and_a_colour_either_side() {
        assert_eq!(parse("2px #ff0000").unwrap().0, 2.0);
        let (w, c) = parse("light-dark(#000, #fff) 1.5px").unwrap();
        assert_eq!(w, 1.5);
        assert!(matches!(c, Some(ColorValue::LightDark(..))));
        assert_eq!(parse("1px").unwrap(), (1.0, None));
        assert_eq!(parse("0 currentcolor").unwrap(), (0.0, None));
        for (text, says) in [
            ("#000", "width"),
            ("1px 2px", "one width"),
            ("-1px", "negative"),
            ("1px red blue", "colour"),
        ] {
            let e = parse(text).unwrap_err();
            assert!(e.contains(says), "{text}: {e}");
        }
    }
}
