//! Rust argument defaults and atomic timed binding for world surfaces.
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
            if generation != sim.generation() {
                self.hook_clock.reset();
                if let Some((_, feed)) = &mut self.render {
                    feed.reset();
                }
                self.perf = Perf::default();
            }
        } else {
            let mut sim = Sim::from_values(values).map_err(SurfaceError)?;
            sim.defer_assets(ASSETS && self.device);
            if let Some(at) = at_ms {
                sim.advance(at, Clock::Seekable);
            }
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
