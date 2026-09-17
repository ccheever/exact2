use super::*;
use crate::values::{quote, value_json};

impl World {
    /// Current canvas arguments, resolved by their declared names.
    pub fn args(&self) -> &crate::Args {
        &self.args
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
        self.rng.insert(0, Rng::new(seed), self.tick());
    }
    /// Keep clock settle running. Reasons expire at the start of the next tick.
    pub fn busy(&mut self, reason: &'static str) {
        self.state.busy.push(reason.into());
    }
    /// Whether all component/resource springs rest and the game reported no work.
    pub fn quiescent(&self) -> bool {
        self.state.busy.is_empty()
            && !self
                .components
                .values()
                .chain(self.resources.values())
                .any(|s| s.moving(self.now()))
    }
    pub(crate) fn settle_tick(&self) -> Option<u64> {
        if !self.state.busy.is_empty() {
            return None;
        }
        self.components
            .values()
            .chain(self.resources.values())
            .try_fold(self.tick(), |at, s| {
                Some(at.max(s.settle_tick(self.now())?))
            })
    }
    pub(crate) fn begin_tick(&mut self) {
        self.state.busy.clear();
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
            let mut w = crate::json::Encoder::rounded();
            if s.write_one(index, &mut w) {
                fields.push(format!("{}:{}", quote(name), w.finish()?));
            }
        }
        Ok(format!("{{{}}}", fields.join(",")))
    }
}
