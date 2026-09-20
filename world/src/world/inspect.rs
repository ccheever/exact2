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
#[derive(Debug, crate::Data, Default)]
pub struct Sample {
    pub hash: u64,
    pub bytes: u64,
    pub components: u64,
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
    pub fn reseed(&mut self, seed: u64) {
        self.state.seed = seed;
        self.rng.insert(Rng::new(seed));
    }
    pub fn derived<T: Default + 'static>(&self) -> std::cell::RefMut<'_, T> {
        let id = TypeId::of::<T>();
        let slot = self
            .derived
            .iter()
            .find(|s| s.get().is_some_and(|(t, _)| *t == id))
            .or_else(|| self.derived.iter().find(|s| s.get().is_none()))
            .expect("derived type limit (64)");
        let (_, value) = slot.get_or_init(|| (id, RefCell::new(Box::<T>::default())));
        std::cell::RefMut::map(value.borrow_mut(), |v| v.downcast_mut().unwrap())
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
        if work.get(key) == Some(&value) {
            return Ok(());
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
    /// Bounded generic report: observation, eight busy/work reasons, and truncation.
    /// At most 64 entries are inspected; text is bounded at admission to 256 bytes.
    pub fn report(&self, w: &mut dyn Writer) -> Result<(), DataError> {
        self.healthy()?;
        let busy = self.state.busy.borrow();
        let work = self.state.work.borrow();
        w.begin_struct();
        w.field("observation");
        self.observation().write(w);
        w.field("busy");
        w.begin_seq(busy.len().min(8));
        for reason in busy.iter().take(8) {
            if w.stopped() {
                break;
            }
            w.item();
            reason.write(w);
        }
        w.end_seq();
        w.field("work");
        w.begin_struct();
        for (name, value) in work.iter().take(8) {
            if w.stopped() {
                break;
            }
            w.key(name);
            value.write(w);
        }
        w.end_struct();
        w.field("truncated");
        (busy.len() > 8 || work.len() > 8).write(w);
        w.end_struct();
        if w.stopped() {
            Err(DataError::new("report visitor refused"))
        } else {
            Ok(())
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
            .filter(|s| s.len() != 0)
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
    /// None means this boundary is unobserved; false means observed change.
    pub fn observation(&self) -> Option<bool> {
        self.observed
            .filter(|(epoch, _)| *epoch == self.mutation_epoch())
            .map(|(_, stable)| stable)
    }
    pub fn quiescent(&self) -> bool {
        self.observation() == Some(true) && self.settle_tick() == Some(self.tick())
    }
    pub fn sample(&self) -> Result<Sample, DataError> {
        self.healthy()?;
        let mut w = hash::Hasher::bounded(32 * 1024 * 1024);
        let mut components = 0;
        let mut probes = 0;
        let columns: Vec<_> = self
            .components
            .iter()
            .filter(|(_, s)| s.len() != 0)
            .collect();
        for e in self.entities().filter(|e| !self.has::<Ambient>(*e)) {
            if w.stopped() {
                break;
            }
            probes += columns.len() + 1;
            if probes > 1_000_000 {
                return Err(DataError::new("observation probe budget exhausted"));
            }
            e.write(&mut w);
            self.state.slots[e.index() as usize].name.write(&mut w);
            for (name, s) in &columns {
                if s.has(e.index() as usize) {
                    w.key(name);
                    s.write_one(e.index() as usize, &mut w);
                    components += 1;
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
        let (hash, bytes) = w.report()?;
        Ok(Sample {
            hash,
            bytes,
            components,
        })
    }
    pub(crate) fn begin_tick(&mut self) {
        self.mutated();
        self.state.busy.get_mut().clear();
    }
    pub(crate) fn observe(&mut self, before: u64) -> Result<u64, DataError> {
        let after = self.sample()?.hash;
        self.observed = Some((self.mutation_epoch(), before == after));
        Ok(after)
    }
    /// Visit one entity's components, or all resources for None, by saved type name.
    pub fn visit(&self, entity: Option<Entity>, w: &mut dyn Writer) -> Result<(), DataError> {
        self.healthy()?;
        if entity.is_some_and(|e| !self.contains(e)) {
            return Err(DataError::new("stale entity"));
        }
        let (values, index) = match entity {
            Some(e) => (&self.components, e.index() as usize),
            None => (&self.resources, 0),
        };
        w.begin_struct();
        for (name, s) in values {
            if w.stopped() {
                break;
            }
            if s.has(index) {
                w.field(name);
                s.write_one(index, w);
            }
        }
        w.end_struct();
        (!w.stopped())
            .then_some(())
            .ok_or_else(|| DataError::new("visitor refused"))
    }
    pub fn state(&self, entity: Entity) -> Result<String, DataError> {
        let mut w = crate::json::Encoder::default();
        self.visit(Some(entity), &mut w)?;
        w.finish()
    }
    pub fn candidate(&self) -> Result<Candidate, DataError> {
        let mut next = self.decoded(&self.save()?, None, false)?.0;
        *next.published.get_mut() = self.published.borrow().clone();
        next.published_pending.set(self.published_pending.get());
        next.published_cost.set(self.published_cost.get());
        *next.journal.get_mut() = self.journal.borrow().clone();
        next.journal_next.set(self.journal_next());
        Ok(Candidate(
            next,
            self.id(),
            self.mutation_epoch(),
            self.journal_next(),
            self.published_pending.get(),
        ))
    }
    pub fn take_messages(&self) -> Vec<String> {
        self.mutated();
        std::mem::take(&mut *self.messages.borrow_mut())
    }
    pub fn publications(&self) -> std::cell::Ref<'_, BTreeMap<std::rc::Rc<str>, crate::Published>> {
        self.published.borrow()
    }
    pub fn take_published(&self) -> Option<BTreeMap<String, crate::Published>> {
        self.published_pending.replace(false).then(|| {
            self.published
                .borrow()
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect()
        })
    }
}

/// Owns an isolated world; failed erased edits poison only this candidate.
pub struct Candidate(World, WorldId, u64, u64, bool);
impl Candidate {
    pub fn world(&self) -> &World {
        &self.0
    }
    pub fn edit(
        &mut self,
        entity: Option<Entity>,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), DataError> {
        self.0.healthy()?;
        if entity.is_some() && name == Parent::NAME {
            return Err(DataError::new("use set_parent for ownership edits"));
        }
        let result = self.0.mutation(|world| {
            if entity.is_some_and(|e| !world.contains(e)) {
                return Err(DataError::new("stale entity"));
            }
            let (values, index) = match entity {
                Some(e) => (&mut world.components, e.index() as usize),
                None => (&mut world.resources, 0),
            };
            let value = values
                .get_mut(name)
                .ok_or_else(|| DataError::new("storage absent").at(name))?;
            if bytes.len() > crate::data::MAX_LOAD_BYTES {
                return Err(DataError::new("candidate edit byte limit"));
            }
            let mut r = bin::Decoder::for_load(bytes, None);
            value.edit(index, &mut r)?;
            r.finish()
        });
        self.0.poisoned |= result.is_err();
        result
    }
    pub fn commit(mut self, destination: &mut World) -> Result<(), DataError> {
        if destination.id() != self.1
            || destination.mutation_epoch() != self.2
            || destination.journal_next() != self.3
            || destination.published_pending.get() != self.4
        {
            return Err(DataError::new("candidate source boundary changed"));
        }
        self.0.rebuild_owners();
        self.0.validate()?;
        destination.adopt(self.0)
    }
}
