//! LLP 1077 §5: the affordances Apple's platforms have and CSS has no name
//! for, as declared rows: an SF Symbol's palette (D10), and the system's
//! label, fill and separator colours (D13), which resolve as `light-dark()`
//! pairs of UIKit's own values on every host.

use super::{Color, ColorValue};

/// `symbol-palette`: `none`, or one to three colours for a symbol drawn
/// with `symbol-rendering: palette`.
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
        if self.0.is_empty() {
            return "none".into();
        }
        let mut out = String::new();
        for (i, c) in self.0.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            crate::gradient::color_css(&mut out, *c);
        }
        out
    }
}

/// UIKit's label, fill and separator colours as WebKit spells them, light
/// and dark (LLP 1077 D13): the one table every host and the web JS
/// target's bound values read.
pub const SYSTEM_COLORS: [(&str, u32, u32); 6] = [
    ("-apple-system-label", 0x0000_00ff, 0xffff_ffff),
    ("-apple-system-secondary-label", 0x3c3c_4399, 0xebeb_f599),
    ("-apple-system-tertiary-label", 0x3c3c_434d, 0xebeb_f54d),
    ("-apple-system-quaternary-label", 0x3c3c_432e, 0xebeb_f529),
    ("-apple-system-separator", 0x3c3c_434a, 0x5454_5899),
    ("-apple-system-fill", 0x7878_8033, 0x7878_805c),
];

pub(super) fn system_color(name: &str) -> Option<ColorValue> {
    let name = name.trim().to_ascii_lowercase();
    SYSTEM_COLORS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|&(_, l, d)| ColorValue::LightDark(Color(l), Color(d)))
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
        assert_eq!(
            system_color("-apple-system-label"),
            Some(ColorValue::LightDark(Color(0xff), Color(0xffff_ffff)))
        );
        assert_eq!(
            ColorValue::parse_light_dark("-apple-system-secondary-label"),
            system_color("-apple-system-secondary-label")
        );
    }
}
