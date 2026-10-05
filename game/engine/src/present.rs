//! `Game::present`'s view of the world: deterministic simulation reads, and
//! writes to presentation components on existing entities only.
//! @ref llp/1046.008-game-presentation-seams.plan.md#amendment-2026-10-04
use crate::world::{Derivation, Presented, NOBODY};
use crate::{Affine3A, Component, Entity, Ref, Resource, Rng, Target, Value, World};
use std::panic::Location;

/// A component `Game::present` may write: one declared `#[derive(Presentation)]`.
/// Excluded from saves, hashes and rest; a tick can neither read nor write it.
pub trait PresentationComponent: Component {}

/// What `Game::present` can do. It reads the simulation as it stands at this
/// boundary and writes presentation components. A present describes all of
/// presentation, as if rebuilt from nothing: a row it does not write again is
/// removed, and one written with the value it holds is not a change, so the
/// renderer's work follows what differs. Rows derived per entity with
/// [`Present::each`] are kept while that entity's keys are unchanged, so a
/// look over every entity costs what changed. It has no simulation RNG,
/// events, entity allocation, publications or busy reasons: those are
/// simulation state a restore would replay. Writes outside it panic (a second,
/// unrecoverable layer behind this type). Interior mutability inside a
/// component (a `Cell` or `RefCell` field, typically `#[data(skip)]`) is outside
/// this guarantee: a present that writes through one is not refused.
///
/// What a present does, and the two things it cannot:
///
/// ```
/// fn present(p: &mut exact_game::Present<'_>, e: exact_game::Entity) {
///     p.insert(e, exact_game::Offset(exact_game::Transform::at(0., 0.1, 0.)));
/// }
/// ```
///
/// ```compile_fail
/// fn present(p: &mut exact_game::Present<'_>, _: &()) {
///     p.spawn(()); // entities are simulation state
/// }
/// ```
///
/// ```
/// fn present(p: &mut exact_game::Present<'_>, e: exact_game::Entity) {
///     p.insert(e, exact_game::Tint::default()); // a presentation component
/// }
/// ```
///
/// ```compile_fail
/// fn present(p: &mut exact_game::Present<'_>, e: exact_game::Entity) {
///     p.insert(e, exact_game::Transform::default()); // not a presentation component
/// }
/// ```
///
/// It reads the simulation, never presentation (which it is writing):
///
/// ```compile_fail
/// fn present(p: &mut exact_game::Present<'_>, e: exact_game::Entity) {
///     let _ = p.get::<exact_game::Offset>(e);
/// }
/// let _reachable: fn(&mut exact_game::Present<'_>, exact_game::Entity) = present;
/// ```
///
/// A hand-written marker on a simulation component does not get through either:
///
/// ```compile_fail
/// #[derive(Default, exact_game::Component)]
/// struct Hp(u32);
/// impl exact_game::PresentationComponent for Hp {}
/// fn present(p: &mut exact_game::Present<'_>, e: exact_game::Entity) {
///     p.insert(e, Hp(1));
/// }
/// let _reachable: fn(&mut exact_game::Present<'_>, exact_game::Entity) = present;
/// ```
pub struct Present<'w> {
    world: &'w mut World,
    // Taken from the world for this present, returned by `finish`.
    presented: Presented,
    deriving: Option<Deriving>,
    // Derivations whose kept rows the frame or another derivation took over
    // before they were called: each must not be called later in this present.
    taken: Vec<u32>,
    scratch: Vec<&'static str>,
}

/// The entity a derivation is deriving, and the components it wrote.
struct Deriving {
    id: u32,
    entity: Entity,
    at: &'static Location<'static>,
    written: Vec<&'static str>,
}

/// How long the rows a derivation wrote for an entity stay ([`Present::each`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Derived {
    /// Until a key row of the entity changes (or it loses the first key).
    Kept,
    /// Until the next present, which derives the entity again: these rows also
    /// follow the time (a shimmer, a sway), so they cost a call per present.
    Animated,
}

/// The simulation components a derivation is keyed on ([`Present::each`]):
/// one component or a tuple of up to four. The entities derived are those with
/// the first; an entity is derived again when a row of any of them is written,
/// inserted or removed (a mutable borrow counts as a write).
pub trait Keys {
    #[doc(hidden)]
    fn revisions(w: &World) -> Vec<u64>;
    #[doc(hidden)]
    fn changed(w: &World, since: &[u64], out: &mut Vec<u32>);
    #[doc(hidden)]
    fn all(w: &World) -> Vec<u32>;
    #[doc(hidden)]
    fn has(w: &World, e: Entity) -> bool;
}
macro_rules! keys {
    ($first:ident $(, $rest:ident)*) => {
        impl<$first: Component $(, $rest: Component)*> Keys for ($first, $($rest,)*) {
            fn revisions(w: &World) -> Vec<u64> {
                const {
                    assert!(
                        !$first::PRESENTATION $(&& !$rest::PRESENTATION)*,
                        "derivations are keyed on simulation components"
                    )
                };
                vec![w.revision::<$first>() $(, w.revision::<$rest>())*]
            }
            fn changed(w: &World, since: &[u64], out: &mut Vec<u32>) {
                let mut since = since.iter().copied();
                let mut next = || since.next().unwrap_or(0);
                out.extend(w.changed::<$first>(next()).map(|e| e.index()));
                $(out.extend(w.changed::<$rest>(next()).map(|e| e.index()));)*
            }
            fn all(w: &World) -> Vec<u32> {
                w.query::<&$first>().iter().map(|(e, _)| e.index()).collect()
            }
            fn has(w: &World, e: Entity) -> bool {
                w.has::<$first>(e)
            }
        }
    };
}
keys!(A);
keys!(A, B);
keys!(A, B, C);
keys!(A, B, C, D);
impl<A: Component> Keys for A {
    fn revisions(w: &World) -> Vec<u64> {
        <(A,)>::revisions(w)
    }
    fn changed(w: &World, since: &[u64], out: &mut Vec<u32>) {
        <(A,)>::changed(w, since, out);
    }
    fn all(w: &World) -> Vec<u32> {
        <(A,)>::all(w)
    }
    fn has(w: &World, e: Entity) -> bool {
        <(A,)>::has(w, e)
    }
}

impl<'w> Present<'w> {
    /// Start a present. A full one (after setup, a restore, an argument change
    /// or a `world_mut` edit) erases every row and derives every entity again.
    pub(crate) fn begin(world: &'w mut World, full: bool) -> Self {
        if full {
            world.clear_presentation();
        }
        let mut presented = std::mem::take(&mut world.presented);
        presented.count += 1;
        std::mem::swap(&mut presented.frame, &mut presented.previous);
        presented.frame.clear();
        for d in &mut presented.derivations {
            d.called = false;
        }
        Self {
            world,
            presented,
            deriving: None,
            taken: Vec::new(),
            scratch: Vec::new(),
        }
    }
    /// End a present: remove the rows it did not write again (outside `each`)
    /// and those of derivations it did not call.
    pub(crate) fn finish(mut self) {
        let stale = Presented::frame_writer(self.presented.count - 1);
        let previous = std::mem::take(&mut self.presented.previous);
        for &(name, e) in &previous {
            let index = e.index() as usize;
            if self.world.contains(e)
                && self.presented.owner(name, index) == stale
                && self.world.has_presentation(name, index)
            {
                self.world.erase_presentation(name, index);
                self.presented.set_owner(name, index, NOBODY);
            }
        }
        self.presented.previous = previous;
        let derivations = std::mem::take(&mut self.presented.derivations);
        let (called, gone): (Vec<Derivation>, Vec<Derivation>) =
            derivations.into_iter().partition(|d| d.called);
        if let Some(d) = called.iter().find(|d| self.taken.contains(&d.id)) {
            panic!(
                "Game::present wrote a row that the `each` at {} keeps, before calling it; each row has one writer per present",
                d.at
            );
        }
        for d in &gone {
            self.presented.retire(d, self.world);
        }
        self.presented.derivations = called;
        self.world.presented = std::mem::take(&mut self.presented);
    }
    /// Read one component by entity or name.
    #[track_caller]
    pub fn get<C: Component>(&self, target: impl Target) -> Option<Ref<'_, C>> {
        const { assert!(!C::PRESENTATION, "Game::present reads the simulation") };
        self.world.get(target)
    }
    /// Read one component, panicking with both names when it is absent.
    #[track_caller]
    pub fn require<C: Component>(&self, target: impl Target) -> Ref<'_, C> {
        const { assert!(!C::PRESENTATION, "Game::present reads the simulation") };
        self.world.require(target)
    }
    /// Whether a living entity has this component.
    pub fn has<C: Component>(&self, e: Entity) -> bool {
        const { assert!(!C::PRESENTATION, "Game::present reads the simulation") };
        self.world.has::<C>(e)
    }
    /// Visit one component column in stable entity order, read-only.
    #[track_caller]
    pub fn for_each<C: Component>(&self, mut visit: impl FnMut(Entity, &C)) {
        const { assert!(!C::PRESENTATION, "Game::present reads the simulation") };
        for (e, value) in self.world.query::<&C>().iter() {
            visit(e, value);
        }
    }
    /// Resolve a named entity.
    pub fn named(&self, name: &str) -> Option<Entity> {
        self.world.named(name)
    }
    /// An optional resource, read-only.
    pub fn resource<R: Resource>(&self) -> Option<Ref<'_, R>> {
        self.world.try_resource::<R>()
    }
    /// The entity's global pose at this boundary, parents included.
    pub fn global(&self, e: Entity) -> Option<Affine3A> {
        self.world.current_global(e)
    }
    /// Whether the entity is drawn at all: its `Visible` and its Parent chain's.
    pub fn is_visible(&self, e: Entity) -> bool {
        self.world.is_visible(e)
    }
    /// The last value published under `key`.
    pub fn published(&self, key: &str) -> Option<Value> {
        self.world.published(key)
    }
    /// The completed tick this present shows.
    pub fn tick(&self) -> u64 {
        self.world.tick()
    }
    /// Simulation ticks per second.
    pub fn hz(&self) -> u32 {
        self.world.hz()
    }
    /// Simulation seconds at this boundary.
    pub fn seconds(&self) -> f64 {
        self.world.seconds()
    }
    /// The world seed.
    pub fn seed(&self) -> u64 {
        self.world.seed()
    }
    /// A presentation random stream from the seed, this tick and `salt`: the
    /// same draws every time this boundary is presented, never the world's.
    pub fn rng(&self, salt: u64) -> Rng {
        self.world.presentation_rng(salt)
    }
    /// Write a presentation component on a living entity (false if it is gone).
    /// Writing the value the row already holds (bit for bit) changes nothing. Each row has one
    /// writer per present: the code outside `each`, or one derivation.
    pub fn insert<C: PresentationComponent>(&mut self, e: Entity, value: C) -> bool {
        // Sealed by value as well as by trait: a hand-written impl on a
        // simulation component does not compile here.
        const { assert!(C::PRESENTATION, "only #[derive(Presentation)] components") };
        if !self.world.contains(e) {
            return false;
        }
        let writer = match &self.deriving {
            Some(d) if d.entity != e => panic!(
                "Game::present: the `each` at {} deriving #{} wrote `{}` on #{}; a derivation writes only its own entity's rows",
                d.at,
                d.entity.index(),
                C::NAME,
                e.index()
            ),
            Some(d) => d.id,
            None => Presented::frame_writer(self.presented.count),
        };
        let index = e.index() as usize;
        let exists = self.world.has::<C>(e);
        let owner = if exists {
            self.presented.owner(C::NAME, index)
        } else {
            NOBODY
        };
        if exists && owner != writer {
            self.claim(C::NAME, e, owner);
        }
        // Bit for bit, as saves and digests compare (negative zero is not zero).
        let same = exists
            && self
                .world
                .get::<C>(e)
                .is_some_and(|row| crate::hash::of(&*row) == crate::hash::of(&value));
        if !same && !self.world.insert(e, value) {
            return false;
        }
        if owner != writer {
            self.presented.set_owner(C::NAME, index, writer);
        }
        match &mut self.deriving {
            Some(d) => {
                if !d.written.contains(&C::NAME) {
                    d.written.push(C::NAME);
                }
            }
            None if owner != writer => self.presented.frame.push((C::NAME, e)),
            None => {}
        }
        true
    }
    // A row another writer holds: the last present's frame row is free to take;
    // a row written earlier in this present by someone else is refused.
    fn claim(&mut self, name: &'static str, e: Entity, owner: u32) {
        let count = self.presented.count;
        if owner == NOBODY || owner == Presented::frame_writer(count - 1) {
            return;
        }
        let by = match self.presented.derivations.iter().find(|d| d.id == owner) {
            Some(d) if !d.called => {
                // Kept by a derivation not called yet: refused at `finish` if
                // it is called later in this present.
                self.taken.push(d.id);
                return;
            }
            Some(d) => format!("the `each` at {}", d.at),
            None if owner == Presented::frame_writer(count) => "code outside `each`".into(),
            None => return,
        };
        let to = match &self.deriving {
            Some(d) => format!("the `each` at {}", d.at),
            None => "code outside `each`".into(),
        };
        panic!(
            "Game::present wrote `{name}` on #{} from {by} and from {to}; each row has one writer per present",
            e.index()
        );
    }
    /// Derive presentation for each entity with a `K` row (`K` is a simulation
    /// component or a tuple of them; see [`Keys`]), and keep it. `derive(p, e)`
    /// writes `e`'s rows only, reading `e`'s keys and anything that cannot
    /// change while they do not (the arguments, constants, another entity's
    /// row fixed for `e`'s life). At a full present every entity is derived;
    /// at later boundaries only those whose key rows changed since, and those
    /// whose last derivation returned [`Derived::Animated`]. Rows it wrote
    /// before and not again are removed; an entity that lost its first key
    /// loses its derived rows. So a look over every plant of a garden costs
    /// the plants that grew. Paranoid modes compare every kept row with a
    /// fresh present. Call it once per present at each call site.
    ///
    /// ```
    /// # use exact_game::*;
    /// # #[derive(Default, Component)] struct Plant { stage: u8 }
    /// fn present(p: &mut Present<'_>) {
    ///     p.each::<Plant>(|p, e| {
    ///         let stage = p.require::<Plant>(e).stage;
    ///         p.insert(e, DrawnMesh::model(format!("plant-{stage}.model")));
    ///         Derived::Kept
    ///     });
    /// }
    /// ```
    #[track_caller]
    pub fn each<K: Keys>(&mut self, mut derive: impl FnMut(&mut Present<'_>, Entity) -> Derived) {
        let at = Location::caller();
        if let Some(d) = &self.deriving {
            panic!(
                "Game::present: `each` at {at} inside the `each` at {}",
                d.at
            );
        }
        let mut todo;
        let slot = match self.presented.derivations.iter().position(|d| d.at == at) {
            Some(slot) => {
                let d = &mut self.presented.derivations[slot];
                assert!(
                    !d.called,
                    "Game::present called the `each` at {at} twice in one present"
                );
                todo = std::mem::take(&mut d.animated)
                    .into_iter()
                    .map(|e| e.index())
                    .collect();
                K::changed(self.world, &d.since, &mut todo);
                todo.sort_unstable();
                todo.dedup();
                slot
            }
            None => {
                let id = self.presented.next_id();
                self.presented.derivations.push(Derivation {
                    at,
                    id,
                    since: Vec::new(),
                    components: Vec::new(),
                    animated: Vec::new(),
                    called: false,
                });
                todo = K::all(self.world);
                self.presented.derivations.len() - 1
            }
        };
        let d = &mut self.presented.derivations[slot];
        d.called = true;
        d.since = K::revisions(self.world);
        let id = d.id;
        let mut animated = Vec::new();
        for index in todo {
            let e = self.world.entity_at(index as usize);
            if !self.world.contains(e) {
                continue;
            }
            self.deriving = Some(Deriving {
                id,
                entity: e,
                at,
                written: std::mem::take(&mut self.scratch),
            });
            let again = K::has(self.world, e) && derive(self, e) == Derived::Animated;
            let mut written = self.deriving.take().map(|d| d.written).unwrap_or_default();
            let mut components = std::mem::take(&mut self.presented.derivations[slot].components);
            for &name in &components {
                let index = index as usize;
                if !written.contains(&name)
                    && self.presented.owner(name, index) == id
                    && self.world.has_presentation(name, index)
                {
                    self.world.erase_presentation(name, index);
                    self.presented.set_owner(name, index, NOBODY);
                }
            }
            for &name in &written {
                if !components.contains(&name) {
                    components.push(name);
                }
            }
            self.presented.derivations[slot].components = components;
            if again {
                animated.push(e);
            }
            written.clear();
            self.scratch = written;
        }
        self.presented.derivations[slot].animated = animated;
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    // The second layer: World writes panic while presenting, naming the write,
    // and the guard stays armed after the panic is caught.
    #[test]
    fn simulation_writes_panic_while_presenting_and_stay_refused() {
        type Change = fn(&mut World);
        let cases: [(Change, &str); 10] = [
            (|w| w.busy("drawing"), "reported busy `drawing`"),
            (
                |w| w.require_mut::<Transform>("crate").position.x += 1.,
                "wrote component `Transform`",
            ),
            (
                |w| {
                    let e = w.named("crate").unwrap();
                    w.insert(e, Mesh::cube(1.));
                },
                "inserted component `Mesh`",
            ),
            (
                |w| {
                    for (_, t) in w.query::<&mut Transform>().iter() {
                        t.position.y = 1.;
                    }
                },
                "queried `Transform` mutably",
            ),
            (
                |w| {
                    w.rand(0..3u32);
                },
                "drew from World::rng",
            ),
            (|w| w.emit("hello"), "emitted a message"),
            (|w| w.log("note"), "journaled `note`"),
            (|w| w.publish("score", 3.0), "published `score`"),
            (
                |w| {
                    w.spawn(());
                },
                "spawned an entity",
            ),
            (|w| emitter::step(w), "queried `Emitter` mutably"),
        ];
        for (change, named) in cases {
            let mut w = World::new(60, 1);
            w.spawn_named("crate", Transform::default());
            w.presenting.set(true);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| change(&mut w)));
            let message = caught
                .expect_err(named)
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default();
            assert!(
                message.contains("Game::present changed simulation state")
                    && message.contains(named),
                "{named}: {message}"
            );
            assert!(
                w.presenting.get(),
                "{named}: a caught panic must not disarm the guard"
            );
            let again = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.emit("retry")));
            assert!(again.is_err(), "{named}: a retry still panics");
        }
    }
}
