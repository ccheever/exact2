use exact_game::{Component, Data, Vec3};

/// How a collider participates in simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum BodyKind {
    /// Never moved by physics; zero inverse mass.
    Static,
    /// The game writes its pose; the solver observes its pose delta.
    Kinematic,
    /// Forces and contact impulses move this body.
    #[default]
    Dynamic,
}

/// A rigid body's controls and observed motion. Bodies must be hierarchy roots.
#[derive(Clone, Debug, PartialEq, Component)]
pub struct Body {
    /// Static, game-controlled, or simulated (the default).
    pub kind: BodyKind,
    /// World-space linear velocity in metres per second.
    pub velocity: Vec3,
    /// World-space angular velocity in radians per second.
    pub spin: Vec3,
    /// Kilograms; zero derives mass from volume at 1000 kg/m³.
    pub mass: f32,
    /// Multiplier of the Physics resource's gravity; defaults to one.
    pub gravity: f32,
    /// Linear drag rate in inverse seconds; defaults to zero.
    pub damping: f32,
    /// Angular drag rate in inverse seconds; defaults to zero.
    pub spin_damping: f32,
    /// Whether the whole touching island has gone to sleep.
    pub asleep: bool,
}
impl Default for Body {
    fn default() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            mass: 0.0,
            gravity: 1.0,
            damping: 0.0,
            spin_damping: 0.0,
            asleep: false,
        }
    }
}
