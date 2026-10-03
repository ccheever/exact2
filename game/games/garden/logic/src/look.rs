//! How the garden looks: the day and night, the sky under each weather, the
//! fence and its lanterns, the path and the seed stall with its keeper, the
//! farmer's walk, and rain and snow. All of it is driven by garden time, so it
//! is deterministic, but none of it changes what grows.

use crate::farm::Farm;
use crate::garden::{now_ms, Sky, Weather, TILE};
use exact_game::emitter::Shape;
use exact_game::*;

/// A lit lantern's candela.
const LANTERN: f32 = 140.0;

/// Garden milliseconds in one day.
pub const DAY_MS: u64 = 600_000;

/// A piece of fence or lantern, rebuilt when the garden grows.
#[derive(Default, Component)]
pub struct Fencing;

/// A lantern on the fence; lit from dusk to dawn.
#[derive(Default, Component)]
pub struct Lantern {
    pub lit: bool,
}

/// A walking person's parts swing by `phase`.
#[derive(Default, Component)]
pub struct Gait {
    pub phase: f32,
    pub yaw: f32,
}

/// Which limb of a person an entity is: 0 head, 1 left arm, 2 right arm,
/// 3 left leg, 4 right leg.
#[derive(Default, Component)]
pub struct Limb {
    pub part: u8,
}

/// The last lighting state the sky was set to, so a tick republishes only
/// what changed.
#[derive(Default, Resource)]
pub struct Lighting {
    pub night: bool,
    pub flash: bool,
    pub sky: u8,
}

/// The time of day in [0, 1): 0 is sunrise, 0.25 noon, 0.5 sunset.
pub fn day_phase(now: u64) -> f32 {
    ((now + DAY_MS / 12) % DAY_MS) as f32 / DAY_MS as f32
}

/// "Day 2 · 14:30"
pub fn clock_label(now: u64) -> String {
    let day = (now + DAY_MS / 12) / DAY_MS + 1;
    let minutes = (day_phase(now) * 24.0 * 60.0) as u32 + 6 * 60;
    format!("Day {day} · {:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

fn part(w: &mut World, name: &str, parent: Entity, model: &str, at: Vec3, limb: u8) -> Entity {
    w.spawn_named(
        name,
        (
            Transform::at(at.x, at.y, at.z),
            Mesh::asset(model),
            Parent(parent),
            Limb { part: limb },
            Ambient,
        ),
    )
}

/// The farmer's head, arms and legs on the player's torso, and the keeper.
pub fn dress(w: &mut World) {
    let player = w.named("player").unwrap();
    *w.require_mut::<Mesh>(player) = Mesh::asset("farmer-torso.model");
    w.insert(player, Gait::default());
    for (name, model, at, limb) in [
        (
            "farmer-head",
            "farmer-head.model",
            Vec3::new(0.0, 0.62, 0.0),
            0,
        ),
        (
            "farmer-arm-l",
            "farmer-arm.model",
            Vec3::new(-0.27, 0.22, 0.0),
            1,
        ),
        (
            "farmer-arm-r",
            "farmer-arm.model",
            Vec3::new(0.27, 0.22, 0.0),
            2,
        ),
        (
            "farmer-leg-l",
            "farmer-leg.model",
            Vec3::new(-0.11, -0.33, 0.0),
            3,
        ),
        (
            "farmer-leg-r",
            "farmer-leg.model",
            Vec3::new(0.11, -0.33, 0.0),
            4,
        ),
    ] {
        part(w, name, player, model, at, limb);
    }
    // The seed stall stands left of the garden's gate, facing the path.
    let stall = Transform::at(-7.0, 0.0, -1.0).looking_at(Vec3::new(-20.0, 0.0, -1.0), Vec3::Y);
    w.spawn_named("stall", (stall, Mesh::asset("stall.model"), Ambient));
    // The stall's sign is Contract text on a plane over the awning.
    let mut sign = Transform::at(-6.8, 3.25, -1.0);
    sign.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    w.spawn_named(
        "stall-sign",
        (
            sign,
            Placed::child("stall-sign").width(2.0).facing(Facing::Fixed),
            Ambient,
        ),
    );
    let keeper = w.spawn_named(
        "keeper",
        (
            Transform::at(-7.9, 0.9, -1.0).looking_at(Vec3::new(-20.0, 0.9, -1.0), Vec3::Y),
            Mesh::asset("keeper-torso.model"),
            Ambient,
        ),
    );
    for (name, model, at, limb) in [
        (
            "keeper-head",
            "keeper-head.model",
            Vec3::new(0.0, 0.62, 0.0),
            0,
        ),
        (
            "keeper-arm-l",
            "keeper-arm.model",
            Vec3::new(-0.27, 0.22, 0.0),
            1,
        ),
        (
            "keeper-arm-r",
            "keeper-arm.model",
            Vec3::new(0.27, 0.22, 0.0),
            2,
        ),
        (
            "keeper-leg-l",
            "keeper-leg.model",
            Vec3::new(-0.11, -0.33, 0.0),
            3,
        ),
        (
            "keeper-leg-r",
            "keeper-leg.model",
            Vec3::new(0.11, -0.33, 0.0),
            4,
        ),
    ] {
        part(w, name, keeper, model, at, limb);
    }
    // Stepping stones from the stall to the gate.
    for i in 0..4 {
        let x = -5.0 + i as f32 * 1.6;
        w.spawn((
            Transform::at(x, 0.01, 0.4 + (i % 2) as f32 * 0.3),
            Mesh::asset(format!("path-{}.model", i % 3)),
            Ambient,
        ));
    }
    // Rain and snow fall around the farmer.
    w.spawn_named(
        "weather",
        (
            Transform::at(0.0, 10.0, -3.0),
            Follow::new(player).offset(0.0, 9.0, -3.0).lag(0.0),
            Emitter {
                running: false,
                ..rain(false)
            },
            Ambient,
        ),
    );
    w.spawn_named(
        "moon",
        (
            Transform::at(-10.0, 20.0, -8.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: [0.55, 0.65, 1.0],
                illuminance: 0.0,
                shadows: false,
            },
        ),
    );
    w.insert_resource(Lighting::default());
    w.insert_resource(AmbientOcclusion {
        radius: 0.45,
        intensity: 0.9,
    });
}

fn rain(storm: bool) -> Emitter {
    Emitter {
        shape: Shape::Sphere(14.0),
        rate: if storm { 2600.0 } else { 1400.0 },
        lifetime: 0.7,
        speed: 0.0,
        spread: 0.0,
        gravity: Vec3::new(if storm { -6.0 } else { -1.0 }, -26.0, 0.0),
        drag: 0.0,
        size: [0.05, 0.04],
        color: [[0.65, 0.75, 0.95, 0.55], [0.65, 0.75, 0.95, 0.25]],
        bound: [-16.0, -22.0, -16.0, 16.0, 16.0, 16.0],
        additive: false,
        seed: 11,
        ..Emitter::default()
    }
}

fn snow() -> Emitter {
    Emitter {
        shape: Shape::Sphere(14.0),
        rate: 500.0,
        lifetime: 4.0,
        speed: 0.4,
        spread: 3.0,
        gravity: Vec3::new(0.3, -1.6, 0.0),
        drag: 0.4,
        size: [0.14, 0.1],
        color: [[1.0, 1.0, 1.0, 0.9], [0.9, 0.95, 1.0, 0.0]],
        bound: [-16.0, -22.0, -16.0, 16.0, 16.0, 16.0],
        additive: false,
        seed: 12,
        ..Emitter::default()
    }
}

/// Posts on every tile edge, rails between them, a lantern every few posts
/// and a gap at the front left for the gate.
pub fn build_fence(w: &mut World) {
    let old: Vec<Entity> = w.query::<&Fencing>().iter().map(|(e, _)| e).collect();
    for e in old {
        w.despawn(e);
    }
    let size = w.resource::<Farm>().size as i32;
    let edge = TILE / 2.0;
    let span = size as f32 * TILE;
    let lantern_every = (size / 6).max(3);
    let night = w.resource::<Lighting>().night;
    // Walk the perimeter post by post: front, right, back, left.
    let corners = [
        Vec3::new(-edge, 0.0, edge),
        Vec3::new(span - edge, 0.0, edge),
        Vec3::new(span - edge, 0.0, edge - span),
        Vec3::new(-edge, 0.0, edge - span),
    ];
    // Grass tufts and flowers in a band outside the fence.
    let count = (size * 6).min(600);
    for i in 0..count {
        let h = |k: u32| {
            ((i as u32)
                .wrapping_mul(2654435761)
                .wrapping_add(k.wrapping_mul(40503))
                >> 8) as f32
                / 16_777_216.0
        };
        let side = i % 4;
        let along = h(1) * (span + 8.0) - 4.0;
        let out = 1.5 + h(2) * 10.0;
        let p = match side {
            0 => Vec3::new(along - edge, 0.0, edge + out),
            1 => Vec3::new(span - edge + out, 0.0, edge - along),
            2 => Vec3::new(along - edge, 0.0, edge - span - out),
            _ => Vec3::new(-edge - out, 0.0, edge - along),
        };
        // Keep the path and the stall clear.
        if p.x < 0.5 && p.x > -10.0 && p.z > -4.0 && p.z < 3.0 {
            continue;
        }
        let mut t = Transform::at(p.x, 0.0, p.z).with_scale(0.8 + h(3) * 0.6);
        t.rotation = Quat::from_rotation_y(h(4) * 6.0);
        w.spawn((
            t,
            Mesh::asset(format!("tuft-{}.model", i % 4)),
            Fencing,
            Ambient,
        ));
    }
    let mut post = 0;
    for side in 0..4 {
        let (a, b) = (corners[side], corners[(side + 1) % 4]);
        let along = (b - a) / size as f32;
        let yaw = math::atan2(-along.z, along.x);
        for i in 0..size {
            let p = a + along * i as f32;
            post += 1;
            w.spawn((
                Transform::at(p.x, 0.0, p.z),
                Mesh::asset("fence-post.model"),
                Fencing,
                Ambient,
            ));
            // The gate: no rail across the first two tiles of the front.
            if side == 0 && i < 2 {
                continue;
            }
            let mid = p + along * 0.5;
            let mut rail = Transform::at(mid.x, 0.0, mid.z);
            rail.rotation = Quat::from_rotation_y(yaw);
            w.spawn((rail, Mesh::asset("fence-rail.model"), Fencing, Ambient));
            if post % lantern_every as usize == 0 {
                let mut t = Transform::at(p.x, 0.0, p.z);
                // Hang the lamp outward from the garden.
                t.rotation = Quat::from_rotation_y(yaw - std::f32::consts::FRAC_PI_2);
                let lamp = t.position + t.rotation * Vec3::new(0.33, 1.42, 0.0);
                w.spawn((
                    t,
                    Mesh::asset(if night {
                        "lantern-lit.model"
                    } else {
                        "lantern-dark.model"
                    }),
                    Lantern { lit: night },
                    Fencing,
                    Ambient,
                ));
                w.spawn((
                    Transform::at(lamp.x, lamp.y, lamp.z),
                    PointLight {
                        color: [1.0, 0.72, 0.4],
                        intensity: if night { LANTERN } else { 0.0 },
                        range: 9.0,
                    },
                    Lantern { lit: night },
                    Fencing,
                    Ambient,
                ));
            }
        }
    }
}

/// One tick of looks: the sun and moon, the sky, lanterns, weather, walking.
pub fn step(w: &mut World) {
    let now = now_ms(w);
    let phase = day_phase(now);
    let angle = phase * std::f32::consts::TAU;
    // Elevation follows the sine of the day; the sun crosses from east to west.
    let elevation = math::sin(angle);
    let sun_dir = Vec3::new(math::cos(angle), elevation.max(-0.3), -0.35).normalize();
    let day = (elevation * 4.0 + 0.4).clamp(0.0, 1.0);
    let dusk = (1.0 - (elevation.abs() * 3.0).min(1.0)) * (elevation > -0.25) as u8 as f32;
    let sky = w.resource::<Weather>().sky;
    let overcast = match sky {
        Sky::Clear => 0.0,
        Sky::Snow => 0.45,
        Sky::Rain => 0.6,
        Sky::Storm => 0.85,
    };
    // Lightning: a storm flashes for a tick or three every seven seconds.
    let flash = sky == Sky::Storm && (now / 100) % 70 < 2;
    {
        let target = sun_dir * 30.0;
        let mut t = w.require_mut::<Transform>("sun");
        *t = Transform::at(target.x, target.y, target.z).looking_at(Vec3::ZERO, Vec3::Y);
    }
    {
        let mut sun = w.require_mut::<DirectionalLight>("sun");
        sun.illuminance = 9000.0 * day * (1.0 - 0.8 * overcast);
        sun.color = mix3([1.0, 0.98, 0.92], [1.0, 0.55, 0.3], dusk);
    }
    {
        let moon_dir = Vec3::new(-sun_dir.x, (-elevation).max(0.15), -0.4).normalize() * 30.0;
        *w.require_mut::<Transform>("moon") =
            Transform::at(moon_dir.x, moon_dir.y, moon_dir.z).looking_at(Vec3::ZERO, Vec3::Y);
        w.require_mut::<DirectionalLight>("moon").illuminance =
            900.0 * (1.0 - day) * (1.0 - 0.6 * overcast) + if flash { 20000.0 } else { 0.0 };
    }
    {
        let mut env = w.resource_mut::<Environment>();
        let day_zenith = mix3([0.18, 0.42, 0.85], [0.35, 0.38, 0.42], overcast);
        let day_horizon = mix3([0.62, 0.78, 0.92], [0.55, 0.57, 0.6], overcast);
        let night_zenith = [0.02, 0.03, 0.09];
        let night_horizon = [0.07, 0.09, 0.2];
        let mut zenith = mix3(night_zenith, day_zenith, day);
        let mut horizon = mix3(night_horizon, day_horizon, day);
        horizon = mix3(horizon, [0.95, 0.5, 0.3], dusk * 0.6 * (1.0 - overcast));
        if flash {
            zenith = [0.7, 0.75, 0.9];
            horizon = [0.8, 0.85, 1.0];
        }
        env.zenith = zenith;
        env.horizon = horizon;
        env.ground = mix3([0.01, 0.01, 0.015], [0.08, 0.07, 0.05], day);
        env.background = Some(horizon);
        env.ambient = 0.5 + 0.25 * day;
        env.exposure = 1.0 + 1.2 * (1.0 - day);
        env.sun_disc = if overcast > 0.0 { 0.0 } else { 0.0055 };
        // From the overview the fog would hide the garden; thin it.
        let thin = if w.resource::<Farm>().overview { 0.15 } else { 1.0 };
        env.fog = Some(Fog {
            color: Some(horizon),
            ..Fog::new((0.003 + 0.012 * overcast) * thin, 0.05)
        });
    }
    // Lanterns light at dusk and go out at dawn.
    let night = day < 0.35;
    if w.resource::<Lighting>().night != night {
        w.resource_mut::<Lighting>().night = night;
        let lanterns: Vec<Entity> = w.query::<&Lantern>().iter().map(|(e, _)| e).collect();
        for e in lanterns {
            w.require_mut::<Lantern>(e).lit = night;
            if let Some(mut light) = w.get_mut::<PointLight>(e) {
                light.intensity = if night { LANTERN } else { 0.0 };
            }
            if let Some(mut mesh) = w.get_mut::<Mesh>(e) {
                *mesh = Mesh::asset(if night {
                    "lantern-lit.model"
                } else {
                    "lantern-dark.model"
                });
            }
        }
    }
    // Weather particles follow the sky.
    let code = sky as u8 + 1;
    if w.resource::<Lighting>().sky != code {
        w.resource_mut::<Lighting>().sky = code;
        let mut e = w.require_mut::<Emitter>("weather");
        let state = e.state.clone();
        *e = match sky {
            Sky::Clear => Emitter {
                running: false,
                ..rain(false)
            },
            Sky::Rain => rain(false),
            Sky::Storm => rain(true),
            Sky::Snow => snow(),
        };
        e.state = state;
    }
    walk(w, now);
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The farmer turns to where they walk and swings arms and legs by the
/// distance covered; the keeper sways and waves now and then.
fn walk(w: &mut World, now: u64) {
    let speed = w
        .get::<character::Character>("player")
        .map(|c| c.velocity.xz())
        .unwrap_or_default();
    let moving = speed.length() > 0.3;
    let player = w.named("player").unwrap();
    {
        let dt = w.dt();
        let mut g = w.require_mut::<Gait>(player);
        if moving {
            g.phase += speed.length() * dt * 2.2;
            let want = math::atan2(speed.x, speed.y);
            let mut d = want - g.yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            g.yaw += d * 0.25;
        } else {
            g.phase *= 0.8;
        }
    }
    let (phase, yaw) = {
        let g = w.require::<Gait>(player);
        (g.phase, g.yaw)
    };
    w.require_mut::<Transform>(player).rotation = Quat::from_rotation_y(yaw);
    let idle = math::sin(now as f32 / 700.0);
    let wave = (now / 1000) % 9 == 0;
    for (_, (t, limb, parent)) in w.query::<(&mut Transform, &Limb, &Parent)>().iter() {
        let farmer = parent.0 == player;
        let swing = if farmer { math::sin(phase) * 0.7 } else { 0.0 };
        let (x, z) = match limb.part {
            0 => (
                if farmer { 0.0 } else { idle * 0.05 },
                if farmer { 0.0 } else { idle * 0.15 },
            ),
            1 => (-swing, 0.0),
            2 => {
                if !farmer && wave {
                    (-2.6, 0.3 + idle * 0.3)
                } else {
                    (swing, 0.0)
                }
            }
            3 => (swing, 0.0),
            _ => (-swing, 0.0),
        };
        t.rotation = Quat::from_rotation_x(x) * Quat::from_rotation_z(z);
    }
}
