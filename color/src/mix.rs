//! Colour interpolation (CSS Color 4 §12).

use crate::{linear_to_srgb, srgb_to_linear, Space, Wide};

/// A `<color-interpolation-method>` space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum MixSpace {
    Srgb,
    SrgbLinear,
    DisplayP3,
    DisplayP3Linear,
    A98Rgb,
    ProphotoRgb,
    Rec2020,
    Lab,
    Oklab,
    XyzD50,
    XyzD65,
    Hsl,
    Hwb,
    Lch,
    Oklch,
}

/// How a polar space's hue goes round (CSS Color 4 §12.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(missing_docs)]
pub enum HueMethod {
    #[default]
    Shorter,
    Longer,
    Increasing,
    Decreasing,
}

/// A `<color-interpolation-method>`: `in oklch longer hue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interpolation {
    /// The space.
    pub space: MixSpace,
    /// The hue method; polar spaces only.
    pub hue: HueMethod,
}

impl Interpolation {
    /// CSS's default for non-legacy colours.
    pub const OKLAB: Interpolation = Interpolation {
        space: MixSpace::Oklab,
        hue: HueMethod::Shorter,
    };

    /// `in <space> [<method> hue]` at the start of `words`, and the words it took.
    pub fn parse(words: &[&str]) -> Option<(Interpolation, usize)> {
        if words.first().map(|w| w.to_ascii_lowercase()) != Some("in".into()) {
            return None;
        }
        let space = MixSpace::from_name(&words.get(1)?.to_ascii_lowercase())?;
        let polar = space.is_polar();
        let method = match (words.get(2), words.get(3)) {
            (Some(m), Some(h)) if polar && h.eq_ignore_ascii_case("hue") => {
                Some(match m.to_ascii_lowercase().as_str() {
                    "shorter" => HueMethod::Shorter,
                    "longer" => HueMethod::Longer,
                    "increasing" => HueMethod::Increasing,
                    "decreasing" => HueMethod::Decreasing,
                    _ => return None,
                })
            }
            _ => None,
        };
        Some((
            Interpolation {
                space,
                hue: method.unwrap_or_default(),
            },
            if method.is_some() { 4 } else { 2 },
        ))
    }

    /// CSS's serialisation, omitting the default hue method.
    pub fn css(self) -> String {
        match self.hue {
            HueMethod::Shorter => format!("in {}", self.space.name()),
            m => format!(
                "in {} {} hue",
                self.space.name(),
                ["shorter", "longer", "increasing", "decreasing"][m as usize]
            ),
        }
    }
}

impl MixSpace {
    fn name(self) -> &'static str {
        match self {
            MixSpace::Srgb => "srgb",
            MixSpace::SrgbLinear => "srgb-linear",
            MixSpace::DisplayP3 => "display-p3",
            MixSpace::DisplayP3Linear => "display-p3-linear",
            MixSpace::A98Rgb => "a98-rgb",
            MixSpace::ProphotoRgb => "prophoto-rgb",
            MixSpace::Rec2020 => "rec2020",
            MixSpace::Lab => "lab",
            MixSpace::Oklab => "oklab",
            MixSpace::XyzD50 => "xyz-d50",
            MixSpace::XyzD65 => "xyz-d65",
            MixSpace::Hsl => "hsl",
            MixSpace::Hwb => "hwb",
            MixSpace::Lch => "lch",
            MixSpace::Oklch => "oklch",
        }
    }

    fn from_name(name: &str) -> Option<MixSpace> {
        Some(match name {
            "srgb" => MixSpace::Srgb,
            "srgb-linear" => MixSpace::SrgbLinear,
            "display-p3" => MixSpace::DisplayP3,
            "display-p3-linear" => MixSpace::DisplayP3Linear,
            "a98-rgb" => MixSpace::A98Rgb,
            "prophoto-rgb" => MixSpace::ProphotoRgb,
            "rec2020" => MixSpace::Rec2020,
            "lab" => MixSpace::Lab,
            "oklab" => MixSpace::Oklab,
            "xyz" | "xyz-d65" => MixSpace::XyzD65,
            "xyz-d50" => MixSpace::XyzD50,
            "hsl" => MixSpace::Hsl,
            "hwb" => MixSpace::Hwb,
            "lch" => MixSpace::Lch,
            "oklch" => MixSpace::Oklch,
            _ => return None,
        })
    }

    fn is_polar(self) -> bool {
        matches!(
            self,
            MixSpace::Hsl | MixSpace::Hwb | MixSpace::Lch | MixSpace::Oklch
        )
    }

    fn hue_index(self) -> usize {
        match self {
            MixSpace::Hsl | MixSpace::Hwb => 0,
            _ => 2,
        }
    }

    /// Extended linear sRGB to this space's components.
    pub fn from_linear_srgb(self, v: [f64; 3]) -> [f64; 3] {
        let encode = |v: [f64; 3]| v.map(linear_to_srgb);
        match self {
            MixSpace::Srgb => encode(v),
            MixSpace::SrgbLinear => v,
            MixSpace::DisplayP3 => encode(crate::from_xyz65(
                Space::DisplayP3Linear,
                crate::to_xyz65_from_srgb(v),
            )),
            MixSpace::DisplayP3Linear => {
                crate::from_xyz65(Space::DisplayP3Linear, crate::to_xyz65_from_srgb(v))
            }
            MixSpace::A98Rgb => crate::from_xyz65(Space::A98Rgb, crate::to_xyz65_from_srgb(v))
                .map(|c| c.abs().powf(256.0 / 563.0).copysign(c)),
            MixSpace::ProphotoRgb => {
                crate::from_xyz65(Space::ProphotoRgb, crate::to_xyz65_from_srgb(v)).map(|c| {
                    let a = c.abs();
                    (if a < 1.0 / 512.0 {
                        a * 16.0
                    } else {
                        a.powf(1.0 / 1.8)
                    })
                    .copysign(c)
                })
            }
            MixSpace::Rec2020 => crate::from_xyz65(Space::Rec2020, crate::to_xyz65_from_srgb(v))
                .map(crate::linear_to_rec2020),
            MixSpace::XyzD65 => crate::to_xyz65_from_srgb(v),
            MixSpace::XyzD50 => crate::xyz65_to_d50(crate::to_xyz65_from_srgb(v)),
            MixSpace::Lab => {
                crate::xyz_d50_to_lab(crate::xyz65_to_d50(crate::to_xyz65_from_srgb(v)))
            }
            MixSpace::Lch => polar(crate::xyz_d50_to_lab(crate::xyz65_to_d50(
                crate::to_xyz65_from_srgb(v),
            ))),
            MixSpace::Oklab => crate::linear_to_oklab(v),
            MixSpace::Oklch => polar(crate::linear_to_oklab(v)),
            MixSpace::Hsl => rgb_to_hsl(encode(v)),
            MixSpace::Hwb => rgb_to_hwb(encode(v)),
        }
    }

    /// This space's components to extended linear sRGB.
    pub fn to_linear_srgb(self, c: [f64; 3]) -> [f64; 3] {
        let as_wide = |space| {
            Wide {
                space,
                c,
                alpha: 1.0,
            }
            .linear_srgb()
        };
        match self {
            MixSpace::Srgb => c.map(srgb_to_linear),
            MixSpace::SrgbLinear => c,
            MixSpace::DisplayP3 => as_wide(Space::DisplayP3),
            MixSpace::DisplayP3Linear => as_wide(Space::DisplayP3Linear),
            MixSpace::A98Rgb => as_wide(Space::A98Rgb),
            MixSpace::ProphotoRgb => as_wide(Space::ProphotoRgb),
            MixSpace::Rec2020 => as_wide(Space::Rec2020),
            MixSpace::XyzD65 => as_wide(Space::XyzD65),
            MixSpace::XyzD50 => as_wide(Space::XyzD50),
            MixSpace::Lab => as_wide(Space::Lab),
            MixSpace::Lch => as_wide(Space::Lch),
            MixSpace::Oklab => as_wide(Space::Oklab),
            MixSpace::Oklch => as_wide(Space::Oklch),
            MixSpace::Hsl => {
                let (r, g, b) = crate::hsl(c[0], c[1] / 100.0, c[2] / 100.0);
                [r, g, b].map(srgb_to_linear)
            }
            MixSpace::Hwb => {
                let (r, g, b) = crate::hwb(c[0], c[1] / 100.0, c[2] / 100.0);
                [r, g, b].map(srgb_to_linear)
            }
        }
    }
}

/// a/b to chroma/hue (degrees).
fn polar([l, a, b]: [f64; 3]) -> [f64; 3] {
    let h = b.atan2(a).to_degrees().rem_euclid(360.0);
    [l, (a * a + b * b).sqrt(), h]
}

fn rgb_to_hsl([r, g, b]: [f64; 3]) -> [f64; 3] {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let (l, d) = ((max + min) / 2.0, max - min);
    let s = if d == 0.0 || l == 0.0 || l == 1.0 {
        0.0
    } else {
        d / (1.0 - (2.0 * l - 1.0).abs())
    };
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    [h, s * 100.0, l * 100.0]
}

fn rgb_to_hwb(rgb: [f64; 3]) -> [f64; 3] {
    let h = rgb_to_hsl(rgb)[0];
    let (w, bl) = (
        rgb[0].min(rgb[1]).min(rgb[2]),
        1.0 - rgb[0].max(rgb[1]).max(rgb[2]),
    );
    [h, w * 100.0, bl * 100.0]
}

/// Components, alpha, and whether the hue is powerless (CSS Color 4 §12.2).
fn endpoint(w: &Wide, space: MixSpace) -> ([f64; 3], f64, bool) {
    let c = space.from_linear_srgb(w.linear_srgb());
    let achromatic = space.is_polar() && {
        let chroma = match space {
            MixSpace::Hsl | MixSpace::Hwb => {
                let rgb = MixSpace::Srgb.from_linear_srgb(w.linear_srgb());
                rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2])
            }
            _ => c[1],
        };
        chroma.abs() < 1e-6
    };
    (c, w.alpha, achromatic)
}

/// The colour `t` of the way from `a` to `b`, premultiplied, as extended
/// linear sRGB and alpha.
pub fn mix(a: &Wide, b: &Wide, t: f64, how: Interpolation) -> ([f64; 3], f64) {
    let space = how.space;
    let (mut ca, aa, a_gray) = endpoint(a, space);
    let (mut cb, ab, b_gray) = endpoint(b, space);
    let hi = space.hue_index();
    if space.is_polar() {
        match (a_gray, b_gray) {
            (true, false) => ca[hi] = cb[hi],
            (false, true) => cb[hi] = ca[hi],
            _ => {}
        }
        let (h1, h2) = (ca[hi], cb[hi]);
        let d = h2 - h1;
        cb[hi] = match how.hue {
            HueMethod::Shorter if d > 180.0 => h2 - 360.0,
            HueMethod::Shorter if d < -180.0 => h2 + 360.0,
            HueMethod::Longer if (0.0..180.0).contains(&d) && d != 0.0 => h2 - 360.0,
            HueMethod::Longer if (-180.0..=0.0).contains(&d) => h2 + 360.0,
            HueMethod::Increasing if d < 0.0 => h2 + 360.0,
            HueMethod::Decreasing if d > 0.0 => h2 - 360.0,
            _ => h2,
        };
    }
    let pre = |c: [f64; 3], alpha: f64| {
        let mut out = c;
        for (i, v) in out.iter_mut().enumerate() {
            if !(space.is_polar() && i == hi) {
                *v *= alpha;
            }
        }
        out
    };
    let (pa, pb) = (pre(ca, aa), pre(cb, ab));
    let alpha = aa + (ab - aa) * t;
    let mut c = [0.0; 3];
    for i in 0..3 {
        let v = pa[i] + (pb[i] - pa[i]) * t;
        c[i] = if space.is_polar() && i == hi {
            v.rem_euclid(360.0)
        } else if alpha > 0.0 {
            v / alpha
        } else {
            v
        };
    }
    (space.to_linear_srgb(c), alpha)
}
