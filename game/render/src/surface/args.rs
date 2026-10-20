//! Rust argument defaults and atomic timed binding for world surfaces, and
//! the quality ladder's rung the first binding resolves (LLP 1139.003 D3).
use super::{observer, Clock, Executor, Game, Perf, Sim, SurfaceError, Value, WorldSurface};
use exact_game::Args;

impl<G: Game, P: Executor, const ASSETS: bool, H: crate::Hooks> WorldSurface<G, P, ASSETS, H> {
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
