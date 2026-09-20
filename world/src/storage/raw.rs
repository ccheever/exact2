//! Layout and Data operations are fixed once per type. Typed wrappers never cast
//! between descriptors. Presence bits own values; all access holds a column lease.
use super::{Lease, PAGE};
use crate::{Data, DataError, Entity, Reader, Writer};
use std::{
    alloc::{alloc, dealloc, handle_alloc_error, Layout},
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
                let value = C::read_new(r)?;
                // SAFETY: caller supplies vacant, aligned storage for C.
                unsafe { p.cast::<C>().write(value) };
                Ok(())
            },
        }
    }
}

pub(super) struct Bytes {
    ptr: NonNull<u8>,
    layout: Layout,
}
impl Bytes {
    pub(super) fn new(layout: Layout) -> Self {
        // SAFETY: layout is valid. ZSTs use a nonnull aligned dangling pointer,
        // never sent to the allocator. Other pages are uninitialized until presence owns a value.
        let ptr = if layout.size() == 0 {
            NonNull::new(std::ptr::without_provenance_mut(layout.align())).unwrap()
        } else {
            NonNull::new(unsafe { alloc(layout) }).unwrap_or_else(|| handle_alloc_error(layout))
        };
        Self { ptr, layout }
    }
    pub(super) fn try_new(layout: Layout) -> Result<Self, DataError> {
        if layout.size() == 0 {
            return Ok(Self::new(layout));
        }
        // SAFETY: checked nonzero layout, allocation failure is returned to the reader.
        let ptr = NonNull::new(unsafe { alloc(layout) })
            .ok_or_else(|| DataError::new("cannot allocate decoded storage"))?;
        Ok(Self { ptr, layout })
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
    pub(super) pages: super::directory::Directory,
    len: usize,
    borrowed: Cell<isize>,
    revision: Cell<u64>,
    membership: u64,
    epoch: Rc<Cell<u64>>,
}
impl RawStorage {
    pub(super) fn new<C: Data>(name: &'static str, epoch: Rc<Cell<u64>>) -> Self {
        Self {
            name,
            desc: &const { Descriptor::of::<C>() },
            pages: super::directory::Directory::new(
                Layout::array::<C>(PAGE).expect("component chunk layout"),
            ),
            len: 0,
            borrowed: Cell::new(0),
            revision: Cell::new(0),
            membership: 0,
            epoch,
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub(super) fn ptr(&self, index: usize) -> *mut u8 {
        self.pages.chunks()[index / PAGE]
            .ptr
            .wrapping_add(index % PAGE * self.desc.layout.size())
    }
    #[inline]
    pub(super) fn word(&self, word: usize) -> u64 {
        self.pages.mask().get(word).copied().unwrap_or(0)
    }
    #[inline]
    pub(crate) fn has(&self, index: usize) -> bool {
        self.word(index / PAGE) & (1 << (index % PAGE)) != 0
    }
    pub(crate) fn indices<'a>(
        &'a self,
        skip: Option<&'a RawStorage>,
    ) -> impl Iterator<Item = usize> + 'a {
        self.pages
            .mask()
            .iter()
            .enumerate()
            .flat_map(move |(word, &mask)| {
                let mut bits = mask & !skip.map_or(0, |s| s.word(word));
                std::iter::from_fn(move || {
                    if bits == 0 {
                        return None;
                    }
                    let index = word * PAGE + bits.trailing_zeros() as usize;
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
    #[inline]
    pub(super) fn mark_page(&self, page: usize) {
        if let Some(p) = self.pages.chunks().get(page) {
            p.generation.set(self.revision.get());
        }
    }
    pub(super) fn mark_slot(&self, index: usize) {
        self.mark_page(index / PAGE);
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
        self.pages.allocate(page);
        self.mark_slot(index);
        // SAFETY: exclusive vacant aligned slot, matching size; transfers ownership
        // including any owned fields, without interpreting potentially padded bytes.
        unsafe { (self.desc.move_to)(value, self.ptr(index)) };
        self.pages.mask_mut()[page] |= 1 << (index % PAGE);
        self.len += 1;
    }
    fn removed(&mut self, index: usize) {
        self.edited();
        self.membership = self.membership.wrapping_add(1);
        self.mark_slot(index);
        self.pages.mask_mut()[index / PAGE] &= !(1 << (index % PAGE));
        self.len -= 1;
    }
    pub(super) unsafe fn remove_into(&mut self, index: usize, out: *mut u8) -> bool {
        if !self.has(index) {
            return false;
        }
        self.removed(index);
        // SAFETY: caller supplies aligned vacant storage for the same type.
        unsafe { (self.desc.move_to)(self.ptr(index), out) };
        true
    }
    pub(super) fn erase(&mut self, index: usize) {
        if self.has(index) {
            self.removed(index);
            // SAFETY: removing the presence bit transfers ownership to this drop.
            // If it panics, later walks cannot touch the partially dropped value.
            unsafe { (self.desc.drop_in_place)(self.ptr(index)) };
        }
    }
    pub(super) fn write_one(&self, index: usize, w: &mut dyn Writer) -> bool {
        if w.stopped() || !self.has(index) {
            return false;
        }
        let _lease = self.lease(false);
        // SAFETY: presence and shared lease protect the value.
        unsafe { (self.desc.write)(self.ptr(index), w) };
        true
    }
    pub(super) fn write(&self, w: &mut dyn Writer, entity: &dyn Fn(usize) -> Entity) {
        let _lease = self.lease(false);
        w.begin_seq(self.len);
        for index in self.indices(None) {
            if w.stopped() {
                break;
            }
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
        r.begin_seq()?;

        let mut last = None;
        r.claim(self.desc.layout.size())?;
        let mut value = Value {
            bytes: Bytes::try_new(self.desc.layout)?,
            desc: self.desc,
            live: false,
        };
        while r.item()? {
            r.begin_seq()?;
            let mut e = Entity::default();
            r.required_item("missing entity")?;
            e.read(r)?;
            if !valid(e) {
                return Err(DataError::new("stale or invalid entity").at(e.index()));
            }
            r.required_item("missing component")?;
            let page = e.index() as usize / PAGE;
            self.pages.prepare(page, r)?;
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
    fn failed_reader_drops_its_owned_partial_value_without_publishing_presence() {
        use crate::{Data, DataError, Reader, Writer};
        #[derive(Default)]
        struct Fails(String);
        impl Data for Fails {
            fn write(&self, w: &mut dyn Writer) {
                self.0.write(w);
            }
            fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
                self.0.read(r)?;
                if self.0 == "refuse" {
                    return Err(DataError::new("reader refused"));
                }
                Ok(())
            }
        }
        impl Component for Fails {
            const NAME: &'static str = "Fails";
        }
        let mut w = World::new(60, 0);
        w.register::<Fails>().unwrap();
        let e = w.spawn(Fails("refuse".into())).unwrap();
        let bad = w.save().unwrap();
        w.get_mut::<Fails>(e).unwrap().0 = "kept".into();
        let good = w.save().unwrap();
        assert!(w.load(&bad).is_err());
        assert_eq!(w.save().unwrap(), good);
        w.load(&good).unwrap();
        assert_eq!(w.get::<Fails>(e).unwrap().0, "kept");
    }
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
        w.register::<Padded>().unwrap();
        let e = w
            .spawn(Padded {
                byte: 1,
                owned: "first".into(),
                word: 2,
            })
            .unwrap();
        w.insert(
            e,
            Padded {
                byte: 3,
                owned: "second".into(),
                word: 4,
            },
        )
        .unwrap();
        let saved = w.save().unwrap();
        w.load(&saved).unwrap();
        assert_eq!(w.save().unwrap(), saved);
        let removed = w.remove::<Padded>(e).unwrap().unwrap();
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
        w.register::<Guard>().unwrap();
        let e = w.spawn(Guard).unwrap();
        assert_eq!(DROPS.get(), 0);
        w.insert(e, Guard).unwrap();
        assert_eq!(DROPS.get(), 1);
        let value = w.remove::<Guard>(e).unwrap().unwrap();
        assert_eq!(DROPS.get(), 1, "remove transfers ownership to its caller");
        drop(value);
        assert_eq!(DROPS.get(), 2);
        w.insert(e, Guard).unwrap();
        let saved = w.save().unwrap();
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
        w.register::<Owner>().unwrap();
        let e = w.spawn(Owner::new(1)).unwrap();
        w.insert(e, Owner::new(2)).unwrap();
        DROPPED.with_borrow(|ids| assert_eq!(ids, &[1]));
        let owner = w.remove::<Owner>(e).unwrap().unwrap();
        assert_eq!(owner.id, 2);
        DROPPED.with_borrow(|ids| assert_eq!(ids, &[1]));
        drop(owner);
        w.insert(e, Owner::new(3)).unwrap();
        let bytes = w.save().unwrap();
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
        w.register::<Aligned>().unwrap();
        let count: usize = if cfg!(miri) { 130 } else { 1030 };
        for n in 0..count {
            w.spawn(Aligned {
                text: "owned".into(),
                n: n as u32,
            })
            .unwrap();
        }
        let bytes = w.save().unwrap();
        w.load(&bytes).unwrap();
        assert_eq!(w.save().unwrap(), bytes);
        for (_, value) in &mut w.query::<&mut Aligned>() {
            assert_eq!((value as *mut Aligned as usize) % 128, 0);
            assert_eq!(value.text, "owned");
            value.n += 1;
        }
        assert_eq!(
            w.pages::<Aligned>().iter().count(),
            count.div_ceil(super::PAGE)
        );
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
        w.register::<Bomb>().unwrap();
        let e = w
            .spawn(Bomb {
                explode: true,
                text: "old".into(),
            })
            .unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            w.insert(
                e,
                Bomb {
                    explode: false,
                    text: "replacement".into(),
                },
            )
            .unwrap();
        }))
        .is_err());
        assert_eq!(w.get::<Bomb>(e).unwrap().text, "replacement");
        assert!(w.validate().unwrap_err().message.contains("poisoned"));
        drop(w);
        let mut w = World::new(60, 0);
        w.register::<Bomb>().unwrap();
        let e = w
            .spawn(Bomb {
                explode: true,
                text: "doomed".into(),
            })
            .unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| w.despawn(e).unwrap())).is_err());
        assert!(w.get::<Bomb>(e).is_none());
        assert!(w.validate().unwrap_err().message.contains("poisoned"));
    }
}

#[cfg(test)]
mod directory_tests {
    use crate::{Component, World, MAX_ENTITIES, PAGE};
    #[derive(Default, Component)]
    struct Wide([u64; 5]);
    #[derive(Default, Component)]
    struct Small(u32);
    #[test]
    fn presence_is_flat_bounded_and_values_stay_lazy_after_churn() {
        let mut w = World::new(60, 0);
        w.register::<Wide>().unwrap().register::<Small>().unwrap();
        for _ in 0..MAX_ENTITIES {
            w.spawn(()).unwrap();
        }
        let high = w.entity_at(MAX_ENTITIES - 1).unwrap();
        w.insert(high, Wide([7; 5])).unwrap();
        let column = w.storage::<Wide>().unwrap();
        assert_eq!(column.pages.mask().len() * 8, 25_000);
        assert_eq!(
            column
                .pages
                .chunks()
                .iter()
                .filter(|p| !p.ptr.is_null())
                .count(),
            1
        );
        assert_eq!(column.pages.layout.size(), 40 * PAGE);
        for i in [0, 63, 64, 8191, 8192] {
            let e = w.entity_at(i).unwrap();
            w.insert(e, Small(i as u32)).unwrap();
        }
        assert_eq!(w.storage::<Small>().unwrap().pages.layout.size(), 4 * PAGE);
        assert_eq!(
            w.query::<&Small>()
                .iter()
                .map(|(_, s)| s.0)
                .collect::<Vec<_>>(),
            [0, 63, 64, 8191, 8192]
        );
        w.remove::<Wide>(high).unwrap();
        assert!(w
            .storage::<Wide>()
            .unwrap()
            .pages
            .mask()
            .iter()
            .all(|m| *m == 0));
        assert_eq!(w.storage::<Wide>().unwrap().pages.mask().len() * 8, 25_000);
        w.insert(high, Wide([9; 5])).unwrap();
        assert_eq!(
            w.pages::<Wide>()
                .iter()
                .next()
                .unwrap()
                .runs()
                .next()
                .unwrap()
                .1[0]
                .0,
            [9; 5]
        );
    }
}
