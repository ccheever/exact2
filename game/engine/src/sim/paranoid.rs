//! Paranoid modes: each sampled boundary is rebuilt through the production
//! restore path (and, for FreshGame, freshly decoded assets), then compared.
use super::*;

impl<G: Game> Sim<G> {
    /// Present a boundary of an advance, keeping what `each` derived. Under a
    /// paranoid mode, a fresh present must then write exactly the rows kept:
    /// that proves every derivation read only its keys.
    pub(super) fn present_boundary(&mut self) {
        Self::present_rows(&mut self.world, &self.args, false);
        if self.paranoid.is_none() {
            return;
        }
        let kept = self.world.presentation_rows();
        Self::present(&mut self.world, &self.args);
        let fresh = self.world.presentation_rows();
        let key = |&(name, index, digest, _): &(&'static str, usize, u64, _)| (name, index, digest);
        if kept.iter().map(key).eq(fresh.iter().map(key)) {
            return;
        }
        let (mut a, mut b) = (kept.iter().peekable(), fresh.iter().peekable());
        let differs = loop {
            match (a.peek(), b.peek()) {
                (Some(x), Some(y)) if key(x) == key(y) => {
                    a.next();
                    b.next();
                }
                (Some(x), Some(y)) => break if (x.0, x.1) <= (y.0, y.1) { *x } else { *y },
                (Some(x), None) => break *x,
                (None, Some(y)) => break *y,
                (None, None) => unreachable!("the rows differ"),
            }
        };
        let (name, index, _, at) = differs;
        let by = at.map_or("outside `each`".into(), |at| {
            format!("by the `each` at {at}")
        });
        panic!(
            "paranoid tick {}: presentation `{name}` of #{index} kept {by} differs from a fresh present; a derivation read something other than its entity's keys. Rerun bun game/games/{}/proof.mjs linux --paranoid",
            self.world.tick(),
            G::ID
        );
    }
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
