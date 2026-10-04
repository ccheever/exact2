//! Paranoid modes: each sampled boundary is rebuilt through the production
//! restore path (and, for FreshGame, freshly decoded assets), then compared.
use super::*;

impl<G: Game> Sim<G> {
    pub(super) fn paranoid_rebuild(&mut self, mode: Paranoid) {
        // An asset first shown this tick is still in flight: no save can be
        // taken until it lands (a model first requested mid-game), so the
        // sample is owed to the next tick and counted, never dropped silently.
        if self.assets_unready().is_some() {
            self.paranoid_owed = true;
            self.paranoid_skipped += 1;
            return;
        }
        self.paranoid_owed = false;
        let skipped = self.paranoid_skipped;
        let tick = self.world.tick();
        let hash = self.world.hash();
        // advance_with owns the seek horizon, but EXSIM checkpoints describe a
        // completed boundary. Retain the horizon outside the reconstructed Sim.
        let horizon = self.world_us;
        self.world_us = ((tick as u128 * 1_000_000).div_ceil(G::HZ as u128)) as i64;
        let bytes = match self.save() {
            Ok(bytes) => bytes,
            Err(error) => panic!("paranoid {:?} {} tick {tick}: {error}; rerun bun game/games/{}/proof.mjs linux --paranoid", mode, G::ID, G::ID),
        };
        let host = self.last_us;
        let mut queue = std::mem::take(&mut self.queue);
        queue.shrink_to_fit();
        let last_ms = self.last_ms;
        let live_time = self.live_time;
        let period_ms = self.period_ms;
        let lookahead = self.lookahead_us_hz;
        let last_motion = self.last_motion;
        let paused_clock = self.paused_clock;
        let rebase_queue = self.rebase_queue;
        let posts_logged = self.posts_logged;
        let observations = std::mem::take(&mut self.observations);
        // Resetting sprite names must still discover removal of the last texture.
        let assets_current = self.asset_mesh_revision == self.world.revision::<crate::Mesh>()
            && self.asset_sprite_names.is_empty();
        let delay = self.settle_delay.get();
        let pending = self.world.published_pending.get();
        let messages = self.take_messages();
        // These are driver outputs/ownership, not dependencies of Game::tick.
        // Keep them outside the rebuild just like advance_with's callback.
        if mode == Paranoid::FreshGame {
            let mut assets = std::mem::take(&mut self.world.assets);
            let models = assets
                .models
                .iter()
                .map(|(name, model)| (name.clone(), bin::to_vec(model.model.as_ref())))
                .collect::<Vec<_>>();
            assets.models = Default::default();
            // Drop all old component/resource values (including skipped fields
            // and physics executors) before decoding the replacement.
            let generation = self.world.presentation_generation;
            self.world = self.world.registered_scratch();
            self.world.presentation_generation = generation;
            for (name, bytes) in models {
                assets.models.insert(
                    name,
                    bin::from_slice::<crate::asset::Model>(&bytes)
                        .expect("paranoid asset decode")
                        .into(),
                );
            }
            self.world.assets = assets;
        }
        self.restore(&bytes)
            .unwrap_or_else(|error| panic!("paranoid {:?} {} tick {tick}: {error}; rerun bun game/games/{}/proof.mjs linux --paranoid", mode, G::ID, G::ID));
        assert_eq!(
            hash,
            self.world.hash(),
            "paranoid {:?} tick {tick}: world hash; rerun bun game/games/{}/proof.mjs linux --paranoid",
            mode, G::ID
        );
        self.world_us = horizon;
        self.paranoid_skipped = skipped;
        self.last_us = host;
        self.last_ms = last_ms;
        self.live_time = live_time;
        self.period_ms = period_ms;
        self.lookahead_us_hz = lookahead;
        self.last_motion = last_motion;
        self.paused_clock = paused_clock;
        self.rebase_queue = rebase_queue;
        self.posts_logged = posts_logged;
        self.queue = queue;
        self.observations = observations;
        if assets_current {
            self.asset_mesh_revision = self.world.revision::<crate::Mesh>();
        }
        self.settle_delay.set(delay);
        self.last_epoch.set(self.world.mutation_epoch());
        self.world.published_pending.set(pending);
        *self.world.messages.borrow_mut() = messages;
        self.restored = false;
        self.restored_from = None;
    }
}
