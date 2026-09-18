use super::*;
use crate::values::{quote, value_json};

impl World {
    /// Mutation lease epoch, shared by all world storage; excluded from saves/hashes.
    pub fn mutation_epoch(&self) -> u64 {
        self.epoch.get()
    }
    pub(crate) fn mutated(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
    }
    pub(crate) fn observed(&self) -> bool {
        self.observed_epoch == self.mutation_epoch()
    }

    /// Current simulation instant, suitable for sampling or retargeting springs.
    pub fn now(&self) -> Now {
        Now {
            tick: self.tick(),
            hz: self.hz(),
        }
    }
    /// Initial random seed, as last set by reseed.
    pub fn seed(&self) -> u64 {
        self.state.seed
    }
    /// Restart the world's random stream from an explicit game argument.
    pub fn reseed(&mut self, seed: u64) {
        self.state.seed = seed;
        self.rng.insert(Rng::new(seed));
    }
    /// Keep clock settle running. Reasons expire at the start of the next tick.
    pub fn busy(&self, reason: &'static str) {
        self.mutated();
        self.state.busy.borrow_mut().push(reason.into());
    }
    /// Whether the observed tick changed no countable component, springs rest and no work was reported.
    pub fn quiescent(&self) -> bool {
        self.observed()
            && self.observation == ObservationState::Still
            && self.state.busy.borrow().is_empty()
            && !self
                .components
                .values()
                .map(|s| (s, self.storage::<crate::Ambient>()))
                .chain(
                    self.resources
                        .iter()
                        .filter(|(name, _)| !self.registry[*name].ambient)
                        .map(|(_, s)| (s, None)),
                )
                .any(|(s, skip)| s.moving(self.now(), skip))
    }
    pub(crate) fn unobserve(&mut self) {
        self.observation = ObservationState::Unknown;
        self.changing.clear();
    }
    pub(crate) fn changing(&self) -> Vec<String> {
        let mut reasons = if self.observed() {
            self.changing.clone()
        } else {
            Vec::new()
        };
        for (&name, storage) in &self.components {
            if reasons.len() == 8 {
                break;
            }
            storage.visit_moving(self.now(), self.storage::<crate::Ambient>(), &mut |index| {
                let e = self.entity_at(index);
                let reason = format!(
                    "{}.{}",
                    self.name(e)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("#{}", e.index())),
                    name
                );
                if !reasons.contains(&reason) {
                    reasons.push(reason);
                }
                reasons.len() < 8
            });
        }
        for (&name, storage) in &self.resources {
            if reasons.len() == 8 {
                break;
            }
            if !self.registry[name].ambient && storage.moving(self.now(), None) {
                let reason = format!("resource.{name}");
                if !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
        }
        for reason in self.state.busy.borrow().iter() {
            if reasons.len() == 8 {
                break;
            }
            if !reasons.iter().any(|s| s == reason) {
                reasons.push(reason.to_string());
            }
        }
        if reasons.is_empty() && (!self.observed() || self.observation == ObservationState::Unknown)
        {
            reasons.push("not observed".into());
        }
        reasons
    }
    pub(crate) fn settle_tick(&self) -> Option<u64> {
        if !self.state.busy.borrow().is_empty() {
            return None;
        }
        self.components
            .values()
            .map(|s| (s, self.storage::<crate::Ambient>()))
            .chain(
                self.resources
                    .iter()
                    .filter(|(name, _)| !self.registry[*name].ambient)
                    .map(|(_, s)| (s, None)),
            )
            .try_fold(self.tick(), |at, (s, skip)| {
                Some(at.max(s.settle_tick(self.now(), skip)?))
            })
    }
    pub(crate) fn begin_tick(&mut self) {
        self.mutated();
        self.in_tick = true;
        self.state.busy.get_mut().clear();
        self.fresh.clear();
    }
    /// The next journal cursor. Draining a host never erases an agent's history.
    pub fn journal_next(&self) -> u64 {
        self.journal_next.get()
    }
    pub(crate) fn restore_journal(&mut self, lines: Vec<Event>, next: u64) {
        *self.journal.borrow_mut() = lines.into();
        self.journal_next.set(next);
    }
    pub(crate) fn publications(&self) -> BTreeMap<String, Value> {
        self.published.borrow().clone()
    }
    pub(crate) fn restore_publications(&mut self, values: BTreeMap<String, Value>) {
        *self.published.borrow_mut() = values;
    }
    pub(crate) fn published_json(&self, rounded: bool) -> String {
        format!(
            "{{{}}}",
            self.published
                .borrow()
                .iter()
                .map(|(k, v)| format!("{}:{}", quote(k), value_json(v, rounded)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    pub(crate) fn component_names(&self, e: Entity) -> Vec<&str> {
        self.components
            .iter()
            .filter_map(|(name, s)| s.has(e.index() as usize).then_some(*name))
            .collect()
    }
    pub(crate) fn components_json(&self, e: Entity) -> Result<String, DataError> {
        self.storages_json(&self.components, e.index() as usize)
    }
    pub(crate) fn resources_json(&self) -> Result<String, DataError> {
        self.storages_json(&self.resources, 0)
    }
    fn storages_json(
        &self,
        storages: &BTreeMap<&str, Box<dyn Erased>>,
        index: usize,
    ) -> Result<String, DataError> {
        let mut fields = Vec::new();
        for (name, s) in storages {
            let mut w = crate::json::Encoder::default(); // State must round-trip; only layout is rounded.
            if s.write_one(index, &mut w) {
                fields.push(format!("{}:{}", quote(name), w.finish()?));
            }
        }
        Ok(format!("{{{}}}", fields.join(",")))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservationState {
    Unknown,
    Still,
    Changing,
}

/// Observations ordered by category, storage name, then entity index.
#[derive(Default)]
pub(crate) struct Observation {
    entries: Vec<(u8, &'static str, Entity, u64)>,
    #[cfg(test)]
    scratch: Vec<(usize, u64)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PageKey {
    entities: u64,
    parents: u64,
    globals: u64,
    mask: [u64; storage::PAGE / 64],
}
#[derive(Default)]
struct ObservedPage {
    key: Option<PageKey>,
    entries: Vec<(Entity, u64)>,
}
#[derive(Default)]
pub(super) struct Cache {
    identity: Option<(WorldId, u64)>,
    exists: Vec<ObservedPage>,
    globals: Vec<ObservedPage>,
}
impl World {
    pub(super) fn mark_entity_page(&mut self, index: usize) {
        let page = index / storage::PAGE;
        if page >= self.entity_pages.len() {
            self.entity_pages.resize(page + 1, 0);
        }
        self.entity_pages[page] = self.entities_revision;
    }
    fn prepare_observation(&self) {
        let mut cache = self.observation_cache.borrow_mut();
        if cache.identity.as_ref().is_some_and(|(id, generation)| {
            *id == self.id && *generation == self.presentation_generation
        }) {
            return;
        }
        *cache = Cache {
            identity: Some((self.id(), self.presentation_generation)),
            ..Cache::default()
        };
        self.rng.reset_observation();
        for storage in self.components.values().chain(self.resources.values()) {
            storage.reset_observation();
        }
    }
    fn observe_structure(&self, out: &mut Observation) {
        const WORDS: usize = storage::PAGE / 64;
        let ambient = self.storage::<crate::Ambient>();
        let parents = self.storage::<crate::Parent>();
        let mut cache = self.observation_cache.borrow_mut();
        for kind in 0..2 {
            let (pages, count, name) = if kind == 0 {
                (
                    &mut cache.exists,
                    self.alive_mask.len().div_ceil(WORDS),
                    "exists",
                )
            } else {
                (
                    &mut cache.globals,
                    parents.map_or(0, Storage::page_count),
                    "global",
                )
            };
            pages.resize_with(count, ObservedPage::default);
            for (page, cached) in pages.iter_mut().enumerate() {
                let mut mask = if kind == 0 {
                    std::array::from_fn(|word| {
                        self.alive_mask
                            .get(page * WORDS + word)
                            .copied()
                            .unwrap_or(0)
                    })
                } else {
                    parents.unwrap().page_mask(page)
                };
                if let Some(ambient) = ambient {
                    for (bits, skip) in mask.iter_mut().zip(ambient.page_mask(page)) {
                        *bits &= !skip;
                    }
                }
                let key = PageKey {
                    entities: self.entity_pages.get(page).copied().unwrap_or(0),
                    parents: if kind == 0 {
                        0
                    } else {
                        parents.unwrap().page_generation(page)
                    },
                    globals: if kind == 0 {
                        0
                    } else {
                        self.hierarchy.page_generation(page)
                    },
                    mask,
                };
                if cached.key != Some(key) {
                    cached.key = None;
                    cached.entries.clear();
                    for (word, &bits) in mask.iter().enumerate() {
                        let mut bits = bits;
                        while bits != 0 {
                            let index =
                                page * storage::PAGE + word * 64 + bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            let entity = self.entity_at(index);
                            let hash = if kind == 0 {
                                Some(0)
                            } else {
                                self.global(entity)
                                    .map(|pose| crate::hash::of(&pose.to_cols_array()))
                            };
                            if let Some(hash) = hash {
                                cached.entries.push((entity, hash));
                            }
                        }
                    }
                    if cached.entries.is_empty() {
                        cached.entries = Vec::new();
                    }
                    cached.key = Some(key);
                }
                out.entries.extend(
                    cached
                        .entries
                        .iter()
                        .map(|&(e, hash)| (kind, name, e, hash)),
                );
            }
            while pages.last().is_some_and(|page| page.entries.is_empty()) {
                pages.pop();
            }
            pages.shrink_to_fit();
        }
    }
    pub(crate) fn observe(&self, out: &mut Observation) {
        self.observe_with_hash(out, false);
    }
    pub(crate) fn observe_with_hash(&self, out: &mut Observation, full: bool) {
        // Bound both the sparse mask walk and flat output before allocating or
        // hashing. Custom Data writers remain trusted, as in save/hash.
        assert!(
            self.state.slots.len() <= 1_000_000,
            "observation exceeds 1000000 entity slots"
        );
        assert!(
            self.components.len() + self.resources.len() <= 256,
            "observation exceeds 256 storage types"
        );
        let visits = self.state.slots.len() * 2
            + self
                .components
                .values()
                .chain(self.resources.values())
                .map(|s| s.len())
                .sum::<usize>();
        assert!(
            visits <= 16_000_000,
            "observation exceeds 16000000 entry visits"
        );
        self.prepare_observation();
        let mut full = full.then(crate::hash::Hasher::default);
        if let Some(w) = &mut full {
            w.begin_struct();
            w.field("state");
            self.state.write(w);
            w.field("rng");
        }
        let rng_hash = self.rng.observation_hash(full.as_mut()).unwrap();
        out.entries.clear();
        let ambient = self.storage::<crate::Ambient>();
        self.observe_structure(out);
        if let Some(w) = &mut full {
            w.field("components");
            w.begin_struct();
        }
        for (&name, storage) in &self.components {
            if let Some(w) = &mut full {
                w.key(name);
            }
            storage.snapshot(
                ambient,
                &mut out.entries,
                full.as_mut(),
                &|i| self.entity_at(i),
                (2, name),
            );
        }
        if let Some(mut w) = full.take() {
            w.end_struct();
            // Leave resources lazy: observing rest must never snapshot ambient executors.
            *self.hash_prefix.borrow_mut() = Some((self.mutation_epoch(), w));
        }
        // Keep the dedicated RNG in a separate observation category so names remain sorted.
        out.entries.push((3, "Rng", SINGLETON, rng_hash));
        for (&name, storage) in &self.resources {
            if self.registry[name].ambient {
                continue;
            }
            storage.snapshot(None, &mut out.entries, None, &|_| SINGLETON, (4, name));
        }
    }
    pub(crate) fn compare(&mut self, before: &Observation, after: &Observation) {
        self.observed_epoch = self.mutation_epoch();
        self.changing.clear();
        if before.entries == after.entries {
            self.observation = ObservationState::Still;
            return;
        }
        self.observation = ObservationState::Changing;
        let (mut a, mut b) = (0, 0);
        while (a < before.entries.len() || b < after.entries.len()) && self.changing.len() < 8 {
            let old = before.entries.get(a);
            let new = after.entries.get(b);
            if old == new {
                a += 1;
                b += 1;
                continue;
            }
            let key = |v: &(u8, &'static str, Entity, u64)| (v.0, v.1, v.2);
            let entry = match (old, new) {
                (Some(old), Some(new)) => match key(old).cmp(&key(new)) {
                    std::cmp::Ordering::Less => {
                        a += 1;
                        old
                    }
                    std::cmp::Ordering::Greater => {
                        b += 1;
                        new
                    }
                    std::cmp::Ordering::Equal => {
                        a += 1;
                        b += 1;
                        new
                    }
                },
                (Some(old), None) => {
                    a += 1;
                    old
                }
                (None, Some(new)) => {
                    b += 1;
                    new
                }
                _ => break,
            };
            let name = self
                .name(entry.2)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("#{}", entry.2.index()));
            self.changing.push(format!(
                "{}.{}",
                if entry.0 >= 3 { "resource" } else { &name },
                entry.1
            ));
        }
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod observation_tests;

#[cfg(test)]
mod measurements {
    use super::*;
    #[test]
    fn an_unwritten_mutable_lease_invalidates_then_observes_still() {
        for full in [false, true] {
            let mut w = World::new(60, 0);
            let e = w.spawn_named("player", crate::Transform::default());
            let mut before = Observation::default();
            let mut after = Observation::default();
            w.observe_with_hash(&mut before, full);
            w.compare(&before, &before);
            assert!(w.quiescent());
            let hash = w.hash();
            let bytes = w.save();
            let epoch = w.mutation_epoch();
            let generation = w
                .pages::<crate::Transform>()
                .iter()
                .next()
                .unwrap()
                .generation;
            drop(w.get_mut::<crate::Transform>(e).unwrap());
            assert_ne!(w.mutation_epoch(), epoch);
            assert_ne!(
                w.pages::<crate::Transform>()
                    .iter()
                    .next()
                    .unwrap()
                    .generation,
                generation
            );
            assert!(!w.quiescent());
            assert_eq!(w.changing(), ["not observed"]);
            w.observe_with_hash(&mut after, full);
            w.compare(&before, &after);
            assert!(w.quiescent());
            assert!(w.changing().is_empty());
            assert_eq!(w.hash(), hash);
            assert_eq!(w.save(), bytes);
        }
    }

    #[test]
    fn recycling_a_slot_observes_the_new_incarnation() {
        for full in [false, true] {
            for component in [false, true] {
                let mut w = World::new(60, 0);
                let old = w.spawn_named("player", ());
                if component {
                    w.insert(old, crate::Transform::default());
                }
                let mut before = Observation::default();
                let mut after = Observation::default();
                w.observe_with_hash(&mut before, full);
                let hash = w.hash();
                assert!(w.despawn(old));
                let new = w.spawn_named("player", ());
                if component {
                    w.insert(new, crate::Transform::default());
                }
                assert_eq!(old.index(), new.index());
                assert_ne!(old.generation(), new.generation());
                w.observe_with_hash(&mut after, full);
                w.compare(&before, &after);
                assert!(!w.quiescent());
                // A removed and an added incarnation each supply a reason, in
                // the existing category/name/entity order (without deduping).
                let mut expected = vec!["#0.exists", "player.exists"];
                if component {
                    expected.extend(["#0.Transform", "player.Transform"]);
                }
                assert_eq!(w.changing(), expected);
                assert_ne!(w.hash(), hash);
                assert_eq!(w.hash(), {
                    let mut canonical = crate::hash::Hasher::default();
                    w.write(&mut canonical, false);
                    canonical.finish()
                });
                w.compare(&after, &after);
                assert!(w.quiescent());
            }
        }
    }

    #[test]
    fn name_index_is_not_saved_hashed_or_observed() {
        let mut w = World::new(60, 0);
        w.spawn_named("name", crate::Transform::default());
        let bytes = w.save();
        let hash = w.hash();
        let mut before = Observation::default();
        w.observe(&mut before);
        w.names.clear();
        w.mutated(); // Force hash recomputation instead of consulting its cache.
        let mut after = Observation::default();
        w.observe(&mut after);
        assert_eq!(before.entries, after.entries);
        assert_eq!(w.save(), bytes);
        assert_eq!(w.hash(), hash);
        w.load(&bytes).unwrap();
        assert!(w.named("name").is_some());
    }

    #[test]
    #[ignore = "release name lookup cost diagnostic"]
    fn named_cost() {
        use std::{hint::black_box, time::Instant};
        for count in [1_000, 10_000, 200_000] {
            let mut w = World::new(60, 0);
            for i in 0..count {
                w.spawn_named(format!("entity-{i:06}"), ());
            }
            let name = format!("entity-{:06}", count - 1);
            let reps = 1_000;
            let start = Instant::now();
            for _ in 0..reps {
                let name = black_box(name.as_str());
                black_box(w.entities().find(|&e| w.name(e) == Some(name)));
            }
            let before = start.elapsed().as_nanos() as f64 / reps as f64;
            let start = Instant::now();
            for _ in 0..100_000 {
                black_box(w.named(black_box(&name)));
            }
            let after = start.elapsed().as_nanos() as f64 / 100_000.0;
            println!("named {count}: scan={before:.1} ns lookup={after:.1} ns");
        }
    }

    #[test]
    #[ignore = "release settle cost diagnostic"]
    fn settle_200k_cost() {
        struct Large;
        impl crate::Game for Large {
            type Args = ();
            const ID: &'static str = "settle-cost";
            fn setup(w: &mut World, _: &()) {
                for i in 0..200_000 {
                    w.spawn(crate::Transform::at(i as f32, 0.0, 0.0));
                }
            }
            fn tick(_: &mut World, _: &crate::Input, _: &()) {}
        }
        let mut s = crate::Sim::<Large>::new(()).unwrap();
        let start = std::time::Instant::now();
        assert!(s.settle());
        println!(
            "settle 200000 initial: {:.6} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        let entity = s.world().entities().next().unwrap();
        let mut samples = Vec::new();
        for sample in 0..105 {
            // Force Unknown without changing a value. Repeated settle() on an
            // already Still world would only measure the early return.
            drop(s.world().get_mut::<crate::Transform>(entity).unwrap());
            let start = std::time::Instant::now();
            assert!(std::hint::black_box(&mut s).settle());
            if sample >= 5 {
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "settle 200000 after unwritten lease: {:.6} ms median, {:.6} ms p95",
            samples[50], samples[95]
        );
    }

    #[test]
    #[ignore = "release stillness cost diagnostic"]
    fn stillness_hash_cost() {
        for count in [1_000, 10_000, 200_000] {
            let mut w = World::new(60, 0);
            for i in 0..count {
                w.spawn(crate::Transform::at(i as f32, 0.0, 0.0));
            }
            let mut snapshot = Observation::default();
            w.observe(&mut snapshot);
            let mut samples = Vec::new();
            for _ in 0..100 {
                let start = std::time::Instant::now();
                std::hint::black_box(&w).observe(std::hint::black_box(&mut snapshot));
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "stillness hash {count} Transform entities: {:.6} ms median, {:.6} ms p95",
                samples[50], samples[95]
            );
        }
        // Spread 100 movers across 100 pages as well as clustering them in one.
        // Advance the tick even in the still case: otherwise World::hash merely
        // reads its whole-world epoch cache and hides the seekable-tick cost.
        for (label, movers, stride) in [
            ("still", 0, 1),
            ("clustered", 100, 1),
            ("spread", 100, 2000),
            ("every-word", 200_000_usize.div_ceil(64), 64),
            ("all-slots", 200_000, 1),
        ] {
            for operation in ["observe", "hash", "observe+hash", "seek-pair"] {
                let mut w = World::new(60, 0);
                for i in 0..200_000 {
                    w.spawn(crate::Transform::at(i as f32, 0.0, 0.0));
                }
                let moving: Vec<_> = (0..movers).map(|i| w.entity_at(i * stride)).collect();
                let mut before = Observation::default();
                let mut after = Observation::default();
                let mut samples = Vec::new();
                for sample in 0..105 {
                    w.begin_tick();
                    w.state.tick += 1;
                    for &e in &moving {
                        w.get_mut::<crate::Transform>(e).unwrap().position.x += 1.0;
                    }
                    let start = std::time::Instant::now();
                    match operation {
                        "observe" => w.observe(&mut after),
                        "hash" => {
                            std::hint::black_box(w.hash());
                        }
                        "observe+hash" => {
                            w.observe_with_hash(&mut after, true);
                            std::hint::black_box(w.hash());
                        }
                        _ => {
                            w.observe(&mut before);
                            // Include one intervening tick's 100 writes, as in a
                            // seek's final two samples, and the reason comparison.
                            w.begin_tick();
                            w.state.tick += 1;
                            for &e in &moving {
                                w.get_mut::<crate::Transform>(e).unwrap().position.x += 1.0;
                            }
                            w.observe_with_hash(&mut after, true);
                            w.compare(&before, &after);
                            std::hint::black_box(w.hash());
                        }
                    }
                    std::hint::black_box(&after);
                    if sample >= 5 {
                        samples.push(start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                samples.sort_by(f64::total_cmp);
                println!(
                    "{label} {movers}/200000 movers {operation}: {:.6} ms median, {:.6} ms p95",
                    samples[50], samples[95]
                );
            }
        }
    }
}
