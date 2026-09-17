//! The only unsafe boundary. Presence bits own initialized slots, and storage
//! leases exclude aliasing. Structural edits require an exclusive world borrow.
use crate::{Component, Data, DataError, Entity, Reader, Writer};
use std::any::Any;
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

mod pages;
mod query;
pub use pages::{Page, Pages, Plain};
pub use query::{Query, QueryBorrow, QueryIter};

/// Number of entity-indexed slots in each component page.
pub const PAGE: usize = 1024;
const WORDS: usize = PAGE / 64;
type Slots<C> = UnsafeCell<[MaybeUninit<C>; PAGE]>;

struct Lease<'a> {
    count: &'a Cell<isize>,
    mutable: bool,
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.count.set(if self.mutable {
            0
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
/// An exclusive component/resource borrow.
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

pub(crate) struct Storage<C> {
    pages: Vec<Option<Box<Slots<C>>>>,
    counts: Vec<usize>,
    mask: Vec<u64>,
    len: usize,
    borrowed: Cell<isize>,
    changed: Cell<u64>,
}
impl<C> Default for Storage<C> {
    fn default() -> Self {
        Self {
            pages: vec![],
            counts: vec![],
            mask: vec![],
            len: 0,
            borrowed: Cell::new(0),
            changed: Cell::new(0),
        }
    }
}
impl<C> Storage<C> {
    #[inline]
    fn ptr(&self, index: usize) -> *mut C {
        self.pages[index / PAGE]
            .as_ref()
            .unwrap()
            .get()
            .cast::<C>()
            .wrapping_add(index % PAGE)
    }
    #[inline]
    pub(crate) fn has(&self, index: usize) -> bool {
        self.mask
            .get(index / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }
    pub(crate) fn changed(&self) -> u64 {
        self.changed.get()
    }
}
impl<C: Component> Storage<C> {
    fn lease(&self, mutable: bool, tick: u64) -> Lease<'_> {
        let n = self.borrowed.get();
        assert!(n >= 0, "{} is already borrowed mutably", C::NAME);
        assert!(
            !mutable || n == 0,
            "{} is already borrowed immutably",
            C::NAME
        );
        self.borrowed.set(if mutable {
            -1
        } else {
            n.checked_add(1).expect("too many borrows")
        });
        if mutable {
            self.changed.set(tick);
        }
        Lease {
            count: &self.borrowed,
            mutable,
        }
    }
    pub(crate) fn insert(&mut self, index: usize, c: C, tick: u64) {
        self.changed.set(tick);
        if self.has(index) {
            // SAFETY: the bit proves initialization; &mut self excludes all leases.
            // Replace before dropping, so even a panicking destructor leaves a live slot.
            drop(unsafe { self.ptr(index).replace(c) });
            return;
        }
        let page = index / PAGE;
        if page >= self.pages.len() {
            self.pages.resize_with(page + 1, || None);
            self.counts.resize(page + 1, 0);
            self.mask.resize((page + 1) * WORDS, 0);
        }
        self.pages[page].get_or_insert_with(|| {
            // MaybeUninit accepts zero bits for every C, including zero-sized types.
            // Zero backing bytes also make absent Plain slots safe to upload.
            // SAFETY: UnsafeCell<[MaybeUninit<C>; PAGE]> accepts all-zero backing.
            unsafe { Box::<Slots<C>>::new_zeroed().assume_init() }
        });
        // SAFETY: the page exists, this slot is absent, and &mut self excludes readers.
        unsafe { self.ptr(index).write(c) };
        self.mask[index / 64] |= 1 << (index % 64);
        self.counts[page] += 1;
        self.len += 1;
    }
    pub(crate) fn remove(&mut self, index: usize, tick: u64) -> Option<C> {
        if !self.has(index) {
            return None;
        }
        self.changed.set(tick);
        let ptr = self.ptr(index);
        self.mask[index / 64] &= !(1 << (index % 64));
        self.len -= 1;
        self.counts[index / PAGE] -= 1;
        // SAFETY: the slot was present and exclusively owned. Moving it out transfers
        // ownership; zeroing its MaybeUninit backing keeps absent byte views initialized.
        let c = unsafe {
            let c = ptr.read();
            ptr.write_bytes(0, 1);
            c
        };
        if self.counts[index / PAGE] == 0 {
            self.pages[index / PAGE] = None;
        }
        Some(c)
    }
    pub(crate) fn get(&self, index: usize) -> Option<Ref<'_, C>> {
        if !self.has(index) {
            return None;
        }
        Some(Ref {
            ptr: self.ptr(index),
            _lease: self.lease(false, 0),
            _life: PhantomData,
        })
    }
    pub(crate) fn get_mut(&self, index: usize, tick: u64) -> Option<RefMut<'_, C>> {
        if !self.has(index) {
            return None;
        }
        Some(RefMut {
            ptr: self.ptr(index),
            _lease: self.lease(true, tick),
            _life: PhantomData,
        })
    }
}

impl<C> Drop for Storage<C> {
    fn drop(&mut self) {
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let index = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                // SAFETY: each presence bit owns exactly one initialized slot; no leases
                // survive &mut self. Boxes subsequently free only MaybeUninit backing.
                unsafe { self.ptr(index).drop_in_place() };
            }
        }
    }
}

pub(crate) trait Erased {
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn remove(&mut self, index: usize, tick: u64);
    fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity);
    fn read(
        &mut self,
        r: &mut dyn Reader,
        valid: &dyn Fn(Entity) -> bool,
        tick: u64,
    ) -> Result<(), DataError>;
}
pub(crate) fn make<C: Component>() -> Box<dyn Erased> {
    Box::new(Storage::<C>::default())
}
impl<C: Component> Erased for Storage<C> {
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn len(&self) -> usize {
        self.len
    }
    fn remove(&mut self, index: usize, tick: u64) {
        self.remove(index, tick);
    }
    fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity) {
        let _lease = self.lease(false, 0);
        w.begin_seq(self.len);
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let index = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                w.item();
                w.begin_seq(2);
                w.item();
                entity(index).write(w);
                w.item();
                // SAFETY: the bit proves initialization and the shared lease excludes writers.
                unsafe { &*self.ptr(index) }.write(w);
                w.end_seq();
            }
        }
        w.end_seq();
    }
    fn read(
        &mut self,
        r: &mut dyn Reader,
        valid: &dyn Fn(Entity) -> bool,
        tick: u64,
    ) -> Result<(), DataError> {
        r.begin_seq()?;
        let mut last = None;
        while r.item()? {
            r.begin_seq()?;
            let mut e = Entity::default();
            let mut c = C::default();
            if !r.item()? {
                return Err(DataError::new("missing entity"));
            }
            e.read(r)?;
            if !valid(e) {
                return Err(DataError::new("stale or invalid entity").at(e.index()));
            }
            if !r.item()? {
                return Err(DataError::new("missing component"));
            }
            c.read(r).map_err(|err| err.at(e.index()))?;
            if r.item()? {
                return Err(DataError::new("extra component entry value"));
            }
            if last.is_some_and(|last| last >= e.index()) {
                return Err(DataError::new("entities are not strictly ordered"));
            }
            last = Some(e.index());
            self.insert(e.index() as usize, c, tick);
        }
        self.changed.set(tick);
        Ok(())
    }
}
