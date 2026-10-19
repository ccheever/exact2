//! The look oracle's tells (LLP 1127.000 D1): what a frame's own numbers say is
//! wrong with it. Each is read from a look frame under the agent's clock, never
//! from a live frame. A tell catches a frame that is near black or clipped,
//! particles that drew and covered nothing, and a shadowed sun whose shadow maps
//! hold no texel nearer than their clear depth; it cannot tell a grey meadow
//! from a green one, a model facing backwards or a panel overflowing its box.
//! Those take a look at the picture.

/// Every tell, by the name `app.json`'s `game.look.silence` uses for it.
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

/// A measurement a tell needs: nothing to measure, taken, or not taken and why.
/// A tell is told only from a measurement; one that was not taken is a gap,
/// which leaves the look unverified rather than passed.
#[derive(Clone, Debug, PartialEq)]
pub enum Measure<T> {
    /// The frame has nothing this tell reads (no particles, no shadowed sun).
    Absent,
    /// The measurement.
    Taken(T),
    /// Why it was not taken.
    Unmeasured(String),
}

/// One particle draw as the look frame counted it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleDraw {
    /// What the draw batches by: dots or a look, blending, softness.
    pub batch: String,
    /// Particles it drew.
    pub live: u64,
    /// Fragments of it that passed the depth test.
    pub covered: u64,
}

/// What a look frame drew besides its pixels, as the tells read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// Each particle draw submitted to the camera after the CPU cull.
    pub particles: Measure<Vec<ParticleDraw>>,
    /// With a shadowed sun: the texels its shadow passes wrote, nearer than
    /// the maps' clear depth (counted on the GPU from the maps themselves).
    pub sun_texels: Measure<u64>,
    /// The caster instances those passes submitted, where the cull counted
    /// them: the listing's figure, which no tell reads.
    pub sun_casters: Option<u64>,
}
impl Default for Drawn {
    fn default() -> Self {
        Self {
            particles: Measure::Absent,
            sun_texels: Measure::Absent,
            sun_casters: None,
        }
    }
}

/// One tell that fired: its name, what it is about and its `logs` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tell {
    /// The name `game.look.silence` uses.
    pub name: &'static str,
    /// Which tell it is from one frame to the next: its name, and for a
    /// particle tell its draw's batch and which of that batch's draws it is.
    pub key: String,
    /// The line `logs` gets, `look: …`.
    pub line: String,
}

/// The tells `logs` last got, by key: a tell is logged when it appears or its
/// line changes, and one that the next judged frame no longer has is
/// forgotten, so its return is logged again.
#[derive(Default)]
pub(crate) struct Logged(Vec<(String, String)>);
impl Logged {
    /// The tells of a judged frame that `logs` gets; that frame's set
    /// replaces the last.
    pub fn fresh<'a>(&mut self, judged: &'a Judgement) -> Vec<&'a Tell> {
        let told = |t: &Tell| (self.0.iter()).any(|(k, l)| *k == t.key && *l == t.line);
        let fresh = judged.tells.iter().filter(|t| !told(t)).collect();
        self.0 = (judged.tells.iter())
            .map(|t| (t.key.clone(), t.line.clone()))
            .collect();
        fresh
    }
}

/// A frame's tells, in [`TELLS`] order, and what could not be measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Judgement {
    /// The tells that fired.
    pub tells: Vec<Tell>,
    /// Each measurement not taken, named: `particles: not measured (…)`.
    pub gaps: Vec<String>,
}

/// Judge a frame from its statistics and what it drew.
pub fn judge(stats: &FrameStats, drawn: &Drawn) -> Judgement {
    let mut out = Judgement::default();
    // A tell's key is its name unless it names its own.
    let mut tell = |name: &'static str, key: Option<String>, line: String| {
        let key = key.unwrap_or_else(|| name.to_owned());
        out.tells.push(Tell { name, key, line })
    };
    if stats.pixels == 0 {
        out.gaps.push("frame: no pixels were read back".into());
    } else {
        if stats.mean < NEAR_BLACK {
            tell(
                "near-black",
                None,
                format!("look: mean luminance {:.3}, near black", stats.mean),
            );
        }
        if stats.mean > NEAR_WHITE {
            tell(
                "near-white",
                None,
                format!("look: mean luminance {:.3}, near white", stats.mean),
            );
        }
        if stats.clipped > CLIPPED {
            tell(
                "clipped",
                None,
                format!("look: {:.0}% of pixels clipped", stats.clipped * 100.),
            );
        }
    }
    match &drawn.particles {
        Measure::Taken(draws) => {
            for (i, d) in draws.iter().enumerate() {
                if d.live == 0 || d.covered > 0 {
                    continue;
                }
                let line = format!(
                    "look: {} particles live, 0 pixels covered ({})",
                    d.live, d.batch
                );
                // Its batch, and which of that batch's draws: a neighbour of
                // the batch that recovers does not renumber it.
                let same = draws[..i].iter().filter(|e| e.batch == d.batch).count();
                let key = format!("particles-uncovered {} {same}", d.batch);
                tell("particles-uncovered", Some(key), line);
            }
        }
        Measure::Unmeasured(why) => out.gaps.push(format!("particles: not measured ({why})")),
        Measure::Absent => {}
    }
    match &drawn.sun_texels {
        Measure::Taken(0) => tell(
            "no-shadow-casters",
            None,
            "look: sun shadow maps hold no texel nearer than their clear depth".into(),
        ),
        Measure::Unmeasured(why) => out
            .gaps
            .push(format!("sun shadow maps: not measured ({why})")),
        _ => {}
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
        let names = |s: &FrameStats, d: &Drawn| {
            let j = judge(s, d);
            assert!(j.gaps.is_empty(), "{:?}", j.gaps);
            j.tells.iter().map(|t| t.name).collect::<Vec<_>>()
        };
        let draw = |live, covered| ParticleDraw {
            batch: "dots".into(),
            live,
            covered,
        };
        let good = FrameStats::of(&frame([120, 160, 140], 100));
        let fine = Drawn {
            particles: Measure::Taken(vec![draw(469, 900)]),
            sun_texels: Measure::Taken(3),
            ..Default::default()
        };
        assert!(names(&good, &fine).is_empty());
        let dark = FrameStats::of(&frame([10, 10, 10], 100));
        assert_eq!(names(&dark, &fine), ["near-black"]);
        // sRGB 250 is linear 0.956, past the near-white line; 245 is 0.913, short of it.
        assert_eq!(
            names(&FrameStats::of(&frame([250; 3], 100)), &fine),
            ["near-white"]
        );
        assert!(names(&FrameStats::of(&frame([245; 3], 100)), &fine).is_empty());
        let mut clipped = frame([120, 160, 140], 90);
        clipped.extend(frame([255; 3], 10));
        assert_eq!(names(&FrameStats::of(&clipped), &fine), ["clipped"]);
        // Each particle draw is judged alone: a covered one does not hide a buried one.
        let mixed = Drawn {
            particles: Measure::Taken(vec![draw(30, 900), draw(469, 0)]),
            ..fine.clone()
        };
        assert_eq!(
            judge(&good, &mixed).tells[0].line,
            "look: 469 particles live, 0 pixels covered (dots)"
        );
        let casterless = Drawn {
            sun_texels: Measure::Taken(0),
            ..fine.clone()
        };
        assert_eq!(names(&good, &casterless), ["no-shadow-casters"]);
        // No particles drawn and no shadowed sun are not tells.
        assert!(names(&good, &Drawn::default()).is_empty());
    }

    #[test]
    fn what_was_not_measured_is_a_gap_never_a_pass_or_a_tell() {
        let unmeasured = Drawn {
            particles: Measure::Unmeasured("no counting pass".into()),
            sun_texels: Measure::Unmeasured("the maps were not read back".into()),
            ..Default::default()
        };
        let good = FrameStats::of(&frame([120, 160, 140], 100));
        let j = judge(&good, &unmeasured);
        assert!(j.tells.is_empty());
        assert_eq!(
            j.gaps,
            [
                "particles: not measured (no counting pass)",
                "sun shadow maps: not measured (the maps were not read back)"
            ]
        );
        // An empty readback is not a black frame.
        let empty = judge(&FrameStats::of(&[]), &Drawn::default());
        assert!(empty.tells.is_empty());
        assert_eq!(empty.gaps, ["frame: no pixels were read back"]);
    }

    #[test]
    fn logs_get_a_tell_when_it_appears_or_changes_and_again_after_it_recovers() {
        let draw = |batch: &str, live| ParticleDraw {
            batch: batch.into(),
            live,
            covered: 0,
        };
        let dark = FrameStats::of(&frame([10, 10, 10], 100));
        let good = FrameStats::of(&frame([120, 160, 140], 100));
        let none = Drawn::default();
        let mut logged = Logged::default();
        let mut log = |s: &FrameStats, d: &Drawn| {
            let judged = judge(s, d);
            let lines: Vec<String> = (logged.fresh(&judged).into_iter())
                .map(|t| t.line.clone())
                .collect();
            lines
        };
        // Near black, recovered, near black again: logged twice.
        assert_eq!(log(&dark, &none).len(), 1);
        assert_eq!(log(&dark, &none).len(), 0);
        assert!(log(&good, &none).is_empty());
        assert_eq!(log(&dark, &none).len(), 1);
        // Two uncovered batches, unchanged across three looks: each logged once.
        let two = Drawn {
            particles: Measure::Taken(vec![draw("dots", 40), draw("sparks.tex", 12)]),
            ..Default::default()
        };
        assert_eq!(log(&good, &two).len(), 2);
        assert!(log(&good, &two).is_empty());
        assert!(log(&good, &two).is_empty());
        // Two uncovered draws of one batch are two tells (the first unchanged
        // since the last look); a changed line logs again.
        let same = Drawn {
            particles: Measure::Taken(vec![draw("dots", 40), draw("dots", 9)]),
            ..Default::default()
        };
        assert_eq!(
            log(&good, &same),
            ["look: 9 particles live, 0 pixels covered (dots)"]
        );
        let fewer = Drawn {
            particles: Measure::Taken(vec![draw("dots", 40), draw("dots", 8)]),
            ..Default::default()
        };
        assert_eq!(
            log(&good, &fewer),
            ["look: 8 particles live, 0 pixels covered (dots)"]
        );
        // The first of them recovers; the second, unchanged, is not logged again.
        let recovered = Drawn {
            particles: Measure::Taken(vec![
                ParticleDraw {
                    covered: 30,
                    ..draw("dots", 40)
                },
                draw("dots", 8),
            ]),
            ..Default::default()
        };
        assert!(log(&good, &recovered).is_empty());
    }
}
