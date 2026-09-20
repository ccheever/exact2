use crate::storage::{self, Erased, Storage};
use crate::{
    bin, hash, Data, DataError, Now, Pages, Parent, Query, QueryBorrow, Reader, Ref, RefMut, Rng,
    Writer,
};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A slot and its incarnation; a recycled index never revives a stale entity.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Entity {
    index: u32,
    generation: u32,
}
impl Data for Entity {
    fn write(&self, w: &mut dyn Writer) {
        w.entity(self.index, self.generation);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field {
                "index" => self.index.read(r)?,
                "generation" => self.generation.read(r)?,
                _ => r.unknown()?,
            }
        }
        Ok(())
    }
}
impl Default for Entity {
    fn default() -> Self {
        Self {
            index: u32::MAX,
            generation: 0,
        }
    }
}
impl Entity {
    pub fn index(self) -> u32 {
        self.index
    }
    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// An entity handle or a name resolved in this world.
pub trait Target {
    /// Resolve a live entity without reviving a stale handle.
    fn entity(&self, world: &World) -> Option<Entity>;
}
impl Target for Entity {
    fn entity(&self, world: &World) -> Option<Entity> {
        world.contains(*self).then_some(*self)
    }
}
impl Target for &str {
    fn entity(&self, world: &World) -> Option<Entity> {
        world.resolve(self)
    }
}

/// Named per-entity Data, with unique names and no semantic interior mutability.
pub trait Component: Data {
    const NAME: &'static str;
    /// Register data this component produces, before restoring a saved world.
    fn register(_world: &mut World) -> Result<(), DataError> {
        Ok(())
    }
}
/// Named singleton Data; the same semantic immutability contract as Component.
pub trait Resource: Data {
    const NAME: &'static str;
    /// Exclude executor bookkeeping from observed rest.
    const AMBIENT: bool = false;
}

mod bundle_sealed {
    pub trait Sealed {}
    impl<C: super::Component> Sealed for C {}
    impl Sealed for () {}
}
pub trait Bundle: bundle_sealed::Sealed {
    fn preflight(&self, world: &World, entity: Entity) -> Result<usize, DataError>;
    fn insert(self, world: &mut World, entity: Entity) -> Result<(), DataError>;
}
impl<C: Component> Bundle for C {
    fn preflight(&self, w: &World, e: Entity) -> Result<usize, DataError> {
        w.registered::<C>(C::NAME, false)?;
        if let Some(parent) = (self as &dyn std::any::Any).downcast_ref::<Parent>() {
            w.check_parent(e, parent.entity())?;
            return Ok(2);
        }
        Ok(1)
    }
    fn insert(self, w: &mut World, e: Entity) -> Result<(), DataError> {
        w.insert(e, self).map(|_| ())
    }
}
impl Bundle for () {
    fn preflight(&self, _: &World, _: Entity) -> Result<usize, DataError> {
        Ok(0)
    }
    fn insert(self, _: &mut World, _: Entity) -> Result<(), DataError> {
        Ok(())
    }
}
macro_rules! bundles {
    ($($T:ident:$i:tt),+) => {
        impl<$($T: Bundle),+> bundle_sealed::Sealed for ($($T,)+) {}
        impl<$($T: Bundle),+> Bundle for ($($T,)+) {
            fn preflight(&self, w: &World, e: Entity) -> Result<usize, DataError> {
                Ok(0usize $(.saturating_add(self.$i.preflight(w, e)?))+)
            }
            fn insert(self, w: &mut World, e: Entity) -> Result<(), DataError> {
                $(self.$i.insert(w, e)?;)+ Ok(())
            }
        }
    };
}
bundles!(A:0);
bundles!(A:0, B:1);
bundles!(A:0, B:1, C:2);
bundles!(A:0, B:1, C:2, D:3);
bundles!(A:0, B:1, C:2, D:3, E:4);
bundles!(A:0, B:1, C:2, D:3, E:4, F:5);
bundles!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
bundles!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

#[derive(Default, Data)]
struct Slot {
    generation: u32,
    alive: bool,
    name: Option<String>,
}
#[derive(Default)]
struct State {
    tick: u64,
    hz: u32,
    seed: u64,
    slots: Vec<Slot>,
    free: Free,
    busy: RefCell<Vec<std::borrow::Cow<'static, str>>>,
    work: RefCell<BTreeMap<String, crate::Work>>,
}
type DerivedSlot = std::cell::OnceCell<(TypeId, RefCell<Box<dyn std::any::Any>>)>;
type StorageFactory = fn(&'static str, std::rc::Rc<std::cell::Cell<u64>>) -> Box<dyn Erased>;
#[derive(Clone, Copy)]
struct Registration {
    ordinal: u8,
    identity: fn() -> (TypeId, &'static str),
    make: Option<StorageFactory>,
    resource_size: usize,
    make_resource: Option<StorageFactory>,
    ambient: bool,
}

/// Retained, opaque identity for derived caches. Moves keep it; new worlds differ.
/// Holding a token prevents its identity from being recycled after the world drops.
#[derive(Clone, Debug)]
pub struct WorldId(std::rc::Rc<std::cell::Cell<u64>>);
impl PartialEq for WorldId {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for WorldId {}

/// Ordered simulation state, with dynamic storage borrows and no host clock.
pub struct World {
    id: WorldId,
    epoch: std::rc::Rc<std::cell::Cell<u64>>,
    state: State,
    // Derived lookup only; never serialized, hashed or observed.
    names: BTreeMap<String, BTreeSet<Entity>>,
    pub(crate) alive_mask: Vec<u64>,
    rng: storage::Singleton<Rng>,
    registry: BTreeMap<&'static str, Registration>,
    registering: bool,
    registration_before: Option<[u8; 256]>,
    components: BTreeMap<&'static str, Box<dyn Erased>>,
    resources: BTreeMap<&'static str, Box<dyn Erased>>,
    // Executor-owned derived data, populated only by linked callers; never saved.
    derived: [DerivedSlot; 64],
    journal: RefCell<VecDeque<crate::Event>>,
    session_journal: RefCell<VecDeque<(u64, crate::Event)>>,
    session_next: std::cell::Cell<u64>,
    journal_next: std::cell::Cell<u64>,
    pub(crate) published_pending: std::cell::Cell<bool>,
    pub(crate) published: RefCell<BTreeMap<std::rc::Rc<str>, crate::Published>>,
    published_cost: std::cell::Cell<usize>,
    pub(crate) messages: RefCell<Vec<String>>,
    entities_revision: u64,
    replacement: u64,
    changes: VecDeque<crate::Change>,
    consumers: Vec<std::rc::Weak<std::cell::Cell<u64>>>,
    change_next: u64,
    observed: Option<(u64, bool)>,
    ownership: RefCell<Vec<u8>>,
    owners: BTreeMap<Entity, BTreeSet<Entity>>,
    orphans: BTreeSet<Entity>,
    poisoned: bool,
    pub(crate) driver_owned: bool,
}
const SINGLETON: Entity = Entity {
    index: 0,
    generation: 0,
};
const MAGIC: &[u8; 8] = b"EXGAME\0\x04";

impl World {
    pub fn new(hz: u32, seed: u64) -> Self {
        assert!(hz > 0, "world hz must be positive");
        let epoch = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut rng = storage::Singleton::new("Rng", epoch.clone());
        rng.insert(Rng::new(seed));
        Self {
            id: WorldId(epoch.clone()),
            epoch,
            state: State {
                hz,
                seed,
                ..State::default()
            },
            alive_mask: vec![],
            names: BTreeMap::new(),
            rng,
            registry: BTreeMap::new(),
            registering: false,
            registration_before: None,
            components: BTreeMap::new(),
            resources: BTreeMap::new(),
            derived: std::array::from_fn(|_| std::cell::OnceCell::new()),
            journal: RefCell::new(VecDeque::new()),
            session_journal: RefCell::new(VecDeque::new()),
            session_next: std::cell::Cell::new(0),
            journal_next: std::cell::Cell::new(0),
            published_pending: std::cell::Cell::new(false),
            published: RefCell::new(BTreeMap::new()),
            published_cost: std::cell::Cell::new(0),
            messages: RefCell::new(Vec::new()),
            entities_revision: 0,
            replacement: 0,
            changes: VecDeque::new(),
            consumers: Vec::new(),
            change_next: 0,
            observed: None,
            ownership: RefCell::new(Vec::new()),
            owners: BTreeMap::new(),
            orphans: BTreeSet::new(),
            poisoned: false,
            driver_owned: false,
        }
    }
    pub fn id(&self) -> WorldId {
        self.id.clone()
    }
    pub fn register<C: Component>(&mut self) -> Result<&mut Self, DataError> {
        let existed = self.registry.contains_key(C::NAME);
        let reg = *self.registration::<C>(C::NAME)?;
        if reg.make.is_none() {
            let outer = self.registration_before.take();
            let registering = std::mem::replace(&mut self.registering, true);
            self.registry.get_mut(C::NAME).unwrap().make = Some(storage::make::<C>);
            let result = self.mutation(C::register);
            self.registering = registering;
            let before = self.registration_before.take();
            self.registration_before = outer;
            if let Some(before) = before.filter(|_| result.is_err()) {
                self.registry.retain(|_, r| {
                    let flags = before[r.ordinal as usize];
                    if flags & 2 == 0 {
                        r.make = None;
                    }
                    if flags & 4 == 0 {
                        r.make_resource = None;
                        r.resource_size = 0;
                        r.ambient = false;
                    }
                    flags != 0
                });
            }
            if let Err(error) = result {
                if existed {
                    self.registry.insert(C::NAME, reg);
                } else {
                    self.registry.remove(C::NAME);
                }
                return Err(error);
            }
        }
        Ok(self)
    }
    pub fn register_resource<R: Resource>(&mut self) -> Result<&mut Self, DataError> {
        let reg = self.registration::<R>(R::NAME)?;
        reg.make_resource = Some(storage::make_cell::<R>);
        reg.resource_size = 64usize.saturating_add(R::default_size());
        reg.ambient = R::AMBIENT;
        Ok(self)
    }
    fn registration<C: Data>(
        &mut self,
        name: &'static str,
    ) -> Result<&mut Registration, DataError> {
        self.healthy()?;
        if C::default_size() > crate::data::MAX_LOAD_BYTES {
            return Err(DataError::new(
                "declared storage admission exceeds load budget",
            ));
        }
        if name.is_empty() || name.len() > 256 {
            return Err(DataError::new("storage name must contain 1..=256 bytes"));
        }
        if self
            .registry
            .get(name)
            .is_some_and(|r| (r.identity)().0 != TypeId::of::<C>())
        {
            return Err(DataError::new(format!(
                "duplicate storage name: {} and {}",
                (self.registry[name].identity)().1,
                std::any::type_name::<C>()
            ))
            .at(name));
        }
        if !self.registry.contains_key(name) && self.registry.len() == 256 {
            return Err(DataError::new("storage type limit (256)").at(name));
        }
        // A declaration-only hook needs rollback state only when it declares dependencies.
        if self.registering && self.registration_before.is_none() {
            let mut before = [0u8; 256];
            for r in self.registry.values() {
                #[cfg(test)]
                registration_tests::VISITS.set(registration_tests::VISITS.get() + 1);
                before[r.ordinal as usize] = 1
                    | (u8::from(r.make.is_some()) * 2)
                    | (u8::from(r.make_resource.is_some()) * 4);
            }
            self.registration_before = Some(before);
        }
        let ordinal = self.registry.len() as u8;
        Ok(self.registry.entry(name).or_insert(Registration {
            ordinal,
            identity: || (TypeId::of::<C>(), std::any::type_name::<C>()),
            make: None,
            make_resource: None,
            resource_size: 0,
            ambient: false,
        }))
    }
    fn registered<C: Data>(&self, name: &str, resource: bool) -> Result<(), DataError> {
        self.healthy()?;
        if !self.registry.get(name).is_some_and(|r| {
            (r.identity)().0 == TypeId::of::<C>()
                && if resource {
                    r.make_resource.is_some()
                } else {
                    r.make.is_some()
                }
        }) {
            return Err(
                DataError::new("unregistered storage; declare it in Game::register").at(name),
            );
        }
        Ok(())
    }
    pub(crate) fn healthy(&self) -> Result<(), DataError> {
        if self.poisoned {
            Err(DataError::new("world poisoned by a panicking mutation"))
        } else {
            Ok(())
        }
    }
    pub(crate) fn mutation<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self))) {
            Ok(value) => value,
            Err(panic) => {
                self.poisoned = true;
                std::panic::resume_unwind(panic)
            }
        }
    }
    pub(crate) fn storage<C: Component>(&self) -> Option<&Storage<C>> {
        self.components.get(C::NAME)?.any().downcast_ref()
    }
    pub fn spawn(&mut self, bundle: impl Bundle) -> Result<Entity, DataError> {
        self.spawn_inner(None, bundle)
    }
    pub fn spawn_named(
        &mut self,
        name: impl AsRef<str>,
        bundle: impl Bundle,
    ) -> Result<Entity, DataError> {
        if name.as_ref().len() > 256 {
            return Err(DataError::new("entity name exceeds 256 bytes"));
        }
        self.spawn_inner(Some(name.as_ref().into()), bundle)
    }
    fn spawn_inner(
        &mut self,
        name: Option<String>,
        bundle: impl Bundle,
    ) -> Result<Entity, DataError> {
        let index = self
            .state
            .free
            .0
            .iter()
            .find(|&&i| self.state.slots[i as usize].generation != u32::MAX)
            .copied()
            .unwrap_or(self.state.slots.len() as u32);
        if index as usize >= crate::MAX_ENTITIES {
            return Err(DataError::new("entity slot limit (200000)"));
        }
        let e = Entity {
            index,
            generation: self
                .state
                .slots
                .get(index as usize)
                .map_or(0, |s| s.generation),
        };
        self.change_room(1usize.saturating_add(bundle.preflight(self, e)?))?;
        self.mutation(|this| this.spawn_commit(e, name, bundle))
    }
    fn spawn_commit(
        &mut self,
        e: Entity,
        name: Option<String>,
        bundle: impl Bundle,
    ) -> Result<Entity, DataError> {
        self.mutated();
        let index = e.index;
        if !self.state.free.0.remove(&index) {
            self.state.slots.push(Slot::default());
        }
        let slot = &mut self.state.slots[index as usize];
        slot.alive = true;
        slot.name = name;
        if let Some(name) = &slot.name {
            self.names.entry(name.clone()).or_default().insert(e);
        }
        let word = index as usize / 64;
        if word >= self.alive_mask.len() {
            self.alive_mask.resize(word + 1, 0);
        }
        self.alive_mask[word] |= 1 << (index % 64);
        self.entities_revision = self.entities_revision.wrapping_add(1);
        self.record_change(e, crate::ChangeKind::Spawn);
        bundle.insert(self, e)?;
        Ok(e)
    }
    /// Remove this entity only; descendants leave at the end of the tick.
    /// A panicking component destructor poisons the world; discard it afterward.
    pub fn despawn(&mut self, e: Entity) -> Result<bool, DataError> {
        if !self.contains(e) {
            return Ok(false);
        }
        self.change_room(1)?;
        let generation = e
            .generation
            .checked_add(1)
            .ok_or_else(|| DataError::new("entity generation exhausted"))?;
        Ok(self.mutation(|this| this.despawn_commit(e, generation)))
    }
    fn despawn_commit(&mut self, e: Entity, generation: u32) -> bool {
        if let Some(mut children) = self.owners.remove(&e) {
            self.orphans.append(&mut children);
        }
        let old = self.get::<Parent>(e).map(|p| p.entity());
        self.change_owner(e, old, None);
        self.mutated();
        for s in self.components.values_mut() {
            s.remove(e.index as usize);
        }
        let slot = &mut self.state.slots[e.index as usize];
        slot.generation = generation;
        slot.alive = false;
        if let Some(name) = slot.name.take() {
            let entries = self.names.get_mut(&name).unwrap();
            entries.remove(&e);
            if entries.is_empty() {
                self.names.remove(&name);
            }
        }
        self.alive_mask[e.index as usize / 64] &= !(1 << (e.index % 64));
        while self.alive_mask.last() == Some(&0) {
            self.alive_mask.pop();
        }
        self.state.free.0.insert(e.index);
        self.entities_revision = self.entities_revision.wrapping_add(1);
        self.record_change(e, crate::ChangeKind::Despawn);
        true
    }
    #[inline]
    pub fn contains(&self, e: Entity) -> bool {
        self.state
            .slots
            .get(e.index as usize)
            .is_some_and(|s| s.alive && s.generation == e.generation)
    }
    pub fn len(&self) -> usize {
        self.state.slots.len() - self.state.free.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive)
            .map(|(i, s)| Entity {
                index: i as u32,
                generation: s.generation,
            })
    }
    pub fn entity_at(&self, index: usize) -> Option<Entity> {
        self.state
            .slots
            .get(index)
            .filter(|slot| slot.alive)
            .map(|_| self.live_entity(index))
    }
    /// Internal walks already proved membership with the alive mask.
    #[inline]
    pub(crate) fn live_entity(&self, index: usize) -> Entity {
        Entity {
            index: index as u32,
            generation: self.state.slots[index].generation,
        }
    }
    pub fn named(&self, name: &str) -> Option<Entity> {
        self.names.get(name)?.first().copied()
    }
    pub fn name(&self, e: Entity) -> Option<&str> {
        if !self.contains(e) {
            return None;
        }
        self.state.slots[e.index as usize].name.as_deref()
    }
    pub fn resolve(&self, target: &str) -> Option<Entity> {
        let Some((name, index)) = target.rsplit_once('#') else {
            return self.named(target);
        };
        let Ok(index) = index.parse::<u32>() else {
            return self.named(target);
        };
        let s = self.state.slots.get(index as usize)?;
        let e = Entity {
            index,
            generation: s.generation,
        };
        (s.alive && (name.is_empty() || s.name.as_deref() == Some(name))).then_some(e)
    }
    pub fn insert<C: Component>(&mut self, e: Entity, c: C) -> Result<bool, DataError> {
        let count = c.preflight(self, e)?;
        if !self.contains(e) {
            return Err(DataError::new("stale entity"));
        }
        self.change_room(count)?;
        self.mutation(|this| this.insert_commit(e, c))
    }
    fn insert_commit<C: Component>(&mut self, e: Entity, c: C) -> Result<bool, DataError> {
        if let Some(parent) = (&c as &dyn std::any::Any).downcast_ref::<Parent>() {
            let old = self.get::<Parent>(e).map(|p| p.entity());
            self.change_owner(e, old, Some(parent.entity()));
            self.record_change(e, crate::ChangeKind::Reparent(Some(parent.entity())));
        }
        let kind = if self.has::<C>(e) {
            crate::ChangeKind::Replace(C::NAME.into())
        } else {
            crate::ChangeKind::Insert(C::NAME.into())
        };
        self.record_change(e, kind);
        self.components
            .entry(C::NAME)
            .or_insert_with(|| storage::make::<C>(C::NAME, self.epoch.clone()))
            .any_mut()
            .downcast_mut::<Storage<C>>()
            .unwrap()
            .insert(e.index as usize, c);
        Ok(true)
    }
    pub fn remove<C: Component>(&mut self, e: Entity) -> Result<Option<C>, DataError> {
        if !self.has::<C>(e) {
            return Ok(None);
        }
        self.change_room(1 + usize::from(TypeId::of::<C>() == TypeId::of::<Parent>()))?;
        self.record_change(e, crate::ChangeKind::Remove(C::NAME.into()));
        if TypeId::of::<C>() == TypeId::of::<Parent>() {
            let old = self.get::<Parent>(e).map(|p| p.entity());
            self.change_owner(e, old, None);
            self.record_change(e, crate::ChangeKind::Reparent(None));
        }
        Ok(self
            .components
            .get_mut(C::NAME)
            .and_then(|s| s.any_mut().downcast_mut::<Storage<C>>())
            .and_then(|s| s.remove(e.index as usize)))
    }
    pub fn has<C: Component>(&self, e: Entity) -> bool {
        self.contains(e) && self.storage::<C>().is_some_and(|s| s.has(e.index as usize))
    }
    /// Borrow one component immutably; conflicts panic with its name.
    pub fn get<C: Component>(&self, target: impl Target) -> Option<Ref<'_, C>> {
        let e = target.entity(self)?;
        if !self.contains(e) {
            return None;
        }
        self.storage::<C>()?.get(e.index as usize)
    }
    /// Borrow one component exclusively, locking the whole column.
    /// A nested get::<C> of another entity also panics; use a query for multiple rows.
    pub fn get_mut<C: Component>(&self, target: impl Target) -> Option<RefMut<'_, C>> {
        let e = target.entity(self)?;
        if !self.contains(e) {
            return None;
        }
        assert!(
            TypeId::of::<C>() != TypeId::of::<Parent>(),
            "use set_parent for ownership edits"
        );
        self.storage::<C>()?.get_mut(e.index as usize)
    }
    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        QueryBorrow::try_new(self).expect("query preparation")
    }
    /// Safe typed runs under a shared column lease, in entity order.
    pub fn pages<C: Component>(&self) -> Pages<'_, C> {
        Pages::new(self.storage::<C>())
    }
    pub fn revision<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, |s| s.revision())
    }
    pub fn membership<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, |s| s.membership())
    }
    pub fn entities_revision(&self) -> u64 {
        self.entities_revision
    }
    pub fn insert_resource<R: Resource>(&mut self, r: R) -> Result<(), DataError> {
        self.registered::<R>(R::NAME, true)?;
        self.mutation(|this| this.insert_resource_commit(r));
        Ok(())
    }
    fn insert_resource_commit<R: Resource>(&mut self, r: R) {
        self.resources
            .entry(R::NAME)
            .or_insert_with(|| storage::make_cell::<R>(R::NAME, self.epoch.clone()))
            .any_mut()
            .downcast_mut::<storage::Singleton<R>>()
            .unwrap()
            .insert(r);
    }
    fn resource_storage<R: Resource>(&self) -> Option<&storage::Singleton<R>> {
        self.resources.get(R::NAME)?.any().downcast_ref()
    }
    pub fn try_resource<R: Resource>(&self) -> Option<Ref<'_, R>> {
        self.resource_storage::<R>()?.get()
    }
    pub fn resource<R: Resource>(&self) -> Ref<'_, R> {
        self.try_resource::<R>()
            .unwrap_or_else(|| panic!("resource {} is absent", R::NAME))
    }
    /// Conservative resource-local write revision; compare within one replacement.
    pub fn resource_revision<R: Resource>(&self) -> Option<u64> {
        self.resource_storage::<R>()
            .map(storage::Singleton::revision)
    }
    pub fn resource_mut<R: Resource>(&self) -> RefMut<'_, R> {
        self.resource_storage::<R>()
            .and_then(storage::Singleton::get_mut)
            .unwrap_or_else(|| panic!("resource {} is absent", R::NAME))
    }
    pub fn tick(&self) -> u64 {
        self.state.tick
    }
    pub fn hz(&self) -> u32 {
        self.state.hz
    }
    pub fn rng(&self) -> RefMut<'_, Rng> {
        self.rng.get_mut().unwrap()
    }
    pub fn publish(&self, key: &str, value: impl Into<crate::Published>) -> Result<(), DataError> {
        let value = value.into();
        if key.len() > 256 {
            return Err(DataError::new("publication key exceeds 256 bytes"));
        }
        let mut p = self.published.borrow_mut();
        if p.get(key) == Some(&value) {
            return Ok(());
        }
        let cost = |v: &crate::Published| {
            let mut remaining = crate::json::LIMIT;
            v.validate(&mut remaining, 0)?;
            Ok::<_, DataError>(crate::json::LIMIT - remaining)
        };
        let total =
            self.published_cost.get() - p.get(key).map_or(0, |v| cost(v).unwrap()) + cost(&value)?;
        if total > crate::json::LIMIT || (!p.contains_key(key) && p.len() == 256) {
            return Err(DataError::new("publication bounds"));
        }
        self.change_room(1)?;
        self.commit_publication(&mut p, key, value);
        self.published_cost.set(total);
        self.published_pending.set(true);
        self.mutated();
        Ok(())
    }
    fn commit_publication(
        &self,
        p: &mut BTreeMap<std::rc::Rc<str>, crate::Published>,
        key: &str,
        value: crate::Published,
    ) {
        if p.get(key) == Some(&value) {
            return;
        }
        let key = p
            .get_key_value(key)
            .map_or_else(|| std::rc::Rc::<str>::from(key), |(k, _)| k.clone());
        self.event(crate::EventKind::Published(key.clone()))
            .expect("publication journal cursor");
        if let Some(stored) = p.get_mut(&key) {
            *stored = value;
        } else {
            p.insert(key, value);
        }
        self.published_pending.set(true);
        self.mutated();
    }
    /// Admit a complete publication update before committing any field or event.
    pub fn publish_batch(
        &self,
        values: BTreeMap<String, crate::Published>,
    ) -> Result<(), DataError> {
        if values.len() > 256 {
            return Err(DataError::new("publication key limit (256)"));
        }
        let current = self.published.borrow();
        let mut budget = crate::json::LIMIT;
        let mut count = current.len();
        let mut changes = 0;
        for (key, value) in &values {
            if key.len() > 256 {
                return Err(DataError::new("publication key exceeds 256 bytes"));
            }
            count += usize::from(!current.contains_key(key.as_str()));
            changes += usize::from(current.get(key.as_str()) != Some(value));
            value.validate(&mut budget, 0)?;
        }
        if count > 256 {
            return Err(DataError::new("publication key limit (256)"));
        }
        for (_, value) in current
            .iter()
            .filter(|(k, _)| !values.contains_key(k.as_ref()))
        {
            value.validate(&mut budget, 0)?;
        }
        self.change_room(changes)?;
        drop(current);
        let mut current = self.published.borrow_mut();
        for (key, value) in values {
            self.commit_publication(&mut current, &key, value);
        }
        self.published_cost.set(crate::json::LIMIT - budget);
        Ok(())
    }
    pub fn emit(&self, text: impl Into<String>) -> Result<(), DataError> {
        let text = text.into();
        let mut messages = self.messages.borrow_mut();
        if text.len() > 4096 || messages.len() >= 1024 {
            return Err(DataError::new("message queue limit (1024 x 4096 bytes)"));
        }
        messages.push(text);
        self.mutated();
        Ok(())
    }
    pub(crate) fn step_clock(&mut self) {
        self.mutated();
        self.state.tick = self
            .state
            .tick
            .checked_add(1)
            .expect("world clock exhausted");
    }

    pub(crate) fn write(&self, w: &mut dyn Writer, delivery: bool) {
        w.begin_struct();
        w.field("state");
        for slot in &self.state.slots {
            if w.stopped() {
                break;
            }
            if let Some(name) = &slot.name {
                w.claim_decoded(512usize.saturating_add(name.len()));
            }
        }
        self.state.write(w);
        w.field("rng");
        self.rng.get().unwrap().write(w);
        for (kind, storages) in [
            ("components", &self.components),
            ("resources", &self.resources),
        ] {
            w.field(kind);
            w.begin_struct();
            for (name, s) in storages.iter().filter(|(_, s)| s.len() != 0) {
                if w.stopped() {
                    break;
                }
                w.static_key(name);
                s.write(w, &|index| {
                    if kind == "resources" {
                        SINGLETON
                    } else {
                        self.entity_at(index).unwrap()
                    }
                });
            }
            w.end_struct();
        }
        if delivery && !self.messages.borrow().is_empty() {
            w.field("messages");
            self.messages.borrow().write(w);
        }
        w.end_struct();
    }
    pub fn hash(&self) -> Result<u64, DataError> {
        self.validate()?;
        let mut w = hash::Hasher::default();
        self.write(&mut w, false);
        w.finish()
    }
    pub fn save(&self) -> Result<Vec<u8>, DataError> {
        self.validate()?;
        let mut w = bin::Encoder::prefixed(MAGIC);
        self.write(&mut w, true);
        w.finish()
    }
    /// Atomically replace simulation state. Registered types survive the replacement;
    /// caches, publications and events do not. The entity table precedes storages.
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        self.load_in(bytes, None, false).map(|_| ())
    }
    pub fn carry(&mut self, bytes: &[u8]) -> Result<bool, DataError> {
        self.load_in(bytes, None, true)
    }
    pub(crate) fn load_in(
        &mut self,
        bytes: &[u8],
        budget: Option<&crate::data::limits::LoadBudget>,
        adapt: bool,
    ) -> Result<bool, DataError> {
        if self.driver_owned {
            return Err(DataError::new("restore the owning Sim, not its World"));
        }
        let (next, changed) = self.decoded(bytes, budget, adapt)?;
        self.adopt(next)?;
        Ok(changed)
    }
    fn decoded(
        &self,
        bytes: &[u8],
        budget: Option<&crate::data::LoadBudget>,
        adapt: bool,
    ) -> Result<(Self, bool), DataError> {
        let payload = Self::saved_payload(bytes)?;
        let mut next = Self::new(self.hz(), 0);
        next.registry = self.registry.clone();
        let mut r = bin::Decoder::for_load(payload, budget);
        next.read(&mut r).map_err(|e| e.at("World"))?;
        r.finish()?;
        let changed = next.save()? != bytes;
        if changed && !adapt {
            return Err(DataError::new(
                "exact save identity differs; use carry for schema adaptation",
            ));
        }
        next.epoch.set(self.epoch.get().wrapping_add(1));
        Ok((next, changed))
    }
    pub(crate) fn saved_payload(bytes: &[u8]) -> Result<&[u8], DataError> {
        if bytes.len() > crate::data::MAX_LOAD_BYTES {
            return Err(DataError::new("save exceeds load size limit"));
        }
        bytes.strip_prefix(MAGIC).ok_or_else(|| {
            DataError::new(format!(
                "unsupported world save format (expected EXGAME v4; saw {:02x?})",
                &bytes[..bytes.len().min(8)]
            ))
        })
    }
    fn validate_state(&self) -> Result<(), DataError> {
        if self.hz() == 0 || self.state.slots.len() > crate::MAX_ENTITIES {
            return Err(DataError::new("hz must be positive"));
        }
        if self
            .state
            .slots
            .iter()
            .any(|s| s.alive && s.generation == u32::MAX)
        {
            return Err(DataError::new("live entity generation exhausted"));
        }
        let work = self.state.work.borrow();
        if work.len() > 64
            || work
                .iter()
                .any(|(k, v)| k.len() > 256 || matches!(v, crate::Work::Failed(s) if s.len() > 256))
            || self.state.busy.borrow().len() > 64
            || self.state.busy.borrow().iter().any(|s| s.len() > 256)
            || self
                .state
                .slots
                .iter()
                .any(|s| s.name.as_ref().is_some_and(|n| n.len() > 256 || !s.alive))
        {
            return Err(DataError::new("world text/reason limit"));
        }
        let free = self
            .state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.alive)
            .map(|(i, _)| i as u32);
        if !free.eq(self.state.free.0.iter().copied()) {
            return Err(DataError::new("free list disagrees with entity table"));
        }
        Ok(())
    }
    pub(crate) fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        let mut seen = 0u8;
        while let Some(field) = r.field()? {
            seen |= match field {
                "state" => 1,
                "rng" => 2,
                "components" => 4,
                "resources" => 8,
                _ => 0,
            };
            match field {
                "state" => {
                    self.state.read(r)?;
                    self.validate_state()?;
                    let words = self.state.slots.len().div_ceil(64);
                    crate::data::limits::reserve(r, &mut self.alive_mask, words)?;
                    self.alive_mask.resize(words, 0);
                    self.names.clear();
                    for (index, slot) in self.state.slots.iter().enumerate() {
                        if slot.alive {
                            self.alive_mask[index / 64] |= 1 << (index % 64);
                            if let Some(name) = &slot.name {
                                r.claim(512usize.saturating_add(name.len()))?;
                                self.names.entry(name.clone()).or_default().insert(Entity {
                                    index: index as u32,
                                    generation: slot.generation,
                                });
                            }
                        }
                    }
                }
                "rng" => self.rng().read(r)?,
                "messages" => {
                    crate::data::limits::read_vec(r, self.messages.get_mut(), 1024)?;
                    if self.messages.get_mut().iter().any(|m| m.len() > 4096) {
                        return Err(DataError::new("message exceeds 4096 bytes"));
                    }
                }
                "components" | "resources" => {
                    if seen & 1 == 0 {
                        return Err(DataError::new("entity table must precede storage"));
                    }
                    r.begin_struct()?;
                    while let Some(name) = r.field()? {
                        if !self.registry.contains_key(name) {
                            match name {
                                "Parent" => {
                                    self.register::<Parent>()?;
                                }
                                "Ambient" => {
                                    self.register::<crate::Ambient>()?;
                                }
                                _ => {}
                            }
                        }
                        let (&key, reg) = self.registry.get_key_value(name).ok_or_else(|| {
                            DataError::new("unregistered storage; declare it in Game::register")
                                .at(name)
                        })?;
                        let resource = field == "resources";
                        let make = if resource {
                            r.claim(reg.resource_size).map_err(|e| e.at(name))?;
                            reg.make_resource
                        } else {
                            reg.make
                        };
                        let make = make.ok_or_else(|| {
                            DataError::new("storage kind differs; declare it in Game::register")
                                .at(name)
                        })?;
                        r.claim(1024)?;
                        let mut s = make(key, self.epoch.clone());
                        s.read(r, &|e| {
                            if resource {
                                e == SINGLETON
                            } else {
                                self.contains(e)
                            }
                        })
                        .map_err(|e| e.at(name))?;
                        if resource && s.len() != 1 {
                            return Err(DataError::new("resource must contain one value").at(name));
                        }
                        let dest = if resource {
                            &mut self.resources
                        } else {
                            &mut self.components
                        };
                        if dest.insert(key, s).is_some() {
                            return Err(DataError::new("duplicate storage").at(name));
                        }
                    }
                }
                _ => r.skip()?,
            }
        }
        if seen != 15 {
            return Err(DataError::new("incomplete world save"));
        }
        while self.alive_mask.last() == Some(&0) {
            self.alive_mask.pop();
        }
        self.rebuild_owners();
        Ok(())
    }
}

pub(crate) mod inspect;
pub(crate) mod journal;
pub(crate) mod ownership;
mod save;
use save::Free;

#[cfg(test)]
mod registration_tests {
    use super::*;
    thread_local! { pub(super) static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    #[derive(Default)]
    struct C<const N: usize>;
    impl<const N: usize> Data for C<N> {
        fn write(&self, w: &mut dyn Writer) {
            ().write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            ().read(r)
        }
    }
    impl<const N: usize> Component for C<N> {
        const NAME: &'static str = NAMES[N];
    }
    macro_rules! types {
        ($($n:literal),*) => {
            const NAMES: &[&str] = &[$(stringify!($n)),*];
            fn declare(w: &mut World, n: usize) {
                let calls: &[fn(&mut World)] = &[$(|w| { w.register::<C<$n>>().unwrap(); }),*];
                for call in &calls[..n] { call(w); }
            }
        };
    }
    types!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
        48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
        71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93,
        94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112,
        113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130,
        131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148,
        149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166,
        167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184,
        185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202,
        203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220,
        221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238,
        239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255
    );
    #[test]
    fn empty_registration_hooks_do_linear_rollback_work() {
        let mut linear = true;
        for n in [16, 64, 128, 256] {
            let mut w = World::new(60, 0);
            VISITS.set(0);
            let (_, allocations) = crate::counting::measure(|| declare(&mut w, n));
            let visits = VISITS.get();
            println!(
                "registration {n}: {visits} snapshot visits; {allocations:?} allocations/bytes"
            );
            assert_eq!(w.registry.len(), n);
            linear &= visits <= n;
        }
        assert!(
            linear,
            "empty hooks must not scan previously declared types"
        );
    }
}
