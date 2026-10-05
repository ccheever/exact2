//! Singleton resources use one initialized value, with the same leases and Data
//! wire representation as storage's former slot zero. No component page or masks.
use super::{At, Conflict, Erased, Holds, Lease, Leases, Party, Ref, RefMut, Storage, Via};
use crate::{Data, DataError, Reader, Writer};
use std::{
    any::Any,
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    rc::Rc,
};

pub(crate) struct Singleton<C> {
    value: UnsafeCell<Option<C>>,
    holds: Holds,
    pub(super) epoch: Rc<Cell<u64>>,
    name: &'static str,
    revision: Cell<u64>,
    instance: u64,
}
impl<C: Data> Singleton<C> {
    pub fn new(name: &'static str, epoch: Rc<Cell<u64>>) -> Self {
        Self {
            value: UnsafeCell::new(None),
            holds: Holds::default(),
            epoch,
            name,
            revision: Cell::new(0),
            instance: super::instance(),
        }
    }
    fn edited(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    pub fn instance(&self) -> u64 {
        self.instance
    }
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub fn insert(&mut self, value: C) {
        self.edited();
        *self.value.get_mut() = Some(value);
    }
    fn lease(&self, mutable: bool, at: At) -> Lease<'_> {
        let slot = self.holds.acquire(0, mutable, at).unwrap_or_else(|| {
            let held = self
                .holds
                .conflict(0, mutable)
                .expect("a refusal has a holder");
            let requested = Party {
                mutable,
                via: Via::Resource,
                at,
            };
            let held = Party {
                via: Via::Resource,
                ..held
            };
            panic!(
                "{}",
                Conflict::row(self.name, 0, held, requested).message(None)
            )
        });
        Lease::Hold {
            holds: &self.holds,
            slot,
        }
    }
    pub fn get(&self, at: At) -> Option<Ref<'_, C>> {
        let lease = self.lease(false, at);
        // SAFETY: the shared lease excludes writers; the world owns the cell.
        let value = unsafe { &*self.value.get() }.as_ref()?;
        Some(Ref {
            ptr: value,
            _lease: lease,
            _life: PhantomData,
        })
    }
    pub fn get_mut(&self, at: At) -> Option<RefMut<'_, C>> {
        let lease = self.lease(true, at);
        self.edited();
        // SAFETY: the exclusive lease excludes all other references to this cell.
        let value = unsafe { &mut *self.value.get() }.as_mut()?;
        Some(RefMut {
            ptr: value,
            _lease: lease,
            _life: PhantomData,
        })
    }
    // Engine walks run no author code; they only need the absence of a writer.
    fn value(&self) -> Option<&C> {
        assert!(
            self.holds.unwritten(),
            "resource {} is borrowed exclusively during an engine read",
            self.name
        );
        // SAFETY: no exclusive lease is live, and this borrow ends before author code runs.
        unsafe { &*self.value.get() }.as_ref()
    }
}
pub(crate) fn make_cell<C: Data>(name: &'static str, epoch: Rc<Cell<u64>>) -> Box<dyn Erased> {
    Box::new(Singleton::<C>::new(name, epoch))
}
impl<C: Data> Erased for Singleton<C> {
    fn has(&self, index: usize) -> bool {
        index == 0 && self.value().is_some()
    }
    fn read_conflict(&self, _: &Leases, at: At) -> Option<Conflict> {
        let (_, held) = self.holds.writer()?;
        let requested = Party {
            mutable: false,
            via: Via::Read,
            at,
        };
        let held = Party {
            via: Via::Resource,
            ..held
        };
        Some(Conflict::row(self.name, 0, held, requested))
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn len(&self) -> usize {
        usize::from(self.value().is_some())
    }
    fn remove(&mut self, index: usize) {
        if index == 0 {
            self.edited();
            *self.value.get_mut() = None;
        }
    }
    fn clear(&mut self) {
        self.remove(0);
    }
    fn instance(&self) -> u64 {
        self.instance
    }
    fn revision(&self) -> u64 {
        self.revision.get()
    }
    fn page_count(&self) -> usize {
        1
    }
    fn page_generation(&self, _: usize) -> u64 {
        self.revision.get()
    }
    fn digest_page(&self, page: usize, each: &mut dyn FnMut(usize, u64)) {
        if let Some(value) = self.value().filter(|_| page == 0) {
            each(0, crate::hash::of(value));
        }
    }
    fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool {
        if index == 0 {
            if let Some(value) = self.value() {
                value.write(w);
                return true;
            }
        }
        false
    }
    fn write_save(&self, w: &mut dyn Writer) {
        if let Some(value) = self.value() {
            value.write(w);
        }
    }
    fn read_save(&mut self, r: &mut dyn Reader, _: &dyn Fn(u32) -> bool) -> Result<(), DataError> {
        let mut value = C::default();
        value.read(r)?;
        self.insert(value);
        Ok(())
    }
    fn moving(&self, now: crate::Now, _: Option<&Storage<crate::Ambient>>) -> bool {
        self.value().is_some_and(|v| v.moving(now))
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
        self.value().map_or(Some(now.tick), |v| v.settle_tick(now))
    }
}
