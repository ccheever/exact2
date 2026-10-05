//! The art pass (`art: "pass"`): baked glTF from `art.mjs` (a model per crop
//! and stage, a fruit shape per crop, fence and lanterns, a seed stall with a
//! keeper and a Contract sign, a jointed farmer), a ten-minute day and night
//! with a moon, a sky and fog for each weather, and rain and snow.
//!
//! Saved causes, derived looks. The simulation keeps what every look shares:
//! the crops' classic models and poses, and this look's props as bare poses
//! (the fence, stall, keeper, trees), placed in every look so the look
//! switches live. `present` draws the rest: each plant's stage model where it
//! stands, each fruit on its branch, the sun, moon, sky and fog for the hour
//! and the weather, a lantern's light, a fruit's mutation and the keeper's
//! wave, rebuilt from the saved garden time and tick, never saved or hashed.
//! The rain and snow emitter is simulation in every look, drawn only in this
//! one. The farmer is the shared gesture rig (`feedback.rs`) under the classic
//! names, so watering, planting and harvest gestures are the same.
use crate::art::{dress, hide, place};
use crate::crops::{self, crop};
use crate::farm::{Farm, BARREL};
use crate::feedback::{Cue, Feedback};
use crate::garden::{self, paint, Fruit, Plant, Sky, Weather, TILE};
use exact_game::emitter::{Shape, WorldSpace};
use exact_game::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The camera mode and this look's props, placed in every look so a switch to
/// the art pass finds them where the garden has grown to.
#[derive(Default, Resource)]
pub struct Props {
    /// The art pass's camera at the farmer's shoulder.
    pub closeup: bool,
    /// The weather the rain and snow emitter was last set for.
    pub sky: u8,
    /// Fence posts, rails, lanterns, their lights and the grass outside,
    /// rebuilt when the garden grows.
    pub fencing: Vec<Fencing>,
}

/// One fence prop: what it is, never how it looks.
#[derive(Clone, Copy, Default, PartialEq, Data)]
pub struct Fencing {
    pub entity: Entity,
    pub prop: Prop,
    /// Which tuft or grass patch.
    pub variant: u8,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Data)]
pub enum Prop {
    #[default]
    Post,
    Rail,
    Lantern,
    /// A lantern's light.
    Lamp,
    Tuft,
    Grass,
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

/// A plant's model at a growth stage, with its far level.
pub fn plant_model(kind: u8, stage: u8) -> DrawnMesh {
    let name = format!("plant-{}-{stage}.model", crop(kind).id);
    DrawnMesh::model(name.clone()).lod(lod(&name))
}

/// A fruit's model, ripe or not, with its far level, in its own colours.
pub fn fruit_model(kind: u8, ripe: bool) -> DrawnMesh {
    let name = format!(
        "fruit-{}{}.model",
        crop(kind).id,
        if ripe { "" } else { "-unripe" }
    );
    DrawnMesh::model(name.clone())
        .lod(lod(&name))
        .material(Material::default())
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

/// This look's props as bare poses (every look has them; only this one draws
/// them), the weather's emitter, and the clock.
pub fn setup(w: &mut World) {
    w.insert_resource(Props::default());
    // The clock moves every few seconds; it does not hold the garden awake.
    w.derived_publication("clock").derived_publication("night");
    let player = w.named("player").unwrap();
    // Turf over the meadow near the garden, where the plane would read as flat paint.
    w.spawn_named("turf", (Transform::at(0.0, -0.08, 0.0), Ambient));
    stall(w);
    for i in 0..TREES.len() {
        w.spawn_named(format!("tree-{i}"), (Transform::default(), Ambient));
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
    let posed = |position: Vec3| Transform {
        position,
        rotation: turn,
        scale: Vec3::ONE,
    };
    w.spawn_named("stall", (posed(STALL), Ambient));
    // The sign is Contract text on a plane over the awning, shown in this look.
    w.spawn_named(
        "stall-sign",
        (
            posed(at(Vec3::new(0.0, 2.91, 0.4))),
            Placed::child("stall-sign")
                .width(1.85)
                .facing(Facing::Fixed),
            Ambient,
        ),
    );
    let keeper = w.spawn_named("keeper", (posed(at(Vec3::new(-0.4, 0.9, -0.62))), Ambient));
    for (name, x, y, _) in LIMBS {
        w.spawn_named(
            format!("keeper-{name}"),
            (Transform::at(x, y, 0.0), Parent(keeper), Ambient),
        );
    }
    // Stepping stones from the stall to the garden's gate.
    let gate = Vec3::new(-3.2, 0.01, 0.0);
    let front = at(Vec3::new(0.0, 0.0, 1.6));
    for i in 0..4 {
        let p = front.lerp(gate, i as f32 / 3.0);
        w.spawn_named(
            format!("path-{i}"),
            (Transform::at(p.x, 0.01, p.z), Ambient),
        );
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
pub fn resize(w: &World, span: f32) {
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
    let old = std::mem::take(&mut w.resource_mut::<Props>().fencing);
    for f in old {
        w.despawn(f.entity);
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
    let mut prop = |w: &mut World, pose: Transform, prop: Prop, variant: u8| {
        let entity = w.spawn((pose, Ambient));
        fencing.push(Fencing {
            entity,
            prop,
            variant,
        });
    };
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
            prop(w, Transform::at(p.x, 0.0, p.z), Prop::Post, 0);
            // The gate, on the west side by the barrel, faces the stall.
            let gate = side == 3 && i == size;
            if !gate {
                let mid = p + along * 0.5;
                let rail = Transform {
                    position: mid,
                    rotation: turn,
                    scale: Vec3::ONE,
                };
                prop(w, rail, Prop::Rail, 0);
            }
            if post % every == 0 || gate || (side == 3 && i == size + 1) {
                // Hung over the garden, so their light falls on the plots.
                let base = p - out * 0.2;
                let hang = Quat::from_rotation_y(math::atan2(out.z, -out.x));
                let lamp = base + hang * Vec3::new(0.36, 1.45, 0.0);
                let body = Transform {
                    position: base,
                    rotation: hang,
                    scale: Vec3::ONE,
                };
                prop(w, body, Prop::Lantern, 0);
                prop(w, Transform::at(lamp.x, lamp.y, lamp.z), Prop::Lamp, 0);
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
        prop(w, t, Prop::Tuft, (i % 4) as u8);
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
            prop(w, t, Prop::Grass, (n % 2) as u8);
        }
    }
    w.resource_mut::<Props>().fencing = fencing;
}

const TUFTS: [&str; 4] = [
    "tuft-0.model",
    "tuft-1.model",
    "tuft-2.model",
    "tuft-3.model",
];

/// A lantern's candela. Lit all day, where the sun drowns it.
const LANTERN: f32 = 2600.0;

pub fn toggle_closeup(w: &World) -> String {
    let c = !w.resource::<Props>().closeup;
    w.resource_mut::<Props>().closeup = c;
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

/// One tick of what the weather and the hour change in the simulation, in
/// every look: the rain and snow emitter (its particles are the weather's
/// timeline; this look draws them) and the HUD's clock. The sky itself is
/// `present`'s.
pub fn step(w: &mut World) {
    let now = garden::now_ms(w);
    let sky = w.resource::<Weather>().sky;
    let code = sky as u8 + 1;
    if w.resource::<Props>().sky != code {
        w.resource_mut::<Props>().sky = code;
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
            night: daylight(now) < 0.35,
        });
    }
}

/// The HUD's clock pill.
#[derive(Clone, Default, Data)]
struct Clock {
    clock: String,
    night: bool,
}

/// The sun and moon, sky, fog and exposure for the time and weather, and
/// lightning in a storm: drawn on the simulation's sun, its spare light and
/// the camera.
fn sky(p: &mut Present, now: u64) {
    let angle = day_phase(now) * TAU;
    let elevation = math::sin(angle);
    let sun_dir = Vec3::new(math::cos(angle), elevation.max(-0.3), -0.45).normalize();
    let day = daylight(now);
    let dusk = (1.0 - (elevation.abs() * 3.0).min(1.0)) * if elevation > -0.25 { 1.0 } else { 0.0 };
    let sky = p.resource::<Weather>().map_or(Sky::Clear, |w| w.sky);
    let overcast = match sky {
        Sky::Clear => 0.0,
        Sky::Snow => 0.45,
        Sky::Rain => 0.6,
        Sky::Storm => 0.85,
    };
    // Lightning: a storm flashes for a few ticks every seven seconds.
    let flash = sky == Sky::Storm && (now / 100) % 70 < 2;
    let aim = |at: Vec3| Transform::at(at.x, at.y, at.z).looking_at(Vec3::ZERO, Vec3::Y);
    if let Some(sun) = p.named("sun") {
        place(p, sun, aim(sun_dir * 30.0));
        p.insert(
            sun,
            DrawnLight::Directional(DirectionalLight {
                color: mix3([1.0, 0.95, 0.86], [1.0, 0.56, 0.3], dusk),
                illuminance: 11000.0 * day * (1.0 - 0.9 * overcast),
                shadows: true,
            }),
        );
    }
    if let Some(moon) = p.named("moon") {
        let moon_dir = Vec3::new(-sun_dir.x, (-elevation).max(0.25), -0.5).normalize();
        place(p, moon, aim(moon_dir * 30.0));
        p.insert(
            moon,
            DrawnLight::Directional(DirectionalLight {
                color: [0.55, 0.66, 1.0],
                illuminance: 1200.0 * (1.0 - day) * (1.0 - 0.6 * overcast)
                    + if flash { 30000.0 } else { 0.0 },
                shadows: false,
            }),
        );
    }
    let overview = p.resource::<Farm>().is_some_and(|f| f.overview);
    let day_zenith = mix3([0.16, 0.38, 0.82], [0.34, 0.37, 0.42], overcast);
    let day_horizon = mix3([0.62, 0.78, 0.92], [0.56, 0.58, 0.6], overcast);
    let mut zenith = mix3([0.01, 0.016, 0.055], day_zenith, day);
    let mut horizon = mix3([0.04, 0.05, 0.12], day_horizon, day);
    horizon = mix3(horizon, [0.98, 0.52, 0.3], dusk * 0.6 * (1.0 - overcast));
    if flash {
        zenith = [0.7, 0.75, 0.9];
        horizon = [0.8, 0.85, 1.0];
    }
    // From the overview the fog would hide the garden; thin it.
    let thin = if overview { 0.15 } else { 1.0 };
    let environment = Environment {
        background: None,
        zenith,
        horizon,
        ground: mix3([0.01, 0.012, 0.02], [0.14, 0.13, 0.08], day),
        ambient: 0.45 + 0.3 * day,
        sun_disc: if overcast > 0.0 { 0.0 } else { 0.006 },
        exposure: 0.92 + 0.5 * (1.0 - day),
        fog: Some(Fog {
            color: Some(horizon),
            ..Fog::new((0.0022 + 0.014 * overcast) * thin, 0.05)
        }),
        bloom: Some(Bloom {
            threshold: 0.9,
            intensity: 0.22,
            radius: 1.8,
        }),
    };
    if let Some(camera) = p.named("camera") {
        p.insert(
            camera,
            DrawnEnvironment {
                environment,
                ambient_occlusion: Some(AmbientOcclusion {
                    radius: 0.5,
                    intensity: 0.9,
                    ..AmbientOcclusion::default()
                }),
            },
        );
    }
}

/// This look's camera: lower and closer than the classic one, or at the
/// farmer's shoulder, drawn from the classic camera's eased follow (the point
/// it follows is its pose less its offset). The overview stays the classic one.
fn view(p: &mut Present) -> Option<Vec3> {
    let camera = p.named("camera")?;
    if p.resource::<Farm>().is_none_or(|f| f.overview) {
        return None;
    }
    let at = p.get::<Transform>(camera).map(|t| t.position)?;
    let followed = at - Vec3::new(0.0, 9.0, 11.0);
    let closeup = p.resource::<Props>().is_some_and(|s| s.closeup);
    let (offset, look) = if closeup {
        (Vec3::new(2.3, 2.0, 3.1), Vec3::new(1.3, -0.45, -1.8))
    } else {
        (Vec3::new(0.0, 6.5, 8.5), Vec3::new(0.0, 0.6, 0.0))
    };
    let eye = followed + offset;
    let pose = Transform::at(eye.x, eye.y, eye.z).looking_at(followed + look, Vec3::Y);
    place(p, camera, pose);
    Some(eye)
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

/// Everything this look draws: the sky and camera, its models on what every
/// look shares (ground, meadow, farmer, barrel), its props, each plant's
/// stage model where it stands and each fruit on its branch with its
/// mutation's look, lantern light and glow, the keeper's sway and wave, and
/// the harvested fruit in flight. `smooth` grows plants between stages.
pub fn present(p: &mut Present, smooth: bool) {
    let Some(offline) = p.resource::<garden::GardenClock>().map(|c| c.offline_ms) else {
        return;
    };
    let now = p.tick() * 1000 / p.hz() as u64 + offline;
    let seconds = (p.tick() % (p.hz() as u64 * 3600)) as f32 / p.hz() as f32;
    sky(p, now);
    let eye = view(p);
    if let Some(ground) = p.named("ground") {
        let plane = p.get::<Mesh>(ground).map(|m| m.clone()).unwrap_or_default();
        let soil = Material::grid([0.085, 0.05, 0.022], TILE);
        p.insert(ground, DrawnMesh::new(plane).material(soil));
    }
    if let Some(meadow) = p.named("meadow") {
        let grass = paint([0.3, 0.52, 0.17]);
        p.insert(
            meadow,
            DrawnMesh::new(Mesh::plane(1600.0, 1600.0)).material(grass),
        );
        place(p, meadow, Transform::at(0.0, -0.14, 0.0));
    }
    for (name, model) in [
        ("gardener", "farmer-body.model"),
        ("arm-left", "farmer-arm.model"),
        ("arm-right", "farmer-arm.model"),
        ("leg-left", "farmer-leg.model"),
        ("leg-right", "farmer-leg.model"),
        ("watering-can", "can.model"),
        ("turf", "meadow.model"),
        ("stall", "stall.model"),
        ("keeper", "keeper-body.model"),
        ("keeper-arm-left", "keeper-arm.model"),
        ("keeper-arm-right", "keeper-arm.model"),
        ("keeper-leg-left", "keeper-leg.model"),
        ("keeper-leg-right", "keeper-leg.model"),
        ("path-0", "path-0.model"),
        ("path-1", "path-1.model"),
        ("path-2", "path-2.model"),
        ("path-3", "path-0.model"),
    ] {
        dress(p, name, DrawnMesh::model(model));
    }
    dress(
        p,
        "water-barrel",
        DrawnMesh::model("barrel.model").material(Material::default()),
    );
    if let Some(water) = p.named("barrel-water") {
        let still = Material {
            color: [0.05, 0.18, 0.24, 1.0],
            roughness: 0.05,
            ..Material::default()
        };
        p.insert(
            water,
            DrawnMesh::new(Mesh::cylinder(0.55, 0.04)).material(still),
        );
        place(p, water, Transform::at(BARREL.x, 0.9, BARREL.z));
    }
    for (i, tree) in TREES.iter().enumerate() {
        dress(
            p,
            &format!("tree-{i}"),
            DrawnMesh::model(tree.0).lod(lod(tree.0)),
        );
    }
    fence(p, now);
    crops(p, now, seconds, smooth);
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
    fade_occluders(p, eye);
    // The fruit flying into the satchel keeps its model and mutation look.
    let picked = p
        .resource::<Feedback>()
        .is_some_and(|f| f.cue == Cue::Harvest);
    if picked {
        let last = p
            .resource::<Farm>()
            .and_then(|farm| farm.bag.last().map(|i| (i.kind, i.muts)));
        if let (Some((kind, muts)), Some(e)) = (last, p.named("picked-fruit")) {
            p.insert(e, fruit_model(kind, true));
            if let Some(look) = mutation(kind, muts, seconds, 0.0, 1.0) {
                p.insert(e, MaterialOverrides(vec![look]));
            }
        }
    }
    hide(p, "ambience-golden");
    hide(p, "ambience-storybook");
}

/// The fence, its lanterns and their lights, and the grass outside it. Kept
/// per prop: a prop's kind is fixed for its life (a grown garden's fence is
/// new entities), so a present redraws only the lanterns, which glow from
/// dusk to dawn.
fn fence(p: &mut Present, now: u64) {
    let glow = (1.0 - daylight(now) * 1.6).clamp(0.0, 1.0);
    // Which prop each fence entity is, read only when one is derived.
    let mut props: Option<std::collections::BTreeMap<Entity, Fencing>> = None;
    p.each::<Ambient>(|p, e| {
        let props = props.get_or_insert_with(|| {
            let fencing = p.resource::<Props>();
            let fencing = fencing.iter().flat_map(|s| s.fencing.iter());
            fencing.map(|f| (f.entity, *f)).collect()
        });
        let Some(f) = props.get(&e) else {
            return Derived::Kept;
        };
        let drawn = match f.prop {
            Prop::Post => DrawnMesh::model("fence-post.model"),
            Prop::Rail => DrawnMesh::model("fence-rail.model"),
            Prop::Lantern => {
                if glow > 0.0 {
                    p.insert(
                        e,
                        MaterialOverrides(vec![MaterialOverride {
                            material: 0,
                            color: None,
                            emissive: [3.2 * glow, 2.0 * glow, 0.8 * glow],
                        }]),
                    );
                }
                p.insert(e, DrawnMesh::model("lantern.model"));
                return Derived::Animated;
            }
            Prop::Lamp => {
                p.insert(
                    e,
                    DrawnLight::Point(PointLight {
                        color: [1.0, 0.7, 0.38],
                        intensity: LANTERN,
                        range: 10.0,
                    }),
                );
                return Derived::Kept;
            }
            Prop::Tuft => DrawnMesh::model(TUFTS[f.variant as usize % 4]),
            Prop::Grass => DrawnMesh::model(
                ["grass-0.model", "grass-1.model"][f.variant as usize % 2],
            )
            .lod(ModelLod {
                levels: Vec::new(),
                hide: Some(80.0),
            }),
        };
        p.insert(e, drawn);
        Derived::Kept
    });
}

/// Each plant's stage model standing on its tile, turned about its stem;
/// each fruit's model on its branch, with its mutation's look when ripe.
/// Derived per entity and kept: a present redraws the plants that grew and
/// the fruit that appeared or ripened, plus what moves with the time (a
/// growing plant under `smooth`, a gold, rainbow or shocked fruit's shimmer).
fn crops(p: &mut Present, now: u64, seconds: f32, smooth: bool) {
    // One model per crop and stage, cloned onto every plant of it.
    let mut models = std::collections::BTreeMap::new();
    p.each::<(Plant, Transform)>(|p, e| {
        let (kind, stage, tile, planted, grow_ms) = {
            let pl = p.require::<Plant>(e);
            (pl.kind, pl.stage, pl.tile, pl.planted, pl.grow_ms)
        };
        let model = models
            .entry((kind, stage))
            .or_insert_with(|| plant_model(kind, stage));
        p.insert(e, model.clone());
        let mut pose = plant_pose(tile);
        let growing = smooth && stage < 4;
        if growing {
            let f = ((now - planted.min(now)) as f32 / grow_ms.max(1) as f32).min(1.0);
            pose.scale = Vec3::splat(0.75 + 0.25 * f);
        }
        place(p, e, pose);
        if growing {
            Derived::Animated
        } else {
            Derived::Kept
        }
    });
    let mut models = std::collections::BTreeMap::new();
    // A fruit reads its plant's tile, which is fixed for the plant's life
    // (and the fruit's: a removed plant takes its fruit with it).
    p.each::<(Fruit, Transform)>(|p, e| {
        let (kind, slot, ripe, plant) = {
            let f = p.require::<Fruit>(e);
            (f.kind, f.slot, f.ripe, f.plant)
        };
        let model = models
            .entry((kind, ripe))
            .or_insert_with(|| fruit_model(kind, ripe));
        p.insert(e, model.clone());
        let tile = plant.and_then(|plant| p.get::<Plant>(plant).map(|pl| pl.tile));
        if let (Some(tile), Some(pose)) = (tile, p.get::<Transform>(e).map(|t| *t)) {
            let hung = Transform {
                position: fruit_position(tile, kind, slot),
                ..pose
            };
            place(p, e, hung);
        }
        Derived::Kept
    });
    // A ripe fruit's mutation, apart: only a shimmer is drawn again each present.
    p.each::<Fruit>(|p, e| {
        let (kind, ripe, muts) = {
            let f = p.require::<Fruit>(e);
            (f.kind, f.ripe, f.muts)
        };
        if !ripe || muts == 0 {
            return Derived::Kept;
        }
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
        if muts & (crops::RAINBOW | crops::GOLD | crops::SHOCKED) != 0 {
            Derived::Animated
        } else {
            Derived::Kept
        }
    });
}

/// Plants standing between a near camera and the farmer fade to a dither, with
/// their fruit, so the close-up never loses the player behind a tree. Visits
/// only the tiles under the sight line, never the garden.
/// `eye` is the drawn camera's position (none in the overview).
fn fade_occluders(p: &mut Present, eye: Option<Vec3>) {
    let (Some(eye), Some(player)) = (eye, p.named("player")) else {
        return;
    };
    let Some(target) = p.global(player) else {
        return;
    };
    let target = Vec3::from(target.translation);
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
