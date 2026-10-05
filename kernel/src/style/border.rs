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
    /// colour, or from Chrome's `#eeeeee` when the side is `currentcolor` —
    /// unwritten or written — so a bare `hr` is the same grey pair whatever
    /// its `color`. Chrome's `getComputedStyle` still reports such a side as
    /// the resolved `color`; only its paint substitutes the grey (measured:
    /// Chrome 154, `color` red, gray, `#000040`, black and `#eeeeee` all
    /// paint `#9a9a9a` over `#eeeeee`).
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
/// `currentcolor` (WebKit's `colorIncludingFallback`, which Blink keeps: the
/// shading below starts from this, not from `color`).
const INSET_CURRENT: Color = Color::rgba(0xee, 0xee, 0xee, 0xff);

/// Each colour of a value shaded, a light/dark pair per appearance.
fn shade(value: ColorValue, shadowed: bool) -> ColorValue {
    let one = |c: Color| inset_shade(c, shadowed);
    if let ColorValue::Wide(id) = value {
        if let Some(w) = super::wide::wide(id) {
            let one = |w: exact_color::Wide| {
                let mut c = w.linear_srgb();
                let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                let light = |c: [f64; 3]| {
                    let peak = c.into_iter().fold(0.0_f64, f64::max);
                    if peak == 0.0 {
                        [0.33; 3]
                    } else {
                        c.map(|v| v * ((peak + 0.33).min(peak.max(1.0)) / peak))
                    }
                };
                if lum <= 0.014_443_844 {
                    c = light(c);
                    if !shadowed {
                        c = light(c);
                    }
                } else if shadowed {
                    let peak = c.into_iter().fold(0.0_f64, f64::max);
                    c = c.map(|v| v * ((peak - 0.33) / peak).max(0.0));
                } else if lum <= 0.830_77 {
                    c = light(c);
                }
                exact_color::Wide {
                    space: exact_color::Space::SrgbLinear,
                    c,
                    alpha: w.alpha,
                }
                .css()
            };
            let text = match w.dark {
                Some(d) => format!("light-dark({}, {})", one(w.light), one(d)),
                None => one(w.light),
            };
            // At the table cap retain the authored color; never clip it to sRGB.
            return super::wide::parse_wide(&text).unwrap_or(value);
        }
    }
    match value {
        ColorValue::Fixed(c) => ColorValue::Fixed(one(c)),
        ColorValue::LightDark(light, dark) => ColorValue::LightDark(one(light), one(dark)),
        other => other,
    }
}

/// Chrome's inset shading: Blink's `CalculateInsetOutsetColor`
/// (third_party/blink/renderer/core/paint/box_border_painter.cc, the
/// `TableDefaultBorderColorCurrentColor` branch Chrome ships, "chosen to match
/// WebKit's behavior"). By relative luminance (`color_utils::
/// GetRelativeLuminance4f`): a colour no brighter than `#202020` is
/// lightened once for the shadowed side and twice for the lit one; otherwise
/// the shadowed side is `Color::Dark()`, and the lit side `Color::Light()`
/// unless the colour is brighter than `#ebebeb`, when it is itself.
fn inset_shade(c: Color, shadowed: bool) -> Color {
    // Luminance of rgb(32, 32, 32) and rgb(235, 235, 235), as Blink spells them.
    const BASE_DARK: f32 = 0.014_443_844;
    const BASE_LIGHT: f32 = 0.830_77;
    let luminance = relative_luminance(c);
    match shadowed {
        _ if luminance <= BASE_DARK => {
            if shadowed {
                light(c)
            } else {
                light(light(c))
            }
        }
        true => dark(c),
        false if luminance > BASE_LIGHT => c,
        false => light(c),
    }
}

/// `color_utils::GetRelativeLuminance4f`: WCAG's, over sRGB linearised
/// with the 0.04045 threshold.
fn relative_luminance(c: Color) -> f32 {
    let linear = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
}

/// Chrome's `Color::Dark()`.
fn dark(c: Color) -> Color {
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

    #[test]
    fn wide_inset_shading_keeps_out_of_gamut_components() {
        let value = super::super::wide::parse_wide(
            "light-dark(color(display-p3 1 0 0), color(rec2100-linear 4 4 4))",
        )
        .unwrap();
        let ColorValue::Wide(id) = shade(value, true) else {
            panic!("wide inset was clipped")
        };
        let shaded = super::super::wide::wide(id).unwrap();
        assert!(shaded.half(false).linear_srgb()[1] < 0.0);
        assert!(shaded.half(true).linear_srgb()[0] > 3.0);
    }

    fn grey(v: u8) -> Color {
        Color::rgba(v, v, v, 0xff)
    }

    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::rgba(r, g, b, 0xff)
    }

    /// Chrome 154's pixels for `border: 4px inset` (top, right, bottom,
    /// left): a `currentcolor` side, unwritten or `border-color:
    /// currentcolor`, is the `#eeeeee` pair under any `color`; a written
    /// colour is shaded itself, whatever `color` is.
    #[test]
    fn currentcolor_inset_is_chromes_grey_pair() {
        let fixed = ColorValue::Fixed;
        let mut s = StyleProps::default();
        s.border_style_top = BorderStyle::Inset;
        s.border_style_right = BorderStyle::Inset;
        s.border_style_bottom = BorderStyle::Inset;
        s.border_style_left = BorderStyle::Inset;
        let pair = [grey(0x9a), grey(0xee), grey(0xee), grey(0x9a)].map(fixed);
        for current in [
            rgb(0xff, 0, 0),
            grey(0x80),
            rgb(0, 0, 0x40),
            grey(0),
            grey(0xee),
        ] {
            assert_eq!(s.border_colors(fixed(current)), pair, "{current:?}");
        }
        let red = Some(fixed(rgb(0xff, 0, 0)));
        s.border_color_top = red;
        s.border_color_right = red;
        s.border_color_bottom = red;
        s.border_color_left = red;
        let reds = [
            rgb(0xab, 0, 0),
            rgb(0xff, 0, 0),
            rgb(0xff, 0, 0),
            rgb(0xab, 0, 0),
        ];
        assert_eq!(s.border_colors(fixed(rgb(0, 0, 0xff))), reds.map(fixed));
    }

    /// Each pair Chrome painted for `border: 2px inset <colour>` (top, then
    /// bottom), read from its pixels (Chrome 154; the dark saturated colours
    /// are where luminance, not distance from black, decides).
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
            (rgb(0x00, 0x00, 0x40), rgb(0, 0, 148), rgb(0, 0, 233)),
            (rgb(0x40, 0x00, 0x00), rgb(148, 0, 0), rgb(233, 0, 0)),
            (rgb(0x00, 0x40, 0x00), rgb(0, 0, 0), rgb(0, 148, 0)),
            (rgb(0x00, 0x00, 0x80), rgb(0, 0, 44), rgb(0, 0, 212)),
            (rgb(0x00, 0x00, 0x60), rgb(0, 0, 180), rgb(0, 0, 255)),
            (rgb(0x0a, 0x0a, 0x40), rgb(23, 23, 148), rgb(36, 36, 233)),
            (rgb(0x10, 0x10, 0x40), rgb(37, 37, 148), rgb(58, 58, 233)),
            (rgb(0x20, 0x20, 0x00), rgb(116, 116, 0), rgb(200, 200, 0)),
            (rgb(0x3a, 0x00, 0x00), rgb(142, 0, 0), rgb(227, 0, 0)),
            (rgb(0x00, 0x3a, 0x3a), rgb(0, 0, 0), rgb(0, 142, 142)),
            (rgb(0x00, 0xff, 0x00), rgb(0, 171, 0), rgb(0, 255, 0)),
            (
                rgb(0xe0, 0xe0, 0xf0),
                rgb(146, 146, 156),
                rgb(238, 238, 255),
            ),
            (
                rgb(0xf0, 0xe0, 0xe0),
                rgb(156, 146, 146),
                rgb(255, 238, 238),
            ),
            (grey(0x1f), grey(115), grey(199)),
            (grey(0x21), grey(0), grey(117)),
            (grey(0x4b), grey(0), grey(159)),
        ] {
            assert_eq!(inset_shade(colour, true), top, "{colour:?} top");
            assert_eq!(inset_shade(colour, false), bottom, "{colour:?} bottom");
        }
    }
}
