//! The colours CSS defines as 8-bit sRGB.

use super::Color;

impl Color {
    /// A legacy CSS colour: hex, `rgb()`, `hsl()`, `hwb()`, a named colour
    /// or `transparent`. Modern forms go through `ColorValue::parse_light_dark`.
    pub fn parse(text: &str) -> Option<Color> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("transparent") {
            return Some(Color::rgba(0, 0, 0, 0));
        }
        Color::parse_hex(text)
            .or_else(|| Color::parse_rgb(text))
            .or_else(|| match exact_color::parse(text)? {
                exact_color::Parsed::Legacy(c) => {
                    Some(Color::rgba(c.r, c.g, c.b, (c.a * 255.0).round() as u8))
                }
                _ => None,
            })
    }

    /// `rgb()` / `rgba()`, either syntax; out-of-range values clamp.
    fn parse_rgb(text: &str) -> Option<Color> {
        let inner = text
            .strip_prefix("rgba(")
            .or_else(|| text.strip_prefix("rgb("))?
            .strip_suffix(')')?;
        let parts: Vec<&str> = if inner.contains(',') {
            inner.split(',').map(str::trim).collect()
        } else {
            let (rgb, alpha) = match inner.split_once('/') {
                Some((rgb, alpha)) => (rgb, Some(alpha.trim())),
                None => (inner, None),
            };
            rgb.split_whitespace().chain(alpha).collect()
        };
        let ([r, g, b], alpha) = match parts[..] {
            [r, g, b] => ([r, g, b], None),
            [r, g, b, a] => ([r, g, b], Some(a)),
            _ => return None,
        };
        // `unit`: 1 for a channel, 255 for alpha.
        let byte = |s: &str, unit: f32| -> Option<u8> {
            let v = match s.strip_suffix('%') {
                Some(p) => exact_num::parse_f32(p).ok()? / 100.0 * 255.0,
                None => exact_num::parse_f32(s).ok()? * unit,
            };
            v.is_finite().then(|| v.round().clamp(0.0, 255.0) as u8)
        };
        Some(Color::rgba(
            byte(r, 1.0)?,
            byte(g, 1.0)?,
            byte(b, 1.0)?,
            alpha.map_or(Some(255), |a| byte(a, 255.0))?,
        ))
    }

    /// Parse CSS hex notation: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`.
    pub fn parse_hex(text: &str) -> Option<Color> {
        let hex = text.strip_prefix('#')?;
        let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        let bytes = hex.as_bytes();
        let (r, g, b, a) = match bytes.len() {
            3 | 4 => {
                let mut v = [0u8; 4];
                for (i, c) in bytes.iter().enumerate() {
                    let d = digit(*c)?;
                    v[i] = d * 17;
                }
                (v[0], v[1], v[2], if bytes.len() == 4 { v[3] } else { 255 })
            }
            6 | 8 => {
                let mut v = [0u8; 4];
                for (i, pair) in bytes.chunks(2).enumerate() {
                    v[i] = digit(pair[0])? * 16 + digit(pair[1])?;
                }
                (v[0], v[1], v[2], if bytes.len() == 8 { v[3] } else { 255 })
            }
            _ => return None,
        };
        Some(Color::rgba(r, g, b, a))
    }

    /// Extended linear sRGB and alpha, sRGB-encoded and clipped.
    pub(crate) fn from_linear_srgb(c: [f64; 3], a: f64) -> Color {
        let [r, g, b] =
            c.map(|v| (exact_color::linear_to_srgb(v).clamp(0.0, 1.0) * 255.0).round() as u8);
        Color::rgba(r, g, b, (a * 255.0).round() as u8)
    }
}
