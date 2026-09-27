//! Canvas colours (LLP 1056 D3): CSS Color 4's sRGB forms — hex, `rgb()`,
//! `rgba()`, `hsl()`, `hsla()`, `hwb()`, the named colours, `transparent`
//! and `currentColor` — and the canvas serialisation (`#rrggbb` when opaque,
//! `rgba(r, g, b, a)` otherwise). The kernel's parser takes hex, `rgb()` and
//! `transparent` only; this one is the canvas's, and the kernel may adopt it.
//! The wide forms (`lab()`, `lch()`, `oklab()`, `oklch()` and `color()` in
//! `srgb`, `srgb-linear`, `display-p3`, `xyz`, `xyz-d50` and `xyz-d65`) are
//! converted to sRGB and clipped, as an sRGB canvas draws them, and keep
//! their own serialisation, as Chrome's getters return it.

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
        if let Some(wide) = WIDE.get().and_then(|f| f(name, body)) {
            return Some(wide);
        }
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

/// The wide forms' parser, once linked ([`link_wide`]).
static WIDE: std::sync::OnceLock<fn(&str, &str) -> Option<Parsed>> = std::sync::OnceLock::new();

/// Link the wide forms (`lab()`, `lch()`, `oklab()`, `oklch()`,
/// `color()`): until a host calls this, they do not parse and their
/// assignments are ignored. Native hosts link them at start; a web artifact
/// links them when its data crate's source names one (LLP 1047 D2: linked
/// by use; the web core's budget).
pub fn link_wide() {
    let _ = WIDE.set(wide);
}

/// The wide forms: modern syntax only, three components and an alpha.
fn wide(name: &str, body: &str) -> Option<Parsed> {
    let (space, body) = if name == "color" {
        let body = body.trim_start();
        let end = body.find(char::is_whitespace)?;
        (Some(&body[..end]), &body[end..])
    } else {
        (None, body)
    };
    let kind = match (name, space) {
        ("lab", None) => 0,
        ("lch", None) => 1,
        ("oklab", None) => 2,
        ("oklch", None) => 3,
        ("color", Some("srgb")) => 4,
        ("color", Some("srgb-linear")) => 5,
        ("color", Some("display-p3")) => 6,
        ("color", Some("xyz" | "xyz-d65")) => 7,
        ("color", Some("xyz-d50")) => 8,
        _ => return None,
    };
    if body.contains(',') {
        return None;
    }
    let (parts, a) = args(body)?;
    let c: Vec<Arg> = parts.iter().map(|p| component(p)).collect::<Option<_>>()?;
    // Percentages' reference ranges (CSS Color 4 §8–§10).
    let pct = [
        [100.0, 125.0, 125.0],
        [100.0, 150.0, 0.0],
        [1.0, 0.4, 0.4],
        [1.0, 0.4, 0.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
    ][kind];
    let mut v = [0.0; 3];
    for i in 0..3 {
        v[i] = match c[i] {
            Arg::Num(n) => n,
            Arg::Pct(p) if !(matches!(kind, 1 | 3) && i == 2) => p / 100.0 * pct[i],
            Arg::Pct(_) => return None,
        };
    }
    if matches!(kind, 0 | 1) {
        v[0] = v[0].clamp(0.0, 100.0);
    }
    if matches!(kind, 2 | 3) {
        v[0] = v[0].clamp(0.0, 1.0);
    }
    if matches!(kind, 1 | 3) {
        v[1] = v[1].max(0.0);
        v[2] = v[2].rem_euclid(360.0);
    }
    let alpha_raw = match a.map(component) {
        None => 1.0,
        Some(Some(Arg::Num(n))) => n,
        Some(Some(Arg::Pct(p))) => p / 100.0,
        Some(None) => return None,
    }
    .clamp(0.0, 1.0);
    let lin = match kind {
        0 => lab_to_linear(v[0], v[1], v[2]),
        1 => {
            let h = v[2].to_radians();
            lab_to_linear(v[0], v[1] * h.cos(), v[1] * h.sin())
        }
        2 => oklab_to_linear(v[0], v[1], v[2]),
        3 => {
            let h = v[2].to_radians();
            oklab_to_linear(v[0], v[1] * h.cos(), v[1] * h.sin())
        }
        4 => v.map(to_linear),
        5 => v,
        6 => {
            let xyz = mul(&P3_TO_XYZ, v.map(to_linear));
            mul(&XYZ_TO_SRGB, xyz)
        }
        7 => mul(&XYZ_TO_SRGB, v),
        _ => mul(&XYZ_TO_SRGB, mul(&D50_TO_D65, v)),
    };
    let byte = |c: f64| channel(from_linear(c) * 255.0);
    let rgba = Rgba {
        r: byte(lin[0]),
        g: byte(lin[1]),
        b: byte(lin[2]),
        a: (alpha_raw * 255.0).round() / 255.0,
    };
    let n = crate::font::js_number;
    let mut text = match space {
        Some(space) => format!("color({space} {} {} {}", n(v[0]), n(v[1]), n(v[2])),
        None => format!("{name}({} {} {}", n(v[0]), n(v[1]), n(v[2])),
    };
    if alpha_raw < 1.0 {
        text.push_str(&format!(" / {}", n(alpha_raw)));
    }
    text.push(')');
    Some(Parsed::Wide(rgba, text))
}

type M3 = [[f64; 3]; 3];
const XYZ_TO_SRGB: M3 = [
    [3.2409699419045226, -1.537383177570094, -0.4986107602930034],
    [-0.9692436362808796, 1.8759675015077202, 0.04155505740717559],
    [
        0.05563007969699366,
        -0.20397695888897652,
        1.0569715142428786,
    ],
];
const D50_TO_D65: M3 = [
    [0.955473421488075, -0.02309845494876471, 0.06325924320057072],
    [
        -0.0283697093338637,
        1.0099953980813041,
        0.021041441191917323,
    ],
    [
        0.012314014864481998,
        -0.020507649298898964,
        1.330365926242124,
    ],
];
const P3_TO_XYZ: M3 = [
    [0.4865709486482162, 0.26566769316909306, 0.1982172852343625],
    [0.2289745640697488, 0.6917385218365064, 0.079286914093745],
    [0.0, 0.04511338185890264, 1.043944368900976],
];

fn mul(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn to_linear(c: f64) -> f64 {
    let a = c.abs();
    let v = if a <= 0.04045 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    v.copysign(c)
}

fn from_linear(c: f64) -> f64 {
    let a = c.abs();
    let v = if a <= 0.0031308 {
        12.92 * a
    } else {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    };
    v.copysign(c).clamp(0.0, 1.0)
}

fn lab_to_linear(l: f64, a: f64, b: f64) -> [f64; 3] {
    const K: f64 = 24389.0 / 27.0;
    const E: f64 = 216.0 / 24389.0;
    let f1 = (l + 16.0) / 116.0;
    let f0 = a / 500.0 + f1;
    let f2 = f1 - b / 200.0;
    let x = if f0.powi(3) > E {
        f0.powi(3)
    } else {
        (116.0 * f0 - 16.0) / K
    };
    let y = if l > K * E { f1.powi(3) } else { l / K };
    let z = if f2.powi(3) > E {
        f2.powi(3)
    } else {
        (116.0 * f2 - 16.0) / K
    };
    let d50 = [x * 0.3457 / 0.3585, y, z * (1.0 - 0.3457 - 0.3585) / 0.3585];
    mul(&XYZ_TO_SRGB, mul(&D50_TO_D65, d50))
}

fn oklab_to_linear(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    ]
}

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
}
