//! Steering: pure functions from a pose to the motion it wants this tick.
//!
//! Each function reads a root entity's [`Transform`] (its position, and its
//! facing along −Z) and returns a [`Steer`]: the facing after turning at most
//! `turn_rate × dt` radians toward where it wants to go, and a velocity along
//! that facing. A mover slows while it faces away from where it wants to go
//! (its speed scales with the cosine of the angle left to turn, and is zero
//! past a right angle), so it turns before it goes rather than circling.
//!
//! Nothing here is saved or stepped. Call the functions from `Game::tick`, in
//! the order the game chooses, and write the result into the motion the entity
//! already has:
//!
//! - a kinematic mover: `pose.position += steer.velocity * dt; pose.rotation = steer.facing`;
//! - a [`Character`](crate::character::Character): step it with
//!   `steer.velocity / character.speed` as the wish, and set the rotation;
//! - a physics body: apply the change of velocity as the game's own force.
//!
//! Positions are 3D. A creature on the ground steers toward targets at its own
//! height (`Vec3::new(target.x, pose.position.y, target.z)`), or it pitches.
//! The facing has no roll. Trigonometry is [`crate::math`]'s, so a steer is
//! the same on every host. Units are metres, seconds and radians.
use crate::{math, Entity, Quat, Rng, Transform, Vec3};

/// The motion a steering function wants this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steer {
    /// World-space velocity in metres per second, along `facing`'s −Z.
    pub velocity: Vec3,
    /// The facing after this tick's turn: at most `turn_rate × dt` radians from
    /// the pose's, with no roll. Unchanged, bit for bit, when no turn is needed.
    pub facing: Quat,
}

/// Distance in metres within which a target counts as reached.
const REACHED: f32 = 1e-3;

/// Head for `to` at `max_speed`, turning at most `turn_rate` radians per second.
/// The step never passes `to`; within a millimetre the mover stops facing as it was.
pub fn seek(from: &Transform, to: Vec3, max_speed: f32, turn_rate: f32, dt: f32) -> Steer {
    check(max_speed, turn_rate, dt);
    let away = to - from.position;
    let distance = away.length();
    if distance <= REACHED {
        return still(from);
    }
    motion(
        from,
        away / distance,
        max_speed.min(distance / dt),
        turn_rate,
        dt,
    )
}

/// Seek that slows inside `radius` metres, in proportion to the distance left,
/// and stops at `to`.
pub fn arrive(
    from: &Transform,
    to: Vec3,
    radius: f32,
    max_speed: f32,
    turn_rate: f32,
    dt: f32,
) -> Steer {
    check(max_speed, turn_rate, dt);
    assert!(
        radius.is_finite() && radius >= 0.0,
        "arrive: radius {radius}"
    );
    let away = to - from.position;
    let distance = away.length();
    if distance <= REACHED {
        return still(from);
    }
    let slowed = if distance < radius {
        max_speed * distance / radius
    } else {
        max_speed
    };
    motion(
        from,
        away / distance,
        slowed.min(distance / dt),
        turn_rate,
        dt,
    )
}

/// Run directly away from `threat` while it is within `radius` metres
/// (inclusive); `None` outside it, so a game can fall back to another
/// behaviour: `flee(..).unwrap_or_else(|| seek(..))`. A mover facing the
/// threat turns before it runs and does not move toward it meanwhile.
pub fn flee(
    from: &Transform,
    threat: Vec3,
    radius: f32,
    max_speed: f32,
    turn_rate: f32,
    dt: f32,
) -> Option<Steer> {
    check(max_speed, turn_rate, dt);
    assert!(radius.is_finite() && radius >= 0.0, "flee: radius {radius}");
    let away = from.position - threat;
    let distance = away.length();
    if distance > radius {
        return None;
    }
    let direction = if distance > 0.0 {
        away / distance
    } else {
        forward(from)
    };
    Some(motion(from, direction, max_speed, turn_rate, dt))
}

/// Meander at `speed` in the horizontal plane, keeping the pose's pitch.
/// `turn` is the mover's current turning rate in radians per second, kept in
/// the game's own component (start it at zero): each tick it takes a random
/// step from the world's `rng`, sized so it crosses its range of ±`turn_rate`
/// in about a second, and the facing turns by `turn × dt` about world +Y.
pub fn wander(
    from: &Transform,
    rng: &mut Rng,
    turn: &mut f32,
    speed: f32,
    turn_rate: f32,
    dt: f32,
) -> Steer {
    check(speed, turn_rate, dt);
    assert!(turn.is_finite(), "wander: turn {turn}");
    // A uniform step of ±a has variance a²/3; 1/dt of them in a second sum to turn_rate².
    let jitter = turn_rate * math::sqrt(3.0 * dt);
    *turn = math::clamp(
        *turn + rng.range(-1.0f32..1.0) * jitter,
        -turn_rate,
        turn_rate,
    );
    let facing = if *turn == 0.0 {
        from.rotation
    } else {
        (Quat::from_rotation_y(*turn * dt) * from.rotation).normalize()
    };
    Steer {
        velocity: facing * Vec3::NEG_Z * speed,
        facing,
    }
}

/// Swim with neighbours: keep `separation` metres from each, turn with their
/// mean facing (`alignment`, a weight) and toward their centre (`cohesion`, a
/// weight), at `max_speed`. The mover's own heading counts with weight one,
/// and a neighbour closer than `separation` pushes away with weight up to eight,
/// rising as it nears, so spacing wins over the other two. With no neighbours
/// it keeps its heading.
///
/// `neighbours` are `(entity, global pose)` rows, as
/// [`World::near_xz`](crate::World::near_xz) yields them; `.limit(n)` bounds
/// the work per mover. Each `near_xz` call scans every entity carrying its
/// component, so a school costs its size squared per tick: hundreds of movers,
/// not thousands. Steer every mover before moving any, so a decision reads the
/// poses as the tick found them.
#[allow(clippy::too_many_arguments)]
pub fn school(
    from: &Transform,
    neighbours: impl IntoIterator<Item = (Entity, Transform)>,
    separation: f32,
    alignment: f32,
    cohesion: f32,
    max_speed: f32,
    turn_rate: f32,
    dt: f32,
) -> Steer {
    check(max_speed, turn_rate, dt);
    for (name, value) in [
        ("separation", separation),
        ("alignment", alignment),
        ("cohesion", cohesion),
    ] {
        assert!(value.is_finite() && value >= 0.0, "school: {name} {value}");
    }
    let ahead = forward(from);
    let (mut apart, mut heading, mut centre, mut count) =
        (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0u32);
    for (_, other) in neighbours {
        let away = from.position - other.position;
        let distance = away.length();
        if distance > 0.0 && distance < separation {
            apart += away / distance * (1.0 - distance / separation);
        }
        heading += forward(&other);
        centre += other.position;
        count += 1;
    }
    let mut want = ahead + apart * SEPARATION_WEIGHT;
    if count > 0 {
        want += heading.normalize_or_zero() * alignment;
        want += (centre / count as f32 - from.position).normalize_or_zero() * cohesion;
    }
    let direction = want.try_normalize().unwrap_or(ahead);
    motion(from, direction, max_speed, turn_rate, dt)
}

/// How hard a neighbour at zero distance pushes, against the mover's own heading's one.
const SEPARATION_WEIGHT: f32 = 8.0;

fn check(speed: f32, turn_rate: f32, dt: f32) {
    assert!(speed.is_finite() && speed >= 0.0, "steer: speed {speed}");
    assert!(
        turn_rate.is_finite() && turn_rate >= 0.0,
        "steer: turn_rate {turn_rate}"
    );
    assert!(dt.is_finite() && dt > 0.0, "steer: dt {dt}");
}

/// The pose's facing direction: its −Z.
fn forward(pose: &Transform) -> Vec3 {
    (pose.rotation * Vec3::NEG_Z).normalize_or(Vec3::NEG_Z)
}

fn still(from: &Transform) -> Steer {
    Steer {
        velocity: Vec3::ZERO,
        facing: from.rotation,
    }
}

/// Turn toward the unit `direction` by at most `turn_rate × dt`, then move
/// along the new facing at `speed` scaled by how nearly it faces `direction`.
fn motion(from: &Transform, direction: Vec3, speed: f32, turn_rate: f32, dt: f32) -> Steer {
    let ahead = forward(from);
    let cos = math::clamp(ahead.dot(direction), -1.0, 1.0);
    let left = math::acos(cos);
    let limit = turn_rate * dt;
    let (facing, ahead) = if left == 0.0 {
        (from.rotation, ahead)
    } else if left <= limit {
        (look(direction, ahead), direction)
    } else {
        let turned = rotate_toward(ahead, direction, cos, limit);
        (look(turned, ahead), turned)
    };
    let along = math::clamp(ahead.dot(direction), 0.0, 1.0);
    Steer {
        velocity: ahead * speed * along,
        facing,
    }
}

/// `ahead` turned by `angle` toward `direction` in the plane they share. Facing
/// exactly away, it turns left about world +Y (or about +X when vertical).
fn rotate_toward(ahead: Vec3, direction: Vec3, cos: f32, angle: f32) -> Vec3 {
    let side = (direction - ahead * cos)
        .try_normalize()
        .or_else(|| Vec3::Y.cross(ahead).try_normalize())
        .unwrap_or(Vec3::X);
    let (sin, cos) = math::sin_cos(angle);
    (ahead * cos + side * sin).normalize()
}

/// The roll-free rotation whose −Z is `ahead`; straight up or down keeps
/// `previous`'s heading.
fn look(ahead: Vec3, previous: Vec3) -> Quat {
    let heading = |v: Vec3| (v.x * v.x + v.z * v.z > 1e-12).then(|| math::atan2(-v.x, -v.z));
    let yaw = heading(ahead).or_else(|| heading(previous)).unwrap_or(0.0);
    let pitch = math::asin(math::clamp(ahead.y, -1.0, 1.0));
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(position: Vec3, yaw: f32) -> Transform {
        Transform {
            position,
            rotation: Quat::from_rotation_y(yaw),
            ..Transform::default()
        }
    }

    #[test]
    fn look_is_the_inverse_of_forward_and_keeps_a_vertical_heading() {
        for ahead in [Vec3::NEG_Z, Vec3::X, Vec3::new(1., 1., -1.).normalize()] {
            let t = Transform {
                rotation: look(ahead, Vec3::NEG_Z),
                ..Transform::default()
            };
            assert!(forward(&t).distance(ahead) < 1e-6, "{ahead}");
        }
        let up = look(Vec3::Y, Vec3::X);
        assert!((up * Vec3::NEG_Z).distance(Vec3::Y) < 1e-6);
        // The heading it kept: +X, so its right (+X of the pose) points to +Z.
        assert!((up * Vec3::X).distance(Vec3::Z) < 1e-6);
    }

    #[test]
    fn a_turn_is_bounded_by_the_rate_and_faces_away_turns_left() {
        let from = pose(Vec3::ZERO, 0.0);
        let s = seek(&from, Vec3::new(0., 0., 5.), 2.0, 3.0, 0.1);
        let turned = s.facing * Vec3::NEG_Z;
        assert!((math::acos(turned.dot(Vec3::NEG_Z)) - 0.3).abs() < 1e-5);
        assert!(
            turned.x < 0.0,
            "a left turn from −Z heads toward −X: {turned}"
        );
        assert_eq!(
            s.velocity,
            Vec3::ZERO,
            "past a right angle it does not move"
        );
        let aligned = seek(&from, Vec3::new(0., 0., -5.), 2.0, 3.0, 0.1);
        assert_eq!(aligned.facing, from.rotation);
        assert_eq!(aligned.velocity, Vec3::new(0., 0., -2.));
    }
}
