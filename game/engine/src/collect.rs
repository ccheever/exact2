//! Pickups that drift to a collector: coins to a player, food to a fish.
//!
//! [`drift`] is a pure function the game calls from its tick for each pickup;
//! the game writes the position back, and on `taken` removes the entity and
//! credits itself. [`Collectable`] marks pickups for queries:
//! `w.query::<&mut Transform>().with::<Collectable>()` or
//! `w.nearest_xz::<Collectable>("player", radius)`. Adding it to a game changes
//! the world hash, as any new component type does.
use crate::Vec3;

/// A pickup: a marker for queries, saved with the entity. A game that spawns
/// pickups after setup registers it in `Game::register`
/// (`w.register::<Collectable>()`), so a save holding them restores in a fresh process.
#[derive(Clone, Copy, Debug, Default, crate::Component)]
pub struct Collectable;

/// Where a pickup is after this tick, and whether it reached its collector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drift {
    /// The pickup's position after this tick.
    pub position: Vec3,
    /// True on the tick the pickup reaches the collector, and never before.
    pub taken: bool,
}

/// Move `item` toward `collector` at `speed` metres per second while it is
/// within `radius` metres of it (inclusive), never past it. On the tick the
/// step reaches the collector, `position` is the collector's and `taken` is
/// true. Outside the radius the pickup stays where it is.
pub fn drift(item: Vec3, collector: Vec3, radius: f32, speed: f32, dt: f32) -> Drift {
    assert!(
        radius.is_finite() && radius >= 0.0,
        "drift: radius {radius}"
    );
    assert!(speed.is_finite() && speed >= 0.0, "drift: speed {speed}");
    assert!(dt.is_finite() && dt > 0.0, "drift: dt {dt}");
    let toward = collector - item;
    let distance = toward.length();
    if distance > radius {
        return Drift {
            position: item,
            taken: false,
        };
    }
    let step = speed * dt;
    if step >= distance {
        return Drift {
            position: collector,
            taken: true,
        };
    }
    Drift {
        position: item + toward * (step / distance),
        taken: false,
    }
}
