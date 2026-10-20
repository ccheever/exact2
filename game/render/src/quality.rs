//! The quality ladder (LLP 1139.003 D3): the rung a world's renderer is made
//! at, from the game's `Game::quality` and the device's GPU tier, and what
//! each rung clamps of what the game left unsaid. Presentation only: nothing
//! here reaches the world, its saves or its hash.

use exact_game::{Quality, SceneSamples, AUTO_RENDER_SCALE};
use exact_gpu::GpuTier;

/// A resolved rung of the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rung {
    /// What the game asked for.
    pub asked: Quality,
    /// The device's tier when the renderer was made.
    pub tier: GpuTier,
    /// The rung itself: `Auto` resolved by the tier.
    pub level: GpuTier,
}

impl Rung {
    /// `asked` under `tier`: `Auto` takes the tier's rung.
    pub fn resolve(asked: Quality, tier: GpuTier) -> Self {
        let level = match asked {
            Quality::Auto => tier,
            Quality::Low => GpuTier::Low,
            Quality::Mid => GpuTier::Mid,
            Quality::High => GpuTier::High,
        };
        Self { asked, tier, level }
    }

    /// Scene samples: one under `Low`; the game's own as the ceiling.
    pub fn samples(self, game: SceneSamples) -> SceneSamples {
        if self.level == GpuTier::Low {
            SceneSamples::One
        } else {
            game
        }
    }

    /// The world raster's scale: the rung's when `render_scale` keeps its
    /// default, [`AUTO_RENDER_SCALE`]; any other value the game returns,
    /// unchanged.
    pub fn scale(self, game: f32) -> f32 {
        if game != AUTO_RENDER_SCALE {
            return game;
        }
        match self.level {
            GpuTier::Low => 0.5,
            GpuTier::Mid => 0.75,
            GpuTier::High => 1.0,
        }
    }

    /// Clamp one frame's input. `authored_shadows`: the game inserted a
    /// `Shadows` resource, which is its own and stands.
    pub fn clamp(self, input: &mut crate::FrameInput<'_>, authored_shadows: bool) {
        if self.level == GpuTier::High {
            return;
        }
        if !authored_shadows {
            if let Some(sun) = &mut input.sun {
                match self.level {
                    GpuTier::Low => sun.shadows = None,
                    _ => {
                        if let Some(shadows) = &mut sun.shadows {
                            shadows.cascades = shadows.cascades.min(1);
                        }
                    }
                }
            }
        }
        // An inserted `AmbientOcclusion` is the game's, quality and all. LLP
        // 1139.002 D4's `caustics` and `shafts` frame-uniform flags are cleared
        // here under `Low` once that lane's flags exist.
    }

    fn json(self, samples: SceneSamples, scale: f32) -> String {
        let asked = match self.asked {
            Quality::Auto => "auto",
            Quality::Low => "low",
            Quality::Mid => "mid",
            Quality::High => "high",
        };
        format!(
            r#"{{"asked":"{asked}","tier":"{}","rung":"{}","samples":{},"scale":{scale}}}"#,
            self.tier.keyword(),
            self.level.keyword(),
            samples.count()
        )
    }
}

/// The raster's scale before a binding resolved a rung: the game's, full
/// size by default.
pub(crate) fn unresolved_scale(game: f32) -> f32 {
    if game == AUTO_RENDER_SCALE {
        1.0
    } else {
        game
    }
}

/// A renderer's scene samples: the rung's, under the game's; the game's
/// before a binding resolved the rung.
pub(crate) fn samples(rung: Option<Rung>, game: SceneSamples) -> SceneSamples {
    rung.map_or(game, |rung| rung.samples(game))
}

/// The world state's `quality` row, once a binding resolved the rung.
pub(crate) fn row(rung: Option<Rung>, game: SceneSamples, scale: f32, reply: &mut String) {
    if let Some(rung) = rung {
        reply.push_str(",\"quality\":");
        reply.push_str(&rung.json(rung.samples(game), rung.scale(scale)));
    }
}

#[cfg(test)]
mod tests {
    use super::Rung;
    use exact_game::{Quality, SceneSamples, AUTO_RENDER_SCALE};
    use exact_gpu::GpuTier;

    #[test]
    fn auto_follows_the_tier_and_a_named_rung_overrides_it() {
        assert_eq!(
            Rung::resolve(Quality::Auto, GpuTier::Low).level,
            GpuTier::Low
        );
        assert_eq!(
            Rung::resolve(Quality::Auto, GpuTier::High).level,
            GpuTier::High
        );
        assert_eq!(
            Rung::resolve(Quality::High, GpuTier::Low).level,
            GpuTier::High
        );
        assert_eq!(
            Rung::resolve(Quality::Low, GpuTier::High).level,
            GpuTier::Low
        );
    }

    #[test]
    fn a_rung_clamps_only_what_the_game_left_unsaid() {
        let low = Rung::resolve(Quality::Low, GpuTier::High);
        let mid = Rung::resolve(Quality::Mid, GpuTier::High);
        let high = Rung::resolve(Quality::High, GpuTier::Low);
        assert_eq!(low.samples(SceneSamples::Four), SceneSamples::One);
        assert_eq!(mid.samples(SceneSamples::Four), SceneSamples::Four);
        assert_eq!(
            mid.samples(SceneSamples::One),
            SceneSamples::One,
            "the game's samples are the ceiling"
        );
        assert_eq!(low.scale(AUTO_RENDER_SCALE), 0.5);
        assert_eq!(mid.scale(AUTO_RENDER_SCALE), 0.75);
        assert_eq!(high.scale(AUTO_RENDER_SCALE), 1.0);
        assert_eq!(low.scale(0.8), 0.8, "a game's own render_scale stands");
        assert_eq!(low.scale(1.0), 1.0, "an explicit full size stands too");
    }

    #[test]
    fn a_clamp_leaves_the_games_own_ambient_occlusion_and_shadows() {
        use exact_game::{AmbientOcclusion, AoQuality, Shadows};
        let sun = || crate::Sun {
            shadows: Some(Shadows::default()),
            ..Default::default()
        };
        let low = Rung::resolve(Quality::Low, GpuTier::High);
        let mut input = crate::FrameInput {
            sun: Some(sun()),
            ambient_occlusion: Some(AmbientOcclusion {
                quality: AoQuality::High,
                ..Default::default()
            }),
            ..Default::default()
        };
        low.clamp(&mut input, false);
        assert!(
            input.sun.unwrap().shadows.is_none(),
            "an unsaid sun shadow goes"
        );
        assert_eq!(input.ambient_occlusion.unwrap().quality, AoQuality::High);
        let mut kept = crate::FrameInput {
            sun: Some(sun()),
            ..Default::default()
        };
        low.clamp(&mut kept, true);
        assert!(
            kept.sun.unwrap().shadows.is_some(),
            "an inserted Shadows stands"
        );
        let mut mid = crate::FrameInput {
            sun: Some(sun()),
            ..Default::default()
        };
        Rung::resolve(Quality::Mid, GpuTier::High).clamp(&mut mid, false);
        assert_eq!(mid.sun.unwrap().shadows.unwrap().cascades, 1);
    }
}
