//! CSS colours, parsed once for every reader (LLP 1056 D3): CSS Color 4's
//! sRGB forms — hex, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `hwb()`, the
//! named colours, `transparent` and `currentColor` — and the canvas
//! serialisation (`#rrggbb` when opaque, `rgba(r, g, b, a)` otherwise).
//! The kernel's colour rows, a keyframe's colours and a canvas's styles all
//! read with this parser, so a colour one host admits every host admits
//! (feed F13: `hsl()` painted on the web and was refused on macOS). The
//! wide forms (`lab()`, `lch()`, `oklab()`, `oklch()` and `color()` in
//! `srgb`, `srgb-linear`, `display-p3`, `xyz`, `xyz-d50` and `xyz-d65`) are
//! converted to sRGB and clipped, as an sRGB canvas draws them, and keep
//! their own serialisation, as Chrome's getters return it. Parsing and
//! arithmetic are `exact-color`'s (LLP 1100).

/// A colour: sRGB channels 0–255 and alpha 0–1, non-premultiplied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha, 0–1.
    pub a: f64,
}

impl Rgba {
    /// Opaque black, the spec's default style.
    pub const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 1.0,
    };

    /// The four list operands.
    pub fn operands(self) -> [f64; 4] {
        [self.r as f64, self.g as f64, self.b as f64, self.a]
    }

    /// The canvas serialisation (HTML "serialization of a color"): `#rrggbb`
    /// when opaque, else `rgba(r, g, b, a)` with the alpha as Chrome writes it.
    pub fn serialize(self) -> String {
        if (self.a * 255.0).round() >= 255.0 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!(
                "rgba({}, {}, {}, {})",
                self.r,
                self.g,
                self.b,
                alpha_text(self.a)
            )
        }
    }
}

/// Alpha as Chrome serialises a canvas colour's: the 8-bit alpha, written
/// with the fewest decimals (up to three) that read back to the same byte.
pub fn alpha_text(a: f64) -> String {
    let byte = (a * 255.0).round();
    if byte <= 0.0 {
        return "0".into();
    }
    for places in 1..=3 {
        let scale = 10f64.powi(places);
        let v = (byte / 255.0 * scale).round() / scale;
        if (v * 255.0).round() == byte {
            let s = format!("{:.*}", places as usize, v);
            return s.trim_end_matches('0').trim_end_matches('.').to_string();
        }
    }
    format!("{:.3}", byte / 255.0)
}

/// What an assignment of a colour string parsed to.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    /// A colour.
    Color(Rgba),
    /// A wide-gamut colour: its sRGB pixels, clipped, and its serialisation
    /// in its own space (`lab(50 40 59.5)`), which is what Chrome's getter
    /// returns.
    Wide(Rgba, String),
    /// `currentColor`: the canvas node's `color`, resolved by the caller.
    Current,
}

/// Parse a CSS colour, or `None` when it is not one (the assignment is then
/// ignored), its alpha held to 8 bits as Chrome stores a canvas colour's.
pub fn parse(input: &str) -> Option<Parsed> {
    let byte = |c: Rgba| Rgba {
        a: (c.a * 255.0).round() / 255.0,
        ..c
    };
    Some(match parse_exact(input)? {
        Parsed::Color(c) => Parsed::Color(byte(c)),
        Parsed::Wide(c, text) => Parsed::Wide(byte(c), text),
        Parsed::Current => Parsed::Current,
    })
}

/// [`parse`], with the alpha as written (clamped to 0–1): a motion value's,
/// which interpolates in floating point.
pub fn parse_exact(input: &str) -> Option<Parsed> {
    match exact_color::parse(input)? {
        exact_color::Parsed::Current => Some(Parsed::Current),
        exact_color::Parsed::Legacy(c) => Some(Parsed::Color(rgba(c))),
        exact_color::Parsed::Wide(w) => WIDE.get().and_then(|f| f(w)),
    }
}

fn rgba(c: exact_color::Rgba8) -> Rgba {
    Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// The wide forms, once linked ([`link_wide`]).
static WIDE: std::sync::OnceLock<fn(exact_color::Wide) -> Option<Parsed>> =
    std::sync::OnceLock::new();

/// Link the wide forms (`lab()`, `lch()`, `oklab()`, `oklch()`,
/// `color()`): until a host calls this, they do not parse and their
/// assignments are ignored. Native hosts link them at start; a web artifact
/// links them when its data crate's source names one (LLP 1047 D2: linked
/// by use; the web core's budget).
pub fn link_wide() {
    let _ = WIDE.set(wide);
}

/// A wide colour clipped to sRGB, with its own serialisation; only the
/// forms Chrome's canvas takes.
fn wide(w: exact_color::Wide) -> Option<Parsed> {
    if w.space.is_hdr() || w.space == exact_color::Space::DisplayP3Linear {
        return None;
    }
    Some(Parsed::Wide(rgba(w.srgb8()), w.css()))
}

/// A number as JavaScript's `String(n)` writes it, for the shapes canvas
/// serialisations take (finite, not huge).
pub fn js_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{v}");
    if s.contains('e') {
        s = format!("{v:.6}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> String {
        link_wide();
        match parse(s) {
            Some(Parsed::Color(c)) => c.serialize(),
            Some(Parsed::Wide(c, text)) => format!("{text} {}", c.serialize()),
            Some(Parsed::Current) => "current".into(),
            None => "none".into(),
        }
    }

    #[test]
    fn the_forms_parse_and_serialise_as_canvas_does() {
        assert_eq!(c("red"), "#ff0000");
        assert_eq!(c("RebeccaPurple"), "#663399");
        assert_eq!(c("#abc"), "#aabbcc");
        assert_eq!(c("#abcd"), "rgba(170, 187, 204, 0.867)");
        assert_eq!(c("#11223380"), "rgba(17, 34, 51, 0.5)");
        assert_eq!(c("rgb(255, 0, 0)"), "#ff0000");
        assert_eq!(c("rgba(0,0,0,0.5)"), "rgba(0, 0, 0, 0.5)");
        assert_eq!(c("rgb(0 128 255 / 25%)"), "rgba(0, 128, 255, 0.25)");
        assert_eq!(c("rgb(100%, 50%, 0%)"), "#ff8000");
        assert_eq!(c("hsl(120, 100%, 50%)"), "#00ff00");
        assert_eq!(c("hsl(240deg 100% 25% / 0.5)"), "rgba(0, 0, 128, 0.5)");
        assert_eq!(c("hwb(0 0% 0%)"), "#ff0000");
        assert_eq!(c("transparent"), "rgba(0, 0, 0, 0)");
        // Chrome's own answers (headless Chrome, 2026-09-27).
        assert_eq!(c("rgba(0,0,0,0.123456)"), "rgba(0, 0, 0, 0.12)");
        assert_eq!(c("rgb(1 2 3 / 0.999)"), "#010203");
        assert_eq!(c("rgba(0,0,0,0.004)"), "rgba(0, 0, 0, 0.004)");
        assert_eq!(c("rgba(0,0,0,0.001)"), "rgba(0, 0, 0, 0)");
        assert_eq!(c("rgb(12.6, 0, 0)"), "#0d0000");
        assert_eq!(c("hsl(120 50 50)"), "#40bf40");
        assert_eq!(c("rgb(50%,50%,50%)"), "#808080");
        assert_eq!(c("currentColor"), "current");
        // The wide forms: their own serialisation (Chrome's), sRGB pixels.
        assert_eq!(c("lab(50% 40 59.5)"), "lab(50 40 59.5) #bf5700");
        assert_eq!(
            c("oklch(0.7 0.1 200 / 0.5)"),
            "oklch(0.7 0.1 200 / 0.5) rgba(64, 177, 183, 0.5)"
        );
        assert_eq!(c("color(srgb 1 0 0.5)"), "color(srgb 1 0 0.5) #ff0080");
        assert_eq!(
            c("color(display-p3 1 0 0)"),
            "color(display-p3 1 0 0) #ff0000"
        );
        for bad in [
            "",
            "#12",
            "rgb(1, 2)",
            "rgb(1, 2%, 3)",
            "blurple",
            "lab(50, 0, 0)",
            "rgb(1 2, 3)",
            "hsl(120, 50, 50)",
        ] {
            assert_eq!(c(bad), "none", "{bad}");
        }
    }

    /// Review C1: an angle is a hue's alone. Chrome 154 drops a colour with
    /// one on an rgb channel, a saturation, a lab axis or an alpha, and takes
    /// it on hsl's, hwb's and lch's hue.
    #[test]
    fn only_a_hue_takes_an_angle() {
        for bad in [
            "rgb(90deg 0 0)",
            "rgb(90deg, 0, 0)",
            "rgb(255 0 0 / 90deg)",
            "hsl(120 90deg 50%)",
            "lab(50 40deg 20)",
            "oklch(0.5 0.1deg 90)",
        ] {
            assert_eq!(c(bad), "none", "{bad}");
        }
        assert_eq!(c("hsl(120deg, 100%, 50%)"), c("hsl(120, 100%, 50%)"));
        assert_eq!(c("hsl(0.5turn 100% 50%)"), c("hsl(180 100% 50%)"));
        assert_eq!(c("lch(50 40 90deg)"), c("lch(50 40 90)"));
        assert_ne!(c("hwb(90deg 10% 10%)"), "none");
    }
}
