//! Walk cycles for part-built characters: an owner's `Gait` advances with the
//! distance it covers, and each `Limb` swings about its pivot from that phase.
use exact_game::*;

/// Distance-driven stride phase (radians) and the last tick's speed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Gait {
    pub phase: f32,
    pub speed: f32,
    /// Metres per full stride.
    pub stride: f32,
}
impl Gait {
    pub fn new(stride: f32) -> Self {
        Self {
            stride,
            ..Self::default()
        }
    }
    /// Advance by a planar displacement over one tick.
    pub fn walk(&mut self, moved: f32, dt: f32) {
        self.phase =
            (self.phase + moved / self.stride * std::f32::consts::TAU) % std::f32::consts::TAU;
        self.speed = moved / dt;
    }
}

/// A part that swings about its local X axis: `rest + swing × sin(phase + offset)`,
/// scaled by how fast its owner moves relative to `full` m/s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Limb {
    pub owner: Entity,
    pub offset: f32,
    pub swing: f32,
    pub rest: f32,
    pub full: f32,
}

/// Spawn a part under `parent` at `at`, swinging from `owner`'s gait.
#[allow(clippy::too_many_arguments)]
pub fn limb(
    w: &mut World,
    parent: Entity,
    owner: Entity,
    model: &str,
    at: Vec3,
    offset: f32,
    swing: f32,
    full: f32,
) -> Entity {
    w.spawn((
        Parent(parent),
        Transform::at(at.x, at.y, at.z),
        Mesh::asset(model),
        Limb {
            owner,
            offset,
            swing,
            rest: 0.0,
            full,
        },
    ))
}

/// The four legs of a quadruped: diagonal pairs move together.
pub fn legs(
    w: &mut World,
    body: Entity,
    model: &str,
    front: Vec3,
    back: Vec3,
    swing: f32,
    full: f32,
) {
    use std::f32::consts::PI;
    for (at, offset) in [
        (Vec3::new(front.x, front.y, front.z), 0.0),
        (Vec3::new(-front.x, front.y, front.z), PI),
        (Vec3::new(back.x, back.y, back.z), PI),
        (Vec3::new(-back.x, back.y, back.z), 0.0),
    ] {
        limb(w, body, body, model, at, offset, swing, full);
    }
}

/// Swing every limb from its owner's gait.
pub fn step(w: &World) {
    for (_, (pose, limb)) in w.query::<(&mut Transform, &Limb)>().iter() {
        let Some(gait) = w.get::<Gait>(limb.owner).map(|g| *g) else {
            continue;
        };
        let pace = (gait.speed / limb.full).min(1.0);
        let angle = limb.rest + limb.swing * pace * math::sin(gait.phase + limb.offset);
        let next = Quat::from_rotation_x(angle);
        if pose.rotation != next {
            pose.rotation = next;
        }
    }
}
