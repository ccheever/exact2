//! `Game::present`'s view of the world: deterministic simulation reads, and
//! writes to presentation components on existing entities only.
//! @ref llp/1046.008-game-presentation-seams.plan.md#amendment-2026-10-04
use crate::{Affine3A, Component, Entity, Ref, RefMut, Resource, Rng, Target, Value, World};

/// A component `Game::present` may write: one declared `#[derive(Presentation)]`.
/// Excluded from saves, hashes and rest; a tick can neither read nor write it.
pub trait PresentationComponent: Component {}

/// What `Game::present` can do. It reads the simulation as it stands at this
/// boundary and writes presentation components, which are cleared before every
/// present so each one rebuilds them from nothing. It has no simulation RNG,
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
}

impl<'w> Present<'w> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        Self { world }
    }
    /// Read one component by entity or name.
    #[track_caller]
    pub fn get<C: Component>(&self, target: impl Target) -> Option<Ref<'_, C>> {
        self.world.get(target)
    }
    /// Read one component, panicking with both names when it is absent.
    #[track_caller]
    pub fn require<C: Component>(&self, target: impl Target) -> Ref<'_, C> {
        self.world.require(target)
    }
    /// Whether a living entity has this component.
    pub fn has<C: Component>(&self, e: Entity) -> bool {
        self.world.has::<C>(e)
    }
    /// Visit one component column in stable entity order, read-only.
    #[track_caller]
    pub fn for_each<C: Component>(&self, mut visit: impl FnMut(Entity, &C)) {
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
    /// Insert or replace a presentation component on a living entity; false if
    /// the entity is gone.
    pub fn insert<C: PresentationComponent>(&mut self, e: Entity, value: C) -> bool {
        // Sealed by value as well as by trait: a hand-written impl on a
        // simulation component does not compile here.
        const { assert!(C::PRESENTATION, "only #[derive(Presentation)] components") };
        self.world.insert(e, value)
    }
    /// Change a presentation component written earlier in this present.
    #[track_caller]
    pub fn get_mut<C: PresentationComponent>(&self, target: impl Target) -> Option<RefMut<'_, C>> {
        const { assert!(C::PRESENTATION, "only #[derive(Presentation)] components") };
        self.world.get_mut(target)
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
