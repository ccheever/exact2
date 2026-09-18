use crate::storage::{self, Erased, Storage};
use crate::{
    bin, hash, Data, DataError, Now, Pages, Parent, Query, QueryBorrow, Reader, Ref, RefMut, Rng,
    Value, Writer,
};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A slot and its incarnation; a recycled index never revives a stale entity.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Data)]
pub struct Entity {
    index: u32,
    generation: u32,
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
    /// The stable ordering key, also used in agent targets such as #12.
    pub fn index(self) -> u32 {
        self.index
    }
    /// The incarnation of this slot.
    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// An entity handle or a name resolved in this world.
pub trait Target {
    /// Resolve a live entity, without reviving a stale handle.
    fn entity(self, world: &World) -> Option<Entity>;
}
impl Target for Entity {
    fn entity(self, world: &World) -> Option<Entity> {
        world.contains(self).then_some(self)
    }
}
impl Target for &str {
    fn entity(self, world: &World) -> Option<Entity> {
        world.resolve(self)
    }
}

/// A named kind of per-entity data. Names must be unique within a world.
/// Semantic state has no interior mutability; derives introduce none. A manual
/// implementation that mutates semantic state through a shared reference is outside
/// the [`Data`] contract: quiescence and the hash cache are undefined for it.
pub trait Component: Data {
    /// Stable save-file and agent spelling.
    const NAME: &'static str;
}
/// World-owned singleton data, named by the Resource derive.
/// Semantic state has no interior mutability; derives introduce none. A manual
/// implementation that mutates semantic state through a shared reference is outside
/// the [`Data`] contract: quiescence and the hash cache are undefined for it.
///
/// ```compile_fail
/// use exact_game::{World, Transform};
/// World::new(60, 0).resource::<Transform>();
/// ```
pub trait Resource: Data {
    /// Stable save-file and agent spelling.
    const NAME: &'static str;
    /// Exclude executor bookkeeping from observed rest.
    const AMBIENT: bool = false;
}

/// One component or a tuple of components supplied to spawn.
pub trait Bundle {
    /// Insert this bundle into an existing entity.
    fn insert(self, world: &mut World, entity: Entity);
}
impl<C: Component> Bundle for C {
    fn insert(self, w: &mut World, e: Entity) {
        w.insert(e, self);
    }
}
impl Bundle for () {
    fn insert(self, _: &mut World, _: Entity) {}
}
macro_rules! bundles {
    ($($T:ident:$i:tt),+) => {
        impl<$($T: Component),+> Bundle for ($($T,)+) {
            fn insert(self, w: &mut World, e: Entity) { $(w.insert(e, self.$i);)+ }
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
}
#[derive(Clone, Copy)]
struct Registration {
    id: TypeId,
    make: fn(&'static str, std::rc::Rc<std::cell::Cell<u64>>) -> Box<dyn Erased>,
    resource_size: usize,
    make_resource: fn(&'static str, std::rc::Rc<std::cell::Cell<u64>>) -> Box<dyn Erased>,
    ambient: bool,
}

/// One journal event. Reads never generate per-tick samples.
#[derive(Clone, Debug, Default, Data)]
pub struct Event {
    /// Monotonically increasing journal cursor.
    pub index: u64,
    /// Simulation tick at the event.
    pub tick: u64,
    /// Simulation seconds at the event.
    pub seconds: f64,
    /// Human-readable event text.
    pub line: String,
}

/// Retained, opaque identity for derived caches. Moves keep it; new worlds differ.
/// Holding a token prevents its identity from being recycled after the world drops.
#[derive(Clone, Debug)]
pub struct WorldId(std::rc::Rc<()>);
impl PartialEq for WorldId {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for WorldId {}

/// Ordered simulation state, with dynamic storage borrows and no host clock.
pub struct World {
    id: WorldId,
    pub(crate) changing: Vec<String>,
    pub(crate) observation: ObservationState,
    epoch: std::rc::Rc<std::cell::Cell<u64>>,
    observed_epoch: u64,
    hash_cache: std::cell::Cell<Option<(u64, u64)>>,
    hash_prefix: RefCell<Option<(u64, hash::Hasher)>>,
    // Executor phase, never saved: audio authored in a tick starts at its end.
    pub(crate) in_tick: bool,
    state: State,
    pub(crate) alive_mask: Vec<u64>,
    rng: storage::Singleton<Rng>,
    registry: BTreeMap<&'static str, Registration>,
    components: BTreeMap<&'static str, Box<dyn Erased>>,
    resources: BTreeMap<&'static str, Box<dyn Erased>>,
    journal: RefCell<VecDeque<Event>>,
    journal_next: std::cell::Cell<u64>,
    pub(crate) published_pending: std::cell::Cell<bool>,
    published: RefCell<BTreeMap<String, Value>>,
    pub(crate) messages: RefCell<Vec<String>>,
    pub(crate) hierarchy: crate::scene::Hierarchy,
    pub(crate) fresh: Vec<Entity>,
    orphans: Vec<Entity>,
    entities_revision: u64,
    pub(crate) presentation_generation: u64,
}
const SINGLETON: Entity = Entity {
    index: 0,
    generation: 0,
};
const MAGIC: &[u8; 8] = b"EXGAME\0\x03";

impl World {
    /// Start at tick zero. A zero tick rate is a programmer error.
    pub fn new(hz: u32, seed: u64) -> Self {
        assert!(hz > 0, "world hz must be positive");
        let epoch = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut rng = storage::Singleton::new("Rng", epoch.clone());
        rng.insert(Rng::new(seed));
        Self {
            id: WorldId(std::rc::Rc::new(())),
            epoch,
            observed_epoch: 0,
            hash_cache: std::cell::Cell::new(None),
            hash_prefix: RefCell::new(None),
            changing: Vec::new(),
            observation: ObservationState::Unknown,
            in_tick: false,
            state: State {
                hz,
                seed,
                ..State::default()
            },
            alive_mask: vec![],
            rng,
            registry: BTreeMap::new(),
            components: BTreeMap::new(),
            resources: BTreeMap::new(),
            journal: RefCell::new(VecDeque::new()),
            journal_next: std::cell::Cell::new(0),
            published_pending: std::cell::Cell::new(false),
            published: RefCell::new(BTreeMap::new()),
            messages: RefCell::new(Vec::new()),
            hierarchy: crate::scene::Hierarchy::default(),
            fresh: vec![],
            orphans: vec![],
            entities_revision: 0,
            presentation_generation: 0,
        }
    }
    /// Identity of this world instance, excluded from saves and hashes.
    pub fn id(&self) -> WorldId {
        self.id.clone()
    }
    /// Replacement epoch for presentation caches, excluded from saves and hashes.
    pub fn presentation_generation(&self) -> u64 {
        self.presentation_generation
    }
    /// Register a component before loading. Registration itself is not state.
    pub fn register<C: Component>(&mut self) -> &mut Self {
        self.register_data::<C>(C::NAME, false)
    }
    /// Register singleton data before loading a save.
    pub fn register_resource<R: Resource>(&mut self) -> &mut Self {
        self.register_data::<R>(R::NAME, R::AMBIENT)
    }
    fn register_data<C: Data>(&mut self, name: &'static str, ambient: bool) -> &mut Self {
        let id = TypeId::of::<C>();
        if let Some(old) = self.registry.get(name) {
            assert_eq!(old.id, id, "duplicate component name {}", name);
        } else {
            self.registry.insert(
                name,
                Registration {
                    id,
                    make: storage::make::<C>,
                    make_resource: storage::make_cell::<C>,
                    resource_size: std::mem::size_of::<storage::Singleton<C>>(),
                    ambient,
                },
            );
        }
        self
    }
    pub(crate) fn storage<C: Component>(&self) -> Option<&Storage<C>> {
        self.components.get(C::NAME)?.any().downcast_ref()
    }
    /// Spawn in the lowest free slot.
    pub fn spawn(&mut self, bundle: impl Bundle) -> Entity {
        self.spawn_inner(None, bundle)
    }
    /// Spawn with an agent-visible name. Repeated names resolve lowest-index first.
    pub fn spawn_named(&mut self, name: impl AsRef<str>, bundle: impl Bundle) -> Entity {
        self.spawn_inner(Some(name.as_ref().into()), bundle)
    }
    fn spawn_inner(&mut self, name: Option<String>, bundle: impl Bundle) -> Entity {
        self.mutated();
        let index = if self.state.free.0.is_empty() {
            let i = u32::try_from(self.state.slots.len()).expect("entity slots exhausted");
            assert_ne!(i, u32::MAX, "entity slots exhausted");
            self.state.slots.push(Slot::default());
            i
        } else {
            self.state.free.0.pop_first().unwrap()
        };
        let slot = &mut self.state.slots[index as usize];
        slot.alive = true;
        slot.name = name;
        let e = Entity {
            index,
            generation: slot.generation,
        };
        let word = index as usize / 64;
        if word >= self.alive_mask.len() {
            self.alive_mask.push(0);
        }
        self.alive_mask[word] |= 1 << (index % 64);
        self.entities_revision = self.entities_revision.wrapping_add(1);
        self.fresh.push(e);
        bundle.insert(self, e);
        self.log(format_args!("spawn #{}", e.index));
        e
    }
    /// Remove this entity only; descendants leave at the end of the tick.
    /// A panicking component destructor leaves the slot alive until a later retry.
    pub fn despawn(&mut self, e: Entity) -> bool {
        if !self.contains(e) {
            return false;
        }
        self.mutated();
        let generation = self.state.slots[e.index as usize]
            .generation
            .checked_add(1)
            .expect("entity generation exhausted");
        for s in self.components.values_mut() {
            s.remove(e.index as usize);
        }
        let slot = &mut self.state.slots[e.index as usize];
        slot.generation = generation;
        slot.alive = false;
        slot.name = None;
        self.alive_mask[e.index as usize / 64] &= !(1 << (e.index % 64));
        self.state.free.0.insert(e.index);
        self.entities_revision = self.entities_revision.wrapping_add(1);
        self.log(format_args!("despawn #{}", e.index));
        true
    }
    /// Reap dead-parent children in entity order, repeating for orphaned chains.
    /// Sim calls this once after Game::tick and before propagate.
    pub fn reap_orphans(&mut self) {
        let mut orphans = std::mem::take(&mut self.orphans);
        loop {
            orphans.clear();
            for (e, p) in self.query::<&Parent>().iter() {
                if !self.contains(p.0) {
                    orphans.push(e);
                }
            }
            if orphans.is_empty() {
                break;
            }
            for &e in &orphans {
                self.despawn(e);
            }
        }
        self.orphans = orphans;
    }
    /// Entities spawned, first given a pose, or teleported since this tick began.
    /// Sim clears this list at the start of each tick.
    /// Entries retain their incarnation, so consumers can ignore entities now dead.
    pub fn fresh(&self) -> &[Entity] {
        &self.fresh
    }
    /// Whether this exact incarnation is alive.
    pub fn contains(&self, e: Entity) -> bool {
        self.state
            .slots
            .get(e.index as usize)
            .is_some_and(|s| s.alive && s.generation == e.generation)
    }
    /// Number of living entities.
    pub fn len(&self) -> usize {
        self.state.slots.len() - self.state.free.0.len()
    }
    /// Whether no entities are alive.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Living entities in ascending slot order.
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
    pub(crate) fn entity_at(&self, index: usize) -> Entity {
        Entity {
            index: index as u32,
            generation: self.state.slots[index].generation,
        }
    }
    /// The lowest-index living entity bearing this name.
    pub fn named(&self, name: &str) -> Option<Entity> {
        self.entities().find(|&e| self.name(e) == Some(name))
    }
    /// The name of a living entity.
    pub fn name(&self, e: Entity) -> Option<&str> {
        if !self.contains(e) {
            return None;
        }
        self.state.slots[e.index as usize].name.as_deref()
    }
    /// Resolve fox, fox#12, or #12; an explicit name must agree with the slot.
    pub fn resolve(&self, target: &str) -> Option<Entity> {
        if let Some(e) = self.named(target) {
            return Some(e);
        }
        if let Some((name, index)) = target.rsplit_once('#') {
            let index: u32 = index.parse().ok()?;
            let s = self.state.slots.get(index as usize)?;
            let e = Entity {
                index,
                generation: s.generation,
            };
            (s.alive && (name.is_empty() || s.name.as_deref() == Some(name))).then_some(e)
        } else {
            self.named(target)
        }
    }
    /// Insert or replace a component, returning false if the entity is gone.
    pub fn insert<C: Component>(&mut self, e: Entity, c: C) -> bool {
        if !self.contains(e) {
            return false;
        }
        // Acquiring a first pose is also a presentation birth, even when an
        // entity was spawned in an earlier tick without a Transform.
        if TypeId::of::<C>() == TypeId::of::<crate::Transform>()
            && !self.has::<C>(e)
            && self.fresh.last() != Some(&e)
        {
            self.fresh.push(e);
        }
        self.register::<C>();
        self.components
            .entry(C::NAME)
            .or_insert_with(|| storage::make::<C>(C::NAME, self.epoch.clone()))
            .any_mut()
            .downcast_mut::<Storage<C>>()
            .unwrap()
            .insert(e.index as usize, c);
        true
    }
    /// Remove a component, returning its last value.
    pub fn remove<C: Component>(&mut self, e: Entity) -> Option<C> {
        if !self.contains(e) {
            return None;
        }
        self.components
            .get_mut(C::NAME)?
            .any_mut()
            .downcast_mut::<Storage<C>>()?
            .remove(e.index as usize)
    }
    /// Test membership without borrowing the component's values.
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
        self.storage::<C>()?.get_mut(e.index as usize)
    }
    /// Construct an entity-ordered join and acquire its storage borrows now.
    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        QueryBorrow::new(self)
    }
    /// Allocated component pages in entity-index order, under a shared lease.
    /// Each view supplies its first index, presence words, and a raw pointer valid
    /// for PAGE slots. Absent slots must not be read as C; only Plain has bytes().
    pub fn pages<C: Component>(&self) -> Pages<'_, C> {
        Pages::new(self.storage::<C>())
    }
    /// Mutation generation, including repeated edits within one tick. Not saved or hashed.
    pub fn revision<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, Storage::revision)
    }
    /// Component membership generation; changing an existing value leaves it alone.
    pub fn membership<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, Storage::membership)
    }
    /// Spawn/despawn generation, including equal-count slot recycling. Not simulation state.
    pub fn entities_revision(&self) -> u64 {
        self.entities_revision
    }
    /// Scan for direct children in entity order, for tools;
    /// a tick that needs children keeps them in a component.
    pub fn children(&self, e: Entity) -> Vec<Entity> {
        if !self.contains(e) {
            return vec![];
        }
        self.query::<&Parent>()
            .iter()
            .filter(|(_, p)| p.0 == e)
            .map(|(e, _)| e)
            .collect()
    }
    /// Insert or replace named singleton state.
    pub fn insert_resource<R: Resource>(&mut self, r: R) {
        self.register_resource::<R>();
        self.resources
            .entry(R::NAME)
            .or_insert_with(|| storage::make_cell::<R>(R::NAME, self.epoch.clone()))
            .any_mut()
            .downcast_mut::<storage::Singleton<R>>()
            .unwrap()
            .insert(r);
    }
    fn resource_storage<R: Resource>(&self) -> &storage::Singleton<R> {
        self.resources
            .get(R::NAME)
            .and_then(|s| s.any().downcast_ref())
            .unwrap_or_else(|| panic!("resource {} is absent", R::NAME))
    }
    /// Borrow optional singleton data without requiring its installation.
    pub fn try_resource<R: Resource>(&self) -> Option<Ref<'_, R>> {
        self.resources
            .get(R::NAME)?
            .any()
            .downcast_ref::<storage::Singleton<R>>()?
            .get()
    }
    /// Borrow a resource; absence panics with its name.
    pub fn resource<R: Resource>(&self) -> Ref<'_, R> {
        self.resource_storage::<R>().get().unwrap()
    }
    /// Borrow a resource exclusively; absence panics with its name.
    pub fn resource_mut<R: Resource>(&self) -> RefMut<'_, R> {
        self.resource_storage::<R>().get_mut().unwrap()
    }
    /// Current fixed-step tick.
    pub fn tick(&self) -> u64 {
        self.state.tick
    }
    /// Fixed steps per second.
    pub fn hz(&self) -> u32 {
        self.state.hz
    }
    /// One fixed step, in seconds.
    pub fn dt(&self) -> f32 {
        1.0 / self.hz() as f32
    }
    /// Simulation time; never wall time.
    pub fn seconds(&self) -> f64 {
        self.tick() as f64 / self.hz() as f64
    }
    /// The world's only source of simulation randomness.
    pub fn rng(&self) -> RefMut<'_, Rng> {
        self.rng.get_mut().unwrap()
    }
    /// Draw one value and release the random column before returning.
    pub fn rand<T: crate::RangeValue>(&self, range: std::ops::Range<T>) -> T {
        self.rng().range(range)
    }
    /// One Bernoulli trial, with probability in [0, 1].
    pub fn chance(&self, p: f32) -> bool {
        self.rng().chance(p)
    }
    /// Choose a slice element, releasing the random column before returning.
    pub fn pick<'a, T>(&self, items: &'a [T]) -> Option<&'a T> {
        self.rng().pick(items)
    }
    /// Append an event to the bounded 4,096-line journal.
    /// The journal is telemetry: a record outside the world hash and observation,
    /// so a read that logs must not change the world's course or mutation epoch.
    pub fn log(&self, line: impl std::fmt::Display) {
        let mut j = self.journal.borrow_mut();
        if j.len() == 4096 {
            j.pop_front();
        }
        let index = self.journal_next.get();
        self.journal_next.set(index + 1);
        j.push_back(Event {
            index,
            tick: self.tick(),
            seconds: self.seconds(),
            line: format!(
                "t={} tick={} {line}",
                self.tick() as u128 * 1000 / self.hz() as u128,
                self.tick()
            ),
        });
    }
    /// Snapshot journal events; journal reads do not affect simulation state.
    pub fn journal(&self) -> Vec<Event> {
        self.journal.borrow().iter().cloned().collect()
    }
    /// Publish to the app and journal only changes to this key.
    pub fn publish(&self, key: &str, value: impl Into<crate::Published>) {
        let value = value.into().0;
        let mut p = self.published.borrow_mut();
        if p.get(key) == Some(&value) {
            return;
        }
        self.log(format_args!("publish {key}: {value:?}"));
        p.insert(key.into(), value);
        self.published_pending.set(true);
        self.mutated();
    }
    /// Queue a string event for the canvas's `message=` handler, in order, once.
    pub fn emit(&self, text: impl Into<String>) {
        self.messages.borrow_mut().push(text.into());
    }
    /// Last value published under a key.
    pub fn published(&self, key: &str) -> Option<Value> {
        self.published.borrow().get(key).cloned()
    }
    // Sim will own clock advancement; keep the primitive private to this crate.
    pub(crate) fn step_clock(&mut self) {
        self.mutated();
        self.in_tick = false;
        self.state.tick = self
            .state
            .tick
            .checked_add(1)
            .expect("world clock exhausted");
    }

    fn write(&self, w: &mut dyn Writer, delivery: bool) {
        w.begin_struct();
        w.field("state");
        self.state.write(w);
        w.field("rng");
        self.rng.get().unwrap().write(w);
        for (kind, storages) in [
            ("components", &self.components),
            ("resources", &self.resources),
        ] {
            w.field(kind);
            w.begin_struct();
            for (name, s) in storages {
                w.key(name);
                s.write(w, &|index| {
                    if kind == "resources" {
                        SINGLETON
                    } else {
                        self.entity_at(index)
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
    /// Hash simulation state in type-name order, excluding saved delivery queues.
    pub fn hash(&self) -> u64 {
        if let Some((epoch, hash)) = self.hash_cache.get() {
            if epoch == self.mutation_epoch() {
                return hash;
            }
        }
        let prefix = self.hash_prefix.borrow();
        let mut w = if let Some((epoch, prefix)) = &*prefix {
            (*epoch == self.mutation_epoch()).then(|| prefix.clone())
        } else {
            None
        };
        let hash = if let Some(w) = &mut w {
            w.field("resources");
            w.begin_struct();
            for (name, s) in &self.resources {
                w.key(name);
                s.write(w, &|_| SINGLETON);
            }
            w.end_struct();
            w.end_struct();
            w.finish()
        } else {
            let mut w = hash::Hasher::default();
            self.write(&mut w, false);
            w.finish()
        };
        self.hash_cache.set(Some((self.mutation_epoch(), hash)));
        hash
    }
    /// Write a versioned save; NaNs are canonicalized and caches are excluded.
    pub fn save(&self) -> Vec<u8> {
        let mut w = bin::Encoder::default();
        self.write(&mut w, true);
        let mut bytes = MAGIC.to_vec();
        bytes.extend(w.finish());
        bytes
    }
    /// Atomically replace simulation state. Registered types survive the replacement;
    /// caches, publications and events do not. The entity table precedes storages.
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        if bytes.len() > crate::data::MAX_LOAD_BYTES {
            return Err(DataError::new("save exceeds load size limit"));
        }
        if !bytes.starts_with(MAGIC) {
            return Err(DataError::new(format!(
                "unsupported world save format (expected EXGAME v3; saw {:02x?})",
                &bytes[..bytes.len().min(8)]
            )));
        }
        let mut next = Self::new(1, 0);
        next.registry = self.registry.clone();
        let mut r = bin::Decoder::new(&bytes[MAGIC.len()..]);
        next.read(&mut r).map_err(|e| e.at("World"))?;
        r.finish()?;
        next.validate_hierarchy(&mut r)?;
        next.presentation_generation = self
            .presentation_generation
            .checked_add(1)
            .expect("presentation generation exhausted");
        next.epoch.set(self.epoch.get().wrapping_add(1));
        *self = next;
        Ok(())
    }
    fn validate_state(&self) -> Result<(), DataError> {
        if self.hz() == 0 {
            return Err(DataError::new("hz must be positive"));
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
        if self
            .state
            .slots
            .iter()
            .any(|s| !s.alive && s.name.is_some())
        {
            return Err(DataError::new("dead entity has a name"));
        }
        Ok(())
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        let mut seen = 0u8;
        while let Some(field) = r.field()? {
            seen |= match field.as_str() {
                "state" => 1,
                "rng" => 2,
                "components" => 4,
                "resources" => 8,
                _ => 0,
            };
            match field.as_str() {
                "state" => {
                    self.state.read(r)?;
                    self.validate_state()?;
                    let words = self.state.slots.len().div_ceil(64);
                    crate::data::limits::reserve(r, &mut self.alive_mask, words)?;
                    self.alive_mask.resize(words, 0);
                    for (index, slot) in self.state.slots.iter().enumerate() {
                        if slot.alive {
                            self.alive_mask[index / 64] |= 1 << (index % 64);
                        }
                    }
                }
                "rng" => self.rng().read(r)?,
                "messages" => self.messages.borrow_mut().read(r)?,
                "components" | "resources" => {
                    if seen & 1 == 0 {
                        return Err(DataError::new("entity table must precede storage"));
                    }
                    r.begin_struct()?;
                    while let Some(name) = r.field()? {
                        let (&key, reg) =
                            self.registry.get_key_value(name.as_str()).ok_or_else(|| {
                                DataError::new(format!(
                                    "unregistered {} `{name}`; call world.{}::<{name}>() in setup",
                                    if field == "resources" {
                                        "resource"
                                    } else {
                                        "component"
                                    },
                                    if field == "resources" {
                                        "register_resource"
                                    } else {
                                        "register"
                                    }
                                ))
                            })?;
                        let resource = field == "resources";
                        let make = if resource {
                            r.claim(reg.resource_size).map_err(|e| e.at(&name))?;
                            reg.make_resource
                        } else {
                            reg.make
                        };
                        let mut s = make(key, self.epoch.clone());
                        s.read(r, &|e| {
                            if resource {
                                e == SINGLETON
                            } else {
                                self.contains(e)
                            }
                        })
                        .map_err(|e| e.at(&name))?;
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
        Ok(())
    }
}

#[cfg(test)]
mod tests;

mod inspect;
pub(crate) use inspect::{Observation, ObservationState};

mod save;
use save::Free;
