use super::lease::{Filter, Registration, Shape, Term};
use super::{At, Lease, Leases, RawStorage, Ref, RefMut, Storage};
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
    /// Guarded rows from consuming iteration; each guard keeps the query's rows leased.
    type Owned<'w>;
    /// Human-readable component names.
    fn names() -> String;
    /// # Safety
    /// Mark its page first. The index must match, and may be yielded only once
    /// while this state lives; `lease` is the registered query it matched.
    #[doc(hidden)]
    unsafe fn owned<'w>(
        state: &Self::State<'w>,
        index: usize,
        lease: &QueryLease<'w>,
    ) -> Self::Owned<'w>;
    /// Storages resolved once at query construction.
    #[doc(hidden)]
    type State<'w>: for<'a> Fetch<Item<'a> = Self::Item<'a>>;
    /// Resolve storages and reject duplicate component types.
    #[doc(hidden)]
    fn prepare<'w>(world: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self::State<'w>;
}

/// Internal query operations, exposed only as an associated bound.
#[doc(hidden)]
pub trait Fetch {
    type Item<'a>;
    fn words(&self) -> usize;
    fn mark_page(&self, page: usize);
    fn word(&self, word: usize) -> u64;
    /// Describe each term's column for the query's registered lease.
    fn shape(&self, shape: &mut Shape);
    /// # Safety
    /// The index must pass this fetch's mask. Call at most once per index within
    /// an exclusive borrow of the state lasting at least 'a, while the query's lease
    /// is registered, and call mark_page for this index's page before fetching.
    unsafe fn fetch<'a>(&self, index: usize) -> Self::Item<'a>;
}

/// One registered query's lease, split into each row guard of consuming iteration.
#[doc(hidden)]
pub struct QueryLease<'w> {
    leases: &'w Leases,
    query: Registration<'w>,
}
impl<'w> QueryLease<'w> {
    #[inline]
    fn split(&self) -> Lease<'w> {
        self.leases.split(self.query);
        Lease::Query {
            leases: self.leases,
            query: self.query,
        }
    }
}

#[doc(hidden)]
pub struct ComponentBorrow<'w, C, const MUT: bool, const OPTIONAL: bool> {
    storage: Option<&'w Storage<C>>,
    page: Cell<*mut C>,
}
impl<'w, C: Component, const M: bool, const O: bool> ComponentBorrow<'w, C, M, O> {
    fn new(world: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self {
        world.sim_reads::<C>();
        let id = TypeId::of::<C>();
        assert!(
            !seen.contains(&Some(id)),
            "{} occurs twice in one query",
            C::NAME
        );
        let slot = seen
            .iter_mut()
            .find(|s| s.is_none())
            .unwrap_or_else(|| panic!("query exceeds 8 terms at {}", C::NAME));
        *slot = Some(id);
        let storage = world.storage::<C>();
        if M && !C::PRESENTATION {
            world.sim_writes(format_args!("queried `{}` mutably", C::NAME));
        }
        if M {
            // A mutable query is a write generation from construction, as before.
            storage.inspect(|s| s.edited());
        }
        Self {
            storage,
            page: Cell::new(std::ptr::null_mut()),
        }
    }
    fn required_words(&self) -> usize {
        if O {
            usize::MAX
        } else {
            self.storage.map_or(0, |s| s.mask.len())
        }
    }
    fn required_word(&self, word: usize) -> u64 {
        if O {
            u64::MAX
        } else {
            self.storage
                .and_then(|s| s.mask.get(word))
                .copied()
                .unwrap_or(0)
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
    // A mutable term records each row it hands out for `World::changed`.
    fn touch(&self, index: usize) {
        if M {
            if let Some(s) = self.storage {
                s.mark_row(index);
            }
        }
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
            unsafe fn owned<'w>(
                state: &Self::State<'w>,
                index: usize,
                lease: &QueryLease<'w>,
            ) -> Self::Owned<'w> {
                state.touch(index);
                let make = || $guard {
                    ptr: state.ptr(index),
                    _lease: lease.split(),
                    _life: PhantomData,
                };
                owned_row!($o, state, index, make)
            }
            type State<'w> = ComponentBorrow<'w, C, $m, $o>;
            fn prepare<'w>(w: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self::State<'w> {
                ComponentBorrow::new(w, seen)
            }
        }
        impl<C: Component> Fetch for ComponentBorrow<'_, C, $m, $o> {
            type Item<'a> = $item;
            fn mark_page(&self, page: usize) {
                if let Some(s) = self.storage {
                    if $m {
                        // A fresh revision per visited page: rows handed out now
                        // compare newer than any revision read before iteration.
                        s.edited();
                        s.mark_page(page);
                    }
                    self.page.set(
                        s.pages
                            .get(page)
                            .and_then(Option::as_ref)
                            .map_or(std::ptr::null_mut(), |slots| slots.get().cast::<C>()),
                    );
                }
            }
            fn words(&self) -> usize {
                self.required_words()
            }
            fn word(&self, word: usize) -> u64 {
                self.required_word(word)
            }
            fn shape(&self, shape: &mut Shape) {
                shape.push_term(Term {
                    column: self
                        .storage
                        .map_or(std::ptr::null(), |s| &s.raw as *const RawStorage),
                    name: C::NAME,
                    mutable: $m,
                    optional: $o,
                });
            }
            unsafe fn fetch<'a>(&self, $i: usize) -> $item {
                let $s = self;
                $s.touch($i);
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
            unsafe fn owned<'w>(
                state: &Self::State<'w>,
                index: usize,
                lease: &QueryLease<'w>,
            ) -> Self::Owned<'w> {
                // SAFETY: the caller yields each matched index once; construction rejects aliases.
                unsafe { ($($T::owned(&state.$i, index, lease),)+) }
            }
            type State<'w> = ($($T::State<'w>,)+);
            fn prepare<'w>(w: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self::State<'w> {
                ($($T::prepare(w, seen),)+)
            }
        }
        impl<$($T: Fetch),+> Fetch for ($($T,)+) {
            type Item<'a> = ($($T::Item<'a>,)+);
            fn mark_page(&self, page: usize) { $(self.$i.mark_page(page);)+ }
            fn words(&self) -> usize { usize::MAX $(.min(self.$i.words()))+ }
            fn word(&self, word: usize) -> u64 { u64::MAX $(& self.$i.word(word))+ }
            fn shape(&self, shape: &mut Shape) { $(self.$i.shape(shape);)+ }
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

/// An entity-ordered query. Its first iteration leases exactly the rows it matches
/// (after filters) in each of its components, until the query and every row guard
/// escaped from it drop. Other rows of the same components stay free to borrow.
/// Iteration borrows this object exclusively; rows may outlive an iterator but
/// cannot outlive the lease or overlap a second iteration.
///
/// ```compile_fail
/// use exact_game::{World, Transform};
/// let mut world = World::new(60, 0);
/// world.spawn((Transform::default(),));
/// let mut query = world.query::<&mut Transform>();
/// let row = query.iter().next().unwrap().1;
/// drop(query);
/// row.position.x = 1.0;
/// ```
///
/// ```compile_fail
/// use exact_game::{World, Transform};
/// let world = World::new(60, 0);
/// let mut query = world.query::<&mut Transform>();
/// let row = query.iter().next().unwrap().1;
/// let again = query.iter().next().unwrap().1;
/// row.position = again.position;
/// ```
pub struct QueryBorrow<'w, Q: Query> {
    world: &'w World,
    state: Q::State<'w>,
    filters: [(&'w [u64], &'w [u64], bool); 4],
    leased: [Filter; 4],
    filter_count: usize,
    words: usize,
    at: At,
    registered: Option<Registration<'w>>,
}
fn raw<C: Component>(world: &World) -> (&[u64], *const RawStorage) {
    world.sim_reads::<C>();
    world.storage::<C>().map_or((&[], std::ptr::null()), |s| {
        (&s.mask, &s.raw as *const RawStorage)
    })
}
impl<'w, Q: Query> QueryBorrow<'w, Q> {
    pub(crate) fn new(world: &'w World, at: At) -> Self {
        let state = Q::prepare(world, &mut [None; 8]);
        let words = state.words().min(world.alive_mask.len());
        let none = Filter {
            a: std::ptr::null(),
            b: std::ptr::null(),
            with: false,
            names: ("", ""),
        };
        Self {
            world,
            state,
            filters: [(&[], &[], false); 4],
            leased: [none; 4],
            filter_count: 0,
            words,
            at,
            registered: None,
        }
    }
    /// Keep entities carrying C, without borrowing its values.
    pub fn with<C: Component>(mut self) -> Self {
        let (mask, column) = raw::<C>(self.world);
        self.words = self.words.min(mask.len());
        self.filter::<C>(mask, column, true);
        self
    }
    /// Keep the union of A/B membership, without borrowing their values.
    pub fn with_any<A: Component, B: Component>(mut self) -> Self {
        let (a, column) = raw::<A>(self.world);
        let (b, other) = raw::<B>(self.world);
        self.words = self.words.min(a.len().max(b.len()));
        self.filter::<A>(a, column, true);
        let last = self.filter_count - 1;
        self.filters[last].1 = b;
        self.leased[last].b = other;
        self.leased[last].names.1 = B::NAME;
        self.refilter();
        self
    }
    /// Keep entities without C, without borrowing its values.
    pub fn without<C: Component>(mut self) -> Self {
        let (mask, column) = raw::<C>(self.world);
        self.filter::<C>(mask, column, false);
        self
    }
    fn filter<C: Component>(&mut self, mask: &'w [u64], column: *const RawStorage, with: bool) {
        assert!(
            self.filter_count < 4,
            "query exceeds 4 filters at {}",
            C::NAME
        );
        self.filters[self.filter_count] = (mask, &[], with);
        self.leased[self.filter_count] = Filter {
            a: column,
            b: std::ptr::null(),
            with,
            names: (C::NAME, ""),
        };
        self.filter_count += 1;
        self.refilter();
    }
    // A filter only narrows the rows a registered lease holds.
    fn refilter(&self) {
        if let Some(query) = self.registered {
            let filters = &self.leased[..self.filter_count];
            self.world.leases.refilter(query.slot, filters);
        }
    }
    // Register the lease before the first reference or guard is handed out. It
    // starts at the first iteration, so filters added by the builder narrow it.
    // @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
    fn lease(&mut self) -> QueryLease<'w> {
        let query = if let Some(query) = self.registered {
            query
        } else {
            let (state, filters) = (&self.state, &self.leased[..self.filter_count]);
            let query = self
                .world
                .leases
                .register(self.at, |shape| {
                    state.shape(shape);
                    shape.set_filters(filters);
                })
                .unwrap_or_else(|conflict| self.world.refuse(conflict));
            self.registered = Some(query);
            query
        };
        QueryLease {
            leases: &self.world.leases,
            query,
        }
    }
    /// The sole item, or None. Multiple matches are refused in every build.
    pub fn one(&mut self) -> Option<Q::Item<'_>> {
        let mut rows = self.iter();
        let (_, first) = rows.next()?;
        let count = 1 + rows.count();
        assert!(count == 1, "expected one {}, found {}", Q::names(), count);
        Some(first)
    }
    /// Visit each matching entity once, yielding plain references.
    pub fn iter(&mut self) -> QueryIter<'_, 'w, Q> {
        self.lease();
        QueryIter {
            query: self,
            word: 0,
            bits: 0,
            page: usize::MAX,
        }
    }
}
impl<Q: Query> Drop for QueryBorrow<'_, Q> {
    fn drop(&mut self) {
        if let Some(query) = self.registered {
            self.world.leases.release(query);
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

/// An owning iterator of guarded items, keeping the lease alive even if a row
/// escapes the loop.
pub struct QueryRows<'w, Q: Query> {
    query: QueryBorrow<'w, Q>,
    lease: QueryLease<'w>,
    word: usize,
    bits: u64,
    page: usize,
}
impl<'w, Q: Query> IntoIterator for QueryBorrow<'w, Q> {
    type Item = Q::Owned<'w>;
    type IntoIter = QueryRows<'w, Q>;
    fn into_iter(mut self) -> Self::IntoIter {
        QueryRows {
            lease: self.lease(),
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
        let index = next_index(&self.query, &mut self.word, &mut self.bits, &mut self.page)?;
        // SAFETY: the mask proves presence and next_index never repeats a slot.
        // Each returned guard splits the registered lease, so dropping this
        // iterator keeps escaped rows leased.
        Some(unsafe { Q::owned(&self.query.state, index, &self.lease) })
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
            let filter = mask.get(i).copied().unwrap_or(0) | other.get(i).copied().unwrap_or(0);
            *bits &= if with { filter } else { !filter };
        }
        // Once per visited page, outside the row loop. Optional columns may mark
        // conservatively; acquiring the query still bumps the world lease epoch.
        let next_page = i / super::WORDS;
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
        // and the exclusive borrow of QueryBorrow keeps its registered lease (taken
        // by iter) alive for every row.
        let item = unsafe { self.query.state.fetch(index) };
        Some((self.query.world.entity_at(index), item))
    }
}
