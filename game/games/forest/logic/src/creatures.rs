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
    /// A stationary warning; the flashlight can interrupt it before the rush.
    Windup,
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
pub const WINDUP: f32 = 0.9;
const FLASH_RANGE: f32 = 14.0;
const FLASH_COS: f32 = 0.766; // 40°, wide enough for eight-way keyboard aim

/// The Deer, its head (the saved pose `deer_art::present` lowers before a charge)
/// and its eyes. The art pass draws baked models around the same pivots; its
/// legs come with the rest of the art pass's scenery, after every simulated entity.
pub fn spawn_deer(w: &mut World, half: f32, pass: bool) {
    let (body, head, eyes, eye) = if pass {
        let (body, head) = (
            Mesh::asset("deer_body.model"),
            Mesh::asset("deer_head.model"),
        );
        (body, head, [0.085, 0.42, 0.74], 0.045)
    } else {
        let body = w
            .generated("deer-body.model", crate::deer_art::body())
            .expect("deer body");
        let head = w
            .generated("deer-head.model", crate::deer_art::head())
            .expect("deer head");
        (body, head, [0.24, 0.47, 0.70], 0.085)
    };
    // The baked fur is near black: a stronger fill keeps it readable at night.
    let fill = if pass { 4.0 } else { 1.0 };
    w.spawn_named(
        "deer",
        (
            Transform::at(0.0, -50.0, half * 0.8),
            body,
            // A faint warm fill keeps the silhouette readable against the night sky.
            Material {
                emissive: [0.006 * fill, 0.004 * fill, 0.002 * fill],
                ..Material::default().rough(0.9)
            },
            Visible(false),
            Deer::default(),
        ),
    );
    let deer = w.resolve("deer").unwrap();
    let head = w.spawn_named(
        "deer-head",
        (
            Parent(deer),
            Transform::at(0., 0.70, 0.10),
            head,
            Material {
                emissive: [0.028, 0.022, 0.014],
                ..Material::default().rough(0.9)
            },
        ),
    );
    for (name, x) in [("deer-eye-l", -eyes[0]), ("deer-eye-r", eyes[0])] {
        w.spawn_named(
            name,
            (
                Parent(head),
                Transform::at(x, eyes[1], eyes[2]),
                Mesh::sphere(eye),
                Material::glow([4.0, 0.25, 0.1]),
            ),
        );
    }
}

pub fn spawn_wolves(w: &mut World, count: u32, half: f32, pass: bool) {
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
        let (mesh, material) = if pass {
            (Mesh::asset("wolf_body.model"), Material::default())
        } else {
            (
                Mesh::cuboid(Vec3::new(0.5, 0.6, 1.3)),
                Material::rgb(0.32, 0.31, 0.3).rough(0.95),
            )
        };
        w.spawn((
            Transform::at(x, height(x, z) + 0.45, z),
            mesh,
            material,
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

/// The immediate threat and its bearing, expressed in the same compass as camp.
pub fn warning(w: &World) -> String {
    let deer = w.require::<Deer>("deer");
    match deer.mind {
        Mind::Hidden => "Deer: hidden by daylight".into(),
        Mind::Flee => "Deer retreating".into(),
        Mind::Stunned => format!(
            "Deer stunned · {} s",
            math::ceil(deer.timer.max(0.0)) as u32
        ),
        Mind::Stalk => "Deer stalking outside the firelight".into(),
        Mind::Windup | Mind::Chase => {
            let direction = crate::player::bearing(
                w.require::<Transform>("deer").position - w.require::<Transform>("player").position,
            );
            let action = if deer.mind == Mind::Windup {
                "Charge warning"
            } else {
                "Deer charging"
            };
            format!("{action} · {direction} · F flashlight")
        }
    }
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
    // The Deer. Visibility is written only when it changes: a mutable borrow of
    // `Visible` alone tells the renderer to rebuild every batch.
    let mut show = None;
    for (e, (pose, deer)) in w.query::<(&mut Transform, &mut Deer)>().iter() {
        if !s.night {
            if deer.mind != Mind::Hidden {
                *deer = Deer {
                    stuns: deer.stuns,
                    strikes: deer.strikes,
                    ..Default::default()
                };
                show = Some((e, false));
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
            show = Some((e, true));
        }
        let to = planar(s.player - pose.position);
        let d = to.length();
        out.deer_distance = d;
        // The flashlight: a cone along the player's facing.
        let lit =
            s.flashlight && d < FLASH_RANGE && (-to).normalize_or_zero().dot(s.facing) > FLASH_COS;
        if lit && matches!(deer.mind, Mind::Stalk | Mind::Windup | Mind::Chase) {
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
                        deer.mind = Mind::Windup;
                        deer.timer = WINDUP;
                        deer.heading = math::atan2(dir.x, dir.z);
                        Vec3::ZERO
                    } else {
                        dir * 3.2
                    }
                }
            }
            Mind::Windup => {
                if !outside || s.dead {
                    deer.mind = Mind::Stalk;
                    deer.timer = 0.0;
                    Vec3::ZERO
                } else {
                    out.chasing += 1;
                    deer.heading = math::atan2(dir.x, dir.z);
                    if deer.timer <= 0.0 {
                        deer.mind = Mind::Chase;
                        deer.timer = 7.0;
                        dir * 6.6
                    } else {
                        Vec3::ZERO
                    }
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
    if let Some((e, shown)) = show {
        w.get_mut::<Visible>(e).unwrap().0 = shown;
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
