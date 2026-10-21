//! Rust argument defaults and atomic timed binding for world surfaces, and
//! the quality ladder's rung the first binding resolves (LLP 1139.003 D3).
use super::{observer, Clock, Executor, Game, Perf, Sim, SurfaceError, Value, WorldSurface};
use exact_game::Args;

impl<G: Game, P: Executor, const ASSETS: bool, H: crate::Hooks> WorldSurface<G, P, ASSETS, H> {
    /// The world raster's scale: the rung's choice, or the game's own.
    pub(super) fn raster_scale(&self) -> f32 {
        let game = self
            .sim
            .as_ref()
            .map_or(exact_game::AUTO_RENDER_SCALE, |sim| {
                G::render_scale(sim.args())
            });
        self.rung
            .map_or(crate::quality::unresolved_scale(game), |rung| {
                rung.scale(game)
            })
    }
    /// The device's tier changed (`prefer gpu-tier`): the rung is resolved
    /// again, and a renderer made at another rung is released, as on a lost
    /// device, to be made again under the same world; a look frame's own
    /// renderer and a readback in flight go with it. The placed children's
    /// textures are the host's on the same device and stay. True when a live
    /// renderer was released.
    pub(super) fn retier_rung(&mut self) -> bool {
        let Some(sim) = &self.sim else { return false };
        let next = crate::quality::Rung::resolve(G::quality(sim.args()), exact_gpu::tier());
        if self.rung == Some(next) {
            return false;
        }
        self.rung = Some(next);
        self.look.retiered();
        if self.render.is_none() {
            return false;
        }
        let children: Vec<_> = self
            .placed
            .children
            .iter()
            .map(|c| c.texture.clone())
            .collect();
        exact_gpu::Surface::device_lost(self);
        for (child, texture) in self.placed.children.iter_mut().zip(children) {
            child.texture = texture;
        }
        true
    }
    pub(super) fn surface_arguments(&self) -> Vec<(&'static str, Value)> {
        G::Args::FIELDS
            .iter()
            .map(|(name, _)| *name)
            .zip(G::Args::default().values())
            .collect()
    }
    pub(super) fn bind_arguments(
        &mut self,
        values: &[Value],
        at_ms: Option<f64>,
    ) -> Result<(), SurfaceError> {
        if at_ms.is_some_and(|at| !at.is_finite()) {
            return Err(SurfaceError("bind clock must be finite".into()));
        }
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        if let Some(sim) = &mut self.sim {
            let generation = sim.generation();
            let models = sim.world().model_revision();
            sim.bind_with(
                values,
                at_ms,
                observer(
                    &mut self.render,
                    &mut self.placed,
                    &mut self.perf,
                    &mut self.trace,
                    &mut self.error,
                    &mut self.hook_poses,
                    false,
                    0,
                ),
            )
            .map_err(SurfaceError)?;
            self.assets_dirty |= models != sim.world().model_revision();
            if generation != sim.generation() {
                self.hook_clock.reset();
                if let Some((_, feed)) = &mut self.render {
                    feed.reset();
                }
                self.perf = Perf::default();
                self.gpu_timing = None;
            }
        } else {
            let mut sim = Sim::from_values(values).map_err(SurfaceError)?;
            sim.development_edits(self.development_edits);
            sim.defer_assets(ASSETS && self.device);
            if let Some(at) = at_ms {
                sim.advance(at, Clock::Seekable);
            }
            // The quality ladder's rung (LLP 1139.003 D3): read once, from the
            // first arguments and the device's tier, before any renderer exists.
            self.rung.get_or_insert_with(|| {
                crate::quality::Rung::resolve(G::quality(sim.args()), exact_gpu::tier())
            });
            self.sim = Some(sim);
        }
        if let Err(error) = self.check_primitive_assets() {
            self.sim = None;
            self.asset_check = None;
            return Err(error);
        }
        self.dirty = true;
        Ok(())
    }
}
