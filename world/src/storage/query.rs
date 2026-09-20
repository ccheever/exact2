use super::{raw::RawStorage, Lease, Ref, RefMut, Storage};
use crate::{Component, Entity, World};
use std::any::TypeId;
use std::cell::Cell;
use std::marker::PhantomData;

mod sealed {
    pub trait Sealed {}
    impl<C: crate::Component> Sealed for &C {}
    impl<C: crate::Component> Sealed for &mut C {}
    impl<C: crate::Component> Sealed for Option<&C> {}
    impl<C: crate::Component> Sealed for Option<&mut C> {}
}

/// The built-in reference, optional-reference and tuple query forms.
/// Sealed so every mutable query can prove its references are disjoint.
pub trait Query: sealed::Sealed {
    /// Plain references bounded by the query borrow, not the world's lifetime.
    type Item<'a>;
    /// Guarded rows from consuming iteration; each guard keeps its column leased.
    type Owned<'w>;
    /// Human-readable component names.
    fn names() -> String;
    /// # Safety
    /// Acquire the state’s leases and mark its page first. The index must match, and may be yielded only once
    /// while this state lives.
    #[doc(hidden)]
    unsafe fn owned<'w>(state: &Self::State<'w>, index: usize) -> Self::Owned<'w>;
    /// Prepared column references, leased once at query construction.
    #[doc(hidden)]
    type State<'w>: for<'a> Fetch<Item<'a> = Self::Item<'a>>;
    /// Resolve columns and reject duplicate component types before acquiring leases.
    #[doc(hidden)]
    fn prepare<'w>(
        world: &'w World,
        seen: &mut [Option<TypeId>; 8],
    ) -> Result<Self::State<'w>, String>;
}

/// Internal query operations, exposed only as an associated bound.
#[doc(hidden)]
pub trait Fetch {
    type Item<'a>;
    fn words(&self) -> usize;
    fn acquire(&mut self);
    fn conflict(&self) -> Option<(&'static str, &'static str)>;
    fn mark_page(&self, page: usize);
    fn word(&self, word: usize) -> u64;
    /// # Safety
    /// The index must pass this fetch's mask. Call at most once per index within
    /// an exclusive borrow of the state lasting at least 'a; keep its leases alive
    /// and call mark_page for this index's page before fetching.
    unsafe fn fetch<'a>(&self, index: usize) -> Self::Item<'a>;
}

#[doc(hidden)]
pub struct ComponentBorrow<'w, C, const MUT: bool, const OPTIONAL: bool> {
    storage: Option<&'w Storage<C>>,
    _lease: Option<Lease<'w>>,
    page: Cell<*mut C>,
}
impl<'w, C: Component, const M: bool, const O: bool> ComponentBorrow<'w, C, M, O> {
    fn new(world: &'w World, seen: &mut [Option<TypeId>; 8]) -> Result<Self, String> {
        let id = TypeId::of::<C>();
        if seen.contains(&Some(id)) {
            return Err(format!("{} occurs twice in one query", C::NAME));
        }
        if M && id == TypeId::of::<crate::Parent>() {
            return Err("use set_parent for ownership edits".into());
        }
        let slot = seen
            .iter_mut()
            .find(|s| s.is_none())
            .ok_or("query exceeds 8 terms")?;
        *slot = Some(id);
        let storage = world.storage::<C>();
        Ok(Self {
            storage,
            _lease: None,
            page: Cell::new(std::ptr::null_mut()),
        })
    }
    fn required_words(&self) -> usize {
        if O {
            usize::MAX
        } else {
            self.storage.map_or(0, |s| s.words())
        }
    }
    fn required_word(&self, word: usize) -> u64 {
        if O {
            u64::MAX
        } else {
            self.storage.map_or(0, |s| s.word(word))
        }
    }
    fn ptr(&self, index: usize) -> *mut C {
        // next_index selects the page once per iteration chunk. The lease keeps
        // its backing alive; optional fetches test presence before dereferencing.
        self.page.get().wrapping_add(index % super::PAGE)
    }
    fn has(&self, index: usize) -> bool {
        self.storage.is_some_and(|s| s.has(index))
    }
}
macro_rules! owned_row {
    (true, $s:ident, $i:ident, $make:ident) => {
        if $s.has($i) {
            Some($make())
        } else {
            None
        }
    };
    (false, $s:ident, $i:ident, $make:ident) => {
        $make()
    };
}
macro_rules! reference {
    ($form:ty, $m:literal, $o:tt, $item:ty, $owned:ty, $guard:ident, $s:ident, $i:ident, $fetch:expr) => {
        impl<'q, C: Component> Query for $form {
            type Item<'a> = $item;
            type Owned<'w> = $owned;
            fn names() -> String {
                C::NAME.into()
            }
            unsafe fn owned<'w>(state: &Self::State<'w>, index: usize) -> Self::Owned<'w> {
                let make = || $guard {
                    ptr: state.ptr(index),
                    _lease: state._lease.as_ref().unwrap().split(),
                    _life: PhantomData,
                };
                owned_row!($o, state, index, make)
            }
            type State<'w> = ComponentBorrow<'w, C, $m, $o>;
            fn prepare<'w>(
                w: &'w World,
                seen: &mut [Option<TypeId>; 8],
            ) -> Result<Self::State<'w>, String> {
                ComponentBorrow::new(w, seen)
            }
        }
        impl<C: Component> Fetch for ComponentBorrow<'_, C, $m, $o> {
            type Item<'a> = $item;
            fn acquire(&mut self) {
                self._lease = self.storage.map(|s| s.lease($m));
            }
            fn conflict(&self) -> Option<(&'static str, &'static str)> {
                self.storage
                    .and_then(|s| s.lease_conflict($m))
                    .map(|why| (C::NAME, why))
            }
            fn mark_page(&self, page: usize) {
                if let Some(s) = self.storage {
                    if $m {
                        s.mark_page(page);
                    }
                    self.page.set(
                        s.pages
                            .get(&page)
                            .map_or(std::ptr::null_mut(), |p| p.bytes.get().cast::<C>()),
                    );
                }
            }
            fn words(&self) -> usize {
                self.required_words()
            }
            fn word(&self, word: usize) -> u64 {
                self.required_word(word)
            }
            unsafe fn fetch<'a>(&self, $i: usize) -> $item {
                let $s = self;
                // SAFETY: the caller holds the exclusive query borrow, selects each
                // present slot once, and bounds returned references by that borrow.
                unsafe { $fetch }
            }
        }
    };
}
reference!(
    &'q C,
    false,
    false,
    &'a C,
    Ref<'w, C>,
    Ref,
    s,
    i,
    &*s.ptr(i)
);
reference!(
    &'q mut C,
    true,
    false,
    &'a mut C,
    RefMut<'w, C>,
    RefMut,
    s,
    i,
    &mut *s.ptr(i)
);
reference!(
    Option<&'q C>,
    false,
    true,
    Option<&'a C>,
    Option<Ref<'w, C>>,
    Ref,
    s,
    i,
    if s.has(i) { Some(&*s.ptr(i)) } else { None }
);
reference!(
    Option<&'q mut C>,
    true,
    true,
    Option<&'a mut C>,
    Option<RefMut<'w, C>>,
    RefMut,
    s,
    i,
    if s.has(i) { Some(&mut *s.ptr(i)) } else { None }
);

macro_rules! tuples {
    ($($T:ident:$i:tt),+) => {
        impl<$($T: Query),+> sealed::Sealed for ($($T,)+) {}
        impl<$($T: Query),+> Query for ($($T,)+) {
            type Item<'a> = ($($T::Item<'a>,)+);
            type Owned<'w> = ($($T::Owned<'w>,)+);
            fn names() -> String { [$($T::names(),)+].join(", ") }
            unsafe fn owned<'w>(state: &Self::State<'w>, index: usize) -> Self::Owned<'w> {
                // SAFETY: the caller yields each matched index once; construction rejects aliases.
                unsafe { ($($T::owned(&state.$i, index),)+) }
            }
            type State<'w> = ($($T::State<'w>,)+);
            fn prepare<'w>(w: &'w World, seen: &mut [Option<TypeId>; 8]) -> Result<Self::State<'w>, String> {
                Ok(($($T::prepare(w, seen)?,)+))
            }
        }
        impl<$($T: Fetch),+> Fetch for ($($T,)+) {
            type Item<'a> = ($($T::Item<'a>,)+);
            fn acquire(&mut self) { $(self.$i.acquire();)+ }
            fn conflict(&self) -> Option<(&'static str, &'static str)> {
                $(if let Some(conflict) = self.$i.conflict() { return Some(conflict); })+
                None
            }
            fn mark_page(&self, page: usize) { $(self.$i.mark_page(page);)+ }
            fn words(&self) -> usize { usize::MAX $(.min(self.$i.words()))+ }
            fn word(&self, word: usize) -> u64 { u64::MAX $(& self.$i.word(word))+ }
            unsafe fn fetch<'a>(&self, index: usize) -> Self::Item<'a> {
                // SAFETY: construction rejects duplicate types, and the caller supplies
                // one fresh index whose bit is set in every required component's mask.
                unsafe { ($(self.$i.fetch(index),)+) }
            }
        }
    };
}
tuples!(A:0);
tuples!(A:0, B:1);
tuples!(A:0, B:1, C:2);
tuples!(A:0, B:1, C:2, D:3);
tuples!(A:0, B:1, C:2, D:3, E:4);
tuples!(A:0, B:1, C:2, D:3, E:4, F:5);
tuples!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
tuples!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

/// An entity-ordered query holding one lease per component until it drops.
/// Iteration borrows this object exclusively; rows may outlive an iterator but
/// cannot outlive the leases or overlap a second iteration.
///
pub struct QueryBorrow<'w, Q: Query> {
    world: &'w World,
    state: Q::State<'w>,
    filters: [(Option<&'w RawStorage>, Option<&'w RawStorage>, bool); 4],
    filter_count: usize,
    words: usize,
}
impl<'w, Q: Query> QueryBorrow<'w, Q> {
    pub(crate) fn new(world: &'w World) -> Self {
        Self::try_new(world).expect("query preparation")
    }
    pub(crate) fn try_new(world: &'w World) -> Result<Self, String> {
        let mut state = Q::prepare(world, &mut [None; 8])?;
        if let Some((name, why)) = state.conflict() {
            return Err(format!("{name} is already borrowed {why}"));
        }
        state.acquire();
        Ok(Self::from_state(world, state))
    }
    fn from_state(world: &'w World, state: Q::State<'w>) -> Self {
        let words = state.words().min(world.alive_mask.len());
        Self {
            world,
            state,
            filters: [(None, None, false); 4],
            filter_count: 0,
            words,
        }
    }
    // Structural singleton lookup uses the same presence-mask join without
    // borrowing values or marking pages. It remains valid during an edit.
    pub fn matching_count(world: &'w World) -> Result<(usize, Option<Entity>), String> {
        let state = Q::prepare(world, &mut [None; 8])?;
        let mut count = 0;
        let mut found = None;
        for word in 0..state.words().min(world.alive_mask.len()) {
            let bits = world.alive_mask[word] & state.word(word);
            count += bits.count_ones() as usize;
            if bits != 0 {
                found = Some(world.entity_at(word * 64 + bits.trailing_zeros() as usize));
            }
        }
        Ok((count, found))
    }
    /// Keep entities carrying C, without borrowing its values.
    pub fn with<C: Component>(mut self) -> Self {
        let mask = self.world.storage::<C>().map(|s| &s.raw);
        self.words = self.words.min(mask.map_or(0, |s| s.words()));
        self.filter::<C>(mask, true);
        self
    }
    /// Keep the union of A/B membership, without borrowing their values.
    pub fn with_any<A: Component, B: Component>(mut self) -> Self {
        let a = self.world.storage::<A>().map(|s| &s.raw);
        let b = self.world.storage::<B>().map(|s| &s.raw);
        self.words = self
            .words
            .min(a.map_or(0, |s| s.words()).max(b.map_or(0, |s| s.words())));
        self.filter::<A>(a, true);
        self.filters[self.filter_count - 1].1 = b;
        self
    }
    /// Keep entities without C, without borrowing its values.
    pub fn without<C: Component>(mut self) -> Self {
        let mask = self.world.storage::<C>().map(|s| &s.raw);
        self.filter::<C>(mask, false);
        self
    }
    fn filter<C: Component>(&mut self, mask: Option<&'w RawStorage>, with: bool) {
        assert!(
            self.filter_count < 4,
            "query exceeds 4 filters at {}",
            C::NAME
        );
        self.filters[self.filter_count] = (mask, None, with);
        self.filter_count += 1;
    }
    /// Fetch one joined row. Its borrow prevents a second overlapping fetch.
    pub fn get(&mut self, entity: Entity) -> Option<Q::Item<'_>> {
        if !self.world.contains(entity) {
            return None;
        }
        let i = entity.index() as usize;
        let word = i / 64;
        if word >= self.words || self.state.word(word) & (1 << (i % 64)) == 0 {
            return None;
        }
        for &(a, b, with) in &self.filters[..self.filter_count] {
            let bits = a.map_or(0, |s| s.word(word)) | b.map_or(0, |s| s.word(word));
            if (bits & (1 << (i % 64)) != 0) != with {
                return None;
            }
        }
        self.state.mark_page(i / super::PAGE);
        // SAFETY: live identity, masks and unique query borrow protect this row.
        Some(unsafe { self.state.fetch(i) })
    }
    /// The sole item, or None. Multiple matches are refused in every build.
    pub fn one(&mut self) -> Option<Q::Item<'_>> {
        let count = self.iter().count();
        assert!(count <= 1, "expected one {}, found {}", Q::names(), count);
        self.iter().next().map(|(_, item)| item)
    }
    /// Visit each matching entity once, yielding plain references.
    pub fn iter(&mut self) -> QueryIter<'_, 'w, Q> {
        QueryIter {
            query: self,
            word: 0,
            bits: 0,
            page: usize::MAX,
        }
    }
}
impl<'a, 'w, Q: Query> IntoIterator for &'a mut QueryBorrow<'w, Q> {
    type Item = (Entity, Q::Item<'a>);
    type IntoIter = QueryIter<'a, 'w, Q>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An owning iterator of guarded items, keeping leases alive even if a row escapes the loop.
pub struct QueryRows<'w, Q: Query> {
    query: QueryBorrow<'w, Q>,
    word: usize,
    bits: u64,
    page: usize,
}
impl<'w, Q: Query> IntoIterator for QueryBorrow<'w, Q> {
    type Item = Q::Owned<'w>;
    type IntoIter = QueryRows<'w, Q>;
    fn into_iter(self) -> Self::IntoIter {
        QueryRows {
            query: self,
            word: 0,
            bits: 0,
            page: usize::MAX,
        }
    }
}
impl<'w, Q: Query> Iterator for QueryRows<'w, Q> {
    type Item = Q::Owned<'w>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_entity().map(|(_, row)| row)
    }
}
impl<'w, Q: Query> QueryRows<'w, Q> {
    /// Next guarded row together with its entity, in the same storage scan.
    #[inline]
    pub fn next_entity(&mut self) -> Option<(Entity, Q::Owned<'w>)> {
        let index = next_index(&self.query, &mut self.word, &mut self.bits, &mut self.page)?;
        // SAFETY: the mask proves presence and next_index never repeats a slot.
        // Each returned guard splits the lease, so dropping this iterator is safe.
        Some((self.query.world.entity_at(index), unsafe {
            Q::owned(&self.query.state, index)
        }))
    }
}
#[inline]
fn next_index<Q: Query>(
    query: &QueryBorrow<'_, Q>,
    word: &mut usize,
    bits: &mut u64,
    page: &mut usize,
) -> Option<usize> {
    while *bits == 0 {
        if *word == query.words {
            return None;
        }
        let i = *word;
        *word += 1;
        *bits = query.world.alive_mask[i] & query.state.word(i);
        for &(mask, other, with) in &query.filters[..query.filter_count] {
            let filter = mask.map_or(0, |s| s.word(i)) | other.map_or(0, |s| s.word(i));
            *bits &= if with { filter } else { !filter };
        }
        // Once per visited page, outside the row loop. Optional columns may mark
        // conservatively; acquiring the query still bumps the world lease epoch.
        let next_page = i;
        if *bits != 0 && *page != next_page {
            query.state.mark_page(next_page);
            *page = next_page;
        }
    }
    let index = (*word - 1) * 64 + bits.trailing_zeros() as usize;
    *bits &= *bits - 1;
    Some(index)
}

/// A word-by-word mask join, borrowing its query's leases.
pub struct QueryIter<'a, 'w, Q: Query> {
    query: &'a mut QueryBorrow<'w, Q>,
    word: usize,
    bits: u64,
    page: usize,
}
impl<'a, Q: Query> Iterator for QueryIter<'a, '_, Q> {
    type Item = (Entity, Q::Item<'a>);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = next_index(self.query, &mut self.word, &mut self.bits, &mut self.page)?;
        // SAFETY: mask intersection proves presence, each index is yielded only once,
        // and the exclusive borrow of QueryBorrow keeps leases alive for every row.
        let item = unsafe { self.query.state.fetch(index) };
        Some((self.query.world.entity_at(index), item))
    }
}
