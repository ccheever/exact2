//! The only unsafe boundary. Presence bits own initialized slots, and row leases
//! exclude aliasing (`lease.rs`). Structural edits require an exclusive world borrow.
use crate::{Data, DataError, Entity, Reader, Writer};
use std::any::Any;
use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

mod cell;
pub(crate) use cell::{make_cell, Singleton};
mod lease;
pub(crate) use lease::{At, Conflict, Leases};
use lease::{Holds, Party, Registration, Via, EVERY};
use raw::RawStorage;
mod pages;
mod query;
mod raw;
pub use pages::{Page, Pages, Plain};
pub use query::{Query, QueryBorrow, QueryIter, QueryRows};

/// Number of entity-indexed slots in each component page.
pub const PAGE: usize = 512;
const WORDS: usize = PAGE / 64;

/// What a guard releases when it drops: its own hold on one row (or a whole
/// column), or one reference to the registered query it was yielded from.
enum Lease<'a> {
    Hold {
        holds: &'a Holds,
        slot: u32,
    },
    Query {
        leases: &'a Leases,
        query: Registration<'a>,
    },
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        match *self {
            Self::Hold { holds, slot } => holds.release(slot),
            Self::Query { leases, query } => leases.release(query),
        }
    }
}

/// A shared component/resource borrow. Keeping it alive keeps its row leased:
/// other rows of the component stay free, and this one refuses exclusive borrows.
pub struct Ref<'a, C> {
    ptr: *const C,
    _lease: Lease<'a>,
    _life: PhantomData<&'a C>,
}
impl<C> Deref for Ref<'_, C> {
    type Target = C;
    fn deref(&self) -> &C {
        // SAFETY: the row lease excludes writers and the world borrow keeps the slot alive.
        unsafe { &*self.ptr }
    }
}
/// An exclusive component/resource borrow of one row. Other rows of the component
/// stay free to borrow; this row refuses every other borrow until the guard drops.
pub struct RefMut<'a, C> {
    ptr: *mut C,
    _lease: Lease<'a>,
    _life: PhantomData<&'a mut C>,
}
impl<C> Deref for RefMut<'_, C> {
    type Target = C;
    fn deref(&self) -> &C {
        // SAFETY: this guard owns the row's exclusive lease.
        unsafe { &*self.ptr }
    }
}
impl<C> DerefMut for RefMut<'_, C> {
    fn deref_mut(&mut self) -> &mut C {
        // SAFETY: this noncloneable guard owns the row's exclusive lease, so it is
        // the only reference to this slot.
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
    /// `Err` means a live lease refused this row; `row_conflict` explains it.
    #[inline]
    pub(crate) fn get(
        &self,
        index: usize,
        leases: &Leases,
        at: At,
    ) -> Result<Option<Ref<'_, C>>, Refused> {
        if !self.has(index) {
            return Ok(None);
        }
        let lease = self
            .raw
            .lease_row(index, false, leases, at)
            .ok_or(Refused)?;
        Ok(Some(Ref {
            ptr: self.ptr(index),
            _lease: lease,
            _life: PhantomData,
        }))
    }
    #[inline]
    pub(crate) fn get_mut(
        &self,
        index: usize,
        leases: &Leases,
        at: At,
    ) -> Result<Option<RefMut<'_, C>>, Refused> {
        if !self.has(index) {
            return Ok(None);
        }
        let lease = self.raw.lease_row(index, true, leases, at).ok_or(Refused)?;
        self.edited();
        self.mark_page(index / PAGE);
        self.mark_row(index);
        Ok(Some(RefMut {
            ptr: self.ptr(index),
            _lease: lease,
            _life: PhantomData,
        }))
    }
}
/// A refused row lease; the storage re-derives who holds the row when asked.
pub(crate) struct Refused;

impl<C: Data + Copy> Storage<C> {
    /// Copy one present row out without registering a lease: no author code runs
    /// while it is read, so it only has to find no exclusive lease on the row.
    /// @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
    #[inline]
    pub(crate) fn copied(&self, index: usize, leases: &Leases) -> Result<Option<C>, Refused> {
        if !self.has(index) {
            return Ok(None);
        }
        if !self.holds.unwritten() && !self.raw.readable(index, leases) {
            return Err(Refused);
        }
        // SAFETY: presence proves initialization and no exclusive lease holds this
        // row; the copy completes before any other code runs.
        Ok(Some(unsafe { self.ptr(index).read() }))
    }
}

pub(crate) trait Erased {
    fn has(&self, index: usize) -> bool;
    /// The exclusive lease an engine read of every row would alias, if any.
    fn read_conflict(&self, leases: &Leases, at: At) -> Option<Conflict>;
    fn snapshot(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
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
    fn read_conflict(&self, leases: &Leases, at: At) -> Option<Conflict> {
        self.raw.read_conflict(leases, Via::Read, at)
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
    fn snapshot(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
    ) {
        self.raw.snapshot(skip, out, full, entity);
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
