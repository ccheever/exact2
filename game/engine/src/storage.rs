//! The only unsafe boundary. Presence bits own initialized slots, and storage
//! leases exclude aliasing. Structural edits require an exclusive world borrow.
use crate::{Data, DataError, Entity, Reader, Writer};
use std::any::Any;
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

mod pages;
mod query;
pub use pages::{Page, Pages, Plain};
pub use query::{Query, QueryBorrow, QueryIter, QueryRows};

/// Number of entity-indexed slots in each component page.
pub const PAGE: usize = 1024;
const WORDS: usize = PAGE / 64;
type Slots<C> = UnsafeCell<[MaybeUninit<C>; PAGE]>;

struct Lease<'a> {
    count: &'a Cell<isize>,
    mutable: bool,
}
impl<'a> Lease<'a> {
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

pub(crate) struct Storage<C> {
    name: &'static str,
    pages: Vec<Option<Box<Slots<C>>>>,
    counts: Vec<usize>,
    mask: Vec<u64>,
    len: usize,
    borrowed: Cell<isize>,
    changed: Cell<u64>,
    revision: Cell<u64>,
    membership: u64,
}
impl<C> Default for Storage<C> {
    fn default() -> Self {
        Self {
            name: crate::data::type_name::<C>(),
            pages: vec![],
            counts: vec![],
            mask: vec![],
            len: 0,
            borrowed: Cell::new(0),
            changed: Cell::new(0),
            revision: Cell::new(0),
            membership: 0,
        }
    }
}
impl<C> Storage<C> {
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }
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
    pub(crate) fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub(crate) fn membership(&self) -> u64 {
        self.membership
    }
    fn edited(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    pub(crate) fn changed(&self) -> u64 {
        self.changed.get()
    }
}
impl<C: Data> Storage<C> {
    fn lease(&self, mutable: bool, tick: u64) -> Lease<'_> {
        let n = self.borrowed.get();
        assert!(n >= 0, "{} is already borrowed mutably", self.name);
        assert!(
            !mutable || n == 0,
            "{} is already borrowed immutably",
            self.name
        );
        self.borrowed.set(if mutable {
            -1
        } else {
            n.checked_add(1).expect("too many borrows")
        });
        if mutable {
            self.edited();
            self.changed.set(tick);
        }
        Lease {
            count: &self.borrowed,
            mutable,
        }
    }
    pub(crate) fn insert(&mut self, index: usize, c: C, tick: u64) {
        self.edited();
        self.changed.set(tick);
        if self.has(index) {
            // SAFETY: the bit proves initialization; &mut self excludes all leases.
            // Replace before dropping, so even a panicking destructor leaves a live slot.
            drop(unsafe { self.ptr(index).replace(c) });
            return;
        }
        self.membership = self.membership.wrapping_add(1);
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
        self.edited();
        self.membership = self.membership.wrapping_add(1);
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
    fn has(&self, index: usize) -> bool;
    fn snapshot(&self, skip: Option<&Storage<crate::Ambient>>, out: &mut Vec<(usize, u64)>);
    fn moving(&self, now: crate::Now) -> bool;
    fn moving_indices(&self, now: crate::Now) -> Vec<usize>;
    fn settle_tick(&self, now: crate::Now) -> Option<u64>;
    fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool;
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
pub(crate) fn make<C: Data>(name: &'static str) -> Box<dyn Erased> {
    let mut storage = Box::new(Storage::<C>::default());
    storage.name = name;
    storage
}
impl<C: Data> Erased for Storage<C> {
    fn snapshot(&self, skip: Option<&Storage<crate::Ambient>>, out: &mut Vec<(usize, u64)>) {
        let _lease = self.lease(false, 0);
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits & !skip.and_then(|s| s.mask.get(word)).copied().unwrap_or(0);
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                // SAFETY: presence proves initialization; the shared lease excludes writers.
                out.push((i, crate::hash::of(unsafe { &*self.ptr(i) })));
            }
        }
    }

    fn has(&self, index: usize) -> bool {
        self.has(index)
    }
    fn moving(&self, now: crate::Now) -> bool {
        let _lease = self.lease(false, 0);
        self.mask.iter().enumerate().any(|(word, &bits)| {
            let mut bits = bits;
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                // SAFETY: presence proves initialization; the shared lease excludes writers.
                if unsafe { &*self.ptr(i) }.moving(now) {
                    return true;
                }
            }
            false
        })
    }
    fn moving_indices(&self, now: crate::Now) -> Vec<usize> {
        let _lease = self.lease(false, 0);
        let mut out = Vec::new();
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                // SAFETY: presence and shared lease protect this slot.
                if unsafe { &*self.ptr(i) }.moving(now) {
                    out.push(i);
                }
            }
        }
        out
    }
    fn settle_tick(&self, now: crate::Now) -> Option<u64> {
        let _lease = self.lease(false, 0);
        let mut at = now.tick;
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                // SAFETY: presence proves initialization; the shared lease excludes writers.
                at = at.max(unsafe { &*self.ptr(i) }.settle_tick(now)?);
            }
        }
        Some(at)
    }
    fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool {
        if let Some(c) = self.get(index) {
            c.write(w);
            true
        } else {
            false
        }
    }
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
            let page = e.index() as usize / PAGE;
            if page >= self.pages.len() {
                let pages = page + 1 - self.pages.len();
                let counts = page + 1 - self.counts.len();
                let words = (page + 1) * WORDS - self.mask.len();
                crate::data::limits::reserve(r, &mut self.pages, pages)?;
                crate::data::limits::reserve(r, &mut self.counts, counts)?;
                crate::data::limits::reserve(r, &mut self.mask, words)?;
            }
            if self.pages.get(page).is_none_or(Option::is_none) {
                r.claim(std::mem::size_of::<Slots<C>>())?;
            }
            self.insert(e.index() as usize, c, tick);
        }
        self.changed.set(tick);
        Ok(())
    }
}
