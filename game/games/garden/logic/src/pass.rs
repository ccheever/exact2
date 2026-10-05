//! The art pass (`art: "pass"`): baked glTF from `art.mjs` (a model per crop
//! and stage, a fruit shape per crop, fence and lanterns, a seed stall with a
//! keeper and a Contract sign, a jointed farmer), a ten-minute day and night
//! with a moon, a sky and fog for each weather, and rain and snow.
//!
//! Saved causes, derived looks: a fruit's mutation, a lantern's glow and the
//! keeper's wave are presentation (`present`), rebuilt from the saved
//! mutation, garden time and tick, never saved or hashed. What the API keeps
//! in the simulation stays there: a plant's model per stage (a `Mesh`), the
//! sun, moon, sky and fog (lights and the `Environment` resource), and the
//! weather emitter. The farmer is the shared gesture rig (`feedback.rs`) under
//! the classic names, so watering, planting and harvest gestures are the same.
use crate::crops::{self, crop};
use crate::farm::Farm;
use crate::feedback::{Cue, Feedback};
use crate::garden::{self, Fruit, Sky, Weather, TILE};
use exact_game::emitter::{Shape, WorldSpace};
use exact_game::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Only this look has it; garden.rs asks for it to choose models and poses.
#[derive(Default, Resource)]
pub struct Pass {
    /// The camera at the farmer's shoulder.
    pub closeup: bool,
    /// The weather the rain and snow emitter was last set for.
    pub sky: u8,
    /// Fence posts, rails, lanterns, their lights and the grass outside,
    /// rebuilt when the garden grows.
    pub fencing: Vec<Entity>,
    /// The lantern bodies, whose glass `present` lights at night.
    pub lanterns: Vec<Entity>,
}

pub fn on(w: &World) -> bool {
    w.try_resource::<Pass>().is_some()
}

macro_rules! crop_models {
    ($($c:literal),*) => {
        &[$(
            concat!("plant-", $c, "-0.model"), concat!("plant-", $c, "-0-far.model"),
            concat!("plant-", $c, "-1.model"), concat!("plant-", $c, "-1-far.model"),
            concat!("plant-", $c, "-2.model"), concat!("plant-", $c, "-2-far.model"),
            concat!("plant-", $c, "-3.model"), concat!("plant-", $c, "-3-far.model"),
            concat!("plant-", $c, "-4.model"), concat!("plant-", $c, "-4-far.model"),
            concat!("fruit-", $c, ".model"), concat!("fruit-", $c, "-far.model"),
            concat!("fruit-", $c, "-unripe.model"), concat!("fruit-", $c, "-unripe-far.model"),
        )*
        "fence-post.model", "fence-rail.model", "lantern.model", "path-0.model",
        "path-1.model", "path-2.model", "tuft-0.model", "tuft-1.model", "tuft-2.model",
        "tuft-3.model", "stall.model", "barrel.model", "can.model", "farmer-body.model",
        "farmer-arm.model", "farmer-leg.model", "keeper-body.model", "keeper-arm.model",
        "keeper-leg.model", "meadow.model", "grass-0.model", "grass-1.model"]
    };
}

/// Every model `art.mjs` writes. `Game::STREAMED`: fetched from the start in
/// every look but never awaited, so the classic look starts as fast as ever;
/// once loaded they stay resident, so a fruit ripening into a model nothing
/// showed a moment ago is drawn at once and never stalls a save.
pub const MODELS: &[&str] = crop_models!(
    "carrot",
    "strawberry",
    "blueberry",
    "tomato",
    "corn",
    "watermelon",
    "pumpkin",
    "apple",
    "bamboo",
    "coconut",
    "cactus",
    "dragon",
    "mango",
    "grape"
);

/// Plants and fruit switch to their far models from here (metres).
const FAR: f32 = 28.0;
const FRUIT_FAR: f32 = 18.0;

fn lod(near: &str) -> ModelLod {
    let far = near.replace(".model", "-far.model");
    ModelLod {
        levels: vec![LodLevel {
            distance: if near.starts_with("fruit") {
                FRUIT_FAR
            } else {
                FAR
            },
            model: far,
        }],
        hide: None,
    }
}

pub fn plant_model(kind: u8, stage: u8) -> (Mesh, ModelLod) {
    let name = format!("plant-{}-{stage}.model", crop(kind).id);
    (Mesh::asset(name.clone()), lod(&name))
}

pub fn fruit_model(kind: u8, ripe: bool) -> (Mesh, ModelLod) {
    let name = format!(
        "fruit-{}{}.model",
        crop(kind).id,
        if ripe { "" } else { "-unripe" }
    );
    (Mesh::asset(name.clone()), lod(&name))
}

/// A plant's turn about its stem, from its tile: neighbours differ, and a
/// regrown fruit hangs where the first one did.
fn yaw(tile: [u16; 2]) -> Quat {
    let h = (tile[0] as u32)
        .wrapping_mul(73_856_093)
        .wrapping_add((tile[1] as u32).wrapping_mul(19_349_663));
    Quat::from_rotation_y(((h >> 8) % 360) as f32 * PI / 180.0)
}

/// Mature models stand on the ground beside the tile centre, unscaled.
pub fn plant_pose(tile: [u16; 2]) -> Transform {
    let c = garden::plant_center(tile);
    Transform {
        position: c,
        rotation: yaw(tile),
        scale: Vec3::ONE,
    }
}

/// Where a slot's fruit hangs, from its plant's base: the shapes and
/// constants `art.mjs` draws (Y0, CANOPY, PALM), before the plant's turn.
pub fn fruit_offset(kind: u8, slot: u8) -> Vec3 {
    const Y0: f32 = 0.08;
    let c = crop(kind);
    let (h, n) = (c.height, c.slots as f32);
    let a = slot as f32 / n * TAU + 0.4;
    let k = (slot % 3) as f32 / 2.0;
    let ring = |r: f32, y: f32| Vec3::new(math::cos(a) * r, y, math::sin(a) * r);
    let canopy = |y: f32, r: f32| {
        let e = [-0.35f32, 0.05, 0.4][slot as usize % 3];
        Vec3::new(0.0, y, 0.0)
            + ring(r * math::cos(e) * 0.98, 0.0)
            + Vec3::Y * (r * 0.9 * math::sin(e))
    };
    match c.id {
        "carrot" => Vec3::new(0.0, 0.04, 0.0),
        "watermelon" => Vec3::new(0.0, c.fruit_size * 0.78 + 0.03, 0.0),
        "pumpkin" => Vec3::new(0.0, c.fruit_size * 0.7 + 0.03, 0.0),
        "bamboo" => Vec3::new(0.3, 0.06, 0.18),
        "strawberry" => ring(0.36, 0.13 + 0.12 * k),
        "blueberry" => ring(0.33, 0.2 + 0.22 * k),
        "tomato" => ring(0.27, 0.3 + h * 0.48 * k),
        "corn" => {
            let s = slot as f32;
            Vec3::new(
                (s - 1.5) * 0.14 + 0.05,
                Y0 + h * (0.4 + 0.12 * s),
                (slot % 2) as f32 * 0.1 - 0.05 + 0.07,
            )
        }
        "apple" => canopy(1.78, 0.8),
        "mango" => canopy(1.95, 0.85),
        "coconut" => {
            let (lean, trunk) = (0.12f32, 2.88f32);
            let top = Vec3::new(
                -math::sin(lean) * trunk,
                Y0 + math::cos(lean) * trunk - 0.2,
                0.0,
            );
            top + ring(0.2, 0.0)
        }
        "grape" => Vec3::new((slot as f32 / (n - 1.0) - 0.5) * 0.8, h * 0.88, -0.24),
        "cactus" => ring(0.21, Y0 + h * (0.5 + 0.3 * k)),
        "dragon" => ring(0.17, Y0 + h * (0.5 + 0.35 * k)),
        _ => ring(0.3, h * 0.5),
    }
}

pub fn fruit_position(tile: [u16; 2], kind: u8, slot: u8) -> Vec3 {
    garden::plant_center(tile) + yaw(tile) * fruit_offset(kind, slot)
}

// ------------------------------------------------------------------ setup

fn model(name: &str) -> Mesh {
    Mesh::asset(name)
}

/// The keeper's and farmer's limbs, on the shared rig's pivots.
const LIMBS: [(&str, f32, f32, bool); 4] = [
    ("arm-left", -0.38, 0.18, true),
    ("arm-right", 0.38, 0.18, true),
    ("leg-left", -0.18, -0.28, false),
    ("leg-right", 0.18, -0.28, false),
];

/// Where the stall stands, and the way it faces (towards the garden's gate).
const STALL: Vec3 = Vec3::new(-9.0, 0.0, -1.5);
const STALL_YAW: f32 = 0.95;

pub fn setup(w: &mut World) {
    w.insert_resource(Pass::default());
    w.insert_resource(AmbientOcclusion {
        radius: 0.5,
        intensity: 0.9,
        ..AmbientOcclusion::default()
    });
    // Day and night move these every tick; they do not hold the clock awake.
    w.ambient_resource::<Environment>();
    w.derived_publication("clock").derived_publication("night");
    let sun = w.named("sun").unwrap();
    w.insert(sun, Ambient);
    w.spawn_named(
        "moon",
        (
            Transform::at(-10.0, 20.0, -8.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: [0.55, 0.66, 1.0],
                illuminance: 0.0,
                shadows: false,
            },
            Ambient,
        ),
    );
    *w.require_mut::<Material>("ground") = Material::grid([0.085, 0.05, 0.022], TILE);
    // The farmer: the classic names on the shared gesture rig.
    let player = w.named("player").unwrap();
    w.remove::<Mesh>(player);
    w.remove::<Material>(player);
    let body = w.spawn_named(
        "gardener",
        (
            Transform::default(),
            model("farmer-body.model"),
            Parent(player),
        ),
    );
    for (name, x, y, arm) in LIMBS {
        let part = if arm {
            "farmer-arm.model"
        } else {
            "farmer-leg.model"
        };
        w.spawn_named(name, (Transform::at(x, y, 0.0), model(part), Parent(body)));
    }
    w.spawn_named(
        "watering-can",
        (
            Transform::at(0., -0.47, 0.16).with_scale(1.25),
            model("can.model"),
            Parent(w.named("arm-left").unwrap()),
            Visible(false),
        ),
    );
    *w.require_mut::<Mesh>("water-barrel") = model("barrel.model");
    w.remove::<Material>(w.named("water-barrel").unwrap());
    *w.require_mut::<Mesh>("barrel-water") = Mesh::cylinder(0.55, 0.04);
    w.require_mut::<Transform>("barrel-water").position.y = 0.9;
    *w.require_mut::<Material>("barrel-water") = Material {
        color: [0.05, 0.18, 0.24, 1.0],
        roughness: 0.05,
        ..Material::default()
    };
    w.spawn_named(
        "meadow",
        (
            Transform::at(0.0, -0.14, 0.0),
            Mesh::plane(1600.0, 1600.0),
            garden::paint([0.3, 0.52, 0.17]),
        ),
    );
    // Turf over it near the garden, where the plane would read as flat paint.
    w.spawn_named(
        "turf",
        (
            Transform::at(0.0, -0.08, 0.0),
            model("meadow.model"),
            Ambient,
        ),
    );
    stall(w);
    for (i, tree) in TREES.iter().enumerate() {
        w.spawn_named(
            format!("tree-{i}"),
            (Transform::default(), model(tree.0), lod(tree.0), Ambient),
        );
    }
    w.spawn_named(
        "weather",
        (
            Transform::at(0.0, 5.0, -5.0),
            Follow::new(player).offset(0.0, 4.1, -5.0).lag(0.0),
            Emitter {
                running: false,
                ..rain(false)
            },
            WorldSpace::default(),
            ParticleLook {
                stretch: 0.05,
                ..ParticleLook::default()
            },
            Ambient,
        ),
    );
    build_fence(w);
    step(w);
}

fn stall(w: &mut World) {
    let turn = Quat::from_rotation_y(STALL_YAW);
    let at = |local: Vec3| STALL + turn * local;
    w.spawn_named(
        "stall",
        (
            Transform {
                position: STALL,
                rotation: turn,
                scale: Vec3::ONE,
            },
            model("stall.model"),
            Ambient,
        ),
    );
    // The sign is Contract text on a plane over the awning.
    let sign = at(Vec3::new(0.0, 2.91, 0.4));
    w.spawn_named(
        "stall-sign",
        (
            Transform {
                position: sign,
                rotation: turn,
                scale: Vec3::ONE,
            },
            Placed::child("stall-sign")
                .width(1.85)
                .facing(Facing::Fixed),
            Ambient,
        ),
    );
    let keeper = at(Vec3::new(-0.4, 0.9, -0.62));
    let keeper = w.spawn_named(
        "keeper",
        (
            Transform {
                position: keeper,
                rotation: turn,
                scale: Vec3::ONE,
            },
            model("keeper-body.model"),
            Ambient,
        ),
    );
    for (name, x, y, arm) in LIMBS {
        let part = if arm {
            "keeper-arm.model"
        } else {
            "keeper-leg.model"
        };
        w.spawn_named(
            format!("keeper-{name}"),
            (
                Transform::at(x, y, 0.0),
                model(part),
                Parent(keeper),
                Ambient,
            ),
        );
    }
    // Stepping stones from the stall to the garden's gate.
    let gate = Vec3::new(-3.2, 0.01, 0.0);
    let front = at(Vec3::new(0.0, 0.0, 1.6));
    for i in 0..4 {
        let p = front.lerp(gate, i as f32 / 3.0);
        w.spawn((
            Transform::at(p.x, 0.01, p.z),
            model(["path-0.model", "path-1.model", "path-2.model"][i % 3]),
            Ambient,
        ));
    }
}

/// The trees around the meadow: grown plant models, scaled up, placed by
/// `resize` around the garden's pad: (model, side, fraction along it,
/// metres out, scale); side 0 is north, 1 west and 2 east.
const TREES: [(&str, u8, f32, f32, f32); 12] = [
    ("plant-apple-4.model", 0, 0.05, 6.0, 2.3),
    ("plant-mango-4.model", 0, 0.3, 9.0, 2.6),
    ("plant-apple-4.model", 0, 0.55, 5.5, 2.1),
    ("plant-coconut-4.model", 0, 0.75, 8.0, 1.9),
    ("plant-mango-4.model", 0, 0.95, 7.0, 2.4),
    ("plant-apple-4.model", 1, 0.85, 6.5, 2.2),
    ("plant-coconut-4.model", 1, 0.6, 11.0, 2.0),
    ("plant-mango-4.model", 1, 0.0, 12.0, 2.5),
    ("plant-apple-4.model", 1, -0.25, 7.0, 2.0),
    ("plant-apple-4.model", 2, 0.2, 7.0, 2.3),
    ("plant-mango-4.model", 2, 0.65, 9.0, 2.6),
    ("plant-coconut-4.model", 2, 0.95, 6.0, 1.8),
];

/// Moves the trees with the garden; the fence is rebuilt by `build_fence`.
pub fn resize(w: &World, span: f32, _mid: f32) {
    let (x0, x1, z0, z1) = (-3.0, span + 1.0, -(span + 1.0), 3.0);
    for (i, &(_, side, t, out, s)) in TREES.iter().enumerate() {
        let position = match side {
            0 => Vec3::new(x0 + t * (x1 - x0), 0.0, z0 - out),
            1 => Vec3::new(x0 - out, 0.0, z1 - t * (z1 - z0)),
            _ => Vec3::new(x1 + out, 0.0, z1 - t * (z1 - z0)),
        };
        let mut pose = w.require_mut::<Transform>(format!("tree-{i}").as_str());
        pose.position = position;
        pose.rotation = Quat::from_rotation_y(i as f32 * 1.7);
        pose.scale = Vec3::splat(s);
    }
}

/// Posts on the pad's edge every two metres, rails between them with a gate
/// facing the stall, a lantern every few posts and grass in a band outside.
pub fn build_fence(w: &mut World) {
    let old = std::mem::take(&mut w.resource_mut::<Pass>().fencing);
    for e in old {
        w.despawn(e);
    }
    let size = w.resource::<Farm>().size as usize;
    let span = size as f32 * TILE;
    let (x0, x1, z0, z1) = (-3.0, span + 1.0, -(span + 1.0), 3.0);
    let segments = size + 2;
    let every = (size / 6).max(3);
    let corners = [
        Vec3::new(x0, 0.0, z1),
        Vec3::new(x1, 0.0, z1),
        Vec3::new(x1, 0.0, z0),
        Vec3::new(x0, 0.0, z0),
    ];
    let mut fencing = Vec::new();
    let mut lanterns = Vec::new();
    let mut post = 0;
    for side in 0..4 {
        let (a, b) = (corners[side], corners[(side + 1) % 4]);
        let along = (b - a) / segments as f32;
        let turn = Quat::from_rotation_y(math::atan2(-along.z, along.x));
        // Outward from the garden: the walk runs clockwise seen from above.
        let out = turn * Vec3::Z;
        for i in 0..segments {
            let p = a + along * i as f32;
            post += 1;
            fencing.push(w.spawn((
                Transform::at(p.x, 0.0, p.z),
                model("fence-post.model"),
                Ambient,
            )));
            // The gate, on the west side by the barrel, faces the stall.
            let gate = side == 3 && i == size;
            if !gate {
                let mid = p + along * 0.5;
                fencing.push(w.spawn((
                    Transform {
                        position: mid,
                        rotation: turn,
                        scale: Vec3::ONE,
                    },
                    model("fence-rail.model"),
                    Ambient,
                )));
            }
            if post % every == 0 || gate || (side == 3 && i == size + 1) {
                // Hung over the garden, so their light falls on the plots.
                let base = p - out * 0.2;
                let hang = Quat::from_rotation_y(math::atan2(out.z, -out.x));
                let lamp = base + hang * Vec3::new(0.36, 1.45, 0.0);
                let body = w.spawn((
                    Transform {
                        position: base,
                        rotation: hang,
                        scale: Vec3::ONE,
                    },
                    model("lantern.model"),
                    Ambient,
                ));
                lanterns.push(body);
                fencing.push(body);
                fencing.push(w.spawn((
                    Transform::at(lamp.x, lamp.y, lamp.z),
                    PointLight {
                        color: [1.0, 0.7, 0.38],
                        intensity: LANTERN,
                        range: 10.0,
                    },
                    Ambient,
                )));
            }
        }
    }
    let count = (size * 6).min(600);
    for i in 0..count {
        let h = |k: u32| {
            ((i as u32)
                .wrapping_mul(2_654_435_761)
                .wrapping_add(k.wrapping_mul(40_503))
                >> 8) as f32
                / 16_777_216.0
        };
        let along = h(1) * (span + 12.0) - 6.0;
        let out = 1.0 + h(2) * 9.0;
        let p = match i % 4 {
            0 => Vec3::new(x0 + along, 0.0, z1 + out),
            1 => Vec3::new(x1 + out, 0.0, z1 - along),
            2 => Vec3::new(x0 + along, 0.0, z0 - out),
            _ => Vec3::new(x0 - out, 0.0, z1 - along),
        };
        // Keep the path to the stall clear.
        if p.x < x0 && p.x > STALL.x - 3.0 && p.z > STALL.z - 3.0 && p.z < 2.5 {
            continue;
        }
        let mut t = Transform::at(p.x, 0.0, p.z).with_scale(0.8 + h(3) * 0.7);
        t.rotation = Quat::from_rotation_y(h(4) * TAU);
        fencing.push(w.spawn((t, model(TUFTS[i % 4]), Ambient)));
    }
    // Patches of long grass and flowers in a band outside the fence; they
    // disappear past 80 m, where they would be a few pixels each.
    let band = 22.0;
    let cells = ((span + 4.0 + 2.0 * band) / 10.0).ceil() as i32;
    for gz in 0..cells {
        for gx in 0..cells {
            let c = Vec3::new(
                x0 - band + 5.0 + gx as f32 * 10.0,
                0.0,
                z1 + band - 5.0 - gz as f32 * 10.0,
            );
            let inside = c.x > x0 - 5.5 && c.x < x1 + 5.5 && c.z > z0 - 5.5 && c.z < z1 + 5.5;
            let path = c.x < x0 + 1.0 && c.x > STALL.x - 7.0 && c.z > STALL.z - 6.0 && c.z < 6.0;
            if inside || path {
                continue;
            }
            let n = (gx * 7 + gz * 13) as usize;
            let mut t = Transform::at(c.x, -0.02, c.z);
            t.rotation = Quat::from_rotation_y((n % 4) as f32 * FRAC_PI_2);
            fencing.push(w.spawn((
                t,
                model(["grass-0.model", "grass-1.model"][n % 2]),
                ModelLod {
                    levels: Vec::new(),
                    hide: Some(80.0),
                },
                Ambient,
            )));
        }
    }
    let mut pass = w.resource_mut::<Pass>();
    pass.fencing = fencing;
    pass.lanterns = lanterns;
}

const TUFTS: [&str; 4] = [
    "tuft-0.model",
    "tuft-1.model",
    "tuft-2.model",
    "tuft-3.model",
];

/// A lantern's candela. Lit all day, where the sun drowns it.
const LANTERN: f32 = 2600.0;

/// The pass's camera: lower and closer than the classic one, or at the
/// farmer's shoulder. The overview stays the classic one.
pub fn camera(w: &World) -> Follow {
    let player = w.named("player").unwrap();
    if w.resource::<Pass>().closeup {
        Follow::new(player)
            .offset(2.3, 2.0, 3.1)
            .look_at_offset(1.3, -0.45, -1.8)
            .lag(0.15)
    } else {
        Follow::new(player)
            .offset(0.0, 6.5, 8.5)
            .look_at_offset(0.0, 0.6, 0.0)
            .lag(0.15)
    }
}

pub fn toggle_closeup(w: &World) -> String {
    let c = !w.resource::<Pass>().closeup;
    w.resource_mut::<Pass>().closeup = c;
    (if c { "Close-up" } else { "Garden view" }).into()
}

// ------------------------------------------------------------- day, night

/// Garden milliseconds in one day.
pub const DAY_MS: u64 = 600_000;

/// The time of day in [0, 1): 0 is sunrise, 0.25 noon, 0.5 sunset. A new
/// garden starts in the morning.
pub fn day_phase(now: u64) -> f32 {
    ((now + DAY_MS / 12) % DAY_MS) as f32 / DAY_MS as f32
}

/// "Day 2 · 14:30", to ten garden minutes.
pub fn clock_label(now: u64) -> String {
    let day = (now + DAY_MS / 12) / DAY_MS + 1;
    let minutes = ((day_phase(now) * 144.0) as u32 * 10 + 6 * 60) % (24 * 60);
    format!("Day {day} · {:02}:{:02}", minutes / 60, minutes % 60)
}

/// Daylight in [0, 1] from the sun's elevation.
fn daylight(now: u64) -> f32 {
    (math::sin(day_phase(now) * TAU) * 4.0 + 0.4).clamp(0.0, 1.0)
}

fn rain(storm: bool) -> Emitter {
    Emitter {
        shape: Shape::Box(Vec3::new(30.0, 9.0, 24.0)),
        rate: if storm { 4800.0 } else { 3200.0 },
        lifetime: 0.6,
        speed: 0.0,
        spread: 0.0,
        gravity: Vec3::new(if storm { -5.0 } else { -0.8 }, -26.0, 0.0),
        size: [0.05, 0.04],
        color: [[0.8, 0.86, 0.96, 0.75], [0.8, 0.86, 0.96, 0.45]],
        bound: [-16.0, -10.0, -13.0, 16.0, 5.0, 13.0],
        seed: 11,
        ..Emitter::default()
    }
}

fn snow() -> Emitter {
    Emitter {
        shape: Shape::Box(Vec3::new(30.0, 9.0, 24.0)),
        rate: 520.0,
        lifetime: 4.0,
        speed: 0.35,
        spread: PI,
        gravity: Vec3::new(0.3, -1.4, 0.0),
        drag: 0.4,
        size: [0.11, 0.08],
        color: [[1.0, 1.0, 1.0, 0.9], [0.92, 0.96, 1.0, 0.0]],
        bound: [-17.0, -12.0, -14.0, 17.0, 6.0, 14.0],
        seed: 12,
        ..Emitter::default()
    }
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// One tick of sky: the sun and moon, sky, fog and exposure for the time and
/// weather, lightning in a storm, the weather's particles and the clock.
pub fn step(w: &mut World) {
    let now = garden::now_ms(w);
    let angle = day_phase(now) * TAU;
    let elevation = math::sin(angle);
    let sun_dir = Vec3::new(math::cos(angle), elevation.max(-0.3), -0.45).normalize();
    let day = daylight(now);
    let dusk = (1.0 - (elevation.abs() * 3.0).min(1.0)) * if elevation > -0.25 { 1.0 } else { 0.0 };
    let sky = w.resource::<Weather>().sky;
    let overcast = match sky {
        Sky::Clear => 0.0,
        Sky::Snow => 0.45,
        Sky::Rain => 0.6,
        Sky::Storm => 0.85,
    };
    // Lightning: a storm flashes for a few ticks every seven seconds.
    let flash = sky == Sky::Storm && (now / 100) % 70 < 2;
    let target = sun_dir * 30.0;
    *w.require_mut::<Transform>("sun") =
        Transform::at(target.x, target.y, target.z).looking_at(Vec3::ZERO, Vec3::Y);
    {
        let mut sun = w.require_mut::<DirectionalLight>("sun");
        sun.illuminance = 11000.0 * day * (1.0 - 0.9 * overcast);
        sun.color = mix3([1.0, 0.95, 0.86], [1.0, 0.56, 0.3], dusk);
        sun.shadows = true;
    }
    let moon_dir = Vec3::new(-sun_dir.x, (-elevation).max(0.25), -0.5).normalize() * 30.0;
    *w.require_mut::<Transform>("moon") =
        Transform::at(moon_dir.x, moon_dir.y, moon_dir.z).looking_at(Vec3::ZERO, Vec3::Y);
    w.require_mut::<DirectionalLight>("moon").illuminance =
        1200.0 * (1.0 - day) * (1.0 - 0.6 * overcast) + if flash { 30000.0 } else { 0.0 };
    let overview = w.resource::<Farm>().overview;
    {
        let day_zenith = mix3([0.16, 0.38, 0.82], [0.34, 0.37, 0.42], overcast);
        let day_horizon = mix3([0.62, 0.78, 0.92], [0.56, 0.58, 0.6], overcast);
        let mut zenith = mix3([0.01, 0.016, 0.055], day_zenith, day);
        let mut horizon = mix3([0.04, 0.05, 0.12], day_horizon, day);
        horizon = mix3(horizon, [0.98, 0.52, 0.3], dusk * 0.6 * (1.0 - overcast));
        if flash {
            zenith = [0.7, 0.75, 0.9];
            horizon = [0.8, 0.85, 1.0];
        }
        let mut env = w.resource_mut::<Environment>();
        env.zenith = zenith;
        env.horizon = horizon;
        env.ground = mix3([0.01, 0.012, 0.02], [0.14, 0.13, 0.08], day);
        env.background = None;
        env.ambient = 0.45 + 0.3 * day;
        env.exposure = 0.92 + 0.5 * (1.0 - day);
        env.sun_disc = if overcast > 0.0 { 0.0 } else { 0.006 };
        // From the overview the fog would hide the garden; thin it.
        let thin = if overview { 0.15 } else { 1.0 };
        env.fog = Some(Fog {
            color: Some(horizon),
            ..Fog::new((0.0022 + 0.014 * overcast) * thin, 0.05)
        });
        env.bloom = Some(Bloom {
            threshold: 0.9,
            intensity: 0.22,
            radius: 1.8,
        });
    }
    let code = sky as u8 + 1;
    if w.resource::<Pass>().sky != code {
        w.resource_mut::<Pass>().sky = code;
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
        drop(e);
        w.require_mut::<ParticleLook>("weather").stretch =
            if sky == Sky::Snow { 0.0 } else { 0.05 };
    }
    let label = clock_label(now);
    if w.published("clock").map(|v| v.text().to_owned()) != Some(label.clone()) {
        w.publish_record(&Clock {
            clock: label,
            night: day < 0.35,
        });
    }
}

/// The HUD's clock pill.
#[derive(Clone, Default, Data)]
struct Clock {
    clock: String,
    night: bool,
}

// ----------------------------------------------------------- presentation

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [f(5.0), f(3.0), f(1.0)].map(|c| c * c)
}

/// A ripe fruit's look for its saved mutations: the strongest shows, and
/// Rainbow, Gold and Shocked move with the clock.
fn mutation(
    kind: u8,
    muts: u8,
    seconds: f32,
    phase: f32,
    flicker: f32,
) -> Option<MaterialOverride> {
    let base = crop(kind).fruit.map(|c| c * c);
    let (color, emissive) = if muts & crops::RAINBOW != 0 {
        let c = hsv((seconds * 0.3 + phase).fract(), 0.7, 1.0);
        (c, c.map(|v| v * 0.7))
    } else if muts & crops::GOLD != 0 {
        let glint = 0.55 + 0.45 * math::sin(seconds * 3.0 + phase * TAU);
        ([1.0, 0.7, 0.16], [0.55 * glint, 0.36 * glint, 0.05 * glint])
    } else if muts & crops::SHOCKED != 0 {
        (
            [1.0, 0.95, 0.55],
            [2.4 * flicker, 2.1 * flicker, 0.5 * flicker],
        )
    } else if muts & crops::FROZEN != 0 {
        ([0.62, 0.86, 1.0], [0.08, 0.22, 0.42])
    } else if muts & crops::WET != 0 {
        (base.map(|c| c * 0.6).map(|c| c + 0.015), [0.0; 3])
    } else if muts & crops::CHILLED != 0 {
        (mix3(base, [0.7, 0.86, 1.0], 0.45), [0.0; 3])
    } else {
        return None;
    };
    Some(MaterialOverride {
        material: 0,
        color: Some([color[0], color[1], color[2], 1.0]),
        emissive,
    })
}

/// Everything in this look that only shows: mutation looks, lantern glow,
/// the keeper's sway and wave, and the harvested fruit's look in flight.
pub fn present(p: &mut Present) {
    let Some((lanterns, offline)) = p
        .resource::<Pass>()
        .zip(p.resource::<garden::GardenClock>())
        .map(|(pass, clock)| (pass.lanterns.clone(), clock.offline_ms))
    else {
        return;
    };
    let now = p.tick() * 1000 / p.hz() as u64 + offline;
    let seconds = (p.tick() % (p.hz() as u64 * 3600)) as f32 / p.hz() as f32;
    let mut looks = Vec::new();
    p.for_each::<Fruit>(|e, f| {
        if f.ripe && f.muts != 0 {
            looks.push((e, f.kind, f.muts));
        }
    });
    for (e, kind, muts) in looks {
        let phase = (e.index() % 97) as f32 / 97.0;
        let flicker = if muts & crops::SHOCKED != 0 {
            let mut r = p.rng(e.index() as u64);
            0.4 + 1.2 * r.next_f32()
        } else {
            0.0
        };
        if let Some(look) = mutation(kind, muts, seconds, phase, flicker) {
            p.insert(e, MaterialOverrides(vec![look]));
        }
    }
    // Lanterns glow from dusk to dawn.
    let glow = (1.0 - daylight(now) * 1.6).clamp(0.0, 1.0);
    if glow > 0.0 {
        for e in lanterns {
            p.insert(
                e,
                MaterialOverrides(vec![MaterialOverride {
                    material: 0,
                    color: None,
                    emissive: [3.2 * glow, 2.0 * glow, 0.8 * glow],
                }]),
            );
        }
    }
    // The keeper sways, nods and waves every nine seconds.
    let idle = math::sin(seconds * 1.4);
    if let Some(keeper) = p.named("keeper") {
        p.insert(
            keeper,
            Offset(Transform {
                rotation: Quat::from_rotation_z(idle * 0.05) * Quat::from_rotation_x(idle * 0.03),
                ..Transform::default()
            }),
        );
    }
    let cycle = seconds % 9.0;
    let wave = if cycle < 1.8 {
        math::sin(cycle / 1.8 * PI)
    } else {
        0.0
    };
    if let Some(arm) = p.named("keeper-arm-right") {
        let raise = Quat::from_rotation_z(wave * 2.5) * Quat::from_rotation_x(-wave * 0.3);
        let flap = Quat::from_rotation_z(wave * 0.35 * math::sin(seconds * 14.0));
        p.insert(
            arm,
            Offset(Transform {
                rotation: raise * flap,
                ..Transform::default()
            }),
        );
    }
    if let Some(arm) = p.named("keeper-arm-left") {
        p.insert(
            arm,
            Offset(Transform {
                rotation: Quat::from_rotation_x(-0.25 + idle * 0.05),
                ..Transform::default()
            }),
        );
    }
    fade_occluders(p);
    // The fruit flying into the satchel keeps its mutation look.
    let picked = p
        .resource::<Feedback>()
        .is_some_and(|f| f.cue == Cue::Harvest);
    if picked {
        let last = p
            .resource::<Farm>()
            .and_then(|farm| farm.bag.last().map(|i| (i.kind, i.muts)));
        if let (Some((kind, muts)), Some(e)) = (last, p.named("picked-fruit")) {
            if let Some(look) = mutation(kind, muts, seconds, 0.0, 1.0) {
                p.insert(e, MaterialOverrides(vec![look]));
            }
        }
    }
}

/// Plants standing between a near camera and the farmer fade to a dither, with
/// their fruit, so the close-up never loses the player behind a tree. Visits
/// only the tiles under the sight line, never the garden.
fn fade_occluders(p: &mut Present) {
    let (Some(camera), Some(player)) = (p.named("camera"), p.named("player")) else {
        return;
    };
    let (Some(eye), Some(target)) = (p.global(camera), p.global(player)) else {
        return;
    };
    let (eye, target) = (Vec3::from(eye.translation), Vec3::from(target.translation));
    let line = Vec3::new(eye.x - target.x, 0.0, eye.z - target.z);
    let length = line.length();
    if !(0.5..15.0).contains(&length) {
        return;
    }
    let mut faded: Vec<(Entity, Vec<Entity>)> = Vec::new();
    if let Some(farm) = p.resource::<Farm>() {
        let steps = (length / 0.5) as i32 + 1;
        for i in 1..=steps {
            let at = target + line * (i as f32 / steps as f32);
            let (x, z) = (at.x / TILE, -at.z / TILE);
            for (tx, tz) in [
                (x.floor(), z.floor()),
                (x.ceil(), z.ceil()),
                (x.floor(), z.ceil()),
                (x.ceil(), z.floor()),
            ] {
                if tx < 0.0 || tz < 0.0 {
                    continue;
                }
                let tile = [tx as u16, tz as u16];
                let Some(plant) = farm.at(tile) else { continue };
                if faded.iter().any(|&(e, _)| e == plant) {
                    continue;
                }
                // Beside the sight line, and tall enough to hide the farmer.
                let base = garden::plant_center(tile) - target;
                let along = base.dot(line) / (length * length);
                let off = (base - line * along.clamp(0.0, 1.0)).length();
                let tall = p.get::<garden::Plant>(plant).is_some_and(|pl| {
                    crop(pl.kind).height * (0.3 + 0.7 * pl.stage as f32 / 4.0) > 0.7
                });
                if off < 1.0 && along > 0.05 && tall {
                    let fruits = p
                        .get::<garden::Plant>(plant)
                        .map(|pl| pl.fruits.iter().flatten().copied().collect())
                        .unwrap_or_default();
                    faded.push((plant, fruits));
                }
            }
        }
    }
    for (plant, fruits) in faded {
        p.insert(plant, Opacity(0.3));
        for fruit in fruits {
            p.insert(fruit, Opacity(0.3));
        }
    }
}
