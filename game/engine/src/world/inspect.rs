use super::*;

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
    pub(crate) fn publications(&self) -> BTreeMap<String, crate::values::Stored> {
        self.published.borrow().clone()
    }
    pub(crate) fn restore_publications(&mut self, values: BTreeMap<String, crate::values::Stored>) {
        *self.published.borrow_mut() = values;
    }
    pub(crate) fn published_json(&self, rounded: bool) -> String {
        let mut out = String::from("{");
        for (i, (key, value)) in self.published.borrow().iter().enumerate() {
            if i != 0 {
                out.push(',');
            }
            crate::json::quote_into(&mut out, key);
            out.push(':');
            value.append_json(&mut out, rounded);
        }
        out.push('}');
        out
    }
    pub(crate) fn has_component_named(&self, e: Entity, name: &str) -> bool {
        self.components
            .get(name)
            .is_some_and(|s| s.has(e.index() as usize))
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
        let mut w = crate::json::Encoder::default(); // State round-trips; only layout is rounded.
        w.begin_struct();
        for (name, s) in storages {
            if s.has(index) {
                w.field(name);
                s.write_one(index, &mut w);
            }
        }
        w.end_struct();
        w.finish()
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
    scratch: Vec<(usize, u64)>,
    published: Vec<(String, u64)>,
}
impl World {
    pub(crate) fn observe(&self, out: &mut Observation) {
        self.observe_with_hash(out, false);
    }
    pub(crate) fn observe_with_hash(&self, out: &mut Observation, full: bool) {
        let mut full = full.then(crate::hash::Hasher::default);
        if let Some(w) = &mut full {
            w.begin_struct();
            w.field("state");
            self.state.write(w);
            w.field("rng");
        }
        let rng = self.rng.get().unwrap();
        let rng_hash = match &mut full {
            Some(w) => w.with_observation(&*rng),
            None => crate::hash::of(&*rng),
        };
        let published = self.published.borrow();
        out.published.resize_with(published.len(), Default::default);
        for ((key, hash), (name, value)) in out.published.iter_mut().zip(published.iter()) {
            if key != name {
                key.clone_from(name);
            }
            *hash = crate::hash::of(value);
        }
        out.entries.clear();
        let ambient = self.storage::<crate::Ambient>();
        for e in self.entities() {
            if ambient.is_some_and(|s| s.has(e.index() as usize)) {
                continue;
            }
            out.entries.push((0, "exists", e, 0));
        }
        for (e, _) in self.query::<&crate::Parent>().iter() {
            if ambient.is_some_and(|s| s.has(e.index() as usize)) {
                continue;
            }
            if let Some(pose) = self.global(e) {
                out.entries
                    .push((1, "global", e, crate::hash::of(&pose.to_cols_array())));
            }
        }
        if let Some(w) = &mut full {
            w.field("components");
            w.begin_struct();
        }
        for (&name, storage) in &self.components {
            out.scratch.clear();
            if let Some(w) = &mut full {
                w.key(name);
            }
            storage.snapshot(ambient, &mut out.scratch, full.as_mut(), &|i| {
                self.entity_at(i)
            });
            out.entries.extend(out.scratch.iter().map(|&(i, hash)| {
                (
                    2,
                    name,
                    Entity {
                        index: i as u32,
                        generation: self.state.slots[i].generation,
                    },
                    hash,
                )
            }));
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
            out.scratch.clear();
            storage.snapshot(None, &mut out.scratch, None, &|_| SINGLETON);
            out.entries.extend(
                out.scratch
                    .iter()
                    .map(|&(_, hash)| (4, name, SINGLETON, hash)),
            );
        }
    }
    pub(crate) fn compare(&mut self, before: &Observation, after: &Observation) {
        self.observed_epoch = self.mutation_epoch();
        self.changing.clear();
        self.observation = if before.entries == after.entries && before.published == after.published
        {
            ObservationState::Still
        } else {
            ObservationState::Changing
        };
        if before.published != after.published {
            for (key, _) in before.published.iter().chain(&after.published) {
                if self.changing.len() >= 8 {
                    break;
                }
                if before.published.iter().find(|v| &v.0 == key)
                    != after.published.iter().find(|v| &v.0 == key)
                {
                    let reason = format!("published.{key}");
                    if !self.changing.contains(&reason) {
                        self.changing.push(reason);
                    }
                }
            }
        }
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
mod measurements {
    use super::*;
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
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;
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
}
