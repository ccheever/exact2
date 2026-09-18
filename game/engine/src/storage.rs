//! The only unsafe boundary. Presence bits own initialized slots, and storage
//! leases exclude aliasing. Structural edits require an exclusive world borrow.
use crate::{Data, DataError, Entity, Reader, Writer};
use std::any::Any;
use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

mod cell;
pub(crate) use cell::{make_cell, Singleton};
mod raw;
use raw::RawStorage;
mod pages;
mod query;
pub use pages::{Page, Pages, Plain};
pub use query::{Query, QueryBorrow, QueryIter, QueryRows};

/// Number of entity-indexed slots in each component page.
pub const PAGE: usize = 1024;
const WORDS: usize = PAGE / 64;

struct Lease<'a> {
    count: &'a Cell<isize>,
    mutable: bool,
}
impl<'a> Lease<'a> {
    fn new(name: &str, count: &'a Cell<isize>, mutable: bool) -> Self {
        let n = count.get();
        assert!(n >= 0, "{} is already borrowed mutably", name);
        assert!(!mutable || n == 0, "{} is already borrowed immutably", name);
        count.set(if mutable {
            -1
        } else {
            n.checked_add(1).expect("too many borrows")
        });
        Self { count, mutable }
    }
    // Split only for disjoint slots yielded once by an owning query iterator.
    fn split(&self) -> Self {
        self.count.set(if self.mutable {
            self.count.get().checked_sub(1).expect("too many borrows")
        } else {
            self.count.get().checked_add(1).expect("too many borrows")
        });
        Self {
            count: self.count,
            mutable: self.mutable,
        }
    }
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.count.set(if self.mutable {
            self.count.get() + 1
        } else {
            self.count.get() - 1
        });
    }
}

/// A shared component/resource borrow. Keeping it alive keeps its storage locked.
pub struct Ref<'a, C> {
    ptr: *const C,
    _lease: Lease<'a>,
    _life: PhantomData<&'a C>,
}
impl<C> Deref for Ref<'_, C> {
    type Target = C;
    fn deref(&self) -> &C {
        // SAFETY: the lease excludes writers and the world borrow keeps the slot alive.
        unsafe { &*self.ptr }
    }
}
/// An exclusive component/resource borrow locking the whole column.
/// Nested get::<C> calls for other entities also conflict; use a query instead.
pub struct RefMut<'a, C> {
    ptr: *mut C,
    _lease: Lease<'a>,
    _life: PhantomData<&'a mut C>,
}
impl<C> Deref for RefMut<'_, C> {
    type Target = C;
    fn deref(&self) -> &C {
        // SAFETY: this guard owns the storage's exclusive lease.
        unsafe { &*self.ptr }
    }
}
impl<C> DerefMut for RefMut<'_, C> {
    fn deref_mut(&mut self) -> &mut C {
        // SAFETY: this noncloneable guard is the only mutable reference to this slot.
        unsafe { &mut *self.ptr }
    }
}

// Typed access retains a concrete pointer stride; allocation, masks, epochs and
// every Data traversal are shared by all component types in RawStorage.
pub(crate) struct Storage<C> {
    raw: RawStorage,
    _type: PhantomData<C>,
}
impl<C> Deref for Storage<C> {
    type Target = RawStorage;
    fn deref(&self) -> &RawStorage {
        &self.raw
    }
}
impl<C> Storage<C> {
    #[inline]
    fn ptr(&self, index: usize) -> *mut C {
        self.raw.pages[index / PAGE]
            .as_ref()
            .unwrap()
            .get()
            .cast::<C>()
            .wrapping_add(index % PAGE)
    }
}
impl<C: Data> Storage<C> {
    pub(crate) fn insert(&mut self, index: usize, value: C) {
        let mut value = std::mem::ManuallyDrop::new(value);
        // SAFETY: this storage's descriptor is C; insert consumes the value, and
        // installs its replacement before dropping an old value (even on panic).
        unsafe { self.raw.insert(index, (&mut *value as *mut C).cast()) };
    }
    pub(crate) fn remove(&mut self, index: usize) -> Option<C> {
        let mut value = MaybeUninit::<C>::uninit();
        // SAFETY: descriptor and destination agree on C; success initializes it.
        unsafe {
            self.raw
                .remove_into(index, value.as_mut_ptr().cast())
                .then(|| value.assume_init())
        }
    }
    pub(crate) fn get(&self, index: usize) -> Option<Ref<'_, C>> {
        self.has(index).then(|| Ref {
            ptr: self.ptr(index),
            _lease: self.lease(false),
            _life: PhantomData,
        })
    }
    pub(crate) fn get_mut(&self, index: usize) -> Option<RefMut<'_, C>> {
        if !self.has(index) {
            return None;
        }
        let lease = self.lease(true);
        self.mark_slot(index);
        Some(RefMut {
            ptr: self.ptr(index),
            _lease: lease,
            _life: PhantomData,
        })
    }
}

pub(crate) trait Erased {
    fn has(&self, index: usize) -> bool;
    fn reset_observation(&self);
    #[cfg(test)]
    fn snapshot_uncached(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
    );
    fn snapshot(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(u8, &'static str, Entity, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
        label: (u8, &'static str),
    );
    fn moving(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> bool;
    fn visit_moving(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
        visit: &mut dyn FnMut(usize) -> bool,
    );
    fn settle_tick(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> Option<u64>;
    fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool;
    fn patch(&self, index: usize, r: &mut dyn Reader) -> Result<(), DataError>;
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn remove(&mut self, index: usize);
    fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity);
    fn read(&mut self, r: &mut dyn Reader, valid: &dyn Fn(Entity) -> bool)
        -> Result<(), DataError>;
}
pub(crate) fn make<C: Data>(name: &'static str, epoch: std::rc::Rc<Cell<u64>>) -> Box<dyn Erased> {
    Box::new(Storage::<C> {
        raw: RawStorage::new::<C>(name, epoch),
        _type: PhantomData,
    })
}
impl<C: Data> Erased for Storage<C> {
    fn has(&self, index: usize) -> bool {
        self.raw.has(index)
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn len(&self) -> usize {
        self.raw.len()
    }
    fn remove(&mut self, index: usize) {
        self.raw.erase(index);
    }
    fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool {
        self.raw.write_one(index, w)
    }
    fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity) {
        self.raw.write(w, entity);
    }
    fn read(
        &mut self,
        r: &mut dyn Reader,
        valid: &dyn Fn(Entity) -> bool,
    ) -> Result<(), DataError> {
        self.raw.read(r, valid)
    }
    #[cfg(test)]
    fn snapshot_uncached(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
    ) {
        self.raw.snapshot_uncached(skip, out, full, entity);
    }
    fn reset_observation(&self) {
        self.raw.reset_observation();
    }
    fn snapshot(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(u8, &'static str, Entity, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
        label: (u8, &'static str),
    ) {
        self.raw.snapshot(skip, out, full, entity, label);
    }
    fn patch(&self, index: usize, r: &mut dyn Reader) -> Result<(), DataError> {
        self.get_mut(index)
            .ok_or_else(|| DataError::new("reload component disappeared"))?
            .read(r)
    }
    fn moving(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> bool {
        self.raw.moving(now, skip)
    }
    fn visit_moving(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
        visit: &mut dyn FnMut(usize) -> bool,
    ) {
        self.raw.visit_moving(now, skip, visit);
    }
    fn settle_tick(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> Option<u64> {
        self.raw.settle_tick(now, skip)
    }
}

#[cfg(test)]
mod kind_tests;
