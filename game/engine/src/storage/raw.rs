//! Layout and Data operations are fixed once per type. Typed wrappers never cast
//! between descriptors. Presence bits own values; all access holds a column lease.
use super::{Lease, Storage, PAGE, WORDS};
use crate::{Data, DataError, Entity, Now, Reader, Writer};
use std::{
    alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout},
    cell::{Cell, RefCell},
    ptr::NonNull,
    rc::Rc,
};

struct Descriptor {
    layout: Layout,
    move_to: unsafe fn(*mut u8, *mut u8),
    swap: unsafe fn(*mut u8, *mut u8),
    drop_in_place: unsafe fn(*mut u8),
    write: unsafe fn(*mut u8, &mut dyn Writer),
    read_new: unsafe fn(*mut u8, &mut dyn Reader) -> Result<(), DataError>,
    moving: unsafe fn(*mut u8, Now) -> bool,
    settle: unsafe fn(*mut u8, Now) -> Option<u64>,
}
impl Descriptor {
    const fn of<C: Data>() -> Self {
        Self {
            layout: Layout::new::<C>(),
            // SAFETY: callers supply a live C with the matching layout and lease.
            move_to: |src, dst| unsafe { dst.cast::<C>().write(src.cast::<C>().read()) },
            swap: |a, b| unsafe {
                let old = a.cast::<C>().replace(b.cast::<C>().read());
                b.cast::<C>().write(old);
            },
            drop_in_place: |p| unsafe { p.cast::<C>().drop_in_place() },
            write: |p, w| unsafe { &*p.cast::<C>() }.write(w),
            read_new: |p, r| {
                let mut value = C::default();
                value.read(r)?;
                // SAFETY: caller supplies vacant, aligned storage for C.
                unsafe { p.cast::<C>().write(value) };
                Ok(())
            },
            moving: |p, now| unsafe { &*p.cast::<C>() }.moving(now),
            settle: |p, now| unsafe { &*p.cast::<C>() }.settle_tick(now),
        }
    }
}

pub(super) struct Bytes {
    ptr: NonNull<u8>,
    layout: Layout,
}
impl Bytes {
    fn new(layout: Layout) -> Self {
        // SAFETY: layout is valid. ZSTs use a nonnull aligned dangling pointer,
        // never sent to the allocator. All other pages start initialized to zero.
        let ptr = if layout.size() == 0 {
            NonNull::new(std::ptr::without_provenance_mut(layout.align())).unwrap()
        } else {
            NonNull::new(unsafe { alloc_zeroed(layout) })
                .unwrap_or_else(|| handle_alloc_error(layout))
        };
        Self { ptr, layout }
    }
    pub(super) fn get(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}
impl Drop for Bytes {
    fn drop(&mut self) {
        if self.layout.size() != 0 {
            // SAFETY: uniquely owned allocation with its original layout.
            unsafe { dealloc(self.get(), self.layout) };
        }
    }
}
struct Value {
    bytes: Bytes,
    desc: &'static Descriptor,
    live: bool,
}
impl Drop for Value {
    fn drop(&mut self) {
        if self.live {
            // SAFETY: read_new initialized this value and insert has not consumed it.
            unsafe { (self.desc.drop_in_place)(self.bytes.get()) };
        }
    }
}

#[derive(Default)]
struct ObservedWord {
    // None also protects against a panicking Data::write during a rebuild.
    mask: Option<u64>,
    entries: Vec<(usize, u64)>,
}

pub(crate) struct RawStorage {
    name: &'static str,
    desc: &'static Descriptor,
    page_layout: Layout,
    pub(super) pages: Vec<Option<Bytes>>,
    counts: Vec<usize>,
    pub(super) generations: Vec<Cell<u64>>,
    pub(super) mask: Vec<u64>,
    len: usize,
    borrowed: Cell<isize>,
    revision: Cell<u64>,
    membership: u64,
    observation: RefCell<Vec<ObservedWord>>,
    observation_dirty: Vec<Cell<u64>>,
    epoch: Rc<Cell<u64>>,
}
impl RawStorage {
    pub(super) fn new<C: Data>(name: &'static str, epoch: Rc<Cell<u64>>) -> Self {
        Self {
            name,
            desc: &const { Descriptor::of::<C>() },
            page_layout: Layout::array::<C>(PAGE).expect("component page layout"),
            pages: vec![],
            counts: vec![],
            generations: vec![],
            mask: vec![],
            len: 0,
            borrowed: Cell::new(0),
            revision: Cell::new(0),
            membership: 0,
            observation: RefCell::new(Vec::new()),
            observation_dirty: Vec::new(),
            epoch,
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    fn ptr(&self, index: usize) -> *mut u8 {
        self.pages[index / PAGE]
            .as_ref()
            .unwrap()
            .get()
            .wrapping_add(index % PAGE * self.desc.layout.size())
    }
    #[inline]
    pub(crate) fn has(&self, index: usize) -> bool {
        self.mask
            .get(index / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }
    // Membership alone takes no value lease; callers can mutate a yielded row.
    pub(crate) fn indices<'a>(
        &'a self,
        skip: Option<&'a RawStorage>,
    ) -> impl Iterator<Item = usize> + 'a {
        self.mask.iter().enumerate().flat_map(move |(word, &bits)| {
            let mut bits = bits & !skip.and_then(|s| s.mask.get(word)).copied().unwrap_or(0);
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let index = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                Some(index)
            })
        })
    }
    pub(crate) fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub(crate) fn membership(&self) -> u64 {
        self.membership
    }
    pub(crate) fn lease_conflict(&self, mutable: bool) -> Option<&'static str> {
        match self.borrowed.get() {
            n if n < 0 => Some("mutably"),
            n if mutable && n > 0 => Some("immutably"),
            _ => None,
        }
    }
    pub(crate) fn page_count(&self) -> usize {
        self.generations.len()
    }
    #[cfg(test)]
    pub(crate) fn observation_capacity(&self) -> (usize, usize) {
        let words = self.observation.borrow();
        (
            words.len(),
            words.iter().map(|w| w.entries.capacity()).sum(),
        )
    }
    pub(crate) fn page_generation(&self, page: usize) -> u64 {
        self.generations.get(page).map_or(0, Cell::get)
    }
    pub(crate) fn page_mask(&self, page: usize) -> [u64; WORDS] {
        self.mask
            .get(page * WORDS..(page + 1) * WORDS)
            .map_or([0; WORDS], |mask| mask.try_into().unwrap())
    }
    pub(super) fn mark_page(&self, page: usize) {
        if let Some(generation) = self.generations.get(page) {
            generation.set(self.revision.get());
        }
    }
    pub(super) fn mark_observation(&self, word: usize, bits: u64) {
        if let Some(dirty) = self.observation_dirty.get(word) {
            dirty.set(dirty.get() | bits);
        }
    }
    pub(super) fn mark_slot(&self, index: usize) {
        self.mark_page(index / PAGE);
        self.mark_observation(index / 64, 1 << (index % 64));
    }
    fn edited(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    pub(super) fn lease(&self, mutable: bool) -> Lease<'_> {
        let lease = Lease::new(self.name, &self.borrowed, mutable);
        if mutable {
            self.edited();
        }
        lease
    }
    // Consumes a matching initialized value. Its source becomes uninitialized.
    pub(super) unsafe fn insert(&mut self, index: usize, value: *mut u8) {
        self.edited();
        if self.has(index) {
            self.mark_slot(index);
            // SAFETY: disjoint initialized values of the descriptor's type. Swap
            // before dropping so a panicking destructor leaves the slot live.
            unsafe {
                (self.desc.swap)(self.ptr(index), value);
                (self.desc.drop_in_place)(value);
            }
            return;
        }
        self.membership = self.membership.wrapping_add(1);
        let page = index / PAGE;
        if page >= self.pages.len() {
            self.pages.resize_with(page + 1, || None);
            self.counts.resize(page + 1, 0);
            self.generations.resize_with(page + 1, || Cell::new(0));
            self.mask.resize((page + 1) * WORDS, 0);
            self.observation_dirty
                .resize_with((page + 1) * WORDS, || Cell::new(0));
        }
        self.mark_slot(index);
        self.pages[page].get_or_insert_with(|| Bytes::new(self.page_layout));
        // SAFETY: exclusive vacant aligned slot, matching size; transfers ownership
        // including any owned fields, without interpreting potentially padded bytes.
        unsafe { (self.desc.move_to)(value, self.ptr(index)) };
        self.mask[index / 64] |= 1 << (index % 64);
        self.counts[page] += 1;
        self.len += 1;
    }
    fn removed(&mut self, index: usize) {
        self.edited();
        self.membership = self.membership.wrapping_add(1);
        self.mark_slot(index);
        self.mask[index / 64] &= !(1 << (index % 64));
        self.len -= 1;
        self.counts[index / PAGE] -= 1;
    }
    fn clear_slot(&mut self, index: usize) {
        // SAFETY: ownership has moved out; initialized zero bytes keep absent Plain
        // slots uploadable. No references remain, and an empty page may be freed.
        unsafe { self.ptr(index).write_bytes(0, self.desc.layout.size()) };
        if self.counts[index / PAGE] == 0 {
            self.pages[index / PAGE] = None;
            let words = self.observation.get_mut();
            for cached in words.iter_mut().skip(index / PAGE * WORDS).take(WORDS) {
                *cached = ObservedWord::default();
            }
            while words.last().is_some_and(|word| word.entries.is_empty()) {
                words.pop();
            }
            words.shrink_to_fit();
        }
    }
    pub(super) unsafe fn remove_into(&mut self, index: usize, out: *mut u8) -> bool {
        if !self.has(index) {
            return false;
        }
        self.removed(index);
        // SAFETY: caller supplies aligned vacant storage for the same type.
        unsafe { (self.desc.move_to)(self.ptr(index), out) };
        self.clear_slot(index);
        true
    }
    pub(super) fn erase(&mut self, index: usize) {
        if self.has(index) {
            self.removed(index);
            // SAFETY: removing the presence bit transfers ownership to this drop.
            // If it panics, later walks cannot touch the partially dropped value.
            unsafe { (self.desc.drop_in_place)(self.ptr(index)) };
            self.clear_slot(index);
        }
    }
    pub(super) fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool {
        if !self.has(index) {
            return false;
        }
        let _lease = self.lease(false);
        // SAFETY: presence and shared lease protect the value.
        unsafe { (self.desc.write)(self.ptr(index), w) };
        true
    }
    #[cfg(test)]
    pub(super) fn snapshot_uncached(
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
        for i in self.indices(None) {
            let observe = !skip.is_some_and(|s| s.has(i));
            if !observe && full.is_none() {
                continue;
            }
            // SAFETY: presence proves initialization; the shared lease excludes writers.
            let value = self.ptr(i);
            if let Some(w) = &mut full {
                w.item();
                w.begin_seq(2);
                w.item();
                entity(i).write(*w);
                w.item();
                if observe {
                    out.push((
                        i,
                        w.with_observation_by(|w| unsafe { (self.desc.write)(value, w) }),
                    ));
                } else {
                    unsafe { (self.desc.write)(value, *w) };
                }
                w.end_seq();
            } else {
                let mut w = crate::hash::Hasher::default();
                unsafe { (self.desc.write)(value, &mut w) };
                out.push((i, w.finish()));
            }
        }
        if let Some(w) = &mut full {
            w.end_seq();
        }
    }

    pub(super) fn reset_observation(&self) {
        self.observation.borrow_mut().clear();
    }
    pub(super) fn snapshot(
        &self,
        skip: Option<&Storage<crate::Ambient>>,
        out: &mut Vec<(u8, &'static str, Entity, u64)>,
        mut full: Option<&mut crate::hash::Hasher>,
        entity: &dyn Fn(usize) -> Entity,
        label: (u8, &'static str),
    ) {
        // Even a cache hit must honor an outstanding mutable lease.
        let _lease = self.lease(false);
        let mut words = self.observation.borrow_mut();
        let count = self
            .mask
            .iter()
            .rposition(|&bits| bits != 0)
            .map_or(0, |i| i + 1);
        words.resize_with(count, ObservedWord::default);
        if let Some(w) = &mut full {
            w.begin_seq(self.len);
        }
        for (word, cached) in words.iter_mut().enumerate() {
            let mask = self.mask[word];
            let observed = mask & !skip.and_then(|s| s.mask.get(word)).copied().unwrap_or(0);
            let rebuild = cached.mask != Some(observed);
            let dirty = observed
                & if rebuild {
                    u64::MAX
                } else {
                    self.observation_dirty[word].get()
                };
            if rebuild {
                cached.mask = None;
                cached.entries.clear();
            }
            // Clean observation-only words never dereference or serialize a value.
            // Fused samples still stream all values, in the original entity order.
            let mut bits = if full.is_some() { mask } else { dirty };
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                let index = word * 64 + bit;
                bits &= bits - 1;
                let rehash = dirty & (1 << bit) != 0;
                // SAFETY: presence proves initialization; the shared lease excludes writers.
                let value = self.ptr(index);
                let digest = if let Some(w) = &mut full {
                    w.item();
                    w.begin_seq(2);
                    w.item();
                    entity(index).write(*w);
                    w.item();
                    let digest = if rehash {
                        Some(w.with_observation_by(|w| unsafe { (self.desc.write)(value, w) }))
                    } else {
                        unsafe { (self.desc.write)(value, *w) };
                        None
                    };
                    w.end_seq();
                    digest
                } else {
                    Some({
                        let mut w = crate::hash::Hasher::default();
                        unsafe { (self.desc.write)(value, &mut w) };
                        w.finish()
                    })
                };
                if let Some(digest) = digest {
                    if rebuild {
                        cached.entries.push((index, digest));
                    } else {
                        let rank = (observed & ((1u64 << bit) - 1)).count_ones() as usize;
                        cached.entries[rank].1 = digest;
                    }
                }
            }
            if observed == 0 {
                cached.entries = Vec::new();
            }
            cached.mask = Some(observed);
            // Clear only after all writes succeed; a panic cannot bless stale rows.
            self.observation_dirty[word].set(0);
            out.extend(
                cached
                    .entries
                    .iter()
                    .map(|&(i, hash)| (label.0, label.1, entity(i), hash)),
            );
        }
        if let Some(w) = &mut full {
            w.end_seq();
        }
    }

    pub(super) fn moving(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> bool {
        let _lease = self.lease(false);
        self.indices(skip.map(|s| &s.raw)).any(|i| {
            // SAFETY: presence proves initialization; the shared lease excludes writers.
            unsafe { (self.desc.moving)(self.ptr(i), now) }
        })
    }
    pub(super) fn visit_moving(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
        visit: &mut dyn FnMut(usize) -> bool,
    ) {
        let _lease = self.lease(false);
        for i in self.indices(skip.map(|s| &s.raw)) {
            // SAFETY: presence and shared lease protect this slot.
            if unsafe { (self.desc.moving)(self.ptr(i), now) } && !visit(i) {
                return;
            }
        }
    }
    pub(super) fn settle_tick(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
    ) -> Option<u64> {
        let _lease = self.lease(false);
        let mut at = now.tick;
        for i in self.indices(skip.map(|s| &s.raw)) {
            // SAFETY: presence proves initialization; the shared lease excludes writers.
            at = at.max(unsafe { (self.desc.settle)(self.ptr(i), now) }?);
        }
        Some(at)
    }
    pub(super) fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity) {
        let _lease = self.lease(false);
        w.begin_seq(self.len);
        for index in self.indices(None) {
            w.item();
            w.begin_seq(2);
            w.item();
            entity(index).write(w);
            w.item();
            // SAFETY: the bit proves initialization and the shared lease excludes writers.
            unsafe { (self.desc.write)(self.ptr(index), w) };
            w.end_seq();
        }
        w.end_seq();
    }
    pub(super) fn read(
        &mut self,
        r: &mut dyn Reader,
        valid: &dyn Fn(Entity) -> bool,
    ) -> Result<(), DataError> {
        self.reset_observation();
        r.begin_seq()?;
        let mut last = None;
        r.claim(self.desc.layout.size())?;
        let mut value = Value {
            bytes: Bytes::new(self.desc.layout),
            desc: self.desc,
            live: false,
        };
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
            // SAFETY: correctly aligned scratch, initialized only on success.
            unsafe { (self.desc.read_new)(value.bytes.get(), r) }
                .map_err(|err| err.at(e.index()))?;
            value.live = true;
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
                crate::data::limits::reserve(r, &mut self.generations, pages)?;
                crate::data::limits::reserve(r, &mut self.mask, words)?;
                crate::data::limits::reserve(r, &mut self.observation_dirty, words)?;
            }
            if self.pages.get(page).is_none_or(Option::is_none) {
                r.claim(self.page_layout.size())?;
            }
            value.live = false;
            // SAFETY: read_new initialized the matching descriptor's type.
            unsafe { self.insert(e.index() as usize, value.bytes.get()) };
        }
        Ok(())
    }
}
impl Drop for RawStorage {
    fn drop(&mut self) {
        for index in self.indices(None) {
            // SAFETY: each presence bit owns one initialized value; no lease
            // survives the owner. Bytes subsequently deallocates the pages.
            unsafe { (self.desc.drop_in_place)(self.ptr(index)) };
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Component, World};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn padded_values_move_replace_remove_and_load() {
        #[repr(C)]
        #[derive(Default, Component)]
        struct Padded {
            byte: u8,
            owned: String,
            word: u64,
        }
        assert!(std::mem::size_of::<Padded>() > 1 + std::mem::size_of::<String>() + 8);
        let mut w = World::new(60, 0);
        let e = w.spawn(Padded {
            byte: 1,
            owned: "first".into(),
            word: 2,
        });
        w.insert(
            e,
            Padded {
                byte: 3,
                owned: "second".into(),
                word: 4,
            },
        );
        let saved = w.save();
        w.load(&saved).unwrap();
        assert_eq!(w.save(), saved);
        let removed = w.remove::<Padded>(e).unwrap();
        assert_eq!(
            (removed.byte, removed.owned.as_str(), removed.word),
            (3, "second", 4)
        );
        assert!(!w.has::<Padded>(e));
    }
    #[test]
    fn zero_sized_drop_ownership_survives_replace_remove_and_load() {
        thread_local! { static DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
        #[derive(Default, Component)]
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                DROPS.set(DROPS.get() + 1);
            }
        }
        assert_eq!(std::mem::size_of::<Guard>(), 0);
        DROPS.set(0);
        let mut w = World::new(60, 0);
        let e = w.spawn(Guard);
        assert_eq!(DROPS.get(), 0);
        w.insert(e, Guard);
        assert_eq!(DROPS.get(), 1);
        let value = w.remove::<Guard>(e).unwrap();
        assert_eq!(DROPS.get(), 1, "remove transfers ownership to its caller");
        drop(value);
        assert_eq!(DROPS.get(), 2);
        w.insert(e, Guard);
        let saved = w.save();
        w.load(&saved).unwrap();
        assert_eq!(DROPS.get(), 3, "load drops the old world");
        drop(w);
        assert_eq!(DROPS.get(), 4);
    }

    // An actual ZST cannot store an identity. This companion locks OLD-vs-NEW
    // ownership; the ZST test above separately locks zero-size layout/lifetimes.
    #[test]
    fn constructor_ids_identify_replaced_removed_and_loaded_owners() {
        thread_local! { static DROPPED: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) }; }
        #[derive(Default, Component)]
        struct Owner {
            id: u32,
        }
        impl Owner {
            fn new(id: u32) -> Self {
                Self { id }
            }
        }
        impl Drop for Owner {
            fn drop(&mut self) {
                DROPPED.with_borrow_mut(|ids| ids.push(self.id));
            }
        }
        DROPPED.with_borrow_mut(Vec::clear);
        let mut w = World::new(60, 0);
        let e = w.spawn(Owner::new(1));
        w.insert(e, Owner::new(2));
        DROPPED.with_borrow(|ids| assert_eq!(ids, &[1]));
        let owner = w.remove::<Owner>(e).unwrap();
        assert_eq!(owner.id, 2);
        DROPPED.with_borrow(|ids| assert_eq!(ids, &[1]));
        drop(owner);
        w.insert(e, Owner::new(3));
        let bytes = w.save();
        w.load(&bytes).unwrap();
        DROPPED.with_borrow(|ids| assert_eq!(ids, &[1, 2, 3]));
        assert_eq!(w.get::<Owner>(e).unwrap().id, 3);
        drop(w);
        DROPPED.with_borrow(|ids| assert_eq!(ids, &[1, 2, 3, 3]));
    }

    #[test]
    fn over_aligned_owned_values_survive_pages_and_decode_scratch() {
        #[repr(align(128))]
        #[derive(Default, Component)]
        struct Aligned {
            text: String,
            n: u32,
        }
        let mut w = World::new(60, 0);
        for n in 0..1030 {
            w.spawn(Aligned {
                text: "owned".into(),
                n,
            });
        }
        let bytes = w.save();
        w.load(&bytes).unwrap();
        assert_eq!(w.save(), bytes);
        for (_, value) in &mut w.query::<&mut Aligned>() {
            assert_eq!((value as *mut Aligned as usize) % 128, 0);
            assert_eq!(value.text, "owned");
            value.n += 1;
        }
        assert_eq!(w.pages::<Aligned>().iter().count(), 2);
    }

    #[test]
    fn panicking_drops_do_not_leave_a_present_dead_value() {
        #[derive(Default, Component)]
        struct Bomb {
            explode: bool,
            text: String,
        }
        impl Drop for Bomb {
            fn drop(&mut self) {
                assert!(!self.explode, "drop bomb");
            }
        }
        let mut w = World::new(60, 0);
        let e = w.spawn(Bomb {
            explode: true,
            text: "old".into(),
        });
        assert!(catch_unwind(AssertUnwindSafe(|| {
            w.insert(
                e,
                Bomb {
                    explode: false,
                    text: "replacement".into(),
                },
            );
        }))
        .is_err());
        assert_eq!(w.get::<Bomb>(e).unwrap().text, "replacement");
        w.get_mut::<Bomb>(e).unwrap().explode = true;
        assert!(catch_unwind(AssertUnwindSafe(|| w.despawn(e))).is_err());
        assert!(w.get::<Bomb>(e).is_none());
        assert!(w.despawn(e));
    }
}
