use crate::storage::{self, Erased, Storage};
use crate::{
    bin, hash, Affine3A, Data, DataError, Pages, Parent, Query, QueryBorrow, Reader, Ref, RefMut,
    Rng, Value, Writer,
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

/// A named kind of per-entity data. Names must be unique within a world.
pub trait Component: Data {
    /// Stable save-file and agent spelling.
    const NAME: &'static str;
}
/// World-owned singleton data; the Component derive supplies its name and data.
pub trait Resource: Component {}
impl<C: Component> Resource for C {}

/// Components supplied to spawn; use a tuple, including a one-element tuple.
pub trait Bundle {
    /// Insert this bundle into an existing entity.
    fn insert(self, world: &mut World, entity: Entity);
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
#[derive(Default, Data)]
struct State {
    tick: u64,
    hz: u32,
    seed: u64,
    slots: Vec<Slot>,
    free: Vec<u32>,
}
#[derive(Clone, Copy)]
struct Registration {
    id: TypeId,
    make: fn() -> Box<dyn Erased>,
}

/// One journal event. Reads never generate per-tick samples.
#[derive(Clone, Debug)]
pub struct Event {
    /// Simulation tick at the event.
    pub tick: u64,
    /// Simulation seconds at the event.
    pub seconds: f64,
    /// Human-readable event text.
    pub line: String,
}

/// Ordered simulation state, with dynamic storage borrows and no host clock.
pub struct World {
    state: State,
    pub(crate) alive_mask: Vec<u64>,
    rng: Storage<Rng>,
    registry: BTreeMap<&'static str, Registration>,
    components: BTreeMap<&'static str, Box<dyn Erased>>,
    resources: BTreeMap<&'static str, Box<dyn Erased>>,
    journal: RefCell<VecDeque<Event>>,
    published: RefCell<BTreeMap<String, Value>>,
    pub(crate) globals: BTreeMap<Entity, Affine3A>,
    pub(crate) previous: BTreeMap<Entity, Affine3A>,
    pub(crate) propagated_tick: Option<u64>,
}
const SINGLETON: Entity = Entity {
    index: 0,
    generation: 0,
};
const MAGIC: &[u8; 8] = b"EXGAME\0\x01";

impl World {
    /// Start at tick zero. A zero tick rate is a programmer error.
    pub fn new(hz: u32, seed: u64) -> Self {
        assert!(hz > 0, "world hz must be positive");
        let mut rng = Storage::default();
        rng.insert(0, Rng::new(seed), 0);
        Self {
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
            published: RefCell::new(BTreeMap::new()),
            globals: BTreeMap::new(),
            previous: BTreeMap::new(),
            propagated_tick: None,
        }
    }
    /// Register a component or resource before loading. Registration itself is not state.
    pub fn register<C: Component>(&mut self) -> &mut Self {
        let id = TypeId::of::<C>();
        if let Some(old) = self.registry.get(C::NAME) {
            assert_eq!(old.id, id, "duplicate component name {}", C::NAME);
        } else {
            self.registry.insert(
                C::NAME,
                Registration {
                    id,
                    make: storage::make::<C>,
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
    pub fn spawn_named(&mut self, name: &str, bundle: impl Bundle) -> Entity {
        self.spawn_inner(Some(name.into()), bundle)
    }
    fn spawn_inner(&mut self, name: Option<String>, bundle: impl Bundle) -> Entity {
        let index = if self.state.free.is_empty() {
            let i = u32::try_from(self.state.slots.len()).expect("entity slots exhausted");
            assert_ne!(i, u32::MAX, "entity slots exhausted");
            self.state.slots.push(Slot::default());
            i
        } else {
            self.state.free.remove(0)
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
        bundle.insert(self, e);
        self.log(format_args!("spawn #{}", e.index));
        e
    }
    /// Remove an entity and all descendants, even if a malformed hierarchy cycles.
    pub fn despawn(&mut self, e: Entity) -> bool {
        if !self.contains(e) {
            return false;
        }
        let mut edges: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
        for (child, parent) in self.query::<&Parent>().iter() {
            edges.entry(parent.0).or_default().push(child);
        }
        let mut stack = vec![e];
        let mut removed = BTreeSet::new();
        while let Some(e) = stack.pop() {
            if !removed.insert(e) {
                continue;
            }
            if let Some(children) = edges.get(&e) {
                stack.extend(children);
            }
        }
        for e in removed {
            if !self.contains(e) {
                continue;
            }
            let slot = &mut self.state.slots[e.index as usize];
            slot.generation = slot
                .generation
                .checked_add(1)
                .expect("entity generation exhausted");
            slot.alive = false;
            self.alive_mask[e.index as usize / 64] &= !(1 << (e.index % 64));
            slot.name = None;
            let p = self.state.free.partition_point(|&i| i < e.index);
            self.state.free.insert(p, e.index);
            for s in self.components.values_mut() {
                s.remove(e.index as usize, self.state.tick);
            }
            self.globals.remove(&e);
            self.previous.remove(&e);
            self.log(format_args!("despawn #{}", e.index));
        }
        true
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
        self.state.slots.len() - self.state.free.len()
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
    /// Insert or replace a component; stale entities are a programmer error.
    pub fn insert<C: Component>(&mut self, e: Entity, c: C) {
        assert!(
            self.contains(e),
            "cannot insert {} into stale entity {:?}",
            C::NAME,
            e
        );
        self.register::<C>();
        self.components
            .entry(C::NAME)
            .or_insert_with(storage::make::<C>)
            .any_mut()
            .downcast_mut::<Storage<C>>()
            .unwrap()
            .insert(e.index as usize, c, self.state.tick);
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
            .remove(e.index as usize, self.state.tick)
    }
    /// Test membership without borrowing the component's values.
    pub fn has<C: Component>(&self, e: Entity) -> bool {
        self.contains(e) && self.storage::<C>().is_some_and(|s| s.has(e.index as usize))
    }
    /// Borrow one component immutably; conflicts panic with its name.
    pub fn get<C: Component>(&self, e: Entity) -> Option<Ref<'_, C>> {
        if !self.contains(e) {
            return None;
        }
        self.storage::<C>()?.get(e.index as usize)
    }
    /// Borrow one component exclusively; conflicts panic with its name.
    pub fn get_mut<C: Component>(&self, e: Entity) -> Option<RefMut<'_, C>> {
        if !self.contains(e) {
            return None;
        }
        self.storage::<C>()?.get_mut(e.index as usize, self.tick())
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
    /// Last tick exclusively leased or structurally changed, or zero if absent.
    /// A same-tick edit keeps the same stamp; this is not a mutation counter.
    pub fn changed<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, Storage::changed)
    }
    /// Direct children in entity order.
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
        self.register::<R>();
        self.resources
            .entry(R::NAME)
            .or_insert_with(storage::make::<R>)
            .any_mut()
            .downcast_mut::<Storage<R>>()
            .unwrap()
            .insert(0, r, self.state.tick);
    }
    fn resource_storage<R: Resource>(&self) -> &Storage<R> {
        self.resources
            .get(R::NAME)
            .and_then(|s| s.any().downcast_ref())
            .unwrap_or_else(|| panic!("resource {} is absent", R::NAME))
    }
    /// Borrow a resource; absence panics with its name.
    pub fn resource<R: Resource>(&self) -> Ref<'_, R> {
        self.resource_storage::<R>().get(0).unwrap()
    }
    /// Borrow a resource exclusively; absence panics with its name.
    pub fn resource_mut<R: Resource>(&self) -> RefMut<'_, R> {
        self.resource_storage::<R>()
            .get_mut(0, self.tick())
            .unwrap()
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
        self.rng.get_mut(0, self.tick()).unwrap()
    }
    /// Append an event to the bounded 4,096-line journal.
    pub fn log(&self, line: impl std::fmt::Display) {
        let mut j = self.journal.borrow_mut();
        if j.len() == 4096 {
            j.pop_front();
        }
        j.push_back(Event {
            tick: self.tick(),
            seconds: self.seconds(),
            line: line.to_string(),
        });
    }
    /// Snapshot journal events; journal reads do not affect simulation state.
    pub fn journal(&self) -> Vec<Event> {
        self.journal.borrow().iter().cloned().collect()
    }
    /// Publish to the app and journal only changes to this key.
    pub fn publish(&self, key: &str, value: Value) {
        let mut p = self.published.borrow_mut();
        if p.get(key) == Some(&value) {
            return;
        }
        self.log(format_args!("publish {key}: {value:?}"));
        p.insert(key.into(), value);
    }
    /// Last value published under a key.
    pub fn published(&self, key: &str) -> Option<Value> {
        self.published.borrow().get(key).cloned()
    }
    // Sim will own clock advancement; keep the primitive private to this crate.
    #[allow(dead_code)]
    pub(crate) fn step_clock(&mut self) {
        self.state.tick = self
            .state
            .tick
            .checked_add(1)
            .expect("world clock exhausted");
    }

    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("state");
        self.state.write(w);
        w.field("rng");
        self.rng.get(0).unwrap().write(w);
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
        w.end_struct();
    }
    /// Hash all simulation state, with component and resource types sorted by name.
    pub fn hash(&self) -> u64 {
        let mut w = hash::Hasher::default();
        self.write(&mut w);
        w.finish()
    }
    /// Write a versioned save; NaNs are canonicalized and caches are excluded.
    pub fn save(&self) -> Vec<u8> {
        let mut w = bin::Encoder::default();
        self.write(&mut w);
        let mut bytes = MAGIC.to_vec();
        bytes.extend(w.finish());
        bytes
    }
    /// Atomically replace simulation state. Registered types survive the replacement;
    /// caches, publications and events do not. The entity table precedes storages.
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        if !bytes.starts_with(MAGIC) {
            return Err(DataError::new("invalid game save magic or version"));
        }
        let mut next = Self::new(1, 0);
        next.registry = self.registry.clone();
        let mut r = bin::Decoder::new(&bytes[MAGIC.len()..]);
        next.read(&mut r).map_err(|e| e.at("World"))?;
        r.finish()?;
        *self = next;
        Ok(())
    }
    fn validate_state(&self) -> Result<(), DataError> {
        if self.hz() == 0 {
            return Err(DataError::new("hz must be positive"));
        }
        let free: Vec<_> = self
            .state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.alive)
            .map(|(i, _)| i as u32)
            .collect();
        if free != self.state.free {
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
        let mut seen = BTreeSet::new();
        while let Some(field) = r.field()? {
            if !seen.insert(field.clone()) {
                return Err(DataError::new("duplicate world field").at(field));
            }
            match field.as_str() {
                "state" => {
                    self.state.read(r)?;
                    self.validate_state()?;
                    self.alive_mask = vec![0; self.state.slots.len().div_ceil(64)];
                    for (index, slot) in self.state.slots.iter().enumerate() {
                        if slot.alive {
                            self.alive_mask[index / 64] |= 1 << (index % 64);
                        }
                    }
                }
                "rng" => self.rng().read(r)?,
                "components" | "resources" => {
                    if !seen.contains("state") {
                        return Err(DataError::new("entity table must precede storage"));
                    }
                    r.begin_struct()?;
                    while let Some(name) = r.field()? {
                        let (&key, reg) =
                            self.registry.get_key_value(name.as_str()).ok_or_else(|| {
                                DataError::new("unregistered component or resource").at(&name)
                            })?;
                        let mut s = (reg.make)();
                        let resource = field == "resources";
                        s.read(
                            r,
                            &|e| {
                                if resource {
                                    e == SINGLETON
                                } else {
                                    self.contains(e)
                                }
                            },
                            self.tick(),
                        )
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
        if ["state", "rng", "components", "resources"]
            .iter()
            .any(|k| !seen.contains(*k))
        {
            return Err(DataError::new("incomplete world save"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
