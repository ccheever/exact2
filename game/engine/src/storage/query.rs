use super::{Lease, Storage};
use crate::{Component, Entity, World};
use std::any::TypeId;

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
    /// Storage leases acquired once at query construction.
    #[doc(hidden)]
    type State<'w>: for<'a> Fetch<Item<'a> = Self::Item<'a>>;
    /// Acquire leases and reject duplicate component types.
    #[doc(hidden)]
    fn prepare<'w>(world: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self::State<'w>;
}

/// Internal query operations, exposed only as an associated bound.
#[doc(hidden)]
pub trait Fetch {
    type Item<'a>;
    fn words(&self) -> usize;
    fn word(&self, word: usize) -> u64;
    /// # Safety
    /// The index must pass this fetch's mask. Call at most once per index within
    /// an exclusive borrow of the state lasting at least 'a; keep its leases alive.
    unsafe fn fetch<'a>(&self, index: usize) -> Self::Item<'a>;
}

#[doc(hidden)]
pub struct ComponentBorrow<'w, C, const MUT: bool, const OPTIONAL: bool> {
    storage: Option<&'w Storage<C>>,
    _lease: Option<Lease<'w>>,
}
impl<'w, C: Component, const M: bool, const O: bool> ComponentBorrow<'w, C, M, O> {
    fn new(world: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self {
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
        Self {
            storage,
            _lease: storage.map(|s| s.lease(M, world.tick())),
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
        self.storage.unwrap().ptr(index)
    }
    fn has(&self, index: usize) -> bool {
        self.storage.is_some_and(|s| s.has(index))
    }
}
macro_rules! reference {
    ($form:ty, $m:literal, $o:literal, $item:ty, $s:ident, $i:ident, $fetch:expr) => {
        impl<'q, C: Component> Query for $form {
            type Item<'a> = $item;
            type State<'w> = ComponentBorrow<'w, C, $m, $o>;
            fn prepare<'w>(w: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self::State<'w> {
                ComponentBorrow::new(w, seen)
            }
        }
        impl<C: Component> Fetch for ComponentBorrow<'_, C, $m, $o> {
            type Item<'a> = $item;
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
reference!(&'q C, false, false, &'a C, s, i, &*s.ptr(i));
reference!(&'q mut C, true, false, &'a mut C, s, i, &mut *s.ptr(i));
reference!(
    Option<&'q C>,
    false,
    true,
    Option<&'a C>,
    s,
    i,
    if s.has(i) { Some(&*s.ptr(i)) } else { None }
);
reference!(
    Option<&'q mut C>,
    true,
    true,
    Option<&'a mut C>,
    s,
    i,
    if s.has(i) { Some(&mut *s.ptr(i)) } else { None }
);

macro_rules! tuples {
    ($($T:ident:$i:tt),+) => {
        impl<$($T: Query),+> sealed::Sealed for ($($T,)+) {}
        impl<$($T: Query),+> Query for ($($T,)+) {
            type Item<'a> = ($($T::Item<'a>,)+);
            type State<'w> = ($($T::State<'w>,)+);
            fn prepare<'w>(w: &'w World, seen: &mut [Option<TypeId>; 8]) -> Self::State<'w> {
                ($($T::prepare(w, seen),)+)
            }
        }
        impl<$($T: Fetch),+> Fetch for ($($T,)+) {
            type Item<'a> = ($($T::Item<'a>,)+);
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
    filters: [(&'w [u64], bool); 4],
    filter_count: usize,
    words: usize,
}
impl<'w, Q: Query> QueryBorrow<'w, Q> {
    pub(crate) fn new(world: &'w World) -> Self {
        let state = Q::prepare(world, &mut [None; 8]);
        let words = state.words().min(world.alive_mask.len());
        Self {
            world,
            state,
            filters: [(&[], false); 4],
            filter_count: 0,
            words,
        }
    }
    /// Keep entities carrying C, without borrowing its values.
    pub fn with<C: Component>(mut self) -> Self {
        let mask = self.world.storage::<C>().map_or(&[][..], |s| &s.mask);
        self.words = self.words.min(mask.len());
        self.filter::<C>(mask, true);
        self
    }
    /// Keep entities without C, without borrowing its values.
    pub fn without<C: Component>(mut self) -> Self {
        let mask = self.world.storage::<C>().map_or(&[][..], |s| &s.mask);
        self.filter::<C>(mask, false);
        self
    }
    fn filter<C: Component>(&mut self, mask: &'w [u64], with: bool) {
        assert!(
            self.filter_count < 4,
            "query exceeds 4 filters at {}",
            C::NAME
        );
        self.filters[self.filter_count] = (mask, with);
        self.filter_count += 1;
    }
    /// The first row, or None; debug builds refuse a second row, naming the query.
    /// This lives on the query so its column leases outlive the returned references.
    pub fn one(&mut self) -> Option<(Entity, Q::Item<'_>)> {
        let mut rows = self.iter();
        let first = rows.next();
        debug_assert!(
            rows.next().is_none(),
            "query {} expected one entity, found two",
            std::any::type_name::<Q>()
        );
        first
    }
    /// Visit each matching entity once, yielding plain references.
    pub fn iter(&mut self) -> QueryIter<'_, 'w, Q> {
        QueryIter {
            query: self,
            word: 0,
            bits: 0,
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

/// A word-by-word mask join, borrowing its query's leases.
pub struct QueryIter<'a, 'w, Q: Query> {
    query: &'a mut QueryBorrow<'w, Q>,
    word: usize,
    bits: u64,
}
impl<'a, Q: Query> Iterator for QueryIter<'a, '_, Q> {
    type Item = (Entity, Q::Item<'a>);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.bits == 0 {
            if self.word == self.query.words {
                return None;
            }
            let word = self.word;
            self.word += 1;
            let mut bits = self.query.world.alive_mask[word] & self.query.state.word(word);
            for &(mask, with) in &self.query.filters[..self.query.filter_count] {
                let filter = mask.get(word).copied().unwrap_or(0);
                bits &= if with { filter } else { !filter };
            }
            self.bits = bits;
        }
        let index = (self.word - 1) * 64 + self.bits.trailing_zeros() as usize;
        self.bits &= self.bits - 1;
        // SAFETY: mask intersection proves presence, each index is yielded only once,
        // and the exclusive borrow of QueryBorrow keeps leases alive for every row.
        let item = unsafe { self.query.state.fetch(index) };
        Some((self.query.world.entity_at(index), item))
    }
}
