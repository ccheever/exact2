//! Singleton resources use one initialized value, with the same leases and Data
//! wire representation as storage's former slot zero. No component page or masks.
use super::{Erased, Lease, Ref, RefMut};
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
    revision: Cell<u64>,
    present: bool,
}
impl<C: Data> Singleton<C> {
    pub fn new(name: &'static str, epoch: Rc<Cell<u64>>) -> Self {
        Self {
            value: UnsafeCell::new(None),
            borrowed: Cell::new(0),
            epoch,
            name,
            revision: Cell::new(0),
            present: false,
        }
    }
    pub(crate) fn revision(&self) -> u64 {
        self.revision.get()
    }
    fn edited(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    pub fn insert(&mut self, value: C) {
        self.edited();
        *self.value.get_mut() = Some(value);
        self.present = true;
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
        self.edited();
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
    fn edit(&mut self, _: usize, r: &mut dyn Reader) -> Result<(), DataError> {
        self.get_mut()
            .ok_or_else(|| DataError::new("resource absent"))?
            .read(r)
    }
    fn has(&self, index: usize) -> bool {
        index == 0 && self.present
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn len(&self) -> usize {
        usize::from(self.present)
    }
    fn remove(&mut self, index: usize) {
        if index == 0 {
            self.edited();
            self.present = false;
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
        r.required_item("resource must contain one value")?;
        r.begin_seq()?;
        r.required_item("missing entity")?;
        let mut e = Entity::default();
        e.read(r)?;
        if e.index() != 0 || !valid(e) {
            return Err(DataError::new("stale or invalid entity"));
        }
        r.required_item("missing component")?;
        let value = C::read_new(r)?;
        if r.item()? {
            return Err(DataError::new("extra component entry value"));
        }
        if r.item()? {
            return Err(DataError::new("resource must contain one value"));
        }
        self.insert(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_reads_do_not_borrow_a_mutably_leased_value() {
        let mut cell = Singleton::<u32>::new("score", Rc::new(Cell::new(0)));
        assert_eq!(cell.len(), 0);
        cell.insert(7);
        let mut value = cell.get_mut().unwrap();
        assert!(cell.has(0));
        assert_eq!(cell.len(), 1);
        *value = 9;
        drop(value);
        cell.remove(0);
        assert!(!cell.has(0));
    }
}
