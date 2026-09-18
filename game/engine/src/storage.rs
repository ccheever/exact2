//! The only unsafe boundary. Presence bits own initialized slots, and storage
//! leases exclude aliasing. Structural edits require an exclusive world borrow.
use crate::{Data, DataError, Entity, Reader, Writer};
use std::any::Any;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

mod cell;
pub(crate) use cell::{make_cell, Singleton};
mod pages;
mod query;
pub use pages::{Page, Pages, Plain};
pub use query::{Query, QueryBorrow, QueryIter, QueryRows};

/// Number of entity-indexed slots in each component page.
pub const PAGE: usize = 1024;
const WORDS: usize = PAGE / 64;
type Slots<C> = UnsafeCell<[MaybeUninit<C>; PAGE]>;

#[derive(Default)]
struct ObservedPage {
    generation: Option<u64>,
    mask: [u64; WORDS],
    skip: [u64; WORDS],
    entries: Vec<(usize, u64)>,
}

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

pub(crate) struct Storage<C> {
    name: &'static str,
    pages: Vec<Option<Box<Slots<C>>>>,
    counts: Vec<usize>,
    generations: Vec<Cell<u64>>,
    mask: Vec<u64>,
    len: usize,
    borrowed: Cell<isize>,
    revision: Cell<u64>,
    membership: u64,
    observation: RefCell<Vec<ObservedPage>>,
    pub(super) epoch: std::rc::Rc<Cell<u64>>,
}
impl<C> Default for Storage<C> {
    fn default() -> Self {
        Self {
            name: crate::data::type_name::<C>(),
            pages: vec![],
            counts: vec![],
            generations: vec![],
            mask: vec![],
            len: 0,
            borrowed: Cell::new(0),
            revision: Cell::new(0),
            membership: 0,
            observation: RefCell::new(Vec::new()),
            epoch: Default::default(),
        }
    }
}
impl<C> Storage<C> {
    pub(crate) fn lease_conflict(&self, mutable: bool) -> Option<&'static str> {
        match self.borrowed.get() {
            n if n < 0 => Some("mutably"),
            n if mutable && n > 0 => Some("immutably"),
            _ => None,
        }
    }
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
    pub(crate) fn page_count(&self) -> usize {
        self.generations.len()
    }
    pub(crate) fn page_generation(&self, page: usize) -> u64 {
        self.generations.get(page).map_or(0, Cell::get)
    }
    pub(crate) fn page_mask(&self, page: usize) -> [u64; WORDS] {
        self.mask
            .get(page * WORDS..(page + 1) * WORDS)
            .map_or([0; WORDS], |mask| mask.try_into().unwrap())
    }
    fn mark_page(&self, page: usize) {
        if let Some(generation) = self.generations.get(page) {
            generation.set(self.revision.get());
        }
    }
    fn edited(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.revision.set(self.revision.get().wrapping_add(1));
    }
}
impl<C: Data> Storage<C> {
    fn lease(&self, mutable: bool) -> Lease<'_> {
        let lease = Lease::new(self.name, &self.borrowed, mutable);
        if mutable {
            self.edited();
        }
        lease
    }
    pub(crate) fn insert(&mut self, index: usize, c: C) {
        self.edited();
        if self.has(index) {
            self.mark_page(index / PAGE);
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
            self.generations.resize_with(page + 1, || Cell::new(0));
            self.mask.resize((page + 1) * WORDS, 0);
        }
        self.mark_page(page);
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
    pub(crate) fn remove(&mut self, index: usize) -> Option<C> {
        if !self.has(index) {
            return None;
        }
        self.edited();
        self.membership = self.membership.wrapping_add(1);
        self.mark_page(index / PAGE);
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
            _lease: self.lease(false),
            _life: PhantomData,
        })
    }
    pub(crate) fn get_mut(&self, index: usize) -> Option<RefMut<'_, C>> {
        if !self.has(index) {
            return None;
        }
        let lease = self.lease(true);
        self.mark_page(index / PAGE);
        Some(RefMut {
            ptr: self.ptr(index),
            _lease: lease,
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
    let mut storage = Box::new(Storage::<C>::default());
    storage.name = name;
    storage.epoch = epoch;
    storage
}
impl<C: Data> Erased for Storage<C> {
    #[cfg(test)]
    fn snapshot_uncached(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        mut full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
    ) {
        let _lease = self.lease(false);
        if let Some(w) = &mut full {
            w.begin_seq(self.len);
        }
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let observe = !skip.is_some_and(|s| s.has(i));
                if !observe && full.is_none() {
                    continue;
                }
                // SAFETY: presence proves initialization; the shared lease excludes writers.
                let value = unsafe { &*self.ptr(i) };
                if let Some(w) = &mut full {
                    w.item();
                    w.begin_seq(2);
                    w.item();
                    entity(i).write(*w);
                    w.item();
                    if observe {
                        out.push((i, w.with_observation(value)));
                    } else {
                        value.write(*w);
                    }
                    w.end_seq();
                } else {
                    out.push((i, crate::hash::of(value)));
                }
            }
        }
        if let Some(w) = &mut full {
            w.end_seq();
        }
    }
    fn reset_observation(&self) {
        self.observation.borrow_mut().clear();
    }
    fn snapshot(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(usize, u64)>,
        mut full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
    ) {
        // Even a cache hit must honor an outstanding mutable lease.
        let _lease = self.lease(false);
        let mut pages = self.observation.borrow_mut();
        pages.resize_with(self.page_count(), ObservedPage::default);
        if let Some(w) = &mut full {
            w.begin_seq(self.len);
        }
        for (page, cached) in pages.iter_mut().enumerate() {
            let generation = self.page_generation(page);
            let mask = self.page_mask(page);
            let mut skipped = skip.map_or([0; WORDS], |s| s.page_mask(page));
            for (skip, present) in skipped.iter_mut().zip(mask) {
                *skip &= present;
            }
            let dirty = cached.generation != Some(generation)
                || cached.mask != mask
                || cached.skip != skipped;
            if dirty {
                // A panicking Data::write must not leave a partially valid page.
                cached.generation = None;
                cached.entries.clear();
            }
            if dirty || full.is_some() {
                for (word, &bits) in mask.iter().enumerate() {
                    let mut bits = bits;
                    while bits != 0 {
                        let bit = bits.trailing_zeros() as usize;
                        let i = page * PAGE + word * 64 + bit;
                        bits &= bits - 1;
                        let observe = skipped[word] & (1 << bit) == 0;
                        if !observe && full.is_none() {
                            continue;
                        }
                        // SAFETY: presence proves initialization; the shared lease excludes writers.
                        let value = unsafe { &*self.ptr(i) };
                        if let Some(w) = &mut full {
                            w.item();
                            w.begin_seq(2);
                            w.item();
                            entity(i).write(*w);
                            w.item();
                            if dirty && observe {
                                cached.entries.push((i, w.with_observation(value)));
                            } else {
                                value.write(*w);
                            }
                            w.end_seq();
                        } else {
                            cached.entries.push((i, crate::hash::of(value)));
                        }
                    }
                }
            }
            if dirty {
                cached.mask = mask;
                cached.skip = skipped;
                cached.generation = Some(generation);
            }
            out.extend_from_slice(&cached.entries);
        }
        if let Some(w) = &mut full {
            w.end_seq();
        }
    }

    fn has(&self, index: usize) -> bool {
        self.has(index)
    }
    fn moving(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> bool {
        let _lease = self.lease(false);
        self.mask.iter().enumerate().any(|(word, &bits)| {
            let mut bits = bits & !skip.and_then(|s| s.mask.get(word)).copied().unwrap_or(0);
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
    fn visit_moving(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
        visit: &mut dyn FnMut(usize) -> bool,
    ) {
        let _lease = self.lease(false);
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits & !skip.and_then(|s| s.mask.get(word)).copied().unwrap_or(0);
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                // SAFETY: presence and shared lease protect this slot.
                if unsafe { &*self.ptr(i) }.moving(now) && !visit(i) {
                    return;
                }
            }
        }
    }
    fn settle_tick(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> Option<u64> {
        let _lease = self.lease(false);
        let mut at = now.tick;
        for (word, &bits) in self.mask.iter().enumerate() {
            let mut bits = bits & !skip.and_then(|s| s.mask.get(word)).copied().unwrap_or(0);
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
    fn remove(&mut self, index: usize) {
        self.remove(index);
    }
    fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity) {
        let _lease = self.lease(false);
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
    ) -> Result<(), DataError> {
        r.begin_seq()?;
        if let Some(count) = r.sequence_len() {
            r.check_allocation(
                count
                    .div_ceil(PAGE)
                    .checked_mul(std::mem::size_of::<Slots<C>>())
                    .ok_or_else(|| DataError::new("allocation size overflow"))?,
            )?;
        }
        let mut last = None;
        while r.item()? {
            r.begin_seq()?;
            let mut e = Entity::default();
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
            let page = e.index() as usize / PAGE;
            if self.pages.get(page).is_none_or(Option::is_none) {
                // A tiny default component can require an entire 1024-slot page.
                // Scoped imports refuse that footprint before running its default/read.
                r.check_allocation(std::mem::size_of::<Slots<C>>())?;
            }
            let mut c = C::default();
            c.read(r).map_err(|err| err.at(e.index()))?;
            if r.item()? {
                return Err(DataError::new("extra component entry value"));
            }
            if last.is_some_and(|last| last >= e.index()) {
                return Err(DataError::new("entities are not strictly ordered"));
            }
            last = Some(e.index());
            if page >= self.pages.len() {
                let pages = page + 1 - self.pages.len();
                let counts = page + 1 - self.counts.len();
                let words = (page + 1) * WORDS - self.mask.len();
                crate::data::limits::reserve(r, &mut self.pages, pages)?;
                crate::data::limits::reserve(r, &mut self.counts, counts)?;
                crate::data::limits::reserve(r, &mut self.generations, pages)?;
                crate::data::limits::reserve(r, &mut self.mask, words)?;
            }
            if self.pages.get(page).is_none_or(Option::is_none) {
                r.claim(std::mem::size_of::<Slots<C>>())?;
            }
            self.insert(e.index() as usize, c);
        }
        Ok(())
    }
}
