//! The look oracle's tells (LLP 1127.000 D1): what a frame's own numbers say is
//! wrong with it. Each is read from a look frame under the agent's clock, never
//! from a live frame. A tell catches a frame that is near black or clipped,
//! particles that drew and covered nothing, and a shadowed sun whose shadow pass
//! kept no caster; it cannot tell a grey meadow from a green one, a model facing
//! backwards or a panel overflowing its box. Those take a look at the picture.

/// Every tell, by the name `app.json`'s `look.silence` uses for it.
pub const TELLS: [&str; 5] = [
    "near-black",
    "near-white",
    "clipped",
    "particles-uncovered",
    "no-shadow-casters",
];
/// Mean relative luminance (linear) below which a frame is near black.
pub const NEAR_BLACK: f32 = 0.02;
/// Mean relative luminance above which a frame is near white.
pub const NEAR_WHITE: f32 = 0.95;
/// Fraction of pixels white in every channel above which a frame is clipped.
pub const CLIPPED: f32 = 0.05;
/// Bins of the luminance histogram, over linear relative luminance 0–1.
pub const BINS: usize = 64;

/// A frame's luminance statistics, from its final target as it would be shown.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameStats {
    /// Pixels counted.
    pub pixels: u32,
    /// Pixels per luminance bin; bin `i` holds `[i, i + 1) / BINS`, the last 1 too.
    pub histogram: [u32; BINS],
    /// Mean relative luminance, linear (Rec. 709 weights over decoded sRGB).
    pub mean: f32,
    /// Fraction of pixels whose every channel is at full scale.
    pub clipped: f32,
}

impl FrameStats {
    /// Statistics of tightly packed sRGB-encoded RGBA rows (alpha ignored), the
    /// bytes a look frame reads back from either target format.
    pub fn of(rgba: &[u8]) -> Self {
        let decode = srgb_table();
        let (mut histogram, mut sum, mut clipped, mut pixels) = ([0u32; BINS], 0f64, 0u32, 0u32);
        for p in rgba.chunks_exact(4) {
            let y = 0.2126 * decode[p[0] as usize]
                + 0.7152 * decode[p[1] as usize]
                + 0.0722 * decode[p[2] as usize];
            histogram[((y * BINS as f32) as usize).min(BINS - 1)] += 1;
            sum += f64::from(y);
            clipped += u32::from(p[0] == 255 && p[1] == 255 && p[2] == 255);
            pixels += 1;
        }
        let n = pixels.max(1);
        Self {
            pixels,
            histogram,
            mean: (sum / f64::from(n)) as f32,
            clipped: clipped as f32 / n as f32,
        }
    }
}

fn srgb_table() -> [f32; 256] {
    std::array::from_fn(|i| {
        let c = i as f32 / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
}

/// What a look frame drew besides its pixels, as the tells read it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drawn {
    /// Particles submitted to the camera after the CPU cull, and whether any
    /// of their draws covered a sample.
    pub particles: Option<(u64, bool)>,
    /// With a shadowed sun: the casters its cascades kept after culling.
    pub sun_casters: Option<u64>,
}

/// One tell that fired: its name and its journal line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tell {
    /// The name `look.silence` uses.
    pub name: &'static str,
    /// The line the world's journal gets, `look: …`.
    pub line: String,
}

/// The tells a frame trips, in [`TELLS`] order.
pub fn tells(stats: &FrameStats, drawn: Drawn) -> Vec<Tell> {
    let mut out = Vec::new();
    let mut tell = |name, line: String| out.push(Tell { name, line });
    if stats.pixels > 0 && stats.mean < NEAR_BLACK {
        tell(
            "near-black",
            format!("look: mean luminance {:.3}, near black", stats.mean),
        );
    }
    if stats.pixels > 0 && stats.mean > NEAR_WHITE {
        tell(
            "near-white",
            format!("look: mean luminance {:.3}, near white", stats.mean),
        );
    }
    if stats.clipped > CLIPPED {
        tell(
            "clipped",
            format!("look: {:.0}% of pixels clipped", stats.clipped * 100.),
        );
    }
    if let Some((live, false)) = drawn.particles.filter(|(live, _)| *live > 0) {
        tell(
            "particles-uncovered",
            format!("look: {live} particles live, 0 pixels covered"),
        );
    }
    if drawn.sun_casters == Some(0) {
        tell(
            "no-shadow-casters",
            "look: sun shadow pass drew no casters".into(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(rgb: [u8; 3], n: usize) -> Vec<u8> {
        [rgb[0], rgb[1], rgb[2], 255].repeat(n)
    }

    #[test]
    fn statistics_decode_srgb_and_count_white() {
        let black = FrameStats::of(&frame([0; 3], 16));
        assert_eq!(
            (black.mean, black.clipped, black.histogram[0]),
            (0., 0., 16)
        );
        let white = FrameStats::of(&frame([255; 3], 16));
        assert_eq!(
            (white.mean, white.clipped, white.histogram[BINS - 1]),
            (1., 1., 16)
        );
        // sRGB 128, mid-grey on screen, is about a fifth of the light.
        let grey = FrameStats::of(&frame([128; 3], 4));
        assert!((grey.mean - 0.2159).abs() < 1e-3, "{}", grey.mean);
    }

    #[test]
    fn each_tell_fires_on_its_defect_and_none_on_a_good_frame() {
        let names = |s: &FrameStats, d| tells(s, d).iter().map(|t| t.name).collect::<Vec<_>>();
        let good = FrameStats::of(&frame([120, 160, 140], 100));
        let fine = Drawn {
            particles: Some((469, true)),
            sun_casters: Some(3),
        };
        assert!(names(&good, fine).is_empty());
        let dark = FrameStats::of(&frame([10, 10, 10], 100));
        assert_eq!(names(&dark, fine), ["near-black"]);
        let mut clipped = frame([120, 160, 140], 90);
        clipped.extend(frame([255; 3], 10));
        assert_eq!(names(&FrameStats::of(&clipped), fine), ["clipped"]);
        let uncovered = Drawn {
            particles: Some((469, false)),
            ..fine
        };
        assert_eq!(
            tells(&good, uncovered)[0].line,
            "look: 469 particles live, 0 pixels covered"
        );
        let casterless = Drawn {
            sun_casters: Some(0),
            ..fine
        };
        assert_eq!(names(&good, casterless), ["no-shadow-casters"]);
        // No particles drawn and no shadowed sun are not tells.
        assert!(names(&good, Drawn::default()).is_empty());
    }
}
