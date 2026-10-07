//! LLP 1077 §5: the affordances Apple's platforms have and CSS has no name
//! for, as declared rows: an SF Symbol's palette (D10). The system colours
//! D13 named are roles now (LLP 1095 D2, `roles.rs`).

use super::{Color, ColorValue};
use crate::gradient::ColorText;

/// `-exact-symbol-palette`: `none`, or one to three colours for a symbol drawn
/// with `-exact-symbol-rendering: palette`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolPalette(pub Vec<ColorValue>);

impl SymbolPalette {
    /// The row's parse: `none` or one to three colours.
    pub fn parse(css: &str) -> Option<Self> {
        let css = css.trim();
        if css.eq_ignore_ascii_case("none") {
            return Some(Self(Vec::new()));
        }
        let mut colors = Vec::new();
        let (mut depth, mut start) = (0usize, None);
        let mut words = Vec::new();
        for (i, c) in css.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                c if c.is_ascii_whitespace() && depth == 0 => {
                    if let Some(s) = start.take() {
                        words.push(&css[s..i]);
                    }
                    continue;
                }
                _ => {}
            }
            start.get_or_insert(i);
        }
        if let Some(s) = start {
            words.push(&css[s..]);
        }
        for w in words {
            colors.push(
                ColorValue::parse_light_dark(w)
                    .or_else(|| Color::parse(w).map(ColorValue::Fixed))?,
            );
        }
        (1..=3).contains(&colors.len()).then_some(Self(colors))
    }

    /// Canonical CSS.
    pub fn css(&self) -> String {
        self.text(ColorText::Css)
    }

    /// The wire form: [`Self::css`], with every reference kept (LLP 1095 D1).
    pub fn wire(&self) -> String {
        self.text(ColorText::Wire)
    }

    fn text(&self, mode: ColorText) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let mut out = String::new();
        for (i, c) in self.0.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            crate::gradient::color_text(&mut out, *c, mode);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palette_takes_one_to_three_colours_and_system_colours_are_pairs() {
        let p = SymbolPalette::parse("#ff0000 light-dark(#000, #fff)").unwrap();
        assert_eq!(p.0.len(), 2);
        assert_eq!(SymbolPalette::parse(&p.css()).unwrap(), p);
        assert_eq!(SymbolPalette::parse("none").unwrap().0, vec![]);
        assert!(SymbolPalette::parse("#000 #111 #222 #333").is_none());
        assert!(SymbolPalette::parse("bogus").is_none());
        // WebKit's names are roles now (LLP 1095 D2), not copied pairs.
        let label = ColorValue::parse_light_dark("-apple-system-label").unwrap();
        assert_eq!(label, ColorValue::parse_light_dark("-exact-label").unwrap());
        assert_eq!(label.resolve(true), Color(0xffff_ffff));
    }
}
