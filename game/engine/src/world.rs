use crate::storage::{self, At, Conflict, Erased, Leases, Storage};
use crate::{
    bin, hash, Data, DataError, Now, Pages, Parent, Query, QueryBorrow, Reader, Ref, RefMut, Rng,
    Value, Writer,
};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::panic::Location;

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
    /// Name or slot for a setup error.
    fn label(&self) -> String;
    /// Resolve a live entity without consuming its diagnostic label or reviving a stale handle.
    fn entity(&self, world: &World) -> Option<Entity>;
}
impl Target for Entity {
    fn label(&self) -> String {
        format!("#{}", self.index())
    }
    fn entity(&self, world: &World) -> Option<Entity> {
        world.contains(*self).then_some(*self)
    }
}
impl Target for &str {
    fn label(&self) -> String {
        (*self).into()
    }
    fn entity(&self, world: &World) -> Option<Entity> {
        world.resolve(self)
    }
}
impl Target for String {
    fn label(&self) -> String {
        self.clone()
    }
    fn entity(&self, world: &World) -> Option<Entity> {
        world.resolve(self.as_str())
    }
}
impl Target for &String {
    fn label(&self) -> String {
        self.as_str().into()
    }
    fn entity(&self, world: &World) -> Option<Entity> {
        world.resolve(self.as_str())
    }
}

/// A named kind of per-entity data. Names must be unique within a world.
/// Semantic state has no interior mutability; derives introduce none. A manual
/// implementation that mutates semantic state through a shared reference is outside
/// the [`Data`] contract: quiescence and the hash cache are undefined for it.
pub trait Component: Data {
    /// Stable save-file and agent spelling.
    const NAME: &'static str;
    /// Register data this component produces, before restoring a saved world.
    fn register(_world: &mut World) {}
    /// Refuse a component combination before changing the entity.
    fn accepts(_world: &World, _entity: Entity) -> bool {
        true
    }
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
        impl<$($T: Bundle),+> Bundle for ($($T,)+) {
            fn insert(self, w: &mut World, e: Entity) { $(self.$i.insert(w, e);)+ }
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
    #[data(skip)]
    fresh: bool,
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
type StorageFactory = fn(&'static str, std::rc::Rc<std::cell::Cell<u64>>) -> Box<dyn Erased>;
#[derive(Clone, Copy)]
struct Registration {
    id: TypeId,
    make: Option<StorageFactory>,
    resource_size: usize,
    make_resource: Option<StorageFactory>,
    ambient: bool,
}

#[derive(Clone, Copy)]
// Installed together by the linked attachment component, never saved as state.
pub(crate) struct Attachments {
    pub pose: fn(&World, Entity, usize) -> Option<crate::Affine3A>,
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

/// The most of one published value a journal line holds.
const PUBLISH_LINE_BYTES: usize = 4096;

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
    pub(crate) assets: crate::asset::AssetStore,
    pub(crate) changing: Vec<String>,
    pub(crate) observation: ObservationState,
    epoch: std::rc::Rc<std::cell::Cell<u64>>,
    observed_epoch: u64,
    hash_cache: std::cell::Cell<Option<(u64, u64)>>,
    // Paged digests behind hash and observation; never saved.
    digests: RefCell<digest::Digests>,
    // Executor phase, never saved: audio authored in a tick starts at its end.
    pub(crate) in_tick: bool,
    pub(crate) followed: std::cell::Cell<bool>,
    pub(crate) attachments: Option<Attachments>,
    pub(crate) detach: Option<fn(&World, Entity)>,
    state: State,
    // Sorted named slots, populated on first lookup; strings remain owned by State.
    names: RefCell<Option<Vec<u32>>>,
    pub(crate) alive_mask: Vec<u64>,
    rng: storage::Singleton<Rng>,
    registry: BTreeMap<&'static str, Registration>,
    components: BTreeMap<&'static str, Box<dyn Erased>>,
    resources: BTreeMap<&'static str, Box<dyn Erased>>,
    // Registered queries over `components`; never saved, replaced with them on load.
    pub(crate) leases: Leases,
    // Animation-owned derived data, populated only when animation is linked and used; never saved.
    pub(crate) animation_runtime: RefCell<Option<Box<dyn std::any::Any>>>,
    journal: RefCell<VecDeque<Event>>,
    journal_next: std::cell::Cell<u64>,
    pub(crate) published_pending: std::cell::Cell<bool>,
    published: RefCell<BTreeMap<String, crate::values::Stored>>,
    /// Each published value's exact JSON, encoded once per change (`take_published`).
    published_text: RefCell<BTreeMap<String, String>>,
    derived_publications: BTreeSet<String>,
    pub(crate) messages: RefCell<Vec<String>>,
    pub(crate) hierarchy: crate::scene::Hierarchy,
    fresh: Vec<Entity>,
    entities_revision: u64,
    pub(crate) presentation_generation: u64,
}
const MAGIC: &[u8; 8] = b"EXGAME\0\x04";

impl World {
    /// Start at tick zero. A zero tick rate is a programmer error.
    pub fn new(hz: u32, seed: u64) -> Self {
        assert!(hz > 0, "world hz must be positive");
        let epoch = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut rng = storage::Singleton::new("Rng", epoch.clone());
        rng.insert(Rng::new(seed));
        let mut world = Self {
            assets: Default::default(),
            epoch,
            observed_epoch: 0,
            hash_cache: std::cell::Cell::new(None),
            digests: RefCell::default(),
            changing: Vec::new(),
            observation: ObservationState::Unknown,
            in_tick: false,
            followed: std::cell::Cell::new(false),
            attachments: None,
            detach: None,
            state: State {
                hz,
                seed,
                ..State::default()
            },
            alive_mask: vec![],
            names: RefCell::new(None),
            rng,
            registry: BTreeMap::new(),
            components: BTreeMap::new(),
            resources: BTreeMap::new(),
            leases: Leases::default(),
            animation_runtime: RefCell::new(None),
            journal: RefCell::new(VecDeque::new()),
            journal_next: std::cell::Cell::new(0),
            published_pending: std::cell::Cell::new(false),
            published: RefCell::new(BTreeMap::new()),
            published_text: RefCell::new(BTreeMap::new()),
            derived_publications: BTreeSet::new(),
            messages: RefCell::new(Vec::new()),
            hierarchy: crate::scene::Hierarchy::default(),
            fresh: vec![],
            entities_revision: 0,
            presentation_generation: 0,
        };
        world.register_scene();
        world
    }
    // The engine's own scene components restore in any process, however late a
    // game first spawns one. Registration is not state; executor-linked
    // components (animation, sockets, audio) still register where they install.
    fn register_scene(&mut self) {
        use crate::scene::*;
        self.register::<Transform>()
            .register::<Parent>()
            .register::<Mesh>()
            .register::<crate::Material>()
            .register::<Camera>()
            .register::<DirectionalLight>()
            .register::<PointLight>()
            .register::<SpotLight>()
            .register::<LightShadows>()
            .register::<ViewModel>()
            .register::<Opacity>()
            .register::<NodeMaterials>()
            .register::<ModelLod>()
            .register::<crate::ParticleLook>()
            .register::<MouseLook>()
            .register::<Visible>()
            .register::<Ambient>()
            .register::<Follow>()
            .register::<Glow>()
            .register::<Lit>();
    }
    /// Identity of this world instance, excluded from saves and hashes.
    pub fn id(&self) -> WorldId {
        WorldId(self.epoch.clone())
    }
    /// Replacement epoch for world-derived presentation histories and draw records.
    /// Device assets keyed by name/content digest survive it. Excluded from saves/hashes.
    pub fn presentation_generation(&self) -> u64 {
        self.presentation_generation
    }
    /// Register a component before loading. Registration itself is not state.
    pub fn register<C: Component>(&mut self) -> &mut Self {
        C::register(self);
        self.registration::<C>(C::NAME).make = Some(storage::make::<C>);
        self
    }
    /// Register singleton data before loading a save.
    pub fn register_resource<R: Resource>(&mut self) -> &mut Self {
        let reg = self.registration::<R>(R::NAME);
        reg.make_resource = Some(storage::make_cell::<R>);
        reg.resource_size = std::mem::size_of::<storage::Singleton<R>>();
        reg.ambient |= R::AMBIENT;
        self
    }
    fn registration<C: Data>(&mut self, name: &'static str) -> &mut Registration {
        let id = TypeId::of::<C>();
        let reg = self.registry.entry(name).or_insert(Registration {
            id,
            make: None,
            make_resource: None,
            resource_size: 0,
            ambient: false,
        });
        assert_eq!(reg.id, id, "duplicate component name {}", name);
        reg
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
        slot.fresh = true;
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
        self.digests.get_mut().touch(index);
        self.entities_revision = self.entities_revision.wrapping_add(1);
        self.fresh.push(e);
        self.index_name(index, true);
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
        if let Some(detach) = self.detach {
            detach(self, e);
        }
        self.hierarchy.despawning(e.index);
        self.mutated();
        let generation = self.state.slots[e.index as usize]
            .generation
            .checked_add(1)
            .expect("entity generation exhausted");
        self.leases.restructure();
        for s in self.components.values_mut() {
            s.remove(e.index as usize);
        }
        self.index_name(e.index, false);
        let slot = &mut self.state.slots[e.index as usize];
        slot.generation = generation;
        slot.alive = false;
        slot.name = None;
        self.alive_mask[e.index as usize / 64] &= !(1 << (e.index % 64));
        self.digests.get_mut().touch(e.index);
        self.state.free.0.insert(e.index);
        self.entities_revision = self.entities_revision.wrapping_add(1);
        self.log(format_args!("despawn #{}", e.index));
        true
    }
    /// Entities spawned, first given a pose, or teleported since this tick began.
    /// Sim clears this list at the start of each tick.
    /// Entries retain their incarnation, so consumers can ignore entities now dead.
    /// Each incarnation appears at most once.
    pub fn fresh(&self) -> &[Entity] {
        &self.fresh
    }
    /// Whether this incarnation was spawned, first posed, or teleported this tick.
    /// Living incarnations use the slot flag; retired ones remain in the list.
    pub fn is_fresh(&self, e: Entity) -> bool {
        match self.state.slots.get(e.index as usize) {
            Some(slot) if slot.alive && slot.generation == e.generation => slot.fresh,
            _ => self.fresh.contains(&e),
        }
    }
    pub(crate) fn mark_fresh(&mut self, e: Entity) {
        let slot = &mut self.state.slots[e.index as usize];
        if !slot.fresh {
            slot.fresh = true;
            self.fresh.push(e);
        }
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
    /// Count living components matching a predicate, without changing the world.
    #[track_caller]
    pub fn count<T: Component>(&self, mut predicate: impl FnMut(&T) -> bool) -> u32 {
        self.query::<&T>()
            .iter()
            .filter(|(_, item)| predicate(item))
            .count() as u32
    }
    /// The lowest-index living entity bearing this name.
    pub fn named(&self, name: &str) -> Option<Entity> {
        let mut cached = self.names.borrow_mut();
        let names = cached.get_or_insert_with(|| {
            let mut names: Vec<_> = self
                .state
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.alive && s.name.is_some())
                .map(|(i, _)| i as u32)
                .collect();
            names.sort_unstable_by_key(|&i| (&self.state.slots[i as usize].name, i));
            names
        });
        let at = names
            .partition_point(|&i| self.state.slots[i as usize].name.as_deref().unwrap() < name);
        names
            .get(at)
            .copied()
            .filter(|&i| self.state.slots[i as usize].name.as_deref() == Some(name))
            .map(|i| self.entity_at(i as usize))
    }
    fn index_name(&mut self, index: u32, insert: bool) {
        let slots = &self.state.slots;
        let Some(name) = slots[index as usize].name.as_deref() else {
            return;
        };
        let Some(names) = self.names.get_mut() else {
            return;
        };
        let at = names
            .partition_point(|&i| (slots[i as usize].name.as_deref().unwrap(), i) < (name, index));
        if insert {
            names.insert(at, index);
        } else {
            names.remove(at);
        }
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
        let (name, index) = target.rsplit_once('#')?;
        let index: u32 = index.parse().ok()?;
        let s = self.state.slots.get(index as usize)?;
        let e = Entity {
            index,
            generation: s.generation,
        };
        (s.alive && (name.is_empty() || s.name.as_deref() == Some(name))).then_some(e)
    }
    /// Insert or replace a component, returning false if the entity is gone.
    pub fn insert<C: Component>(&mut self, e: Entity, c: C) -> bool {
        if !self.contains(e) {
            return false;
        }
        if !C::accepts(self, e) {
            return false;
        }
        // Acquiring a first pose is also a presentation birth, even when an
        // entity was spawned in an earlier tick without a Transform.
        if TypeId::of::<C>() == TypeId::of::<crate::Transform>() && !self.has::<C>(e) {
            self.mark_fresh(e);
        }
        self.register::<C>();
        // Retire leaked query shapes before `&mut` storage access.
        // @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
        self.leases.restructure();
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
        let removed = self.remove_component::<C>(e);
        if removed.is_some()
            && [
                TypeId::of::<crate::Animation>(),
                TypeId::of::<crate::Blend>(),
                TypeId::of::<crate::Animator>(),
                TypeId::of::<crate::animation::Layers>(),
            ]
            .contains(&TypeId::of::<C>())
            && !self.has::<crate::Animation>(e)
            && !self.has::<crate::Blend>(e)
            && !self.has::<crate::Animator>(e)
            && !self.has::<crate::animation::Layers>(e)
            && !self.has::<crate::Ik>(e)
        {
            self.remove::<crate::Pose>(e);
        }
        removed
    }
    /// Remove storage only when replacing a controller while preserving its sampled Pose.
    pub(crate) fn remove_component<C: Component>(&mut self, e: Entity) -> Option<C> {
        if !self.contains(e) {
            return None;
        }
        self.leases.restructure();
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
    /// Borrow one component row immutably. Other rows of C stay free; a conflict
    /// on this row panics naming C, the entity and both callers.
    #[track_caller]
    pub fn get<C: Component>(&self, target: impl Target) -> Option<Ref<'_, C>> {
        self.get_at(target, Location::caller())
    }
    /// Borrow one component row exclusively. Other rows of C stay free to borrow;
    /// this row refuses every other borrow until the guard drops.
    #[track_caller]
    pub fn get_mut<C: Component>(&self, target: impl Target) -> Option<RefMut<'_, C>> {
        self.get_mut_at(target, Location::caller())
    }
    pub(crate) fn get_at<C: Component>(&self, target: impl Target, at: At) -> Option<Ref<'_, C>> {
        let e = target.entity(self)?;
        if !self.contains(e) {
            return None;
        }
        let (s, i) = (self.storage::<C>()?, e.index as usize);
        s.get(i, &self.leases, at)
            .unwrap_or_else(|_| self.refuse(s.row_conflict(i, false, &self.leases, at)))
    }
    pub(crate) fn get_mut_at<C: Component>(
        &self,
        target: impl Target,
        at: At,
    ) -> Option<RefMut<'_, C>> {
        let e = target.entity(self)?;
        if !self.contains(e) {
            return None;
        }
        let (s, i) = (self.storage::<C>()?, e.index as usize);
        s.get_mut(i, &self.leases, at)
            .unwrap_or_else(|_| self.refuse(s.row_conflict(i, true, &self.leases, at)))
    }
    /// Copy one row out, refusing only a live exclusive lease on it.
    pub(crate) fn copied_at<C: Component + Copy>(&self, e: Entity, at: At) -> Option<C> {
        if !self.contains(e) {
            return None;
        }
        let (s, i) = (self.storage::<C>()?, e.index as usize);
        s.copied(i, &self.leases)
            .unwrap_or_else(|_| self.refuse(s.row_conflict(i, false, &self.leases, at)))
    }
    /// Panic with a refused lease, naming the entity whose row both sides want.
    #[cold]
    pub(crate) fn refuse(&self, conflict: Conflict) -> ! {
        let entity = conflict
            .row
            .map(|i| i as usize)
            .filter(|&i| self.state.slots.get(i).is_some_and(|s| s.alive))
            .map(|i| {
                let e = self.entity_at(i);
                match self.name(e) {
                    Some(name) => format!("`{name}` (#{}, generation {})", e.index, e.generation),
                    None => format!("#{} (generation {})", e.index, e.generation),
                }
            });
        panic!("{}", conflict.message(entity))
    }
    // Engine reads of every row (hash, save, inspection) refuse live exclusive leases.
    pub(crate) fn check_reads(&self, at: At) {
        for s in self.components.values().chain(self.resources.values()) {
            if let Some(conflict) = s.read_conflict(&self.leases, at) {
                self.refuse(conflict);
            }
        }
    }
    /// Require a component, reporting both the target and component on failure.
    #[track_caller]
    pub fn require<C: Component>(&self, target: impl Target) -> Ref<'_, C> {
        let at = Location::caller();
        target
            .entity(self)
            .and_then(|e| self.get_at::<C>(e, at))
            .unwrap_or_else(|| {
                panic!(
                    "entity `{}` requires component `{}`",
                    target.label(),
                    C::NAME
                )
            })
    }
    /// Mutably require a component row; other rows of C stay free to borrow.
    #[track_caller]
    pub fn require_mut<C: Component>(&self, target: impl Target) -> RefMut<'_, C> {
        let at = Location::caller();
        target
            .entity(self)
            .and_then(|e| self.get_mut_at::<C>(e, at))
            .unwrap_or_else(|| {
                panic!(
                    "entity `{}` requires component `{}`",
                    target.label(),
                    C::NAME
                )
            })
    }
    /// Construct an entity-ordered join. Its first iteration leases the rows it
    /// matches, after filters, until the query and its escaped row guards drop.
    #[track_caller]
    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        QueryBorrow::new(self, Location::caller())
    }
    /// Current local position, without ancestor transforms; None if missing.
    #[track_caller]
    pub fn local_position(&self, target: impl Target) -> Option<crate::Vec3> {
        let e = target.entity(self)?;
        self.copied_at::<crate::Transform>(e, Location::caller())
            .map(|t| t.position)
    }
    /// Current global position, including parents.
    #[track_caller]
    pub fn global_position(&self, target: impl Target) -> Option<crate::Vec3> {
        self.current_global(target.entity(self)?)
            .map(|pose| pose.translation.into())
    }
    /// Closest other entity of C, including every component value. Ties use entity index.
    #[track_caller]
    pub fn nearest_xz<C: Component>(&self, origin: impl Target, radius: f32) -> Option<Entity> {
        self.nearest_xz_where::<C>(origin, radius, |_| true)
    }
    /// Closest other entity carrying C in an inclusive XZ radius. Equal distances
    /// choose the lowest entity index. The predicate runs before distance selection.
    #[track_caller]
    pub fn nearest_xz_where<C: Component>(
        &self,
        origin: impl Target,
        radius: f32,
        mut predicate: impl FnMut(&C) -> bool,
    ) -> Option<Entity> {
        let at = Location::caller();
        let candidates = self.within::<C>(origin, radius, true, at);
        let s = self.storage::<C>()?;
        candidates
            .filter(|(e, _, _)| {
                let i = e.index as usize;
                s.get(i, &self.leases, at)
                    .unwrap_or_else(|_| self.refuse(s.row_conflict(i, false, &self.leases, at)))
                    .is_some_and(|c| predicate(&c))
            })
            .min_by(|(_, _, a), (_, _, b)| a.total_cmp(b))
            .map(|(entity, _, _)| entity)
    }
    /// Mutably borrow the closest matching component in an inclusive XZ radius.
    /// Selection uses nearest_xz_where's global positions and entity-index ties.
    #[track_caller]
    pub fn nearest_xz_mut<C: Component>(
        &self,
        origin: impl Target,
        radius: f32,
        predicate: impl FnMut(&C) -> bool,
    ) -> Option<(Entity, RefMut<'_, C>)> {
        let entity = self.nearest_xz_where::<C>(origin, radius, predicate)?;
        Some((entity, self.get_mut::<C>(entity)?))
    }
    /// Other entities carrying C within an inclusive radius, in entity order.
    /// Distances and returned poses use global transforms; missing origins yield no rows.
    /// Poses are copied, so neither component storage stays borrowed.
    #[track_caller]
    pub fn near<C: Component>(
        &self,
        origin: impl Target,
        radius: f32,
    ) -> impl Iterator<Item = (Entity, crate::Transform)> + '_ {
        self.near_in::<C>(origin, radius, false, Location::caller())
    }
    /// Like near, ignoring Y. Rows contain the entity handle and its pose;
    /// both C and Transform remain free to borrow mutably inside the loop.
    #[track_caller]
    pub fn near_xz<C: Component>(
        &self,
        origin: impl Target,
        radius: f32,
    ) -> impl Iterator<Item = (Entity, crate::Transform)> + '_ {
        self.near_in::<C>(origin, radius, true, Location::caller())
    }
    fn near_in<C: Component>(
        &self,
        origin: impl Target,
        radius: f32,
        planar: bool,
        at: At,
    ) -> impl Iterator<Item = (Entity, crate::Transform)> + '_ {
        self.within::<C>(origin, radius, planar, at)
            .map(|(entity, pose, _)| {
                let (scale, rotation, position) = pose.to_scale_rotation_translation();
                (
                    entity,
                    crate::Transform {
                        position,
                        rotation,
                        scale,
                    },
                )
            })
    }
    fn within<C: Component>(
        &self,
        origin: impl Target,
        radius: f32,
        planar: bool,
        at: At,
    ) -> impl Iterator<Item = (Entity, crate::Affine3A, f32)> + '_ {
        assert!(radius.is_finite() && radius >= 0.0);
        let origin_entity = origin.entity(self);
        let origin = origin_entity
            .and_then(|e| self.current_global_at(e, at))
            .map(|pose| crate::Vec3::from(pose.translation));
        let candidates = origin.and(self.storage::<C>());
        candidates
            .into_iter()
            .flat_map(|s| s.indices(None))
            .filter_map(move |index| {
                let entity = self.entity_at(index);
                if Some(entity) == origin_entity {
                    return None;
                }
                let pose = self.current_global_at(entity, at)?;
                let mut delta = crate::Vec3::from(pose.translation) - origin?;
                if planar {
                    delta.y = 0.0;
                }
                let distance = delta.length_squared();
                (distance <= radius * radius).then_some((entity, pose, distance))
            })
    }
    /// Allocated component pages in entity-index order, under a shared lease.
    /// Each view supplies its first index, presence words, and a raw pointer valid
    /// for PAGE slots. Absent slots must not be read as C; only Plain has bytes().
    /// Exclusive borrows of any row of C are refused while the pages are held.
    #[track_caller]
    pub fn pages<C: Component>(&self) -> Pages<'_, C> {
        Pages::new(self.storage::<C>(), &self.leases, Location::caller())
            .unwrap_or_else(|conflict| self.refuse(conflict))
    }
    /// Mutation generation, including repeated edits within one tick. Not saved or hashed.
    pub fn revision<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, |s| s.revision())
    }
    /// Slots whose C row was handed out mutably, inserted or removed after `since`,
    /// a value of `revision::<C>()` read earlier in this presentation generation,
    /// in index order. Conservative: a mutable borrow counts as a write. Each item is
    /// the slot's current incarnation, which may be dead after a despawn; callers
    /// keyed by entity look the index up in their own records. Not saved or hashed.
    pub fn changed<C: Component>(&self, since: u64) -> impl Iterator<Item = Entity> + '_ {
        self.storage::<C>()
            .into_iter()
            .flat_map(move |s| s.changed(since))
            .map(|i| self.entity_at(i as usize))
    }
    /// Component membership generation; changing an existing value leaves it alone.
    pub fn membership<C: Component>(&self) -> u64 {
        self.storage::<C>().map_or(0, |s| s.membership())
    }
    /// Spawn/despawn generation, including equal-count slot recycling. Not simulation state.
    pub fn entities_revision(&self) -> u64 {
        self.entities_revision
    }
    /// Direct children in entity order. After a propagate with no Parent written
    /// or entity despawned since, this reads the hierarchy's links (O(children));
    /// otherwise it scans every Parent row.
    #[track_caller]
    pub fn children(&self, e: Entity) -> Vec<Entity> {
        if !self.contains(e) {
            return vec![];
        }
        if let Some(children) = self.hierarchy.children(self, e) {
            return children;
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
    #[track_caller]
    pub fn try_resource<R: Resource>(&self) -> Option<Ref<'_, R>> {
        self.resources
            .get(R::NAME)?
            .any()
            .downcast_ref::<storage::Singleton<R>>()?
            .get(Location::caller())
    }
    /// Borrow a resource; absence panics with its name.
    #[track_caller]
    pub fn resource<R: Resource>(&self) -> Ref<'_, R> {
        let at = Location::caller();
        self.resource_storage::<R>().get(at).unwrap()
    }
    /// Borrow a resource exclusively; absence panics with its name.
    #[track_caller]
    pub fn resource_mut<R: Resource>(&self) -> RefMut<'_, R> {
        let at = Location::caller();
        self.resource_storage::<R>().get_mut(at).unwrap()
    }
    /// Current fixed-step tick.
    pub fn tick(&self) -> u64 {
        self.state.tick
    }
    /// Fixed steps per second.
    pub fn hz(&self) -> u32 {
        self.state.hz
    }
    /// End of the step being authored, in the same units as `now()`.
    pub fn tick_end(&self) -> crate::Now {
        crate::Now {
            tick: self.tick().checked_add(1).expect("world clock exhausted"),
            hz: self.hz(),
        }
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
    #[track_caller]
    pub fn rng(&self) -> RefMut<'_, Rng> {
        self.rng.get_mut(Location::caller()).unwrap()
    }
    /// Draw one value and release the random column before returning.
    #[track_caller]
    pub fn rand<T: crate::RangeValue>(&self, range: std::ops::Range<T>) -> T {
        self.rng().range(range)
    }
    /// One Bernoulli trial, with probability in [0, 1].
    #[track_caller]
    pub fn chance(&self, p: f32) -> bool {
        self.rng().chance(p)
    }
    /// Choose a slice element, releasing the random column before returning.
    #[track_caller]
    pub fn pick<'a, T>(&self, items: &'a [T]) -> Option<&'a T> {
        self.rng().pick(items)
    }
    /// Append an event to the bounded 4,096-line journal.
    /// The journal is telemetry: a record outside the world hash and observation,
    /// so a read that logs must not change the world's course or mutation epoch.
    pub fn log(&self, line: impl std::fmt::Display) {
        self.log_args(format_args!("{line}"));
    }
    fn log_args(&self, line: std::fmt::Arguments<'_>) {
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
    pub fn publish<'a>(&self, key: &str, value: impl Into<crate::Published<'a>>) {
        self.publish_value(key, value.into().0);
    }
    pub(crate) fn publish_value(&self, key: &str, value: crate::values::Incoming<'_>) {
        let mut p = self.published.borrow_mut();
        let stored = p.get_mut(key);
        if stored
            .as_deref()
            .is_some_and(|stored| value.matches(stored))
        {
            return;
        }
        let value = value.into_stored();
        let mut line = crate::json::to_string(&value).unwrap_or_else(|e| e.to_string());
        // A large value (a whole inventory) journals its head and its size:
        // 4,096 lines of a 16 MiB record would hold 64 GiB.
        if line.len() > PUBLISH_LINE_BYTES {
            let bytes = line.len();
            line.truncate(line.floor_char_boundary(PUBLISH_LINE_BYTES));
            line.push_str(&format!("… ({bytes} bytes)"));
        }
        self.log(format_args!("publish {key}: {line}"));
        self.published_text.borrow_mut().remove(key);
        if let Some(stored) = stored {
            *stored = value;
        } else {
            p.insert(key.into(), value);
        }
        self.published_pending.set(true);
        self.mutated();
    }
    /// Queue a string event for the canvas's `message=` handler, in order, once.
    pub fn emit(&self, text: impl Into<String>) {
        self.messages.borrow_mut().push(text.into());
    }
    /// Last scalar, list or positional Contract value published under a key.
    /// Named nested records remain in take_published/agent JSON until shaped by the app.
    pub fn published(&self, key: &str) -> Option<Value> {
        self.published
            .borrow()
            .get(key)
            .and_then(crate::values::Stored::value)
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

    fn write(&self, w: &mut dyn Writer, at: At) {
        self.check_reads(at);
        w.begin_struct();
        w.field("state");
        self.state.write(w);
        w.field("rng");
        self.rng.get(at).unwrap().write(w);
        for (kind, storages) in [
            ("components", &self.components),
            ("resources", &self.resources),
        ] {
            w.field(kind);
            w.begin_struct();
            for (name, s) in storages {
                w.key(name);
                s.write_save(w);
            }
            w.end_struct();
        }
        self.assets.write_identity(w);
        if !self.messages.borrow().is_empty() {
            w.field("messages");
            self.messages.borrow().write(w);
        }
        w.end_struct();
    }
    /// Write a versioned save; NaNs are canonicalized and caches are excluded.
    #[track_caller]
    pub fn save(&self) -> Vec<u8> {
        let mut w = bin::Encoder::prefixed(MAGIC);
        self.write(&mut w, Location::caller());
        w.finish()
    }
    /// Atomically replace simulation state. Registered types survive the replacement;
    /// caches, publications and events do not. The entity table precedes storages.
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let payload = Self::saved_payload(bytes)?;
        let mut next = self.registered_scratch();
        let mut r = bin::Decoder::new(payload);
        next.read(&mut r).map_err(|e| e.at("World"))?;
        r.finish()?;
        next.validate_hierarchy(&mut r)?;
        next.presentation_generation = self
            .presentation_generation
            .checked_add(1)
            .expect("presentation generation exhausted");
        next.epoch.set(self.epoch.get().wrapping_add(1));
        // Delivery ownership is not saved state; transfer it only after validation.
        next.assets = std::mem::take(&mut self.assets);
        *self = next;
        Ok(())
    }
    // Decode with the live registration table, without gameplay setup side effects.
    pub(crate) fn registered_scratch(&self) -> Self {
        let mut scratch = Self::new(self.hz(), 0);
        scratch.registry = self.registry.clone();
        scratch.derived_publications = self.derived_publications.clone();
        scratch.attachments = self.attachments;
        scratch.detach = self.detach;
        scratch.assets = self.assets.clone();
        scratch
    }
    pub(crate) fn validate_saved(&self, bytes: &[u8]) -> Result<Self, DataError> {
        let mut scratch = self.registered_scratch();
        scratch.load(bytes)?;
        Ok(scratch)
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
        if self.hz() == 0 {
            return Err(DataError::new("hz must be positive"));
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
        *self.names.get_mut() = None;
        r.begin_struct()?;
        let mut seen = 0u8;
        while let Some(field) = r.field()? {
            seen |= match field.as_str() {
                "state" => 1,
                "rng" => 2,
                "components" => 4,
                "resources" => 8,
                "assetIdentity" => 16,
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
                "assetIdentity" => self.assets.read_identity(r)?,
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
                                    "unregistered {} `{name}`; call world.{}::<{name}>() in Game::register",
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
                        let make = make.ok_or_else(|| {
                            DataError::new(format!(
                                "`{name}` is registered as a {}; call world.{}::<{name}>() in Game::register to load {}",
                                if resource { "component" } else { "resource" },
                                if resource { "register_resource" } else { "register" },
                                if resource { "resources" } else { "components" }
                            ))
                        })?;
                        let mut s = make(key, self.epoch.clone());
                        let slots = &self.state.slots;
                        s.read_save(r, &|i| slots.get(i as usize).is_some_and(|s| s.alive))
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
        self.assets.require_identity(seen & 16 != 0)?;
        if seen & 15 != 15 {
            return Err(DataError::new("incomplete world save"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

mod inspect;
pub(crate) use inspect::ObservationState;

mod save;
use save::Free;

mod digest;
pub(crate) use digest::Observation;

#[cfg(test)]
mod nearest_xz_mut_tests {
    use super::*;
    use crate::Transform;
    #[derive(Default, crate::Component)]
    struct Beacon {
        lit: bool,
    }

    #[test]
    fn nearest_predicates_keep_entity_order_and_observe_in_loop_pose_edits() {
        let mut w = World::new(60, 0);
        w.spawn_named("player", crate::Transform::default());
        let a = w.spawn((crate::Transform::at(1., 0., 0.), Beacon { lit: true }));
        let b = w.spawn((crate::Transform::at(4., 0., 0.), Beacon { lit: false }));
        w.spawn((crate::Transform::at(9., 0., 0.), Beacon { lit: true }));
        let mut visits = Vec::new();
        let nearest = w.nearest_xz_where::<Beacon>("player", 1., |beacon| {
            visits.push(beacon.lit);
            w.get_mut::<crate::Transform>(b).unwrap().position.x = 1.;
            true
        });
        assert_eq!(nearest, Some(a));
        assert_eq!(visits, [true, false]);
        let held = w.get_mut::<Beacon>(a).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.nearest_xz::<Beacon>("player", 1.);
        }))
        .is_err());
        drop(held);
        assert_eq!(w.nearest_xz::<Beacon>("player", 1.), Some(a));
    }

    #[test]
    fn nearest_xz_mut_selects_filters_and_releases_its_guard() {
        let mut w = World::new(120, 7);
        w.spawn_named("player", Transform::default());
        let first = w.spawn((Transform::at(1.5, 10.0, 0.0), Beacon::default()));
        let second = w.spawn((Transform::at(-1.5, 0.0, 0.0), Beacon::default()));
        assert!(w
            .nearest_xz_mut::<Beacon>("missing", 1.5, |_| true)
            .is_none());
        assert!(w
            .nearest_xz_mut::<Beacon>("player", 1.49, |_| true)
            .is_none());
        if let Some((entity, mut beacon)) = w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit) {
            assert_eq!(entity, first);
            beacon.lit = true;
            assert_eq!(w.named("player").unwrap().index(), 0);
            assert_eq!(w.get::<Transform>(entity).unwrap().position.x, 1.5);
            // Other rows of Beacon stay free; the leased row refuses a second borrow.
            assert!(!w.get::<Beacon>(second).unwrap().lit);
            w.get_mut::<Beacon>(second).unwrap().lit = false;
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _same_row = w.get::<Beacon>(first);
            }))
            .is_err());
        }
        assert!(w.get::<Beacon>(first).unwrap().lit);
        assert!(!w.get::<Beacon>(second).unwrap().lit);
        assert_eq!(
            w.nearest_xz_where::<Beacon>("player", 1.5, |b| !b.lit),
            Some(second)
        );
        w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit)
            .unwrap()
            .1
            .lit = true;
        assert!(w
            .nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit)
            .is_none());
        let saved = w.save();
        w.load(&saved).unwrap();
        assert!(w.get::<Beacon>(first).unwrap().lit);
    }
}

#[cfg(test)]
mod required_tests {
    use super::*;
    #[test]
    fn required_components_accept_nearest_entities_and_reject_stale_handles() {
        let mut w = World::new(60, 0);
        w.spawn_named("player", crate::Transform::default());
        let fox = w.spawn(crate::Transform::at(1., 0., 0.));
        let nearest = w
            .nearest_xz_where::<crate::Transform>("player", 2., |t| t.position.x > 0.)
            .unwrap();
        assert_eq!(nearest, fox);
        w.require_mut::<crate::Transform>(nearest).position.x = 2.;
        assert_eq!(w.require::<crate::Transform>(nearest).position.x, 2.);
        w.despawn(fox);
        let replacement = w.spawn(crate::Transform::default());
        assert_eq!(replacement.index(), fox.index());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.require::<crate::Transform>(fox);
        }))
        .is_err());
    }
    #[test]
    fn required_components_name_both_failures() {
        let mut w = World::new(60, 0);
        w.spawn_named("fox", crate::Transform::default());
        w.require_mut::<crate::Transform>("fox").position.x = 3.;
        assert_eq!(w.require::<crate::Transform>("fox").position.x, 3.);
        for name in ["fox", "missing"] {
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                w.require_mut::<crate::Mesh>(name);
            }))
            .unwrap_err();
            let message = failure.downcast_ref::<String>().unwrap();
            assert!(
                message.contains(name) && message.contains("Mesh"),
                "{message}"
            );
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                w.require::<crate::Mesh>(name);
            }))
            .unwrap_err();
            let message = failure.downcast_ref::<String>().unwrap();
            assert!(
                message.contains(name) && message.contains("Mesh"),
                "{message}"
            );
        }
    }
}
