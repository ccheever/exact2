use super::*;
/// Components on this entity still save/hash, but do not keep settle running.
#[derive(Default, crate::Component)]
pub struct Ambient;
/// Module-produced saved work, retained until replaced or explicitly removed.
#[derive(Clone, Debug, Default, Data, PartialEq, Eq)]
pub enum Work {
    #[default]
    Ready,
    Pending,
    Failed(String),
    /// Simulation deadline; external readiness must use Pending instead.
    Deadline(u64),
}
#[derive(Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Pending(Vec<String>),
    Failed(Vec<String>),
}
impl World {
    pub fn mutation_epoch(&self) -> u64 {
        self.epoch.get()
    }
    pub(crate) fn mutated(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
    }
    pub fn now(&self) -> Now {
        Now {
            tick: self.tick(),
            hz: self.hz(),
        }
    }
    pub fn seed(&self) -> u64 {
        self.state.seed
    }
    pub fn reseed(&mut self, seed: u64) {
        self.state.seed = seed;
        self.rng.insert(Rng::new(seed));
    }
    pub fn derived<T: Default + 'static>(&self) -> std::cell::RefMut<'_, T> {
        std::cell::RefMut::map(self.derived.borrow_mut(), |slots| {
            assert!(
                slots.contains_key(&TypeId::of::<T>()) || slots.len() < 64,
                "derived type limit (64)"
            );
            slots
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Box::<T>::default())
                .downcast_mut()
                .unwrap()
        })
    }
    pub fn try_query<Q: Query>(&self) -> Result<QueryBorrow<'_, Q>, String> {
        QueryBorrow::try_new(self)
    }
    pub fn busy(&self, reason: &'static str) -> Result<(), DataError> {
        let mut busy = self.state.busy.borrow_mut();
        if busy.len() == 64 || reason.len() > 256 {
            return Err(DataError::new("busy reason limit"));
        }
        busy.push(reason.into());
        self.mutated();
        Ok(())
    }
    pub fn work(&self, key: &str, value: Work) -> Result<(), DataError> {
        let mut work = self.state.work.borrow_mut();
        if key.len() > 256
            || (!work.contains_key(key) && work.len() == 64)
            || matches!(&value, Work::Failed(s) if s.len() > 256)
        {
            return Err(DataError::new(
                "work reason limit (64 reasons, 256 bytes each)",
            ));
        }
        work.insert(key.into(), value);
        self.mutated();
        Ok(())
    }
    pub fn clear_work(&self, key: &str) {
        if self.state.work.borrow_mut().remove(key).is_some() {
            self.mutated();
        }
    }
    pub fn readiness(&self) -> Readiness {
        let work = self.state.work.borrow();
        let failed: Vec<_> = work
            .iter()
            .filter_map(|(k, v)| {
                if let Work::Failed(s) = v {
                    Some(format!("{k}: {s}"))
                } else {
                    None
                }
            })
            .take(8)
            .collect();
        if !failed.is_empty() {
            return Readiness::Failed(failed);
        }
        let pending: Vec<_> = work
            .iter()
            .filter(|(_, v)| matches!(v, Work::Pending))
            .map(|(k, _)| k.clone())
            .take(8)
            .collect();
        if pending.is_empty() {
            Readiness::Ready
        } else {
            Readiness::Pending(pending)
        }
    }
    pub fn settle_tick(&self) -> Option<u64> {
        if !self.state.busy.borrow().is_empty() || self.readiness() != Readiness::Ready {
            return None;
        }
        let deadline = self
            .state
            .work
            .borrow()
            .values()
            .filter_map(|w| {
                if let Work::Deadline(t) = w {
                    Some(*t)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(self.tick());
        self.components
            .values()
            .map(|s| (s, self.storage::<Ambient>()))
            .chain(
                self.resources
                    .iter()
                    .filter(|(n, _)| !self.registry[*n].ambient)
                    .map(|(_, s)| (s, None)),
            )
            .try_fold(deadline.max(self.tick()), |at, (s, skip)| {
                Some(at.max(s.settle_tick(self.now(), skip)?))
            })
    }
    pub fn quiescent(&self) -> bool {
        self.observed
            .is_some_and(|(epoch, _)| epoch == self.mutation_epoch())
            && self.settle_tick() == Some(self.tick())
    }
    pub(crate) fn observation_hash(&self) -> u64 {
        let mut w = hash::Hasher::default();
        for e in self.entities().filter(|e| !self.has::<Ambient>(*e)) {
            e.write(&mut w);
            self.state.slots[e.index() as usize].name.write(&mut w);
            for (name, s) in &self.components {
                if s.has(e.index() as usize) {
                    w.key(name);
                    s.write_one(e.index() as usize, &mut w);
                }
            }
        }
        self.rng.get().unwrap().write(&mut w);
        for (name, s) in &self.resources {
            if !self.registry[name].ambient {
                w.key(name);
                s.write(&mut w, &|_| SINGLETON);
            }
        }
        self.published.borrow().write(&mut w);
        w.finish()
    }
    pub(crate) fn begin_tick(&mut self) {
        self.mutated();
        self.in_tick = true;
        self.state.busy.get_mut().clear();
    }
    pub(crate) fn observe(&mut self, before: u64) {
        let after = self.observation_hash();
        self.observed = (before == after).then_some((self.mutation_epoch(), after));
    }
    pub fn state(&self, entity: Entity) -> Result<String, DataError> {
        if !self.contains(entity) {
            return Err(DataError::new("stale entity"));
        }
        let mut w = crate::json::Encoder::default();
        w.begin_struct();
        for (name, s) in &self.components {
            if s.has(entity.index() as usize) {
                w.field(name);
                s.write_one(entity.index() as usize, &mut w);
            }
        }
        w.end_struct();
        w.finish()
    }
    /// Entity ordered ownership tree rows, capped at 512, with an explicit omitted count.
    pub fn tree(&self) -> (Vec<(Entity, Option<&str>, Option<Entity>)>, usize) {
        let rows = self
            .entities()
            .take(512)
            .map(|e| (e, self.name(e), self.get::<Parent>(e).map(|p| p.entity())))
            .collect();
        (rows, self.len().saturating_sub(512))
    }
    pub fn take_messages(&self) -> Vec<String> {
        std::mem::take(&mut *self.messages.borrow_mut())
    }
    pub fn publications(&self) -> Result<String, DataError> {
        let mut out = String::from("{");
        for (i, (k, v)) in self.published.borrow().iter().enumerate() {
            if i != 0 {
                out.push(',');
            }
            crate::json::quote_into(&mut out, k);
            out.push(':');
            v.append_json(&mut out, false);
        }
        out.push('}');
        if out.len() > crate::json::LIMIT {
            return Err(DataError::new("publication output limit"));
        }
        Ok(out)
    }
}
