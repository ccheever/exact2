//! Small deterministic rigid bodies: every persistent value lives in the World.
//!
//! Register during setup, move characters and kinematics in game code, then call
//! [`step`] once per fixed tick. Coordinates are metres, kilograms, and seconds.
//! Curved colliders use uniform scale; parented boxes must not acquire shear.
#![deny(unsafe_code)]
#![deny(missing_docs)]

mod body;
mod character;
mod math;
mod queries;
mod state;
mod step;
mod types;

pub use character::{capsule, CapsuleHandle, CapsuleStep};
use exact_game::{Ref, World};
pub use queries::{overlap, queries, raycast, sweep, Queries};
pub use step::step;
pub use types::*;

/// Register all physics data before setup or loading a save, and install defaults.
/// Call once during game setup; calling again resets the Physics resource.
pub fn register(world: &mut World) {
    world
        .register::<exact_game::Transform>()
        .register::<Body>()
        .register::<Collider>()
        .register::<Announce>()
        .register::<CapsuleController>();
    world.insert_resource(Physics::default());
}

/// A borrowed event slice; keeps the engine's resource read lease alive.
pub struct Touches<'a>(Ref<'a, Physics>);
impl std::ops::Deref for Touches<'_> {
    type Target = [Touch];
    fn deref(&self) -> &[Touch] {
        &self.0.events
    }
}
/// This tick's ordered transitions. The guard dereferences to `[Touch]`.
/// A bare slice would outlive the engine's dynamic storage lease and be unsound.
pub fn events(world: &World) -> Touches<'_> {
    Touches(world.resource::<Physics>())
}

/// Whether every dynamic Body is asleep, including bodies without colliders.
pub fn quiescent(world: &World) -> bool {
    world
        .query::<&Body>()
        .iter()
        .all(|(_, b)| b.kind != BodyKind::Dynamic || b.asleep)
}
