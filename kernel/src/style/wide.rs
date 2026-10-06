//! Colours in their own spaces (LLP 1100 D2), alone or as a half of
//! `light-dark()`, interned as `ColorValue::Wide(id)`.

use super::{Color, ColorValue};
use exact_color::{Space, Wide};
use std::sync::{Arc, Mutex};

/// One colour, or a `light-dark()` pair with at least one wide half.
#[derive(Debug, PartialEq)]
pub struct WideValue {
    /// The colour, or the light half.
    pub light: Wide,
    /// The dark half of a `light-dark()` pair.
    pub dark: Option<Wide>,
    /// CSS's serialisation.
    pub text: Box<str>,
    /// Whether each authored half uses modern syntax (CSS interpolation).
    pub modern: [bool; 2],
}

impl WideValue {
    /// The half an appearance uses.
    pub fn half(&self, dark: bool) -> Wide {
        match (dark, self.dark) {
            (true, Some(d)) => d,
            _ => self.light,
        }
    }

    /// The 8-bit sRGB clip.
    pub fn fallback(&self) -> ColorValue {
        let clip = |w: Wide| {
            let c = w.srgb8();
            Color::rgba(c.r, c.g, c.b, (c.a * 255.0).round() as u8)
        };
        match self.dark {
            Some(d) => ColorValue::LightDark(clip(self.light), clip(d)),
            None => ColorValue::Fixed(clip(self.light)),
        }
    }
}

static WIDE: Mutex<Vec<Arc<WideValue>>> = Mutex::new(Vec::new());

/// What this host can show, when not every colour (LLP 1100 D1): others
/// are refused as invalid.
static AVAILABLE: std::sync::OnceLock<fn(&Wide) -> bool> = std::sync::OnceLock::new();

/// Set once at boot by a host that shows only some colours.
pub fn set_available(shows: fn(&Wide) -> bool) {
    let _ = AVAILABLE.set(shows);
}

/// Whether this host shows only some colours; then it shows no profile's
/// colour either (LLP 1100 D10).
pub fn limited() -> bool {
    AVAILABLE.get().is_some()
}

/// Bounds colours a data source writes at run time.
const WIDE_CAP: usize = 4096;

/// An interned wide colour by id.
pub fn wide(id: u16) -> Option<Arc<WideValue>> {
    WIDE.lock().ok()?.get(usize::from(id)).cloned()
}

/// One CSS colour as a half; legacy sRGB as `color(srgb …)`.
fn half(text: &str) -> Option<Wide> {
    match exact_color::parse(text)? {
        exact_color::Parsed::Wide(w) => Some(w),
        exact_color::Parsed::Legacy(c) => Some(Wide {
            space: Space::Srgb,
            c: [c.r, c.g, c.b].map(|v| f64::from(v) / 255.0),
            alpha: c.a,
        }),
        exact_color::Parsed::Current => None,
    }
}

/// A modern colour, or a `light-dark()` with a modern half, interned.
pub fn parse_wide(text: &str) -> Option<ColorValue> {
    let text = text.trim();
    let (light, dark, canonical, modern) = if let Some(inner) = text
        .strip_prefix("light-dark(")
        .and_then(|t| t.strip_suffix(')'))
    {
        let comma = super::top_level_comma(inner)?;
        let (a, b) = (inner[..comma].trim(), inner[comma + 1..].trim());
        let modern = |t: &str| matches!(exact_color::parse(t), Some(exact_color::Parsed::Wide(_)));
        if !modern(a) && !modern(b) {
            return None;
        }
        let (l, d) = (half(a)?, half(b)?);
        let css = |w: Wide, t: &str| {
            if modern(t) {
                w.css()
            } else {
                let c = w.srgb8();
                format!(
                    "#{:02x}{:02x}{:02x}{:02x}",
                    c.r,
                    c.g,
                    c.b,
                    (c.a * 255.0).round() as u8
                )
            }
        };
        let canonical = format!("light-dark({}, {})", css(l, a), css(d, b));
        (l, Some(d), canonical, [modern(a), modern(b)])
    } else {
        match exact_color::parse(text)? {
            exact_color::Parsed::Wide(w) => (w, None, w.css(), [true; 2]),
            _ => return None,
        }
    };
    if let Some(shows) = AVAILABLE.get() {
        if !shows(&light) || dark.is_some_and(|d| !shows(&d)) {
            return None;
        }
    }
    let value = WideValue {
        light,
        dark,
        text: canonical.into(),
        modern,
    };
    intern(&mut *WIDE.lock().ok()?, value)
}

fn intern(table: &mut Vec<Arc<WideValue>>, value: WideValue) -> Option<ColorValue> {
    if let Some(i) = table.iter().position(|w| w.text == value.text) {
        return Some(ColorValue::Wide(u16::try_from(i).ok()?));
    }
    if table.len() >= WIDE_CAP {
        return None;
    }
    table.push(Arc::new(value));
    Some(ColorValue::Wide(u16::try_from(table.len() - 1).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_table_overflow_refuses_without_clipping() {
        let value = |text: &str| WideValue {
            light: half("color(display-p3 1 0 0)").unwrap(),
            dark: None,
            text: text.into(),
            modern: [true; 2],
        };
        let existing = Arc::new(value("existing"));
        let mut table = vec![existing; WIDE_CAP];
        assert_eq!(intern(&mut table, value("new")), None);
        assert_eq!(
            intern(&mut table, value("existing")),
            Some(ColorValue::Wide(0))
        );
        assert_eq!(table.len(), WIDE_CAP);
    }
}
