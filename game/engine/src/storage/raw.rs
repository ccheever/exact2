//! Layout and Data operations are fixed once per type. Typed wrappers never cast
//! between descriptors. Presence bits own values; every access holds a row lease,
//! or checks that no exclusive lease is live before reading every row.
use super::{At, Conflict, Holds, Lease, Leases, Party, Storage, Via, EVERY, PAGE, WORDS};
use crate::{Data, DataError, Now, Reader, Writer};
use std::{
    alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout},
    cell::Cell,
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

pub(crate) struct RawStorage {
    name: &'static str,
    desc: &'static Descriptor,
    page_layout: Layout,
    pub(super) pages: Vec<Option<Bytes>>,
    counts: Vec<usize>,
    pub(super) generations: Vec<Cell<u64>>,
    // The revision of each slot's last write, insertion or removal (PAGE per page),
    // and the newest of each 64 consecutive slots, so a scan skips quiet runs.
    rows: Vec<Cell<u64>>,
    runs: Vec<Cell<u64>>,
    pub(super) mask: Vec<u64>,
    len: usize,
    pub(super) holds: Holds,
    revision: Cell<u64>,
    membership: u64,
    epoch: Rc<Cell<u64>>,
    instance: u64,
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
            rows: vec![],
            runs: vec![],
            mask: vec![],
            len: 0,
            holds: Holds::default(),
            revision: Cell::new(0),
            membership: 0,
            epoch,
            instance: super::instance(),
        }
    }
    /// Process-unique identity of this storage, for caches keyed by page generation.
    pub(crate) fn instance(&self) -> u64 {
        self.instance
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
    /// Pages with a row handed out mutably, inserted or removed after the
    /// storage revision `since`, in ascending order.
    pub(crate) fn changed_pages(&self, since: u64) -> impl Iterator<Item = usize> + '_ {
        let generations = self.generations.iter().enumerate();
        generations.filter_map(move |(page, g)| (g.get() > since).then_some(page))
    }
    /// The storage revision at which this page was last marked; 0 if never.
    pub(crate) fn page_generation(&self, page: usize) -> u64 {
        self.generations.get(page).map_or(0, Cell::get)
    }
    pub(crate) fn page_count(&self) -> usize {
        self.generations.len()
    }
    pub(super) fn mark_page(&self, page: usize) {
        if let Some(generation) = self.generations.get(page) {
            generation.set(self.revision.get());
        }
    }
    pub(super) fn mark_row(&self, index: usize) {
        if let Some(row) = self.rows.get(index) {
            row.set(self.revision.get());
            self.runs[index / 64].set(self.revision.get());
        }
    }
    /// Indices of rows written, inserted or removed after revision `since`, ascending.
    pub(crate) fn changed(&self, since: u64) -> impl Iterator<Item = u32> + '_ {
        self.generations
            .iter()
            .enumerate()
            .filter(move |(_, g)| g.get() > since)
            .flat_map(move |(page, _)| page * WORDS..(page + 1) * WORDS)
            .filter(move |&run| self.runs[run].get() > since)
            .flat_map(move |run| {
                let rows = &self.rows[run * 64..(run + 1) * 64];
                (0..64)
                    .filter(move |&i| rows[i].get() > since)
                    .map(move |i| (run * 64 + i) as u32)
            })
    }
    pub(super) fn edited(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    /// Lease one present row unless a live query or hold on the same row refuses.
    #[inline]
    pub(super) fn lease_row(
        &self,
        index: usize,
        mutable: bool,
        leases: &Leases,
        at: At,
    ) -> Option<Lease<'_>> {
        if !leases.admits(self, index, mutable) {
            return None;
        }
        let slot = self.holds.acquire(index as u32, mutable, at)?;
        Some(Lease::Hold {
            holds: &self.holds,
            slot,
        })
    }
    /// Whether a shared read of this row would alias no exclusive lease.
    pub(super) fn readable(&self, index: usize, leases: &Leases) -> bool {
        leases.admits(self, index, false) && self.holds.conflict(index as u32, false).is_none()
    }
    /// Why lease_row refused, re-derived off the hot path.
    #[cold]
    pub(crate) fn row_conflict(
        &self,
        index: usize,
        mutable: bool,
        leases: &Leases,
        at: At,
    ) -> Conflict {
        let held = leases
            .holder(self, index, mutable)
            .or_else(|| self.holds.conflict(index as u32, mutable))
            .expect("a refused row lease has a holder");
        let requested = Party {
            mutable,
            via: Via::Row,
            at,
        };
        Conflict::row(self.name, index as u32, held, requested)
    }
    /// A shared hold on every row, for page views.
    pub(super) fn lease_column(&self, leases: &Leases, at: At) -> Result<Lease<'_>, Conflict> {
        if let Some(conflict) = self.read_conflict(leases, Via::Column, at) {
            return Err(conflict);
        }
        let slot = self
            .holds
            .acquire(EVERY, false, at)
            .expect("no exclusive hold is live");
        Ok(Lease::Hold {
            holds: &self.holds,
            slot,
        })
    }
    /// The exclusive lease a shared read of every row would alias, if any.
    pub(super) fn read_conflict(&self, leases: &Leases, via: Via, at: At) -> Option<Conflict> {
        if self.holds.unwritten() {
            return None;
        }
        let (row, held) = match self.holds.writer() {
            Some((row, held)) => (Some(row), held),
            None => (
                None,
                leases.writer(self).expect("writers are counted holds"),
            ),
        };
        Some(Conflict {
            component: self.name,
            row,
            held,
            requested: Party {
                mutable: false,
                via,
                at,
            },
        })
    }
    // Engine walks run no author code; the World refuses by name before them.
    fn reading(&self) {
        assert!(
            self.holds.unwritten(),
            "{} is borrowed exclusively during an engine read of every row",
            self.name
        );
    }
    // Consumes a matching initialized value. Its source becomes uninitialized.
    pub(super) unsafe fn insert(&mut self, index: usize, value: *mut u8) {
        self.edited();
        if self.has(index) {
            self.mark_page(index / PAGE);
            self.mark_row(index);
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
            self.rows.resize_with((page + 1) * PAGE, || Cell::new(0));
            self.runs.resize_with((page + 1) * WORDS, || Cell::new(0));
            self.mask.resize((page + 1) * WORDS, 0);
        }
        self.mark_page(page);
        self.mark_row(index);
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
        self.mark_page(index / PAGE);
        self.mark_row(index);
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
        self.reading();
        // SAFETY: presence proves initialization; no exclusive lease is live.
        unsafe { (self.desc.write)(self.ptr(index), w) };
        true
    }
    /// Hash each present row of one page, in index order.
    pub(super) fn digest_page(&self, page: usize, each: &mut dyn FnMut(usize, u64)) {
        self.reading();
        let words = (page * WORDS..(page + 1) * WORDS).filter(|&w| w < self.mask.len());
        for word in words {
            let mut bits = self.mask[word];
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let mut w = crate::hash::Hasher::default();
                // SAFETY: presence proves initialization; no exclusive lease is live.
                unsafe { (self.desc.write)(self.ptr(i), &mut w) };
                each(i, w.finish());
            }
        }
    }
    pub(super) fn moving(&self, now: crate::Now, skip: Option<&Storage<crate::Ambient>>) -> bool {
        self.reading();
        self.indices(skip.map(|s| &s.raw)).any(|i| {
            // SAFETY: presence proves initialization; no exclusive lease is live.
            unsafe { (self.desc.moving)(self.ptr(i), now) }
        })
    }
    pub(super) fn visit_moving(
        &self,
        now: crate::Now,
        skip: Option<&Storage<crate::Ambient>>,
        visit: &mut dyn FnMut(usize) -> bool,
    ) {
        self.reading();
        for i in self.indices(skip.map(|s| &s.raw)) {
            // SAFETY: presence proves initialization; no exclusive lease is live.
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
        self.reading();
        let mut at = now.tick;
        for i in self.indices(skip.map(|s| &s.raw)) {
            // SAFETY: presence proves initialization; no exclusive lease is live.
            at = at.max(unsafe { (self.desc.settle)(self.ptr(i), now) }?);
        }
        Some(at)
    }
    /// The columnar save payload: runs of present indices, then the rows'
    /// shapes and scalar columns. Generations live in the entity table.
    pub(super) fn write_save(&self, w: &mut dyn Writer) {
        self.reading();
        let mut columns = crate::data::columns::Columns::default();
        for index in self.indices(None) {
            // SAFETY: the bit proves initialization; no exclusive lease is live.
            columns.row_at(index as u32, |w| unsafe {
                (self.desc.write)(self.ptr(index), w)
            });
        }
        w.bytes(crate::data::Bulk::U8(&columns.finish()));
    }
    pub(super) fn read_save(
        &mut self,
        r: &mut dyn Reader,
        alive: &dyn Fn(u32) -> bool,
    ) -> Result<(), DataError> {
        let bytes = r
            .bytes(crate::data::BulkKind::U8)?
            .ok_or_else(|| DataError::new("expected a columnar storage"))?
            .to_vec();
        let (indices, mut rows) =
            crate::data::columns::Rows::decode(&bytes, crate::data::MAX_LOAD_ENTITIES, true, r)?;
        r.claim(self.desc.layout.size())?;
        let mut value = Value {
            bytes: Bytes::new(self.desc.layout),
            desc: self.desc,
            live: false,
        };
        let mut last = None;
        for index in indices {
            if last.is_some_and(|last| last >= index) {
                return Err(DataError::new("entities are not strictly ordered").at(index));
            }
            last = Some(index);
            if !alive(index) {
                return Err(DataError::new("stale or invalid entity").at(index));
            }
            // SAFETY: correctly aligned scratch, initialized only on success.
            rows.read(r, |r| unsafe { (self.desc.read_new)(value.bytes.get(), r) })
                .map_err(|err| err.at(index))?;
            value.live = true;
            let page = index as usize / PAGE;
            if page >= self.pages.len() {
                let pages = page + 1 - self.pages.len();
                let counts = page + 1 - self.counts.len();
                let words = (page + 1) * WORDS - self.mask.len();
                crate::data::limits::reserve(r, &mut self.pages, pages)?;
                crate::data::limits::reserve(r, &mut self.counts, counts)?;
                crate::data::limits::reserve(r, &mut self.generations, pages)?;
                crate::data::limits::reserve(r, &mut self.rows, pages * PAGE)?;
                crate::data::limits::reserve(r, &mut self.runs, words)?;
                crate::data::limits::reserve(r, &mut self.mask, words)?;
            }
            if self.pages.get(page).is_none_or(Option::is_none) {
                r.claim(self.page_layout.size())?;
            }
            value.live = false;
            // SAFETY: read_new initialized the matching descriptor's type.
            unsafe { self.insert(index as usize, value.bytes.get()) };
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
        for n in 0..(crate::PAGE + 6) as u32 {
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

    // Row leases move and drop nothing: only replacing a value through a guard
    // drops, and each owner drops exactly once. @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
    #[test]
    fn row_leases_on_zero_sized_owners_neither_move_nor_drop() {
        thread_local! { static DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
        #[derive(Default, Component)]
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                DROPS.set(DROPS.get() + 1);
            }
        }
        DROPS.set(0);
        let mut w = World::new(60, 0);
        let [a, b, c] = [(); 3].map(|_| w.spawn(Guard));
        {
            let mut first = w.get_mut::<Guard>(a).unwrap();
            let second = w.get_mut::<Guard>(b).unwrap();
            let third = w.get::<Guard>(c).unwrap();
            assert!(catch_unwind(AssertUnwindSafe(|| w.get::<Guard>(b))).is_err());
            *first = Guard;
            assert_eq!(
                DROPS.get(),
                1,
                "assignment through a guard drops the old owner"
            );
            drop((second, third));
        }
        assert_eq!(DROPS.get(), 1);
        let rows: Vec<_> = w.query::<&mut Guard>().into_iter().collect();
        assert_eq!(rows.len(), 3);
        assert!(catch_unwind(AssertUnwindSafe(|| w.get::<Guard>(a))).is_err());
        drop(rows);
        assert_eq!(DROPS.get(), 1, "escaped rows release leases, not values");
        assert!(w.get_mut::<Guard>(a).is_some());
        drop(w);
        assert_eq!(DROPS.get(), 4);
    }

    #[test]
    fn over_aligned_rows_lease_apart_inside_a_filtered_query() {
        #[repr(align(128))]
        #[derive(Default, Component)]
        struct Aligned {
            text: String,
            n: u32,
        }
        #[derive(Default, Component)]
        struct Marked;
        let mut w = World::new(60, 0);
        let entities: Vec<_> = (0..(crate::PAGE + 6) as u32)
            .map(|n| {
                let e = w.spawn(Aligned {
                    text: "owned".into(),
                    n,
                });
                if n % 2 == 1 {
                    w.insert(e, Marked);
                }
                e
            })
            .collect();
        // An unmarked row on the second page, borrowed beside each marked row.
        let far_row = entities[crate::PAGE];
        for (_, value) in w.query::<&mut Aligned>().with::<Marked>().iter() {
            let other = w.get::<Aligned>(entities[0]).unwrap();
            let mut far = w.get_mut::<Aligned>(far_row).unwrap();
            for row in [&*value as *const Aligned, &*other, &*far] {
                assert_eq!(row as usize % 128, 0);
            }
            value.n += other.n + 1;
            far.text.push('+');
        }
        assert_eq!(w.get::<Aligned>(entities[1]).unwrap().n, 2);
        let marked = (crate::PAGE + 6) / 2;
        assert_eq!(w.get::<Aligned>(far_row).unwrap().text.len(), 5 + marked);
        let saved = w.save();
        w.load(&saved).unwrap();
        assert_eq!(w.save(), saved);
    }

    #[test]
    fn a_panicking_destructor_under_a_row_lease_releases_it() {
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
        let armed = w.spawn(Bomb {
            explode: true,
            text: "old".into(),
        });
        let other = w.spawn(Bomb::default());
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _neighbour = w.get_mut::<Bomb>(other).unwrap();
            let mut row = w.get_mut::<Bomb>(armed).unwrap();
            *row = Bomb {
                explode: false,
                text: "replacement".into(),
            };
        }))
        .is_err());
        // Assignment completes on unwind; both guards released their rows.
        assert_eq!(w.get_mut::<Bomb>(armed).unwrap().text, "replacement");
        assert!(w.get_mut::<Bomb>(other).is_some());
        for (_, bomb) in w.query::<&mut Bomb>().iter() {
            bomb.text.clear();
        }
    }
}
