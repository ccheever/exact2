//! The art pass (`art: "pass"`): baked glTF from `art.mjs` (a model per crop
//! and stage, a fruit shape per crop, fence and lanterns, a seed stall with a
//! keeper and a Contract sign, a jointed farmer), a ten-minute day and night
//! with a moon, a sky and fog for each weather, and rain and snow.
//!
//! The shared farmer rig keeps watering, planting and harvest gestures in
//! every look. World updates select meshes and lighting; presentation applies
//! the camera, crop poses, mutation colours and keeper animation.
use crate::art::each;
use crate::art::{
    apply, dress, hide, materials, offset, opacity, place_world as place, Appearance,
};
use crate::crops::{self, shown_balance, Balance};
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

/// The art pass's presentation data (`assets/looks.level.json`'s `pass`): the
/// day's sky and lights, the soil and meadow, the barrel's water, the lanterns,
/// the camera, the levels of detail and the mutations' looks.
#[derive(Clone, Default, Data)]
pub struct PassLook {
    pub sky: SkyLook,
    pub sun: SunLook,
    pub moon: MoonLook,
    pub soil: [f32; 3],
    pub meadow: [f32; 3],
    pub meadow_y: f32,
    pub water: crate::looks::Water,
    pub lantern: LanternLook,
    pub camera: CameraLook,
    pub lod: LodLook,
    pub mutations: MutationLooks,
}

/// The sky through the day: colours at noon, overcast, night, dusk and a
/// lightning flash; `overcast` by weather (clear, rain, snow, storm); ambient
/// and exposure as (base, by daylight) and (base, by night); fog density as
/// (base, by overcast).
#[derive(Clone, Default, Data)]
pub struct SkyLook {
    pub overcast: [f32; 4],
    pub day_zenith: [f32; 3],
    pub overcast_zenith: [f32; 3],
    pub day_horizon: [f32; 3],
    pub overcast_horizon: [f32; 3],
    pub night_zenith: [f32; 3],
    pub night_horizon: [f32; 3],
    pub dusk_horizon: [f32; 3],
    pub dusk: f32,
    pub night_ground: [f32; 3],
    pub day_ground: [f32; 3],
    pub flash_zenith: [f32; 3],
    pub flash_horizon: [f32; 3],
    pub ambient: [f32; 2],
    pub exposure: [f32; 2],
    pub sun_disc: f32,
    pub fog: [f32; 2],
    pub fog_falloff: f32,
    /// The fog's density from the overview, as a fraction.
    pub overview_fog: f32,
    pub bloom: Bloom,
    pub occlusion: AmbientOcclusion,
}

#[derive(Clone, Default, Data)]
pub struct SunLook {
    pub day: [f32; 3],
    pub dusk: [f32; 3],
    pub illuminance: f32,
    /// How much an overcast sky dims it.
    pub overcast: f32,
}

#[derive(Clone, Default, Data)]
pub struct MoonLook {
    pub color: [f32; 3],
    pub illuminance: f32,
    pub overcast: f32,
    /// Lux added by a lightning flash.
    pub flash: f32,
}

/// A lantern's glass glow, how soon after dusk it lights, and its light.
#[derive(Clone, Default, Data)]
pub struct LanternLook {
    pub glow: [f32; 3],
    pub dusk: f32,
    pub color: [f32; 3],
    pub candela: f32,
    pub range: f32,
}

/// The camera, as an offset from the followed point and where it looks.
#[derive(Clone, Copy, Default, Data)]
pub struct Shot {
    pub offset: [f32; 3],
    pub look: [f32; 3],
}

#[derive(Clone, Default, Data)]
pub struct CameraLook {
    pub near: Shot,
    pub closeup: Shot,
}

/// Plants and fruit switch to their far models from here (metres); grass
/// patches disappear past `grass_hide`.
#[derive(Clone, Default, Data)]
pub struct LodLook {
    pub far: f32,
    pub fruit_far: f32,
    pub grass_hide: f32,
}

#[derive(Clone, Default, Data)]
pub struct MutationLooks {
    pub rainbow: RainbowLook,
    pub gold: GlowLook,
    pub shocked: GlowLook,
    pub frozen: GlowLook,
    pub wet: WetLook,
    pub chilled: ChilledLook,
}

#[derive(Clone, Default, Data)]
pub struct RainbowLook {
    pub saturation: f32,
    pub glow: f32,
    /// Hue cycles a second.
    pub rate: f32,
}

/// A colour and its glow; Gold's glint and Shocked's flicker as (low, high).
#[derive(Clone, Default, Data)]
pub struct GlowLook {
    pub color: [f32; 3],
    pub emissive: [f32; 3],
    pub roughness: f32,
    pub glint: [f32; 2],
    pub flicker: [f32; 2],
}

#[derive(Clone, Default, Data)]
pub struct WetLook {
    pub scale: f32,
    pub lift: f32,
}

#[derive(Clone, Default, Data)]
pub struct ChilledLook {
    pub color: [f32; 3],
    pub mix: f32,
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
            concat!("fruit-", $c, "-gold.model"), concat!("fruit-", $c, "-gold-far.model"),
            concat!("golden-fruit-", $c, "-gold.model"), concat!("storybook-fruit-", $c, "-gold.model"),
            concat!("fruit-", $c, "-unripe.model"), concat!("fruit-", $c, "-unripe-far.model"),
            concat!("golden-plant-", $c, ".model"), concat!("golden-fruit-", $c, ".model"),
            concat!("storybook-plant-", $c, ".model"), concat!("storybook-fruit-", $c, ".model"),
        )*
        "fence-post.model", "fence-rail.model", "lantern.model", "path-0.model",
        "path-1.model", "path-2.model", "tuft-0.model", "tuft-1.model", "tuft-2.model",
        "tuft-3.model", "stall.model", "barrel.model", "can.model", "farmer-body.model",
        "farmer-arm.model", "farmer-leg.model", "keeper-body.model", "keeper-arm.model",
        "keeper-leg.model", "meadow.model", "grass-0.model", "grass-1.model",
        "golden-gardener.model", "golden-gardener-arm.model", "golden-gardener-leg.model",
        "golden-watering-can.model", "golden-orchard.model", "golden-flowers.model",
        "golden-meadow-grass.model", "golden-backdrop.model", "golden-rail.model",
        "golden-rain-barrel.model", "storybook-gardener.model", "storybook-gardener-arm.model",
        "storybook-gardener-leg.model", "storybook-watering-can.model", "storybook-orchard.model",
        "storybook-flowers.model", "storybook-meadow-grass.model", "storybook-backdrop.model",
        "storybook-rail.model", "storybook-rain-barrel.model"]
    };
}

/// Every baked model, streamed without blocking the first frame. Models stay
/// resident so ripening fruit and look switches reuse the same assets.
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

fn lod(l: &LodLook, near: &str) -> ModelLod {
    let far = near.replace(".model", "-far.model");
    ModelLod {
        levels: vec![LodLevel {
            distance: if near.starts_with("fruit") {
                l.fruit_far
            } else {
                l.far
            },
            model: far,
        }],
        hide: None,
    }
}

/// A plant's model at a growth stage, with its far level.
fn plant_model(b: &Balance, l: &LodLook, kind: u8, stage: u8) -> Appearance {
    let name = format!("plant-{}-{stage}.model", b.crop(kind).id);
    Appearance::model(name.clone())
        .lod(lod(l, &name))
        .material(Material::default())
}

/// A fruit's model, ripe or not, with its far level, in its own colours.
fn fruit_model(b: &Balance, l: &LodLook, kind: u8, ripe: bool, gold: bool) -> Appearance {
    let name = format!(
        "fruit-{}{}.model",
        b.crop(kind).id,
        if !ripe {
            "-unripe"
        } else if gold {
            "-gold"
        } else {
            ""
        }
    );
    Appearance::model(name.clone())
        .lod(lod(l, &name))
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
fn fruit_offset(c: &crops::Crop, slot: u8) -> Vec3 {
    const Y0: f32 = 0.08;
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
    match c.id.as_str() {
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

fn fruit_position(c: &crops::Crop, tile: [u16; 2], slot: u8) -> Vec3 {
    garden::plant_center(tile) + yaw(tile) * fruit_offset(c, slot)
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
    w.derived_publication("day")
        .derived_publication("minutes")
        .derived_publication("night");
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

pub fn toggle_closeup(w: &World) -> crate::hud::Note {
    let c = !w.resource::<Props>().closeup;
    w.resource_mut::<Props>().closeup = c;
    crate::hud::Note::new(if c { "closeup" } else { "garden_view" })
}

// ------------------------------------------------------------- day, night

/// Garden milliseconds in one day.
pub const DAY_MS: u64 = 600_000;

/// The time of day in [0, 1): 0 is sunrise, 0.25 noon, 0.5 sunset. A new
/// garden starts in the morning.
pub fn day_phase(now: u64) -> f32 {
    ((now + DAY_MS / 12) % DAY_MS) as f32 / DAY_MS as f32
}

/// The day (from 1) and its minute, to ten garden minutes: Day 2 · 14:30.
pub fn clock(now: u64) -> Clock {
    let day = (now + DAY_MS / 12) / DAY_MS + 1;
    let minutes = ((day_phase(now) * 144.0) as u32 * 10 + 6 * 60) % (24 * 60);
    Clock {
        day,
        minutes,
        night: daylight(now) < 0.35,
    }
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

/// Advance weather particles and publish the garden clock in every look.
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
    let clock = clock(now);
    let shown = |key: &str| w.published(key).and_then(|v| v.as_number());
    if shown("day") != Some(clock.day as f64) || shown("minutes") != Some(clock.minutes as f64) {
        w.publish_record(&clock);
    }
}

/// The HUD's clock pill.
#[derive(Clone, Default, Data)]
pub struct Clock {
    pub day: u64,
    pub minutes: u32,
    pub night: bool,
}

/// The sun and moon, sky, fog and exposure for the time and weather, and
/// lightning in a storm: drawn on the simulation's sun, its spare light and
/// the camera.
fn sky(p: &mut World, l: &PassLook, now: u64) {
    let s = &l.sky;
    let angle = day_phase(now) * TAU;
    let elevation = math::sin(angle);
    let sun_dir = Vec3::new(math::cos(angle), elevation.max(-0.3), -0.45).normalize();
    let day = daylight(now);
    let dusk = (1.0 - (elevation.abs() * 3.0).min(1.0)) * if elevation > -0.25 { 1.0 } else { 0.0 };
    let sky = p.try_resource::<Weather>().map_or(Sky::Clear, |w| w.sky);
    let overcast = s.overcast[sky as usize];
    // Lightning: a storm flashes for a few ticks every seven seconds.
    let flash = sky == Sky::Storm && (now / 100) % 70 < 2;
    let aim = |at: Vec3| Transform::at(at.x, at.y, at.z).looking_at(Vec3::ZERO, Vec3::Y);
    if let Some(sun) = p.named("sun") {
        place(p, sun, aim(sun_dir * 30.0));
        p.insert(
            sun,
            DirectionalLight {
                color: mix3(l.sun.day, l.sun.dusk, dusk),
                illuminance: l.sun.illuminance * day * (1.0 - l.sun.overcast * overcast),
                shadows: true,
            },
        );
    }
    if let Some(moon) = p.named("moon") {
        let moon_dir = Vec3::new(-sun_dir.x, (-elevation).max(0.25), -0.5).normalize();
        place(p, moon, aim(moon_dir * 30.0));
        p.insert(
            moon,
            DirectionalLight {
                color: l.moon.color,
                illuminance: l.moon.illuminance * (1.0 - day) * (1.0 - l.moon.overcast * overcast)
                    + if flash { l.moon.flash } else { 0.0 },
                shadows: false,
            },
        );
    }
    let overview = p.try_resource::<Farm>().is_some_and(|f| f.overview);
    let day_zenith = mix3(s.day_zenith, s.overcast_zenith, overcast);
    let day_horizon = mix3(s.day_horizon, s.overcast_horizon, overcast);
    let mut zenith = mix3(s.night_zenith, day_zenith, day);
    let mut horizon = mix3(s.night_horizon, day_horizon, day);
    horizon = mix3(horizon, s.dusk_horizon, dusk * s.dusk * (1.0 - overcast));
    if flash {
        zenith = s.flash_zenith;
        horizon = s.flash_horizon;
    }
    // From the overview the fog would hide the garden; thin it.
    let thin = if overview { s.overview_fog } else { 1.0 };
    let environment = Environment {
        background: None,
        zenith,
        horizon,
        ground: mix3(s.night_ground, s.day_ground, day),
        ambient: s.ambient[0] + s.ambient[1] * day,
        sun_disc: if overcast > 0.0 { 0.0 } else { s.sun_disc },
        exposure: s.exposure[0] + s.exposure[1] * (1.0 - day),
        fog: Some(Fog {
            color: Some(horizon.into()),
            ..Fog::new((s.fog[0] + s.fog[1] * overcast) * thin, s.fog_falloff)
        }),
        bloom: Some(s.bloom),
        ..Environment::default()
    };
    p.insert_resource(environment);
    p.insert_resource(s.occlusion);
}

/// This look's camera: lower and closer than the classic one, or at the
/// farmer's shoulder, drawn from the classic camera's eased follow (the point
/// it follows is its pose less its offset). The overview stays the classic one.
fn view(p: &mut World, b: &Balance, l: &CameraLook) -> Option<Vec3> {
    let camera = p.named("camera")?;
    if p.try_resource::<Farm>().is_none_or(|f| f.overview) {
        return None;
    }
    let at = p.get::<Transform>(camera).map(|t| t.position)?;
    let followed = at - Vec3::from(b.camera.offset);
    let closeup = p.try_resource::<Props>().is_some_and(|s| s.closeup);
    let shot = if closeup { l.closeup } else { l.near };
    let eye = followed + Vec3::from(shot.offset);
    let pose =
        Transform::at(eye.x, eye.y, eye.z).looking_at(followed + Vec3::from(shot.look), Vec3::Y);
    place(p, camera, pose);
    Some(eye)
}

// ----------------------------------------------------------- presentation

pub(crate) fn is_gold(muts: u8) -> bool {
    muts & (crops::RAINBOW | crops::GOLD) == crops::GOLD
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [f(5.0), f(3.0), f(1.0)].map(|c| c * c)
}

/// The strongest mutation sets the fruit's colour and emission. The saved
/// garden clock drives rainbow hue, gold glints and storm flicker.
fn mutation(
    l: &PassLook,
    base: [f32; 3],
    muts: u8,
    phase: f32,
    hz: f32,
) -> Option<MaterialOverride> {
    let m = &l.mutations;
    let base = base.map(|c| c * c);
    let (color, emissive) = if muts & crops::RAINBOW != 0 {
        let c = hsv((phase * m.rainbow.rate).fract(), m.rainbow.saturation, 1.0);
        (c, c.map(|v| v * m.rainbow.glow))
    } else if muts & crops::GOLD != 0 {
        let glow = m.gold.glint[0]
            + (m.gold.glint[1] - m.gold.glint[0]) * (0.5 + 0.5 * math::sin(phase * 3.0));
        (m.gold.color, m.gold.emissive.map(|v| v * glow))
    } else if muts & crops::SHOCKED != 0 {
        let glow = if (phase * hz) as u64 % 2 == 0 {
            m.shocked.flicker[0]
        } else {
            m.shocked.flicker[1]
        };
        (m.shocked.color, m.shocked.emissive.map(|v| v * glow))
    } else if muts & crops::FROZEN != 0 {
        (m.frozen.color, m.frozen.emissive)
    } else if muts & crops::WET != 0 {
        (base.map(|c| c * m.wet.scale + m.wet.lift), [0.0; 3])
    } else if muts & crops::CHILLED != 0 {
        (mix3(base, m.chilled.color, m.chilled.mix), [0.0; 3])
    } else {
        return None;
    };
    Some(MaterialOverride {
        material: 0,
        color: Some([color[0], color[1], color[2], 1.0]),
        emissive,
    })
}

/// Gold colour and emission for the fruit skin; `art::update` sets its metal.
pub(crate) fn gilded(l: &PassLook) -> MaterialOverride {
    MaterialOverride {
        material: 0,
        color: Some([
            l.mutations.gold.color[0],
            l.mutations.gold.color[1],
            l.mutations.gold.color[2],
            1.0,
        ]),
        emissive: l.mutations.gold.emissive,
        ..MaterialOverride::default()
    }
}

/// Everything this look draws: the sky and camera, its models on what every
/// look shares (ground, meadow, farmer, barrel), its props, each plant's
/// stage model where it stands and each fruit on its branch with its
/// mutation's look, lantern light and glow, the keeper's sway and wave, and
/// the harvested fruit in flight. `smooth` grows plants between stages.
pub fn update(p: &mut World, smooth: bool) {
    let Some(offline) = p
        .try_resource::<garden::GardenClock>()
        .map(|c| c.offline_ms)
    else {
        return;
    };
    let now = p.tick() * 1000 / p.hz() as u64 + offline;
    let seconds = (p.tick() % (p.hz() as u64 * 3600)) as f32 / p.hz() as f32;
    let looks = crate::looks::looks(p);
    let l = &looks.pass;
    let b = shown_balance(p);
    sky(p, l, now);
    let eye = view(p, &b, &l.camera);
    if let Some(ground) = p.named("ground") {
        let plane = p.get::<Mesh>(ground).map(|m| m.clone()).unwrap_or_default();
        let soil = Material::grid(l.soil, TILE);
        apply(p, ground, Appearance::new(plane).material(soil));
    }
    if let Some(meadow) = p.named("meadow") {
        let grass = paint(l.meadow);
        apply(
            p,
            meadow,
            Appearance::new(Mesh::plane(1600.0, 1600.0)).material(grass),
        );
        place(p, meadow, Transform::at(0.0, l.meadow_y, 0.0));
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
        dress(p, name, Appearance::model(model));
    }
    dress(
        p,
        "water-barrel",
        Appearance::model("barrel.model").material(Material::default()),
    );
    if let Some(water) = p.named("barrel-water") {
        let [r, g, bl] = l.water.color;
        let still = Material {
            color: [r, g, bl, 1.0],
            roughness: l.water.roughness,
            ..Material::default()
        };
        apply(
            p,
            water,
            Appearance::new(Mesh::cylinder(0.55, 0.04)).material(still),
        );
        place(p, water, Transform::at(BARREL.x, 0.9, BARREL.z));
    }
    for (i, tree) in TREES.iter().enumerate() {
        dress(
            p,
            &format!("tree-{i}"),
            Appearance::model(tree.0).lod(lod(&l.lod, tree.0)),
        );
    }
    fence(p, l, now);
    crops(p, &b, l, now, smooth);
    // The keeper sways, nods and waves every nine seconds.
    let idle = math::sin(seconds * 1.4);
    if let Some(keeper) = p.named("keeper") {
        offset(
            p,
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
        offset(
            p,
            arm,
            Offset(Transform {
                rotation: raise * flap,
                ..Transform::default()
            }),
        );
    }
    if let Some(arm) = p.named("keeper-arm-left") {
        offset(
            p,
            arm,
            Offset(Transform {
                rotation: Quat::from_rotation_x(-0.25 + idle * 0.05),
                ..Transform::default()
            }),
        );
    }
    fade_occluders(p, &b, eye);
    // The fruit flying into the satchel keeps its model and mutation look.
    let picked = p
        .try_resource::<Feedback>()
        .is_some_and(|f| f.cue == Cue::Harvest);
    if picked {
        let last = p
            .try_resource::<Farm>()
            .and_then(|farm| farm.bag.last().map(|i| (i.kind, i.muts)));
        if let (Some((kind, muts)), Some(e)) = (last, p.named("picked-fruit")) {
            apply(p, e, fruit_model(&b, &l.lod, kind, true, is_gold(muts)));
            if let Some(look) = mutation(l, b.crop(kind).fruit, muts, 0.0, p.hz() as f32) {
                materials(p, e, MaterialOverrides(vec![look]));
            }
        }
    }
    hide(p, "ambience-golden");
    hide(p, "ambience-storybook");
}

/// Fence models and lantern emission for the current time of day.
fn fence(p: &mut World, l: &PassLook, now: u64) {
    let lantern = &l.lantern;
    let glow = (1.0 - daylight(now) * lantern.dusk).clamp(0.0, 1.0);
    // Match the shared ambient entities to this look's fence props.
    let mut props: Option<std::collections::BTreeMap<Entity, Fencing>> = None;
    each::<Ambient>(p, |p, e| {
        let props = props.get_or_insert_with(|| {
            let fencing = p.try_resource::<Props>();
            let fencing = fencing.iter().flat_map(|s| s.fencing.iter());
            fencing.map(|f| (f.entity, *f)).collect()
        });
        let Some(f) = props.get(&e) else {
            return;
        };
        let drawn = match f.prop {
            Prop::Post => Appearance::model("fence-post.model"),
            Prop::Rail => Appearance::model("fence-rail.model"),
            Prop::Lantern => {
                if glow > 0.0 {
                    materials(
                        p,
                        e,
                        MaterialOverrides(vec![MaterialOverride {
                            material: 0,
                            color: None,
                            emissive: lantern.glow.map(|c| c * glow),
                            ..MaterialOverride::default()
                        }]),
                    );
                }
                apply(p, e, Appearance::model("lantern.model"));
                return;
            }
            Prop::Lamp => {
                p.insert(
                    e,
                    PointLight {
                        color: lantern.color,
                        intensity: lantern.candela,
                        range: lantern.range,
                    },
                );
                return;
            }
            Prop::Tuft => Appearance::model(TUFTS[f.variant as usize % 4]),
            Prop::Grass => Appearance::model(
                ["grass-0.model", "grass-1.model"][f.variant as usize % 2],
            )
            .lod(ModelLod {
                levels: Vec::new(),
                hide: Some(l.lod.grass_hide),
            }),
        };
        apply(p, e, drawn);
    });
}

/// Each plant's stage model standing on its tile, turned about its stem;
/// each fruit's model on its branch, with its mutation's look when ripe.
fn crops(p: &mut World, b: &Balance, l: &PassLook, now: u64, smooth: bool) {
    // One model per crop and stage, cloned onto every plant of it.
    let mut models = std::collections::BTreeMap::new();
    each::<Plant>(p, |p, e| {
        let (kind, stage, tile, planted, grow_ms) = {
            let pl = p.require::<Plant>(e);
            (pl.kind, pl.stage, pl.tile, pl.planted, pl.grow_ms)
        };
        let model = models
            .entry((kind, stage))
            .or_insert_with(|| plant_model(b, &l.lod, kind, stage));
        apply(p, e, model.clone());
        let mut pose = plant_pose(tile);
        let growing = smooth && stage < 4;
        if growing {
            let f = ((now - planted.min(now)) as f32 / grow_ms.max(1) as f32).min(1.0);
            pose.scale = Vec3::splat(0.75 + 0.25 * f);
        }
        place(p, e, pose);
    });
    let mut models = std::collections::BTreeMap::new();
    // A fruit reads its plant's tile, which is fixed for the plant's life
    // (and the fruit's: a removed plant takes its fruit with it).
    each::<Fruit>(p, |p, e| {
        let (kind, slot, ripe, plant, gold) = {
            let f = p.require::<Fruit>(e);
            (f.kind, f.slot, f.ripe, f.plant, f.ripe && is_gold(f.muts))
        };
        let model = models
            .entry((kind, ripe, gold))
            .or_insert_with(|| fruit_model(b, &l.lod, kind, ripe, gold));
        apply(p, e, model.clone());
        let tile = plant.and_then(|plant| p.get::<Plant>(plant).map(|pl| pl.tile));
        if let (Some(tile), Some(pose)) = (tile, p.get::<Transform>(e).map(|t| *t)) {
            let hung = Transform {
                position: fruit_position(b.crop(kind), tile, slot),
                ..pose
            };
            place(p, e, hung);
        }
    });
    // Mutation animation uses the same saved clock as crop growth.
    let hz = p.hz() as f32;
    each::<Fruit>(p, |p, e| {
        let (kind, ripe, muts) = {
            let f = p.require::<Fruit>(e);
            (f.kind, f.ripe, f.muts)
        };
        if !ripe || muts == 0 {
            return;
        }
        let phase = p.seconds() as f32 + (e.index() % 97) as f32 / 97.0;
        if let Some(look) = mutation(l, b.crop(kind).fruit, muts, phase, hz) {
            materials(p, e, MaterialOverrides(vec![look]));
        }
    });
}

/// Plants standing between a near camera and the farmer fade to a dither, with
/// their fruit, so the close-up never loses the player behind a tree. Visits
/// only the tiles under the sight line, never the garden.
/// `eye` is the drawn camera's position (none in the overview).
fn fade_occluders(p: &mut World, b: &Balance, eye: Option<Vec3>) {
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
    if let Some(farm) = p.try_resource::<Farm>() {
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
                    b.crop(pl.kind).height * (0.3 + 0.7 * pl.stage as f32 / 4.0) > 0.7
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
        opacity(p, plant, 0.3);
        for fruit in fruits {
            opacity(p, fruit, 0.3);
        }
    }
}
