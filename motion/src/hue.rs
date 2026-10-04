//! CSS's hue colours: `hsl()`, `hsla()` and `hwb()` (CSS Color 4 §7, §8),
//! as 8-bit sRGB. Motion sits below the kernel, so the one parser is here:
//! a keyframe's colour and the kernel's `Color::parse` both take these.
//! The rules follow `exact-canvas`'s parser, which Chrome checked: legacy
//! commas need percentages, the modern space form takes numbers too, a hue
//! takes `deg`, `grad`, `rad` or `turn`, and alpha is a number or a
//! percentage after `/` (or a fourth comma value).

/// `hsl(…)`, `hsla(…)` or `hwb(…)` as `[r, g, b, a]`, or `None`.
pub fn hue(text: &str) -> Option<[u8; 4]> {
    let lower = text.trim().to_ascii_lowercase();
    let (name, rest) = lower.split_once('(')?;
    let body = rest.strip_suffix(')')?;
    let name = name.trim();
    if !matches!(name, "hsl" | "hsla" | "hwb") {
        return None;
    }
    let legacy = body.contains(',');
    let (parts, alpha) = args(body)?;
    let Part::Num(h) = part(parts[0])? else {
        return None;
    };
    let frac = |t: &str| match part(t)? {
        Part::Pct(p) => Some((p / 100.0).clamp(0.0, 1.0)),
        Part::Num(n) if !legacy => Some((n / 100.0).clamp(0.0, 1.0)),
        Part::Num(_) => None,
    };
    let (x, y) = (frac(parts[1])?, frac(parts[2])?);
    let (r, g, b) = if name == "hwb" {
        if legacy {
            return None;
        }
        hwb(h, x, y)
    } else {
        hsl(h, x, y)
    };
    let a = match alpha.map(part) {
        None => 1.0,
        Some(Some(Part::Num(v))) => v,
        Some(Some(Part::Pct(p))) => p / 100.0,
        Some(None) => return None,
    };
    let byte = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Some([byte(r), byte(g), byte(b), byte(a.clamp(0.0, 1.0))])
}

#[derive(Clone, Copy)]
enum Part {
    Num(f64),
    Pct(f64),
}

/// A component: a number, a percentage, an angle in degrees, or `none`.
fn part(t: &str) -> Option<Part> {
    if t == "none" {
        return Some(Part::Num(0.0));
    }
    if let Some(p) = t.strip_suffix('%') {
        return number(p).map(Part::Pct);
    }
    for (unit, degrees) in [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / core::f64::consts::PI),
        ("turn", 360.0),
    ] {
        if let Some(v) = t.strip_suffix(unit) {
            return number(v).map(|v| Part::Num(v * degrees));
        }
    }
    number(t).map(Part::Num)
}

fn number(t: &str) -> Option<f64> {
    let digits = !t.is_empty()
        && t.bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'e'));
    let v = if digits {
        exact_num::parse_f64(t).ok()?
    } else {
        return None;
    };
    v.is_finite().then_some(v)
}

/// `a, b, c[, d]` or `a b c [/ d]`.
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

#[cfg(test)]
mod tests {
    use super::hue;

    #[test]
    fn hue_colours_parse_as_a_browser_does() {
        let red = Some([255, 0, 0, 255]);
        assert_eq!(hue("hsl(0, 100%, 50%)"), red);
        assert_eq!(hue("hsl(0 100% 50%)"), red);
        assert_eq!(hue("HSL(360deg 100 50)"), red);
        assert_eq!(hue("hsl(0.5turn 100% 50%)"), Some([0, 255, 255, 255]));
        assert_eq!(hue("hsla(120, 100%, 25%, 0.5)"), Some([0, 128, 0, 128]));
        assert_eq!(hue("hsl(240 100% 50% / 50%)"), Some([0, 0, 255, 128]));
        assert_eq!(hue("hwb(0 0% 0%)"), red);
        assert_eq!(hue("hwb(0 60% 60%)"), Some([128, 128, 128, 255]));
        for bad in [
            "hsl(0, 100, 50)",
            "hwb(0, 0%, 0%)",
            "hsl(0 100%)",
            "hsl(red 1% 1%)",
            "hsl(0 1% 1% /)",
            "rgb(0 0 0)",
        ] {
            assert_eq!(hue(bad), None, "{bad}");
        }
    }
}
