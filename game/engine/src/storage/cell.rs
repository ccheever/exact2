//! Singleton resources use one initialized value, with the same leases and Data
//! wire representation as storage's former slot zero. No component page or masks.
use super::{Erased, Lease, Ref, RefMut, Storage};
use crate::{Data, DataError, Entity, Reader, Writer};
use std::{
    any::Any,
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    rc::Rc,
};

pub(crate) struct Singleton<C> {
    value: UnsafeCell<Option<C>>,
    borrowed: Cell<isize>,
    pub(super) epoch: Rc<Cell<u64>>,
    name: &'static str,
    observation: Cell<Option<u64>>,
}
impl<C: Data> Singleton<C> {
    pub fn new(name: &'static str, epoch: Rc<Cell<u64>>) -> Self {
        Self {
            value: UnsafeCell::new(None),
            borrowed: Cell::new(0),
            epoch,
            name,
            observation: Cell::new(None),
        }
    }
    pub fn insert(&mut self, value: C) {
        self.observation.set(None);
        self.epoch.set(self.epoch.get().wrapping_add(1));
        *self.value.get_mut() = Some(value);
    }
    pub fn get(&self) -> Option<Ref<'_, C>> {
        let lease = Lease::new(self.name, &self.borrowed, false);
        // SAFETY: the shared lease excludes writers; the world owns the cell.
        let value = unsafe { &*self.value.get() }.as_ref()?;
        Some(Ref {
            ptr: value,
            _lease: lease,
            _life: PhantomData,
        })
    }
    pub fn get_mut(&self) -> Option<RefMut<'_, C>> {
        let lease = Lease::new(self.name, &self.borrowed, true);
        self.observation.set(None);
        self.epoch.set(self.epoch.get().wrapping_add(1));
        // SAFETY: the exclusive lease excludes all other references to this cell.
        let value = unsafe { &mut *self.value.get() }.as_mut()?;
        Some(RefMut {
            ptr: value,
            _lease: lease,
            _life: PhantomData,
        })
    }
    pub(crate) fn observation_hash(&self, full: Option<&mut crate::hash::Hasher>) -> Option<u64> {
        let value = self.get()?; // Cache hits still enforce the shared lease.
        let hash = match (self.observation.get(), full) {
            (Some(hash), Some(w)) => {
                value.write(w);
                hash
            }
            (Some(hash), None) => hash,
            (None, Some(w)) => w.with_observation(&*value),
            (None, None) => crate::hash::of(&*value),
        };
        self.observation.set(Some(hash));
        Some(hash)
    }
}
pub(crate) fn make_cell<C: Data>(name: &'static str, epoch: Rc<Cell<u64>>) -> Box<dyn Erased> {
    Box::new(Singleton::<C>::new(name, epoch))
}
impl<C: Data> Erased for Singleton<C> {
    fn patch(&self, index: usize, r: &mut dyn Reader) -> Result<(), DataError> {
        if index != 0 {
            return Err(DataError::new("reload resource index differs"));
        }
        self.get_mut()
            .ok_or_else(|| DataError::new("reload resource disappeared"))?
            .read(r)
    }
    fn has(&self, index: usize) -> bool {
        index == 0 && self.get().is_some()
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn len(&self) -> usize {
        usize::from(self.get().is_some())
    }
    fn remove(&mut self, index: usize) {
        if index == 0 {
            self.observation.set(None);
            self.epoch.set(self.epoch.get().wrapping_add(1));
            *self.value.get_mut() = None;
        }
    }
    fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool {
        if index == 0 {
            if let Some(value) = self.get() {
                value.write(w);
                return true;
            }
        }
        false
    }
    fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity) {
        let value = self.get();
        w.begin_seq(usize::from(value.is_some()));
        if let Some(value) = value {
            w.item();
            w.begin_seq(2);
            w.item();
            entity(0).write(w);
            w.item();
            value.write(w);
            w.end_seq();
        }
        w.end_seq();
    }
    fn read(
        &mut self,
        r: &mut dyn Reader,
        valid: &dyn Fn(Entity) -> bool,
    ) -> Result<(), DataError> {
        r.begin_seq()?;
        if !r.item()? {
            return Err(DataError::new("resource must contain one value"));
        }
        r.begin_seq()?;
        if !r.item()? {
            return Err(DataError::new("missing entity"));
        }
        let mut e = Entity::default();
        e.read(r)?;
        if e.index() != 0 || !valid(e) {
            return Err(DataError::new("stale or invalid entity"));
        }
        if !r.item()? {
            return Err(DataError::new("missing component"));
        }
        let mut value = C::default();
        value.read(r)?;
        if r.item()? {
            return Err(DataError::new("extra component entry value"));
        }
        if r.item()? {
            return Err(DataError::new("resource must contain one value"));
        }
        self.insert(value);
        Ok(())
    }
    #[cfg(test)]
    fn snapshot_uncached(
        &self,
        _: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
    ) {
        let value = self.get();
        if let Some(w) = full {
            w.begin_seq(usize::from(value.is_some()));
            if let Some(value) = value {
                w.item();
                w.begin_seq(2);
                w.item();
                entity(0).write(w);
                w.item();
                out.push((0, w.with_observation(&*value)));
                w.end_seq();
            }
            w.end_seq();
        } else if let Some(value) = value {
            out.push((0, crate::hash::of(&*value)));
        }
    }
    fn reset_observation(&self) {
        self.observation.set(None);
    }
    fn snapshot(
        &self,
        _: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(u8, &'static str, Entity, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
        label: (u8, &'static str),
    ) {
        if let Some(w) = full {
            w.begin_seq(self.len());
            if self.has(0) {
                w.item();
                w.begin_seq(2);
                w.item();
                entity(0).write(w);
                w.item();
                out.push((
                    label.0,
                    label.1,
                    entity(0),
                    self.observation_hash(Some(w)).unwrap(),
                ));
                w.end_seq();
            }
            w.end_seq();
        } else if let Some(hash) = self.observation_hash(None) {
            out.push((label.0, label.1, entity(0), hash));
        }
    }
    fn moving(&self, now: crate::Now, _: Option<&Storage<crate::Ambient>>) -> bool {
        self.get().is_some_and(|v| v.moving(now))
    }
    fn visit_moving(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
        visit: &mut dyn FnMut(usize) -> bool,
    ) {
        if self.moving(now, skip) {
            visit(0);
        }
    }
    fn settle_tick(&self, now: crate::Now, _: Option<&Storage<crate::Ambient>>) -> Option<u64> {
        self.get().map_or(Some(now.tick), |v| v.settle_tick(now))
    }
}
