use exact_game::{Component, Data, Entity, Resource, Transform, Vec2, Vec3};

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

/// A rigid body's complete persistent state. Bodies must be hierarchy roots.
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
    /// Consecutive ticks below both sleep speed thresholds.
    pub calm: u32,
    /// Last post-step state, saved to detect game edits and infer kinematic motion.
    pub previous: Option<BodyState>,
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
            calm: 0,
            previous: None,
        }
    }
}

/// Post-step values used for change detection; these are simulation state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub struct BodyState {
    /// Previous post-step root pose.
    pub pose: Transform,
    /// Previous post-step linear velocity.
    pub velocity: Vec3,
    /// Previous post-step angular velocity.
    pub spin: Vec3,
}

/// Convex geometry in local metres. Curved shapes require uniform positive scale.
#[derive(Clone, Debug, PartialEq, Data)]
pub enum Shape {
    /// A sphere centered on the entity.
    Sphere {
        /// Positive radius in metres.
        radius: f32,
    },
    /// A Y-axis capsule centered on the entity, including its hemispheres.
    Capsule {
        /// Positive hemisphere radius.
        radius: f32,
        /// Total height, at least twice the radius.
        height: f32,
    },
    /// An oriented box centered on the entity.
    Box {
        /// Positive half-extents along local X, Y, Z.
        half: Vec3,
    },
}
impl Default for Shape {
    fn default() -> Self {
        Self::Box {
            half: Vec3::splat(0.5),
        }
    }
}

/// Collision material and filtering; a collider without a Body is static.
#[derive(Clone, Debug, PartialEq, Component)]
pub struct Collider {
    /// Local geometry.
    pub shape: Shape,
    /// Coulomb coefficient; pair coefficients combine geometrically. Default 0.6.
    pub friction: f32,
    /// Restitution in [0, 1]; a pair uses the larger value. Default zero.
    pub bounce: f32,
    /// Report touches but never apply impulses.
    pub sensor: bool,
    /// Membership bits; defaults to bit one.
    pub layer: u32,
    /// Accepted membership bits; defaults to all bits.
    pub mask: u32,
}
impl Default for Collider {
    fn default() -> Self {
        Self {
            shape: Shape::default(),
            friction: 0.6,
            bounce: 0.0,
            sensor: false,
            layer: 1,
            mask: u32::MAX,
        }
    }
}

/// Opt a collider into begin/end journal messages. No per-tick samples are logged.
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct Announce;

/// One world-owned, warm-started contact point.
#[derive(Clone, Debug, Default, Data)]
pub struct Contact {
    /// Stable geometric feature identifier within the ordered pair.
    pub feature: u32,
    /// Anchor relative to A's position in A's rotation frame (scale baked in).
    pub local_a: Vec3,
    /// Anchor relative to B's position in B's rotation frame (scale baked in).
    pub local_b: Vec3,
    /// Signed surface separation in metres; negative means penetration.
    pub separation: f32,
    /// Accumulated nonnegative normal impulse, in newton seconds.
    pub normal_impulse: f32,
    /// Accumulated impulses along the manifold's deterministic tangent basis.
    pub tangent_impulse: Vec2,
}

/// Persistent contact manifold; Physics stores these in ordered entity-pair order.
#[derive(Clone, Debug, Default, Data)]
pub struct Manifold {
    /// Lower-index entity (including its generation).
    pub a: Entity,
    /// Higher-index entity (including its generation).
    pub b: Entity,
    /// Unit normal from A toward B.
    pub normal: Vec3,
    /// At most four points, sorted by feature id.
    pub points: Vec<Contact>,
    /// Whether either collider is a sensor.
    pub sensor: bool,
    /// Whether the surfaces are touching, rather than merely speculative.
    pub touching: bool,
}

/// One begin/end transition, ordered by (a.index, b.index).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub struct Touch {
    /// First entity, possibly despawned for an end event.
    pub a: Entity,
    /// Second entity, possibly despawned for an end event.
    pub b: Entity,
    /// True on begin, false on end.
    pub began: bool,
}

/// Collider state used to wake sleepers when a supporter changes or disappears.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct ColliderState {
    /// Generational entity identity.
    pub entity: Entity,
    /// Previous world pose.
    pub pose: Transform,
    /// Previous geometry and material/filter values.
    pub collider: Collider,
}

/// All cross-tick physics state; no opaque solver exists beside the world.
#[derive(Clone, Debug, Resource)]
pub struct Physics {
    #[data(skip)]
    pub(crate) scratch: crate::scratch::Scratch,
    /// World-space acceleration; defaults to (0, -9.81, 0) m/s².
    pub gravity: Vec3,
    /// Soft steps per world tick, at least one; defaults to eight.
    pub substeps: u32,
    /// Persistent, sorted contact manifolds including their warm-start impulses.
    pub manifolds: Vec<Manifold>,
    /// This tick's sorted begin/end transitions, including sensors.
    pub events: Vec<Touch>,
    /// Last collider poses and geometry, sorted by entity, for supporter invalidation.
    pub previous: Vec<ColliderState>,
}
impl Default for Physics {
    fn default() -> Self {
        Self {
            scratch: Default::default(),
            gravity: Vec3::new(0.0, -9.81, 0.0),
            substeps: 8,
            manifolds: Vec::new(),
            events: Vec::new(),
            previous: Vec::new(),
        }
    }
}

/// A game-controlled upright capsule. The game owns vertical velocity and gravity.
#[derive(Clone, Debug, Component)]
pub struct Character {
    /// Capsule radius, default 0.3 m.
    pub radius: f32,
    /// Total capsule height, default 1.8 m.
    pub height: f32,
    /// Maximum upward step, default 0.3 m.
    pub step: f32,
    /// Maximum walkable slope, default 50 degrees.
    pub slope_degrees: f32,
    /// Actual horizontal velocity and game-owned vertical velocity, in m/s.
    pub velocity: Vec3,
    /// Whether the last move found walkable ground.
    pub grounded: bool,
    /// Mass budget for pushing dynamic bodies, default 80 kg.
    pub mass: f32,
    /// Last supporting entity, if any.
    pub support: Option<Entity>,
    /// Saved support pose for moving-platform transport.
    pub support_pose: Transform,
}
impl Default for Character {
    fn default() -> Self {
        Self {
            radius: 0.3,
            height: 1.8,
            step: 0.3,
            slope_degrees: 50.0,
            velocity: Vec3::ZERO,
            grounded: false,
            mass: 80.0,
            support: None,
            support_pose: Transform::default(),
        }
    }
}

/// The first intersection along a ray or translational sweep.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    /// Collider hit, with ties resolved in entity order.
    pub entity: Entity,
    /// Travel in metres, measured along the normalized direction.
    pub distance: f32,
    /// World-space point on the target surface.
    pub point: Vec3,
    /// Outward target surface normal.
    pub normal: Vec3,
}
