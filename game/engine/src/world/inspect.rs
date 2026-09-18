use super::*;
use crate::values::{quote, value_json};

impl World {
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
        self.rng.insert(0, Rng::new(seed), self.tick());
    }
    /// Keep clock settle running. Reasons expire at the start of the next tick.
    pub fn busy(&self, reason: &'static str) {
        self.state.busy.borrow_mut().push(reason.into());
    }
    /// Whether the observed tick changed no countable component, springs rest and no work was reported.
    pub fn quiescent(&self) -> bool {
        self.observation == ObservationState::Still
            && self.state.busy.borrow().is_empty()
            && !self
                .components
                .values()
                .chain(
                    self.resources
                        .iter()
                        .filter(|(name, _)| !self.registry[*name].ambient)
                        .map(|(_, s)| s),
                )
                .any(|s| s.moving(self.now()))
    }
    pub(crate) fn unobserve(&mut self) {
        self.observation = ObservationState::Unknown;
        self.changing.clear();
    }
    pub(crate) fn changing(&self) -> Vec<String> {
        let mut reasons = self.changing.clone();
        let mut add = |reason: String| {
            if reasons.len() < 8 && !reasons.contains(&reason) {
                reasons.push(reason);
            }
        };
        for (&name, storage) in &self.components {
            for index in storage.moving_indices(self.now()) {
                let e = self.entity_at(index);
                add(format!(
                    "{}.{}",
                    self.name(e)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("#{}", e.index())),
                    name
                ));
            }
        }
        for (&name, storage) in &self.resources {
            if !self.registry[name].ambient && storage.moving(self.now()) {
                add(format!("resource.{name}"));
            }
        }
        for reason in self.state.busy.borrow().iter() {
            add(reason.to_string());
        }
        if reasons.is_empty() && self.observation == ObservationState::Unknown {
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
            .chain(
                self.resources
                    .iter()
                    .filter(|(name, _)| !self.registry[*name].ambient)
                    .map(|(_, s)| s),
            )
            .try_fold(self.tick(), |at, s| {
                Some(at.max(s.settle_tick(self.now())?))
            })
    }
    pub(crate) fn begin_tick(&mut self) {
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
    scratch: Vec<(usize, u64)>,
}
impl World {
    pub(crate) fn observe(&self, out: &mut Observation) {
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
        for (&name, storage) in &self.components {
            out.scratch.clear();
            storage.snapshot(ambient, &mut out.scratch);
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
        for (&name, storage) in &self.resources {
            if self.registry[name].ambient {
                continue;
            }
            out.scratch.clear();
            storage.snapshot(None, &mut out.scratch);
            out.entries.extend(
                out.scratch
                    .iter()
                    .map(|&(_, hash)| (3, name, SINGLETON, hash)),
            );
        }
    }
    pub(crate) fn compare(&mut self, before: &Observation, after: &Observation) {
        self.changing.clear();
        self.observation = if before.entries == after.entries {
            ObservationState::Still
        } else {
            ObservationState::Changing
        };
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
                if entry.0 == 3 { "resource" } else { &name },
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
