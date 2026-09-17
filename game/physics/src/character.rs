use crate::{
    geometry::{world_pose, Geometry},
    queries::{raycast, sweep_filtered},
    Body, BodyKind, Character, Collider, Hit, Shape,
};
use exact_game::{Entity, Transform, Vec3, World};
const SKIN: f32 = 0.01;
fn cast(world: &World, e: Entity, shape: &Shape, pose: Transform, motion: Vec3) -> Option<Hit> {
    let mask = world.get::<Collider>(e).map_or(u32::MAX, |c| c.mask);
    sweep_filtered(world, shape, pose, motion, mask, Some(e), true)
}
fn push(world: &World, hit: Hit, velocity: Vec3, budget: f32) {
    let Some(mut body) = world.get_mut::<Body>(hit.entity) else {
        return;
    };
    if body.kind != BodyKind::Dynamic {
        return;
    }
    let mass = if body.mass > 0.0 {
        body.mass
    } else {
        let Some(c) = world.get::<Collider>(hit.entity) else {
            return;
        };
        1.0 / Geometry::new(&c.shape, world_pose(world, hit.entity))
            .mass(0.0)
            .0
    };
    if mass > budget {
        return;
    }
    let approach = -(velocity - body.velocity).dot(hit.normal);
    if approach > 0.0 {
        body.velocity -= hit.normal * approach;
        body.asleep = false;
        body.calm = 0;
    }
}
fn step_up(
    world: &World,
    e: Entity,
    shape: &Shape,
    pose: Transform,
    motion: Vec3,
    height: f32,
    slope: f32,
) -> Option<Transform> {
    if height <= 0.0 || motion.length_squared() < 1e-12 {
        return None;
    }
    let up = Vec3::Y * (height + SKIN);
    if cast(world, e, shape, pose, up).is_some() {
        return None;
    }
    let mut raised = pose;
    raised.position += up;
    if cast(world, e, shape, raised, motion).is_some() {
        return None;
    }
    raised.position += motion;
    let down = -Vec3::Y * (height + 2.0 * SKIN);
    let hit = cast(world, e, shape, raised, down)?;
    if hit.normal.y < slope {
        // At a stair lip the rounded foot first hits the corner. Probe the tread
        // just beyond that lip; a steep ramp has no walkable tread and is refused.
        let Shape::Capsule {
            radius,
            height: total,
        } = *shape
        else {
            unreachable!()
        };
        let probe = raised.position + motion.normalize() * (radius + SKIN);
        let mask = world.get::<Collider>(e).map_or(u32::MAX, |c| c.mask);
        let tread = raycast(
            world,
            probe,
            -Vec3::Y,
            total * 0.5 + height + 2.0 * SKIN,
            mask,
        )?;
        if tread.entity != hit.entity || tread.normal.y < slope {
            return None;
        }
        raised.position.y = tread.point.y + total * 0.5 + SKIN;
    } else {
        raised.position.y -= (hit.distance - SKIN).max(0.0);
    }
    if raised.position.y - pose.position.y > height + SKIN {
        return None;
    }
    Some(raised)
}
/// Move an upright kinematic capsule with four collide-and-slide iterations.
/// `desired_velocity.x/z` control horizontal motion; Character.velocity.y belongs
/// to the game (gravity/jump). Ground contact clears downward vertical velocity.
/// A missing Body/Collider is installed. The character's collider is a sensor:
/// collision response and its finite push budget are owned by this controller,
/// preventing the rigid solver from treating the character as infinite push mass.
/// Call before [`crate::step`]; dimensions are metres and speeds are m/s.
pub fn move_character(world: &mut World, e: Entity, desired_velocity: Vec3) {
    let mut c = world
        .get::<Character>(e)
        .expect("physics: entity needs Character")
        .clone();
    let mut pose = *world
        .get::<Transform>(e)
        .expect("physics: character needs Transform");
    assert!(
        pose.rotation == exact_game::Quat::IDENTITY && pose.scale == Vec3::ONE,
        "physics: characters must be upright with unit scale"
    );
    assert!(
        desired_velocity.is_finite()
            && c.velocity.is_finite()
            && c.mass > 0.0
            && c.step >= 0.0
            && (0.0..90.0).contains(&c.slope_degrees),
        "physics: invalid Character"
    );
    let shape = Shape::Capsule {
        radius: c.radius,
        height: c.height,
    };
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
    collider.shape = shape.clone();
    collider.sensor = true;
    world.insert(e, collider);
    let slope = exact_game::math::cos(c.slope_degrees * (std::f32::consts::PI / 180.0));
    if c.grounded {
        if let Some(support) = c.support.filter(|s| world.contains(*s)) {
            let current = world_pose(world, support);
            let local =
                c.support_pose.rotation.conjugate() * (pose.position - c.support_pose.position);
            let delta = current.position + current.rotation * local - pose.position;
            if let Some(hit) = cast(world, e, &shape, pose, delta) {
                pose.position += delta.normalize_or_zero() * (hit.distance - SKIN).max(0.0);
            } else {
                pose.position += delta;
            }
        }
    }
    let start = pose.position;
    let velocity = Vec3::new(desired_velocity.x, c.velocity.y, desired_velocity.z);
    let mut motion = velocity * world.dt();
    let was_grounded = c.grounded;
    c.grounded = false;
    c.support = None;
    for _ in 0..4 {
        let length = motion.length();
        if length < 1e-7 {
            break;
        }
        let Some(hit) = cast(world, e, &shape, pose, motion) else {
            pose.position += motion;
            break;
        };
        let advance = (hit.distance - SKIN).max(0.0).min(length);
        pose.position += motion * (advance / length);
        motion *= 1.0 - advance / length;
        push(world, hit, velocity, c.mass);
        if hit.normal.y >= slope {
            c.grounded = true;
            c.support = Some(hit.entity);
            if c.velocity.y < 0.0 {
                c.velocity.y = 0.0;
            }
        } else if was_grounded && velocity.y <= 0.0 {
            let horizontal = Vec3::new(motion.x, 0.0, motion.z);
            if let Some(up) = step_up(world, e, &shape, pose, horizontal, c.step, slope) {
                pose = up;
                motion = Vec3::ZERO;
                continue;
            }
        }
        let mut normal = hit.normal;
        // Steep ramps act as walls for horizontal motion, rather than providing lift.
        if normal.y > 0.0 && normal.y < slope {
            normal.y = 0.0;
            normal = normal.normalize_or_zero();
        }
        let inward = motion.dot(normal);
        if inward < 0.0 {
            motion -= normal * inward;
        }
    }
    if velocity.y <= 0.0 {
        let snap = if was_grounded {
            c.step + 2.0 * SKIN
        } else {
            2.0 * SKIN
        };
        if let Some(hit) = cast(world, e, &shape, pose, -Vec3::Y * snap) {
            if hit.normal.y >= slope {
                pose.position.y -= (hit.distance - SKIN).max(0.0);
                c.grounded = true;
                c.support = Some(hit.entity);
                c.velocity.y = 0.0;
            }
        }
    }
    c.velocity.x = (pose.position.x - start.x) / world.dt();
    c.velocity.z = (pose.position.z - start.z) / world.dt();
    if let Some(s) = c.support {
        c.support_pose = world_pose(world, s);
    }
    world.insert(e, pose);
    world.insert(e, c);
}
