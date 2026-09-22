use crate::{math, queries::queries, Body, BodyKind, CapsuleController, Collider, Shape};
use exact_game::{Entity, Target, Transform, Vec3, World};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::{
    control::{CharacterAutostep, CharacterLength, KinematicCharacterController},
    prelude::*,
};

/// Move an upright capsule through Rapier's character controller, then apply its
/// documented collision impulses to bodies within CapsuleController.mass's push budget.
/// Horizontal input is m/s; the game owns CapsuleController.velocity.y (gravity/jumps).
/// Installs a kinematic sensor Body/Collider. Call before physics::step.
fn move_capsule(world: &mut World, e: Entity, desired_velocity: Vec3) -> CapsuleStep {
    let mut c = world
        .get::<CapsuleController>(e)
        .expect("physics: entity needs CapsuleController")
        .clone();
    let mut pose = *world
        .get::<Transform>(e)
        .expect("physics: character needs Transform");
    let before = pose.position;
    assert!(
        pose.rotation == exact_game::Quat::IDENTITY && pose.scale == Vec3::ONE,
        "physics: characters must be upright with unit scale"
    );
    assert!(
        desired_velocity.is_finite()
            && c.velocity.is_finite()
            && c.mass.is_finite()
            && c.mass > 0.0
            && c.step.is_finite()
            && c.step >= 0.0
            && (0.0..90.0).contains(&c.slope_degrees),
        "physics: invalid CapsuleController"
    );
    if !world.has::<Body>(e) {
        world.insert(
            e,
            Body {
                kind: BodyKind::Kinematic,
                ..Body::default()
            },
        );
    }
    assert!(
        world.get::<Body>(e).unwrap().kind == BodyKind::Kinematic,
        "physics: character Body must be Kinematic"
    );
    let mut collider = world
        .get::<Collider>(e)
        .map(|c| c.clone())
        .unwrap_or_default();
    collider.shape = Shape::Capsule {
        radius: c.radius,
        height: c.height,
    };
    collider.sensor = true;
    let shape = math::shape(&collider.shape, Vec3::ONE);
    let mask = collider.mask;
    if world.get::<Collider>(e).as_deref() != Some(&collider) {
        world.insert(e, collider);
    }
    let view = queries(world);
    let mut scene_guard = view.scene();
    let scene = &mut *scene_guard;
    let own = scene
        .rapier
        .colliders
        .iter()
        .find_map(|(h, _)| (scene.entity(h) == e).then_some(h))
        .unwrap();
    let predicate = |_: ColliderHandle, co: &rapier3d::prelude::Collider| {
        co.collision_groups().memberships.bits() & mask != 0
    };
    let filter = QueryFilter::default()
        .exclude_sensors()
        .exclude_collider(own)
        .predicate(&predicate);
    let angle = c.slope_degrees * (std::f32::consts::PI / 180.0);
    let controller = KinematicCharacterController {
        offset: CharacterLength::Absolute(0.01),
        autostep: (c.step > 0.0).then_some(CharacterAutostep {
            max_height: CharacterLength::Absolute(c.step),
            min_width: CharacterLength::Absolute(c.radius + 0.01),
            include_dynamic_bodies: false,
        }),
        max_slope_climb_angle: angle,
        min_slope_slide_angle: angle,
        snap_to_ground: Some(CharacterLength::Absolute(c.step + 0.02)),
        ..KinematicCharacterController::default()
    };
    let q = scene.queries(filter);
    if c.grounded {
        if let Some(support) = c.support.filter(|s| world.contains(*s)) {
            let current = math::world_pose(world, support);
            let local =
                c.support_pose.rotation.conjugate() * (pose.position - c.support_pose.position);
            let delta = current.position + current.rotation * local - pose.position;
            let carry = controller.move_shape(
                world.dt(),
                &q,
                &*shape,
                &math::pose(pose),
                math::vector(delta),
                |_| {},
            );
            pose.position += math::vec3(carry.translation);
        }
    }
    if c.grounded && c.velocity.y < 0.0 {
        c.velocity.y = 0.0;
    }
    let velocity = Vec3::new(desired_velocity.x, c.velocity.y, desired_velocity.z);
    let mut collisions = Vec::new();
    let movement = controller.move_shape(
        world.dt(),
        &q,
        &*shape,
        &math::pose(pose),
        math::vector(velocity * world.dt()),
        |hit| collisions.push(hit),
    );
    pose.position += math::vec3(movement.translation);
    c.grounded = movement.grounded;
    c.velocity.x = movement.translation.x / world.dt();
    c.velocity.z = movement.translation.z / world.dt();
    if c.grounded && c.velocity.y < 0.0 {
        c.velocity.y = 0.0;
    }
    c.support = q
        .cast_shape(
            &math::pose(pose),
            -Vector::Y,
            &*shape,
            ShapeCastOptions {
                max_time_of_impact: 0.03,
                ..ShapeCastOptions::default()
            },
        )
        .filter(|(_, h)| h.normal1.y >= exact_game::math::cos(angle))
        .map(|(h, _)| scene.entity(h));
    if let Some(s) = c.support {
        c.support_pose = math::world_pose(world, s);
    }
    let allowed: std::collections::BTreeSet<_> = scene
        .rapier
        .colliders
        .iter()
        .filter(|(_, co)| {
            co.parent()
                .is_some_and(|h| scene.rapier.bodies[h].mass() <= c.mass)
        })
        .map(|(h, _)| crate::state::raw(h))
        .collect();
    let push_filter = |h: ColliderHandle, co: &rapier3d::prelude::Collider| {
        predicate(h, co) && allowed.contains(&crate::state::raw(h))
    };
    let entities = &scene.entities;
    let r = &mut scene.rapier;
    let mut q = QueryPipelineMut {
        dispatcher: r.narrow_phase.query_dispatcher(),
        bvh: &scene.bvh,
        bodies: &mut r.bodies,
        colliders: &mut r.colliders,
        filter: filter.predicate(&push_filter),
    };
    controller.solve_character_collision_impulses(world.dt(), &mut q, &*shape, c.mass, &collisions);
    for (_, co) in r.colliders.iter() {
        if let Some(rb) = co.parent().map(|h| &r.bodies[h]).filter(|b| b.is_dynamic()) {
            let entity = entities[co.user_data as usize];
            let v = math::vec3(rb.linvel());
            let spin = math::vec3(rb.angvel());
            let changed = world
                .get::<Body>(entity)
                .is_some_and(|b| b.velocity != v || b.spin != spin);
            if changed {
                let mut b = world.get_mut::<Body>(entity).unwrap();
                b.velocity = v;
                b.spin = spin;
                b.asleep = false;
            }
        }
    }
    drop(scene_guard);
    drop(view);
    let result = CapsuleStep {
        displacement: pose.position - before,
        grounded: c.grounded,
    };
    world.insert(e, pose);
    world.insert(e, c);
    result
}

/// Actual collision-constrained movement, including support/platform carry.
#[derive(Clone, Copy, Debug)]
pub struct CapsuleStep {
    /// World displacement applied this step.
    pub displacement: Vec3,
    /// Ground contact at the end of this step.
    pub grounded: bool,
}
/// Named collision controller; its step releases all world/component leases.
pub struct CapsuleHandle<'a> {
    world: &'a mut World,
    entity: Entity,
}
/// Address a CapsuleController + Transform by name or handle.
pub fn capsule(world: &mut World, target: impl Target) -> CapsuleHandle<'_> {
    let label = || target.label();
    let entity = target
        .entity(world)
        .unwrap_or_else(|| panic!("capsule target `{}` does not exist", label()));
    assert!(
        world.has::<CapsuleController>(entity),
        "capsule target `{}` has no CapsuleController",
        label()
    );
    assert!(!world.has::<exact_game::character::Character>(entity),
        "capsule target `{}` has both Character and CapsuleController; pick one movement controller", label());
    assert!(
        world.has::<Transform>(entity),
        "capsule target `{}` has no Transform",
        label()
    );
    CapsuleHandle { world, entity }
}
impl CapsuleHandle<'_> {
    /// Move using horizontal metres/second and the controller's saved vertical velocity.
    pub fn step(self, velocity: Vec3) -> CapsuleStep {
        move_capsule(self.world, self.entity, velocity)
    }
}
