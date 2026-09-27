//! Canvas colours (LLP 1056 D3): CSS Color 4's sRGB forms — hex, `rgb()`,
//! `rgba()`, `hsl()`, `hsla()`, `hwb()`, the named colours, `transparent`
//! and `currentColor` — and the canvas serialisation (`#rrggbb` when opaque,
//! `rgba(r, g, b, a)` otherwise). The kernel's parser takes hex, `rgb()` and
//! `transparent` only; this one is the canvas's, and the kernel may adopt it.
//! A form outside these (`lab()`, `color()`) does not parse, so an assignment
//! of it is ignored as any unparseable colour is.

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Parsed {
    /// A colour.
    Color(Rgba),
    /// `currentColor`: the canvas node's `color`, resolved by the caller.
    Current,
}

/// Parse a CSS colour, or `None` when it is not one (the assignment is then
/// ignored).
pub fn parse(input: &str) -> Option<Parsed> {
    let s = input.trim().to_ascii_lowercase();
    if s == "currentcolor" {
        return Some(Parsed::Current);
    }
    if s == "transparent" {
        return Some(Parsed::Color(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        }));
    }
    if let Some(hex) = s.strip_prefix('#') {
        return hex_color(hex).map(Parsed::Color);
    }
    if let Some(open) = s.find('(') {
        let name = s[..open].trim();
        let body = s[open + 1..].strip_suffix(')')?;
        return functional(name, body).map(Parsed::Color);
    }
    named(&s).map(Parsed::Color)
}

fn hex_color(hex: &str) -> Option<Rgba> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok();
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let (r, g, b, a) = match hex.len() {
        3 | 4 => {
            let d = |i| digit(i).map(|v| v * 17);
            let a = if hex.len() == 4 { d(3)? } else { 255 };
            (d(0)?, d(1)?, d(2)?, a)
        }
        6 | 8 => {
            let a = if hex.len() == 8 { pair(6)? } else { 255 };
            (pair(0)?, pair(2)?, pair(4)?, a)
        }
        _ => return None,
    };
    Some(Rgba {
        r,
        g,
        b,
        a: a as f64 / 255.0,
    })
}

/// A component: a number, a percentage, an angle, or `none`.
#[derive(Debug, Clone, Copy)]
enum Arg {
    Num(f64),
    Pct(f64),
}

fn component(t: &str) -> Option<Arg> {
    if t == "none" {
        return Some(Arg::Num(0.0));
    }
    if let Some(p) = t.strip_suffix('%') {
        return number(p).map(Arg::Pct);
    }
    for (unit, per_degree) in [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f64::consts::PI),
        ("turn", 360.0),
    ] {
        if let Some(v) = t.strip_suffix(unit) {
            return number(v).map(|v| Arg::Num(v * per_degree));
        }
    }
    number(t).map(Arg::Num)
}

fn number(t: &str) -> Option<f64> {
    let ok = !t.is_empty()
        && t.bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'e'));
    let v: f64 = if ok { t.parse().ok()? } else { return None };
    v.is_finite().then_some(v)
}

/// Split `a, b, c[, d]` or `a b c [/ d]` into components.
fn args(body: &str) -> Option<(Vec<&str>, Option<&str>)> {
    let body = body.trim();
    if body.contains(',') {
        let parts: Vec<&str> = body.split(',').map(str::trim).collect();
        if parts
            .iter()
            .any(|p| p.is_empty() || p.contains(' ') || p.contains('/'))
        {
            return None;
        }
        return match parts.len() {
            3 => Some((parts, None)),
            4 => Some((parts[..3].to_vec(), Some(parts[3]))),
            _ => None,
        };
    }
    let (main, alpha) = match body.split_once('/') {
        Some((m, a)) => (m, Some(a.trim())),
        None => (body, None),
    };
    let parts: Vec<&str> = main.split_whitespace().collect();
    (parts.len() == 3 && alpha != Some("")).then_some((parts, alpha))
}

/// Alpha, clamped and held to 8 bits as Chrome stores a canvas colour's.
fn alpha(t: Option<&str>) -> Option<f64> {
    let a = match t.map(component) {
        None => 1.0,
        Some(Some(Arg::Num(v))) => v,
        Some(Some(Arg::Pct(p))) => p / 100.0,
        Some(None) => return None,
    };
    Some((a.clamp(0.0, 1.0) * 255.0).round() / 255.0)
}

fn channel(v: f64) -> u8 {
    v.clamp(0.0, 255.0).round() as u8
}

fn functional(name: &str, body: &str) -> Option<Rgba> {
    let (parts, a) = args(body)?;
    let c: Vec<Arg> = parts.iter().map(|p| component(p)).collect::<Option<_>>()?;
    let a = alpha(a)?;
    let legacy = body.contains(',');
    match name {
        "rgb" | "rgba" => {
            // Legacy syntax needs all numbers or all percentages.
            let pct = c.iter().filter(|x| matches!(x, Arg::Pct(_))).count();
            if legacy && pct != 0 && pct != 3 {
                return None;
            }
            let v = |x: Arg| match x {
                Arg::Num(n) => n,
                Arg::Pct(p) => p * 255.0 / 100.0,
            };
            Some(Rgba {
                r: channel(v(c[0])),
                g: channel(v(c[1])),
                b: channel(v(c[2])),
                a,
            })
        }
        "hsl" | "hsla" | "hwb" => {
            let Arg::Num(h) = c[0] else { return None };
            let frac = |x: Arg| match x {
                Arg::Pct(p) => Some((p / 100.0).clamp(0.0, 1.0)),
                Arg::Num(n) if !legacy => Some((n / 100.0).clamp(0.0, 1.0)),
                Arg::Num(_) => None,
            };
            let (s, l) = (frac(c[1])?, frac(c[2])?);
            let (r, g, b) = if name == "hwb" {
                if legacy {
                    return None;
                }
                hwb(h, s, l)
            } else {
                hsl(h, s, l)
            };
            Some(Rgba {
                r: channel(r * 255.0),
                g: channel(g * 255.0),
                b: channel(b * 255.0),
                a,
            })
        }
        _ => None,
    }
}

fn hsl(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let h = h.rem_euclid(360.0);
    let f = |n: f64| {
        let k = (n + h / 30.0) % 12.0;
        let a = s * l.min(1.0 - l);
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    (f(0.0), f(8.0), f(4.0))
}

fn hwb(h: f64, w: f64, b: f64) -> (f64, f64, f64) {
    if w + b >= 1.0 {
        let g = w / (w + b);
        return (g, g, g);
    }
    let (r, g, bl) = hsl(h, 1.0, 0.5);
    let f = |c: f64| c * (1.0 - w - b) + w;
    (f(r), f(g), f(bl))
}

/// The CSS named colours.
const NAMED: &str =
    "aliceblue f0f8ff antiquewhite faebd7 aqua 00ffff aquamarine 7fffd4 azure f0ffff \
beige f5f5dc bisque ffe4c4 black 000000 blanchedalmond ffebcd blue 0000ff blueviolet 8a2be2 \
brown a52a2a burlywood deb887 cadetblue 5f9ea0 chartreuse 7fff00 chocolate d2691e coral ff7f50 \
cornflowerblue 6495ed cornsilk fff8dc crimson dc143c cyan 00ffff darkblue 00008b darkcyan 008b8b \
darkgoldenrod b8860b darkgray a9a9a9 darkgreen 006400 darkgrey a9a9a9 darkkhaki bdb76b \
darkmagenta 8b008b darkolivegreen 556b2f darkorange ff8c00 darkorchid 9932cc darkred 8b0000 \
darksalmon e9967a darkseagreen 8fbc8f darkslateblue 483d8b darkslategray 2f4f4f \
darkslategrey 2f4f4f darkturquoise 00ced1 darkviolet 9400d3 deeppink ff1493 deepskyblue 00bfff \
dimgray 696969 dimgrey 696969 dodgerblue 1e90ff firebrick b22222 floralwhite fffaf0 \
forestgreen 228b22 fuchsia ff00ff gainsboro dcdcdc ghostwhite f8f8ff gold ffd700 \
goldenrod daa520 gray 808080 green 008000 greenyellow adff2f grey 808080 honeydew f0fff0 \
hotpink ff69b4 indianred cd5c5c indigo 4b0082 ivory fffff0 khaki f0e68c lavender e6e6fa \
lavenderblush fff0f5 lawngreen 7cfc00 lemonchiffon fffacd lightblue add8e6 lightcoral f08080 \
lightcyan e0ffff lightgoldenrodyellow fafad2 lightgray d3d3d3 lightgreen 90ee90 lightgrey d3d3d3 \
lightpink ffb6c1 lightsalmon ffa07a lightseagreen 20b2aa lightskyblue 87cefa \
lightslategray 778899 lightslategrey 778899 lightsteelblue b0c4de lightyellow ffffe0 lime 00ff00 \
limegreen 32cd32 linen faf0e6 magenta ff00ff maroon 800000 mediumaquamarine 66cdaa \
mediumblue 0000cd mediumorchid ba55d3 mediumpurple 9370db mediumseagreen 3cb371 \
mediumslateblue 7b68ee mediumspringgreen 00fa9a mediumturquoise 48d1cc mediumvioletred c71585 \
midnightblue 191970 mintcream f5fffa mistyrose ffe4e1 moccasin ffe4b5 navajowhite ffdead \
navy 000080 oldlace fdf5e6 olive 808000 olivedrab 6b8e23 orange ffa500 orangered ff4500 \
orchid da70d6 palegoldenrod eee8aa palegreen 98fb98 paleturquoise afeeee palevioletred db7093 \
papayawhip ffefd5 peachpuff ffdab9 peru cd853f pink ffc0cb plum dda0dd powderblue b0e0e6 \
purple 800080 rebeccapurple 663399 red ff0000 rosybrown bc8f8f royalblue 4169e1 \
saddlebrown 8b4513 salmon fa8072 sandybrown f4a460 seagreen 2e8b57 seashell fff5ee \
sienna a0522d silver c0c0c0 skyblue 87ceeb slateblue 6a5acd slategray 708090 slategrey 708090 \
snow fffafa springgreen 00ff7f steelblue 4682b4 tan d2b48c teal 008080 thistle d8bfd8 \
tomato ff6347 turquoise 40e0d0 violet ee82ee wheat f5deb3 white ffffff whitesmoke f5f5f5 \
yellow ffff00 yellowgreen 9acd32";

fn named(s: &str) -> Option<Rgba> {
    let mut it = NAMED.split_whitespace();
    while let (Some(name), Some(hex)) = (it.next(), it.next()) {
        if name == s {
            return hex_color(hex);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> String {
        match parse(s) {
            Some(Parsed::Color(c)) => c.serialize(),
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
        for bad in [
            "",
            "#12",
            "rgb(1, 2)",
            "rgb(1, 2%, 3)",
            "blurple",
            "lab(50 0 0)",
            "rgb(1 2, 3)",
            "hsl(120, 50, 50)",
        ] {
            assert_eq!(c(bad), "none", "{bad}");
        }
    }
}
