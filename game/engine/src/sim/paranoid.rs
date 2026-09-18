use super::*;

impl<G: Game> Sim<G> {
    pub(super) fn paranoid_rebuild(&mut self) {
        if self.paranoid == Paranoid::Off {
            return;
        }
        let tick = self.world.tick();
        let hash = self.world.hash();
        // advance_with owns the seek horizon, but EXSIM checkpoints describe a
        // completed boundary. Retain the horizon outside the reconstructed Sim.
        let horizon = self.world_us;
        let live_clock = (
            self.live_time,
            self.last_ms,
            self.period_ms,
            self.paused_clock,
            self.lookahead_us_hz,
        );
        self.world_us = ((tick as u128 * 1_000_000).div_ceil(G::HZ as u128)) as i64;
        let bytes = self.save();
        let host = self.last_us;
        let recorder = self.recorder.take();
        let reload = std::mem::take(&mut self.reload);
        let observations = std::mem::take(&mut self.observations);
        let delay = self.settle_delay.get();
        let pending = self.world.published_pending.get();
        let messages = self.take_messages();
        // These are driver outputs/ownership, not dependencies of Game::tick.
        // Keep them outside the rebuild just like advance_with's callback.
        if self.paranoid == Paranoid::FreshGame {
            let mut assets = std::mem::take(&mut self.world.assets);
            let models = assets
                .models
                .iter()
                .map(|(name, model)| (name.clone(), bin::to_vec(model.as_ref())))
                .collect::<Vec<_>>();
            assets.models.clear();
            // Drop all old component/resource values (including skipped fields
            // and physics executors) before invoking setup for the replacement.
            let generation = self.world.presentation_generation;
            self.world = World::new(G::HZ, 0);
            self.world.presentation_generation = generation;
            for (name, bytes) in models {
                assets.models.insert(
                    name,
                    std::sync::Arc::new(bin::from_slice(&bytes).expect("paranoid asset decode")),
                );
            }
            self.world.assets = assets;
        }
        self.restore(&bytes)
            .unwrap_or_else(|error| panic!("paranoid {} tick {tick}: {error}", G::ID));
        if let Some(host) = host {
            self.rebase(host as f64 / 1000.0, false)
                .expect("paranoid clock rebase");
        }
        assert_eq!(
            hash,
            self.world.hash(),
            "paranoid {} tick {tick}: world hash",
            G::ID
        );
        self.world_us = horizon;
        (
            self.live_time,
            self.last_ms,
            self.period_ms,
            self.paused_clock,
            self.lookahead_us_hz,
        ) = live_clock;
        self.recorder = recorder;
        self.reload = reload;
        self.observations = observations;
        self.settle_delay.set(delay);
        self.last_epoch.set(self.world.mutation_epoch());
        self.world.published_pending.set(pending);
        *self.world.messages.borrow_mut() = messages;
        self.restored = false;
        self.restored_from = None;
    }
}
