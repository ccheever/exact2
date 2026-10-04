//! A box's border as it paints: the widths that occupy space and the colour
//! of each side, `inset`'s two shades included (LLP 1001 §1, "Border
//! semantics"; LLP 1021 D1 — HTML's `hr` is an inset rule).

use super::{Color, ColorValue};
use crate::generated::{BorderStyle, StyleProps};

impl StyleProps {
    /// CSS effective border widths: none and hidden occupy no border area.
    pub fn border_widths(&self) -> [f32; 4] {
        [
            (self.border_style_top, self.border_width_top),
            (self.border_style_right, self.border_width_right),
            (self.border_style_bottom, self.border_width_bottom),
            (self.border_style_left, self.border_width_left),
        ]
        .map(|(style, width)| match style {
            BorderStyle::Solid | BorderStyle::Inset => width.max(0.0),
            BorderStyle::None | BorderStyle::Hidden => 0.0,
        })
    }

    /// Border colours after resolving currentColor against this node's
    /// computed colour. An `inset` side is the shade the browser paints: the
    /// top and left darkened, the bottom and right lightened, from the side's
    /// colour, or from Chrome's `#eeeeee` when the side names none (so a bare
    /// `hr` is the same grey pair whatever its `color`).
    pub fn border_colors(&self, current: ColorValue) -> [ColorValue; 4] {
        [
            (self.border_color_top, self.border_style_top, true),
            (self.border_color_right, self.border_style_right, false),
            (self.border_color_bottom, self.border_style_bottom, false),
            (self.border_color_left, self.border_style_left, true),
        ]
        .map(|(color, style, shadowed)| match style {
            BorderStyle::Inset => {
                let base = color.unwrap_or(ColorValue::Fixed(INSET_CURRENT));
                shade(base, shadowed)
            }
            _ => color.unwrap_or(current),
        })
    }
}

/// What Chrome draws an `inset` or `outset` side in when its colour is
/// `currentcolor` (WebKit's `colorIncludingFallback`).
const INSET_CURRENT: Color = Color::rgba(0xee, 0xee, 0xee, 0xff);

/// Each colour of a value shaded, a light/dark pair per appearance.
fn shade(value: ColorValue, shadowed: bool) -> ColorValue {
    let one = |c: Color| inset_shade(c, shadowed);
    match value.pair() {
        ColorValue::Fixed(c) => ColorValue::Fixed(one(c)),
        ColorValue::LightDark(light, dark) => ColorValue::LightDark(one(light), one(dark)),
        other => other,
    }
}

/// Chrome's inset shading, measured against it: the shadowed side is
/// `Color::Dark()`, the lit side `Color::Light()`. A colour too near black
/// to darken (within `#202020`) is lightened once for the shadowed side and
/// twice for the lit one; one too near white to lighten (within `#ebebeb`,
/// exclusive) is its own lit side.
fn inset_shade(c: Color, shadowed: bool) -> Color {
    let distance = |to: u8| {
        [c.r(), c.g(), c.b()]
            .iter()
            .map(|&v| (v as i32 - to as i32).pow(2))
            .sum::<i32>()
    };
    let near_black = distance(0) <= 3 * 0x20 * 0x20;
    match (shadowed, near_black) {
        (true, false) => dark(c),
        (true, true) => light(c),
        (false, true) => light(light(c)),
        (false, false) if distance(0xff) < 3 * 0x14 * 0x14 => c,
        (false, false) => light(c),
    }
}

/// Chrome's `Color::Dark()`.
fn dark(c: Color) -> Color {
    if c.0 | 0xff == Color::BLACK.0 {
        return Color::rgba(0x54, 0x54, 0x54, c.a());
    }
    let v = channels_max(c);
    let multiplier = if v == 0.0 {
        0.0
    } else {
        ((v - 0.33) / v).max(0.0)
    };
    scaled(c, multiplier)
}

/// Chrome's `Color::Light()`.
fn light(c: Color) -> Color {
    let v = channels_max(c);
    if v == 0.0 {
        return Color::rgba(0x54, 0x54, 0x54, c.a());
    }
    scaled(c, (v + 0.33).min(1.0) / v)
}

fn channels_max(c: Color) -> f32 {
    c.r().max(c.g()).max(c.b()) as f32 / 255.0
}

fn scaled(c: Color, multiplier: f32) -> Color {
    // Chrome's `nextafterf(256, 0)`: a full channel maps to 255.
    let channel = |v: u8| ((v as f32 / 255.0) * multiplier * 255.999_98).clamp(0.0, 255.0) as u8;
    Color::rgba(channel(c.r()), channel(c.g()), channel(c.b()), c.a())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(v: u8) -> Color {
        Color::rgba(v, v, v, 0xff)
    }

    /// Each pair Chrome painted for `border: 2px inset <colour>` (top, then
    /// bottom), read from its pixels.
    #[test]
    fn inset_shades_are_chromes() {
        for (colour, top, bottom) in [
            (grey(0x00), grey(84), grey(168)),
            (grey(0x10), grey(100), grey(184)),
            (grey(0x20), grey(116), grey(200)),
            (grey(0x30), grey(0), grey(132)),
            (grey(0x54), grey(0), grey(168)),
            (grey(0x80), grey(44), grey(212)),
            (grey(0xa8), grey(84), grey(253)),
            (grey(0xd0), grey(124), grey(255)),
            (grey(0xeb), grey(151), grey(255)),
            (grey(0xee), grey(154), grey(238)),
            (grey(0xf8), grey(164), grey(248)),
            (grey(0xff), grey(171), grey(255)),
            (
                Color::rgba(0xff, 0, 0, 0xff),
                Color::rgba(171, 0, 0, 0xff),
                Color::rgba(255, 0, 0, 0xff),
            ),
            (
                Color::rgba(0x33, 0x66, 0x99, 0xff),
                Color::rgba(23, 46, 69, 0xff),
                Color::rgba(79, 158, 238, 0xff),
            ),
        ] {
            assert_eq!(inset_shade(colour, true), top, "{colour:?} top");
            assert_eq!(inset_shade(colour, false), bottom, "{colour:?} bottom");
        }
    }
}
