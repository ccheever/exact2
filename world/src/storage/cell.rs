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
}
impl<C: Data> Singleton<C> {
    pub fn new(name: &'static str, epoch: Rc<Cell<u64>>) -> Self {
        Self {
            value: UnsafeCell::new(None),
            borrowed: Cell::new(0),
            epoch,
            name,
        }
    }
    pub fn insert(&mut self, value: C) {
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
        self.epoch.set(self.epoch.get().wrapping_add(1));
        // SAFETY: the exclusive lease excludes all other references to this cell.
        let value = unsafe { &mut *self.value.get() }.as_mut()?;
        Some(RefMut {
            ptr: value,
            _lease: lease,
            _life: PhantomData,
        })
    }
}

pub(crate) fn make_cell<C: Data>(name: &'static str, epoch: Rc<Cell<u64>>) -> Box<dyn Erased> {
    Box::new(Singleton::<C>::new(name, epoch))
}
impl<C: Data> Erased for Singleton<C> {
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
    fn settle_tick(&self, now: crate::Now, _: Option<&Storage<crate::Ambient>>) -> Option<u64> {
        self.get().map_or(Some(now.tick), |v| v.settle_tick(now))
    }
}
