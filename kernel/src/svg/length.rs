//! SVG lengths: user units, `px`, the absolute units, and percentages of
//! the nearest viewport.
//!
//! @ref LLP 1055.000 D4; SVG 2 §8.9 (units), CSS Values 4 §6.2 (absolute
//! lengths at 96 px per inch)

use crate::style::Dimension;

/// A length as authored: a number of user units, or a percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// User units (`px` and the absolute units already converted).
    Units(f32),
    /// A percentage, 0–100, of the axis the property measures.
    Percent(f32),
}

impl Length {
    /// Zero user units.
    pub const ZERO: Length = Length::Units(0.0);

    /// A number with an optional unit: none or `px` (user units), `in`,
    /// `cm`, `mm`, `pt`, `pc`, or `%`. `None` for anything else (`em`,
    /// `ex`, the viewport units are refused until a consumer needs them).
    pub fn parse(text: &str) -> Option<Length> {
        let t = text.trim();
        if let Some(n) = t.strip_suffix('%') {
            return number(n).map(Length::Percent);
        }
        let split = t
            .find(|c: char| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
            .unwrap_or(t.len());
        // An exponent's `e` is part of the number; a unit starting with `e`
        // (`em`, `ex`) is not admitted, so `e` is only ever an exponent.
        let (n, unit) = t.split_at(split);
        let scale = match unit.to_ascii_lowercase().as_str() {
            "" | "px" => 1.0,
            "in" => 96.0,
            "cm" => 96.0 / 2.54,
            "mm" => 96.0 / 25.4,
            "pt" => 96.0 / 72.0,
            "pc" => 16.0,
            _ => return None,
        };
        number(n).map(|v| Length::Units(v * scale))
    }

    /// The length in user units against `basis`, the percentage basis.
    pub fn resolve(self, basis: f32) -> f32 {
        match self {
            Length::Units(v) => v,
            Length::Percent(p) => basis * p / 100.0,
        }
    }

    /// As a kernel dimension (percent kept as authored, 0–100).
    pub fn dimension(self) -> Dimension {
        match self {
            Length::Units(v) => Dimension::Points(v),
            Length::Percent(p) => Dimension::Percent(p),
        }
    }
}

fn number(text: &str) -> Option<f32> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let v = exact_num::parse_f64(t).ok()? as f32;
    v.is_finite().then_some(v)
}

/// The nearest SVG viewport's size in user units (SVG 2 §8.9): its view
/// box's size when it has one, else its own width and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Width in user units.
    pub width: f32,
    /// Height in user units.
    pub height: f32,
}

impl Viewport {
    /// The basis of a percentage that is neither horizontal nor vertical
    /// (`r`, `stroke-width`): the normalized diagonal √((w²+h²)/2).
    pub fn diagonal(&self) -> f32 {
        ((self.width * self.width + self.height * self.height) / 2.0).sqrt()
    }

    /// A dimension row on the horizontal axis (`x`, `cx`, `width`, `rx`).
    pub fn x(&self, d: Dimension) -> f32 {
        resolve(d, self.width)
    }

    /// A dimension row on the vertical axis (`y`, `cy`, `height`, `ry`).
    pub fn y(&self, d: Dimension) -> f32 {
        resolve(d, self.height)
    }

    /// A dimension row on the diagonal (`r`).
    pub fn d(&self, d: Dimension) -> f32 {
        resolve(d, self.diagonal())
    }
}

/// A dimension in user units against `basis`; `auto` is zero.
pub fn resolve(d: Dimension, basis: f32) -> f32 {
    match d {
        Dimension::Points(v) => v,
        Dimension::Percent(p) => basis * p / 100.0,
        Dimension::Calc(p, v) => basis * p / 100.0 + v,
        _ => 0.0,
    }
}

/// A length prop's text (`x1`, `y1`, …) against `basis`, `0` when absent or
/// invalid (SVG 2: an invalid length is an error, and Chrome renders it as
/// the initial value, 0).
pub fn prop(text: Option<&str>, basis: f32) -> f32 {
    text.and_then(Length::parse)
        .map_or(0.0, |l| l.resolve(basis))
}
