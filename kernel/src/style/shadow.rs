//! CSS `box-shadow` and `text-shadow`.
//!
//! @ref LLP 1077 D4 — `box-shadow` is one row holding CSS's list: outer and
//! inset shadows with spread, each with its colour. It replaced LLP 1064's
//! four rows, which held one outer shadow. Each row's parse is the kernel's
//! one, so a literal, a style block and a computed string all set it alike,
//! and a refusal is the same text at compile time and at run time.

use super::{parse_pixel_length, Color, ColorValue, Vec2};

/// Most shadows one `box-shadow` takes: a bound on what crosses to a host.
pub const MAX_SHADOWS: usize = 8;

/// The row: `none` (the initial value, no shadows) or a list, painted as
/// CSS paints it, the first shadow on top.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoxShadows(pub Vec<BoxShadow>);

/// One shadow of a `box-shadow` list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    /// The colour, scheme-aware as any colour row.
    pub color: ColorValue,
    /// `<offset-x> <offset-y>`, points.
    pub offset: Vec2,
    /// The blur radius, points (a Gaussian of standard deviation half of it).
    pub blur: f32,
    /// How far the shape grows (or, negative, shrinks) before it blurs.
    pub spread: f32,
    /// Inside the padding box rather than outside the border box.
    pub inset: bool,
}

impl BoxShadows {
    /// The row's parse, as every codec's: `None` for anything not drawn.
    pub fn parse(css: &str) -> Option<Self> {
        Self::check(css).ok()
    }

    /// `none`, or up to [`MAX_SHADOWS`] shadows separated by commas, each
    /// `inset? <length>{2,4} <color>` in any order, lengths in `px`
    /// (unitless zero). Refused, by name: a missing colour (CSS's default is
    /// `currentcolor`, which LLP 1064 D1 refused and this keeps refusing).
    pub fn check(css: &str) -> Result<Self, &'static str> {
        let css = css.trim();
        if css.eq_ignore_ascii_case("none") {
            return Ok(Self(Vec::new()));
        }
        let list = split_commas(css);
        if list.len() > MAX_SHADOWS {
            return Err("at most 8 shadows in one box-shadow");
        }
        list.into_iter()
            .map(one_shadow)
            .collect::<Result<_, _>>()
            .map(Self)
    }

    /// The shadows, the first painted on top.
    pub fn shadows(&self) -> &[BoxShadow] {
        &self.0
    }

    /// Canonical CSS, also the wire form.
    pub fn css(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let mut out = String::new();
        for (i, s) in self.0.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            if s.inset {
                out.push_str("inset ");
            }
            out.push_str(&exact_num::text!(
                "{}px {}px {}px {}px ",
                exact_num::Shortest32(s.offset.x),
                exact_num::Shortest32(s.offset.y),
                exact_num::Shortest32(s.blur),
                exact_num::Shortest32(s.spread)
            ));
            crate::gradient::color_css(&mut out, s.color);
        }
        out
    }
}

fn one_shadow(text: &str) -> Result<BoxShadow, &'static str> {
    let (mut lengths, mut color, mut inset, mut closed) = (Vec::new(), None, false, false);
    for token in tokens(text)? {
        if token.eq_ignore_ascii_case("inset") {
            if inset {
                return Err("`inset` once per shadow");
            }
            inset = true;
            closed = !lengths.is_empty();
            continue;
        }
        if let Some(n) = parse_pixel_length(token) {
            if closed {
                return Err(
                    "the lengths are written together: <offset-x> <offset-y> [<blur> [<spread>]]",
                );
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
    let (x, y, blur, spread) = match lengths[..] {
        [x, y] => (x, y, 0.0, 0.0),
        [x, y, blur] => (x, y, blur, 0.0),
        [x, y, blur, spread] => (x, y, blur, spread),
        _ => return Err("two to four lengths: <offset-x> <offset-y> [<blur> [<spread>]]"),
    };
    if blur < 0.0 {
        return Err("a blur radius is never negative");
    }
    let color = color
        .ok_or("write the colour: CSS's default, currentcolor, is not one a shadow row holds")?;
    Ok(BoxShadow {
        color,
        offset: Vec2 { x, y },
        blur,
        spread,
        inset,
    })
}

/// A list's items: split at commas outside parentheses.
fn split_commas(text: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, 0);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(text[start..].trim());
    out
}

/// CSS `text-shadow`: `none`, or one shadow under the node's glyphs and
/// decorations, inherited (LLP 1077 D3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextShadow(Option<GlyphShadow>);

/// One text shadow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphShadow {
    /// The colour; `None` is `currentcolor`, the text's own colour.
    pub color: Option<ColorValue>,
    /// `<offset-x> <offset-y>`, points.
    pub offset: Vec2,
    /// The blur radius, points (a Gaussian of standard deviation half of it).
    pub blur: f32,
}

impl TextShadow {
    /// The row's parse, as every codec's: `None` for anything not drawn.
    pub fn parse(css: &str) -> Option<Self> {
        Self::check(css).ok()
    }

    /// `none`, or `<color>? <length>{2,3} <color>?` with lengths in `px`
    /// (unitless zero). Refused, by name: a list and a fourth length (text
    /// shadows have no spread).
    pub fn check(css: &str) -> Result<Self, &'static str> {
        let css = css.trim();
        if css.eq_ignore_ascii_case("none") {
            return Ok(Self(None));
        }
        let (mut lengths, mut color, mut closed) = (Vec::new(), None, false);
        for token in tokens(css)? {
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
            color = Some(if token.eq_ignore_ascii_case("currentcolor") {
                None
            } else {
                Some(
                    ColorValue::parse_light_dark(token)
                        .or_else(|| Color::parse(token).map(ColorValue::Fixed))
                        .ok_or("a word is neither a length in px (unitless zero) nor a colour")?,
                )
            });
        }
        let (x, y, blur) = match lengths[..] {
            [x, y] => (x, y, 0.0),
            [x, y, blur] => (x, y, blur),
            [_, _, _, _] => {
                return Err("a text shadow has no spread: <offset-x> <offset-y> [<blur>]")
            }
            _ => return Err("two or three lengths: <offset-x> <offset-y> [<blur>]"),
        };
        if blur < 0.0 {
            return Err("a blur radius is never negative");
        }
        Ok(Self(Some(GlyphShadow {
            color: color.flatten(),
            offset: Vec2 { x, y },
            blur,
        })))
    }

    /// The shadow, or `None` for `none`.
    pub fn shadow(&self) -> Option<&GlyphShadow> {
        self.0.as_ref()
    }

    /// Canonical CSS, also the wire form: `none`, or offsets and blur in px
    /// and the colour (`currentcolor` when none was written).
    pub fn css(&self) -> String {
        let Some(s) = &self.0 else {
            return "none".into();
        };
        let mut out = exact_num::text!(
            "{}px {}px {}px ",
            exact_num::Shortest32(s.offset.x),
            exact_num::Shortest32(s.offset.y),
            exact_num::Shortest32(s.blur)
        );
        match s.color {
            Some(c) => crate::gradient::color_css(&mut out, c),
            None => out.push_str("currentcolor"),
        }
        out
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
    fn a_list_of_outer_and_inset_shadows_with_spread_round_trips() {
        let black = ColorValue::Fixed(Color::parse("rgba(0,0,0,0.2)").unwrap());
        let s = BoxShadows::check("0 2px 12px rgba(0, 0, 0, 0.2), inset #fff 0 1px 0 1px").unwrap();
        assert_eq!(s.0.len(), 2);
        assert_eq!(s.0[0].color, black);
        assert_eq!(
            (s.0[0].offset.y, s.0[0].blur, s.0[0].spread, s.0[0].inset),
            (2.0, 12.0, 0.0, false)
        );
        assert_eq!(
            (s.0[1].offset.y, s.0[1].blur, s.0[1].spread, s.0[1].inset),
            (1.0, 0.0, 1.0, true)
        );
        assert_eq!(
            s.css(),
            "0px 2px 12px 0px #00000033, inset 0px 1px 0px 1px #ffffffff"
        );
        assert_eq!(BoxShadows::check(&s.css()).unwrap(), s);
        let s = BoxShadows::check("-1px 3px light-dark(#000, #fff)").unwrap();
        assert!(matches!(s.0[0].color, ColorValue::LightDark(..)));
        assert_eq!(BoxShadows::check(" None ").unwrap().0, vec![]);
    }

    #[test]
    fn what_is_refused_is_named() {
        for (text, says) in [
            ("0 1px 2px", "currentcolor"),
            ("0 1px #f00 2px", "together"),
            ("0 1px 2px #f00 #00f", "one colour"),
            ("0 1 2 #f00", "neither"),
            ("0 1px -2px #f00", "negative"),
            ("0 #f00", "two to four"),
            ("inset inset 0 1px #000", "once"),
            (
                "0 0 #000,0 0 #000,0 0 #000,0 0 #000,0 0 #000,0 0 #000,0 0 #000,0 0 #000,0 0 #000",
                "at most 8",
            ),
        ] {
            let e = BoxShadows::check(text).unwrap_err();
            assert!(e.contains(says), "{text}: {e}");
        }
    }

    #[test]
    fn a_text_shadow_takes_currentcolor_and_round_trips() {
        let s = TextShadow::check("1px 2px 3px rgba(0,0,0,0.5)").unwrap();
        assert_eq!(s.css(), "1px 2px 3px #00000080");
        assert_eq!(TextShadow::check(&s.css()).unwrap(), s);
        let s = TextShadow::check("currentcolor 0 1px").unwrap();
        let g = s.shadow().unwrap();
        assert_eq!((g.color, g.offset.y, g.blur), (None, 1.0, 0.0));
        assert_eq!(s.css(), "0px 1px 0px currentcolor");
        assert_eq!(
            TextShadow::check("2px 2px").unwrap().css(),
            "2px 2px 0px currentcolor"
        );
        assert_eq!(TextShadow::check(" NONE ").unwrap().shadow(), None);
        for (text, says) in [
            ("1px 1px #000, 2px 2px #fff", "one shadow"),
            ("1px 1px 2px 3px #000", "no spread"),
            ("1px #000", "two or three"),
            ("1px 1px -2px", "negative"),
        ] {
            let e = TextShadow::check(text).unwrap_err();
            assert!(e.contains(says), "{text}: {e}");
        }
    }
}
