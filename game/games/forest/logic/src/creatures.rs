//! The Deer and the wolves: small state machines over grid steering.
//! Neither has a physics body; both walk the analytic terrain and slide
//! around trunks through the grove's cells.
use crate::forest::{height, Grove};
use exact_game::*;

/// What the Deer is doing. It exists only at night.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Mind {
    /// Daytime: hidden in the far forest.
    #[default]
    Hidden,
    /// Night: closing in, or circling the light when the player is inside it.
    Stalk,
    /// Running the player down.
    Chase,
    /// Frozen by the flashlight.
    Stunned,
    /// Running away after a stun or a strike.
    Flee,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Deer {
    pub mind: Mind,
    /// Seconds left in a timed state.
    pub timer: f32,
    /// Accumulated flashlight exposure, seconds.
    pub glare: f32,
    pub heading: f32,
    pub stuns: u32,
    pub strikes: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Data)]
pub enum Pack {
    #[default]
    Wander,
    Chase,
    Flee,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub struct Wolf {
    pub mode: Pack,
    pub timer: f32,
    pub heading: f32,
    pub home: Vec3,
    pub cooldown: f32,
}

/// What the creatures need to know about the player and the fire this tick.
pub struct Scene {
    pub player: Vec3,
    pub facing: Vec3,
    pub flashlight: bool,
    pub night: bool,
    /// The fire's safe radius; the light the creatures will not enter.
    pub safe: f32,
    pub dead: bool,
}

/// Damage dealt to the player this tick and the Deer's distance.
#[derive(Default)]
pub struct Outcome {
    pub damage: f32,
    pub deer_distance: f32,
    pub chasing: u32,
}

pub const DEER_REACH: f32 = 1.4;
const FLASH_RANGE: f32 = 14.0;
const FLASH_COS: f32 = 0.766; // 40°, wide enough for eight-way keyboard aim

pub fn spawn_deer(w: &mut World, half: f32) {
    w.spawn_named(
        "deer",
        (
            Transform::at(0.0, -50.0, half * 0.8),
            Mesh::capsule(0.45, 3.2),
            Material::rgb(0.08, 0.06, 0.05).rough(0.9),
            Visible(false),
            Deer::default(),
        ),
    );
    let deer = w.resolve("deer").unwrap();
    for (name, x) in [("deer-eye-l", -0.17), ("deer-eye-r", 0.17)] {
        w.spawn_named(
            name,
            (
                Parent(deer),
                Transform::at(x, 1.25, 0.42),
                Mesh::sphere(0.07),
                Material::glow([4.0, 0.25, 0.1]),
            ),
        );
    }
    for (name, x) in [("antler-l", -0.35), ("antler-r", 0.35)] {
        w.spawn_named(
            name,
            (
                Parent(deer),
                Transform::at(x, 1.85, 0.0).with_scale(Vec3::new(0.08, 0.9, 0.08)),
                Mesh::cube(1.0),
                Material::rgb(0.75, 0.7, 0.6),
            ),
        );
    }
}

pub fn spawn_wolves(w: &mut World, count: u32, half: f32) {
    let mut home = Vec3::ZERO;
    for k in 0..count {
        // Packs of four share a den somewhere outside the clearing.
        if k % 4 == 0 {
            let a = w.rand(0.0..std::f32::consts::TAU);
            let r = w.rand(0.25..0.9) * half;
            let (s, c) = math::sin_cos(a);
            home = Vec3::new(c * r, 0.0, s * r);
        }
        let x = home.x + w.rand(-4.0..4.0);
        let z = home.z + w.rand(-4.0..4.0);
        let heading = w.rand(0.0..std::f32::consts::TAU);
        w.spawn((
            Transform::at(x, height(x, z) + 0.45, z),
            Mesh::cuboid(Vec3::new(0.5, 0.6, 1.3)),
            Material::rgb(0.32, 0.31, 0.3).rough(0.95),
            Wolf {
                home,
                heading,
                ..Default::default()
            },
        ));
    }
}

fn planar(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Move a creature: steer around trunks, resolve overlaps, stay out of the light,
/// and follow the ground. Returns the new position and heading.
fn walk(
    g: &Grove,
    from: Vec3,
    want: Vec3,
    radius: f32,
    safe: f32,
    lift: f32,
    dt: f32,
) -> (Vec3, f32) {
    let (vx, vz) = g.steer(from.x, from.z, want.x, want.z, radius);
    let (mut x, mut z) = g.resolve(from.x + vx * dt, from.z + vz * dt, radius);
    let r = (x * x + z * z).sqrt();
    if safe > 0.0 && r < safe && r > 1e-3 {
        x *= safe / r;
        z *= safe / r;
    }
    let heading = if vx * vx + vz * vz > 1e-6 {
        math::atan2(vx, vz)
    } else {
        f32::NAN
    };
    (Vec3::new(x, height(x, z) + lift, z), heading)
}

pub fn step(w: &World, s: &Scene) -> Outcome {
    let dt = w.dt();
    let g = w.resource::<Grove>();
    let mut out = Outcome {
        deer_distance: f32::INFINITY,
        ..Default::default()
    };
    let outside = s.player.length() > s.safe;
    // The Deer.
    for (_, (pose, deer, visible)) in w
        .query::<(&mut Transform, &mut Deer, &mut Visible)>()
        .iter()
    {
        if !s.night {
            if deer.mind != Mind::Hidden {
                *deer = Deer {
                    stuns: deer.stuns,
                    strikes: deer.strikes,
                    ..Default::default()
                };
                visible.0 = false;
                pose.position.y = -50.0;
            }
            continue;
        }
        if deer.mind == Mind::Hidden {
            // Emerge at the edge of the light, on the far side from the player.
            let away = planar(-s.player).normalize_or(Vec3::Z);
            let at = away * (s.safe + 22.0);
            let (x, z) = g.resolve(at.x, at.z, 0.5);
            pose.position = Vec3::new(x, height(x, z) + 1.6, z);
            deer.mind = Mind::Stalk;
            visible.0 = true;
        }
        let to = planar(s.player - pose.position);
        let d = to.length();
        out.deer_distance = d;
        // The flashlight: a cone along the player's facing.
        let lit =
            s.flashlight && d < FLASH_RANGE && (-to).normalize_or_zero().dot(s.facing) > FLASH_COS;
        if lit && matches!(deer.mind, Mind::Stalk | Mind::Chase) {
            deer.glare += dt;
            if deer.glare >= 0.6 {
                deer.mind = Mind::Stunned;
                deer.timer = 3.5;
                deer.glare = 0.0;
                deer.stuns += 1;
            }
        } else {
            deer.glare = (deer.glare - dt * 0.5).max(0.0);
        }
        deer.timer -= dt;
        let dir = to.normalize_or_zero();
        let want = match deer.mind {
            Mind::Hidden => Vec3::ZERO,
            Mind::Stalk => {
                if !outside || s.dead {
                    // Circle at the edge of the light.
                    let p = planar(pose.position);
                    let r = p.length().max(1e-3);
                    let tangent = Vec3::new(-p.z, 0.0, p.x) / r;
                    let inward = (s.safe + 5.0 - r) * 0.8;
                    tangent * 2.0 + p / r * inward
                } else {
                    if d < 16.0 && deer.timer < -3.0 {
                        deer.mind = Mind::Chase;
                        deer.timer = 7.0;
                    }
                    dir * 3.2
                }
            }
            Mind::Chase => {
                if !outside || deer.timer <= 0.0 {
                    deer.mind = Mind::Stalk;
                }
                out.chasing += 1;
                dir * 6.6
            }
            Mind::Stunned => {
                if deer.timer <= 0.0 {
                    deer.mind = Mind::Flee;
                    deer.timer = 5.0;
                }
                Vec3::ZERO
            }
            Mind::Flee => {
                if deer.timer <= 0.0 {
                    deer.mind = Mind::Stalk;
                }
                -dir * 7.5
            }
        };
        if deer.mind == Mind::Chase && d < DEER_REACH && outside && !s.dead {
            out.damage += 35.0;
            deer.strikes += 1;
            deer.mind = Mind::Flee;
            deer.timer = 6.0;
        }
        // The beam slows it as well as stunning it.
        let want = if lit { want * 0.35 } else { want };
        let (p, heading) = walk(&g, pose.position, want, 0.5, s.safe + 1.5, 1.6, dt);
        pose.position = p;
        if heading.is_finite() {
            deer.heading = heading;
        }
        if deer.mind != Mind::Stunned {
            pose.rotation = Quat::from_rotation_y(deer.heading);
        }
    }
    // Wolves.
    for (_, (pose, wolf)) in w.query::<(&mut Transform, &mut Wolf)>().iter() {
        let to = planar(s.player - pose.position);
        let d = to.length();
        let sense = if s.night { 22.0 } else { 12.0 };
        wolf.timer -= dt;
        wolf.cooldown = (wolf.cooldown - dt).max(0.0);
        let lit = s.flashlight && d < 9.0 && (-to).normalize_or_zero().dot(s.facing) > FLASH_COS;
        if lit && wolf.mode == Pack::Chase {
            wolf.mode = Pack::Flee;
            wolf.timer = 2.5;
        }
        match wolf.mode {
            Pack::Wander if d < sense && outside && !s.dead => wolf.mode = Pack::Chase,
            Pack::Chase if !outside || d > sense * 1.6 || s.dead => {
                wolf.mode = Pack::Wander;
                wolf.timer = 0.0;
            }
            Pack::Flee if wolf.timer <= 0.0 => wolf.mode = Pack::Wander,
            _ => {}
        }
        let want = match wolf.mode {
            Pack::Wander => {
                if wolf.timer <= 0.0 {
                    // A deterministic turn per wolf, drawn from its own state.
                    let jitter = math::sin(wolf.heading * 12.9898 + pose.position.x * 0.37) * 1.4;
                    let home = planar(wolf.home - pose.position);
                    wolf.heading = if home.length() > 18.0 {
                        math::atan2(home.x, home.z)
                    } else {
                        wolf.heading + jitter
                    };
                    wolf.timer = 2.0 + jitter.abs();
                }
                let (sn, cs) = math::sin_cos(wolf.heading);
                Vec3::new(sn, 0.0, cs) * 1.6
            }
            Pack::Chase => {
                out.chasing += 1;
                to.normalize_or_zero() * 5.4
            }
            Pack::Flee => -to.normalize_or_zero() * 6.0,
        };
        if wolf.mode == Pack::Chase && d < 1.3 && wolf.cooldown <= 0.0 {
            out.damage += 8.0;
            wolf.cooldown = 1.4;
        }
        let (p, heading) = walk(&g, pose.position, want, 0.4, s.safe + 1.0, 0.45, dt);
        pose.position = p;
        if heading.is_finite() {
            if wolf.mode != Pack::Wander {
                wolf.heading = heading;
            }
            pose.rotation = Quat::from_rotation_y(heading);
        }
    }
    out
}
