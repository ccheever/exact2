//! The only unsafe boundary: a lease protects an entire dense allocation, and
//! each query yields an entity at most once. Items own leases, so dropping the
//! iterator before its items cannot unlock the storage.
use crate::{Component, Data, DataError, Entity, Reader, World, Writer};
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

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
    _lease: Rc<Lease<'a>>,
    _life: PhantomData<&'a C>,
}
impl<C> Deref for Ref<'_, C> {
    type Target = C;
    fn deref(&self) -> &C {
        // SAFETY: the shared lease excludes writers; the world borrow keeps the allocation alive.
        unsafe { &*self.ptr }
    }
}
/// An exclusive component/resource borrow, independent of the iterator's lifetime.
pub struct RefMut<'a, C> {
    ptr: *mut C,
    _lease: Rc<Lease<'a>>,
    _life: PhantomData<&'a mut C>,
}
impl<C> Deref for RefMut<'_, C> {
    type Target = C;
    fn deref(&self) -> &C {
        // SAFETY: an exclusive lease and one yield per entity guarantee this pointer's validity.
        unsafe { &*self.ptr }
    }
}
impl<C> DerefMut for RefMut<'_, C> {
    fn deref_mut(&mut self) -> &mut C {
        // SAFETY: this noncloneable item is the only mutable reference to this row.
        unsafe { &mut *self.ptr }
    }
}

pub(crate) struct Storage<C> {
    sparse: Vec<u32>,
    entities: Vec<Entity>,
    values: UnsafeCell<Vec<C>>,
    borrowed: Cell<isize>,
}
impl<C: Component> Default for Storage<C> {
    fn default() -> Self {
        Self {
            sparse: vec![],
            entities: vec![],
            values: UnsafeCell::new(vec![]),
            borrowed: Cell::new(0),
        }
    }
}
impl<C: Component> Storage<C> {
    fn lease(&self, mutable: bool) -> Rc<Lease<'_>> {
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
        Rc::new(Lease {
            count: &self.borrowed,
            mutable,
        })
    }
    fn position(&self, e: Entity) -> Option<usize> {
        let p = *self.sparse.get(e.index() as usize)? as usize;
        (self.entities.get(p) == Some(&e)).then_some(p)
    }
    pub(crate) fn has(&self, e: Entity) -> bool {
        self.position(e).is_some()
    }
    fn repair(&mut self, start: usize) {
        for (p, e) in self.entities.iter().enumerate().skip(start) {
            self.sparse[e.index() as usize] = p as u32;
        }
    }
    pub(crate) fn insert(&mut self, e: Entity, c: C) {
        if let Some(p) = self.position(e) {
            self.values.get_mut()[p] = c;
            return;
        }
        assert!(
            self.entities.len() < u32::MAX as usize,
            "component storage exhausted"
        );
        let p = if self
            .entities
            .last()
            .is_none_or(|last| last.index() < e.index())
        {
            self.entities.len()
        } else {
            self.entities.partition_point(|v| v.index() < e.index())
        };
        self.sparse
            .resize(self.sparse.len().max(e.index() as usize + 1), u32::MAX);
        self.entities.insert(p, e);
        self.values.get_mut().insert(p, c);
        self.repair(p);
    }
    pub(crate) fn remove(&mut self, e: Entity) -> Option<C> {
        let p = self.position(e)?;
        self.sparse[e.index() as usize] = u32::MAX;
        self.entities.remove(p);
        let c = self.values.get_mut().remove(p);
        self.repair(p);
        Some(c)
    }
    fn ptr(&self) -> *mut C {
        // SAFETY: structural changes require &mut World. A caller acquires a lease
        // before dereferencing; obtaining a Vec pointer does not borrow its elements.
        unsafe { (*self.values.get()).as_mut_ptr() }
    }
    pub(crate) fn get(&self, e: Entity) -> Option<Ref<'_, C>> {
        let p = self.position(e)?;
        let lease = self.lease(false);
        Some(Ref {
            ptr: self.ptr().wrapping_add(p),
            _lease: lease,
            _life: PhantomData,
        })
    }
    pub(crate) fn get_mut(&self, e: Entity) -> Option<RefMut<'_, C>> {
        let p = self.position(e)?;
        let lease = self.lease(true);
        Some(RefMut {
            ptr: self.ptr().wrapping_add(p),
            _lease: lease,
            _life: PhantomData,
        })
    }
}

pub(crate) trait Erased {
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
    fn entities(&self) -> &[Entity];
    fn remove(&mut self, e: Entity);
    fn write(&self, w: &mut dyn Writer);
    fn read(&mut self, r: &mut dyn Reader, valid: &dyn Fn(Entity) -> bool)
        -> Result<(), DataError>;
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
    fn entities(&self) -> &[Entity] {
        &self.entities
    }
    fn remove(&mut self, e: Entity) {
        self.remove(e);
    }
    fn write(&self, w: &mut dyn Writer) {
        let _lease = self.lease(false);
        // SAFETY: the shared lease excludes mutable references for this entire walk.
        let values = unsafe { &*self.values.get() };
        w.begin_seq(self.entities.len());
        for (e, c) in self.entities.iter().zip(values) {
            w.item();
            w.begin_seq(2);
            w.item();
            e.write(w);
            w.item();
            c.write(w);
            w.end_seq();
        }
        w.end_seq();
    }
    fn read(
        &mut self,
        r: &mut dyn Reader,
        valid: &dyn Fn(Entity) -> bool,
    ) -> Result<(), DataError> {
        r.begin_seq()?;
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
            if self
                .entities
                .last()
                .is_some_and(|last| last.index() >= e.index())
            {
                return Err(DataError::new("entities are not strictly ordered"));
            }
            self.insert(e, c);
        }
        Ok(())
    }
}

mod sealed {
    pub trait Sealed {}
    impl<C: crate::Component> Sealed for &C {}
    impl<C: crate::Component> Sealed for &mut C {}
    impl<C: crate::Component> Sealed for Option<&C> {}
    impl<C: crate::Component> Sealed for Option<&mut C> {}
    pub(super) use Sealed as TupleSealed;
}

/// The built-in reference, optional-reference and tuple query forms.
///
/// The trait is sealed: an implementation must prove that its mutable references
/// are disjoint. Duplicate components are refused even when both uses are shared.
pub trait Query: sealed::Sealed {
    /// One row, whose borrow guards can outlive the iterator.
    type Item<'w>;
    /// Storage leases acquired once at query construction.
    #[doc(hidden)]
    type State<'w>: Fetch<'w, Item = Self::Item<'w>>;
    /// Acquire leases, checking duplicate types before iteration starts.
    #[doc(hidden)]
    fn prepare<'w>(world: &'w World, seen: &mut Vec<TypeId>) -> Self::State<'w>;
}

/// Internal query operation, exposed only because it is an associated bound.
#[doc(hidden)]
pub trait Fetch<'w> {
    type Item;
    fn candidates(&self) -> Option<&'w [Entity]>;
    /// # Safety
    /// Call at most once for each entity on this state, including through tuples.
    unsafe fn fetch(&self, e: Entity) -> Option<Self::Item>;
}

/// One storage lease; the const parameters keep the four reference forms small.
#[doc(hidden)]
pub struct QueryBorrow<'w, C, const MUT: bool, const OPTIONAL: bool> {
    storage: Option<&'w Storage<C>>,
    lease: Option<Rc<Lease<'w>>>,
    ptr: *mut C,
}
impl<'w, C: Component, const M: bool, const O: bool> QueryBorrow<'w, C, M, O> {
    fn new(world: &'w World, seen: &mut Vec<TypeId>) -> Self {
        let id = TypeId::of::<C>();
        assert!(!seen.contains(&id), "{} occurs twice in one query", C::NAME);
        seen.push(id);
        let storage = world.storage::<C>();
        let lease = storage.map(|s| s.lease(M));
        let ptr = storage.map_or(std::ptr::null_mut(), Storage::ptr);
        Self {
            storage,
            lease,
            ptr,
        }
    }
    fn candidate_slice(&self) -> Option<&'w [Entity]> {
        if O {
            None
        } else {
            Some(self.storage.map_or(&[], |s| &s.entities))
        }
    }
    fn shared(&self, e: Entity) -> Option<Ref<'w, C>> {
        let p = self.storage?.position(e)?;
        Some(Ref {
            ptr: self.ptr.wrapping_add(p),
            _lease: self.lease.as_ref()?.clone(),
            _life: PhantomData,
        })
    }
    // Only called by unsafe fetch, after the iterator has selected a fresh entity.
    fn exclusive(&self, e: Entity) -> Option<RefMut<'w, C>> {
        let p = self.storage?.position(e)?;
        Some(RefMut {
            ptr: self.ptr.wrapping_add(p),
            _lease: self.lease.as_ref()?.clone(),
            _life: PhantomData,
        })
    }
}
macro_rules! reference {
    ($form:ty, $m:literal, $o:literal, $item:ty, $fetch:expr) => {
        impl<C: Component> Query for $form {
            type Item<'w> = $item;
            type State<'w> = QueryBorrow<'w, C, $m, $o>;
            fn prepare<'w>(w: &'w World, seen: &mut Vec<TypeId>) -> Self::State<'w> {
                QueryBorrow::new(w, seen)
            }
        }
        impl<'w, C: Component> Fetch<'w> for QueryBorrow<'w, C, $m, $o> {
            type Item = $item;
            fn candidates(&self) -> Option<&'w [Entity]> {
                self.candidate_slice()
            }
            unsafe fn fetch(&self, e: Entity) -> Option<Self::Item> {
                ($fetch)(self, e)
            }
        }
    };
}
reference!(&C, false, false, Ref<'w, C>, |s: &Self, e| s.shared(e));
reference!(&mut C, true, false, RefMut<'w, C>, |s: &Self, e| s
    .exclusive(e));
reference!(
    Option<&C>,
    false,
    true,
    Option<Ref<'w, C>>,
    |s: &Self, e| Some(s.shared(e))
);
reference!(
    Option<&mut C>,
    true,
    true,
    Option<RefMut<'w, C>>,
    |s: &Self, e| Some(s.exclusive(e))
);

macro_rules! tuples {
    ($($T:ident:$i:tt),+) => {
        impl<$($T: Query),+> sealed::TupleSealed for ($($T,)+) {}
        impl<$($T: Query),+> Query for ($($T,)+) {
            type Item<'w> = ($($T::Item<'w>,)+);
            type State<'w> = ($($T::State<'w>,)+);
            fn prepare<'w>(w: &'w World, seen: &mut Vec<TypeId>) -> Self::State<'w> { ($($T::prepare(w, seen),)+) }
        }
        impl<'w, $($T: Fetch<'w>),+> Fetch<'w> for ($($T,)+) {
            type Item = ($($T::Item,)+);
            fn candidates(&self) -> Option<&'w [Entity]> {
                [$(self.$i.candidates(),)+].into_iter().flatten().min_by_key(|s| s.len())
            }
            unsafe fn fetch(&self, e: Entity) -> Option<Self::Item> {
                // SAFETY: query construction rejects duplicate components and the caller yields each entity once.
                unsafe { Some(($(self.$i.fetch(e)?,)+)) }
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

type Filter<'w> = Box<dyn Fn(Entity) -> bool + 'w>;
/// An entity-ordered join over the smallest required storage.
pub struct QueryIter<'w, Q: Query> {
    world: &'w World,
    state: Q::State<'w>,
    entities: Cow<'w, [Entity]>,
    cursor: usize,
    filters: Vec<Filter<'w>>,
}
impl<'w, Q: Query> QueryIter<'w, Q> {
    pub(crate) fn new(world: &'w World) -> Self {
        let state = Q::prepare(world, &mut vec![]);
        let entities = state
            .candidates()
            .map_or_else(|| Cow::Owned(world.entities().collect()), Cow::Borrowed);
        Self {
            world,
            state,
            entities,
            cursor: 0,
            filters: vec![],
        }
    }
    /// Keep entities carrying C, without borrowing its values.
    pub fn with<C: Component>(mut self) -> Self {
        let w = self.world;
        self.filters.push(Box::new(move |e| w.has::<C>(e)));
        self
    }
    /// Keep entities without C, without borrowing its values.
    pub fn without<C: Component>(mut self) -> Self {
        let w = self.world;
        self.filters.push(Box::new(move |e| !w.has::<C>(e)));
        self
    }
}
impl<'w, Q: Query> Iterator for QueryIter<'w, Q> {
    type Item = (Entity, Q::Item<'w>);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&e) = self.entities.get(self.cursor) {
            self.cursor += 1;
            if !self.filters.iter().all(|f| f(e)) {
                continue;
            }
            // SAFETY: sorted unique candidates are consumed exactly once; leases
            // exclude other writers and duplicate query components were rejected.
            if let Some(item) = unsafe { self.state.fetch(e) } {
                return Some((e, item));
            }
        }
        None
    }
}
