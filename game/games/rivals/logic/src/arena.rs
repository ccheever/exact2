//! The arena: a walled 44 m square with cover at three heights, a raised deck
//! reached by a ramp, and the spawn and cover points the bots read.
use exact_game::*;
use exact_game_physics::Collider;

/// Collision layer of everything static; fighters take one bit each above it.
pub const WORLD: u32 = 1;
/// Half the arena's side, inside the walls.
pub const HALF: f32 = 22.0;

/// One block of `arena.json`, the layout the art generator dresses too.
#[derive(Clone, Debug, Default, Data)]
pub struct Block {
    pub kind: String,
    pub at: [f32; 3],
    pub size: [f32; 3],
    pub tilt: f32,
}
#[derive(Clone, Debug, Default, Data)]
pub struct Layout {
    pub half: f32,
    pub lights: Vec<[f32; 2]>,
    pub blocks: Vec<Block>,
}
/// The arena's collision layout: the same file `art-src/gen.mjs` builds the
/// arena model from, so what you see and what you hit cannot drift apart.
pub fn layout() -> Layout {
    exact_game::json::from_str(include_str!("../../arena.json")).expect("arena.json")
}

/// Spawns face the middle; duels use the first two.
pub const SPAWNS: &[[f32; 2]] = &[
    [0.0, 18.0],
    [0.0, -18.0],
    [-18.0, 0.0],
    [18.0, 0.0],
    [-18.0, 18.0],
    [18.0, -18.0],
    [18.0, 18.0],
    [-18.0, -18.0],
    [-8.0, 18.0],
    [8.0, -18.0],
];

/// Spots tucked against cover, where a hurt or reloading bot hides.
pub fn cover_points() -> Vec<Vec3> {
    let mut points = Vec::new();
    for b in &layout().blocks[4..] {
        if b.tilt != 0.0 || b.size[1] < 1.0 {
            continue;
        }
        let [x, _, z] = b.at;
        let (hx, hz) = (b.size[0] / 2.0 + 0.8, b.size[2] / 2.0 + 0.8);
        for (dx, dz) in [(hx, 0.0), (-hx, 0.0), (0.0, hz), (0.0, -hz)] {
            let (px, pz) = (x + dx, z + dz);
            if px.abs() < HALF - 1.0 && pz.abs() < HALF - 1.0 {
                points.push(Vec3::new(px, 0.9, pz));
            }
        }
    }
    points
}

/// The sky, sun, fog and post: a warm dusk, or a cold night lit by the pylons.
pub fn lighting(w: &mut World, night: bool) {
    let (zenith, horizon, ground) = if night {
        (
            [0.01, 0.015, 0.04],
            [0.03, 0.035, 0.06],
            [0.01, 0.01, 0.012],
        )
    } else {
        ([0.07, 0.13, 0.42], [0.95, 0.52, 0.32], [0.08, 0.06, 0.05])
    };
    w.insert_resource(Environment {
        background: None,
        zenith,
        horizon,
        ground,
        ambient: if night { 0.35 } else { 0.8 },
        sun_disc: if night { 0.012 } else { 0.01 },
        exposure: if night { 1.6 } else { 1.05 },
        fog: Some(Fog {
            color: Some(if night {
                [0.03, 0.04, 0.07]
            } else {
                [0.72, 0.55, 0.5]
            }),
            ..Fog::new(if night { 0.01 } else { 0.0028 }, 0.05)
        }),
        bloom: Some(Bloom {
            intensity: 0.22,
            ..Bloom::default()
        }),
    });
    w.insert_resource(EnvironmentMap {
        texture: if night { "sky_night.tex" } else { "sky.tex" }.into(),
        intensity: if night { 0.6 } else { 1.0 },
        rgbm: 8.0,
    });
    w.insert_resource(AmbientOcclusion {
        radius: 0.6,
        intensity: 0.9,
    });
    // A low sun from behind the red end (or the moon, blue and dim).
    let sun = if night {
        Vec3::new(-14.0, 22.0, -9.0)
    } else {
        Vec3::new(-24.0, 9.0, -34.0)
    };
    w.spawn_named(
        "sun",
        (
            Transform::at(sun.x, sun.y, sun.z).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: if night {
                    [0.55, 0.65, 1.0]
                } else {
                    [1.0, 0.72, 0.48]
                },
                illuminance: if night { 900.0 } else { 9000.0 },
                shadows: true,
            },
        ),
    );
    // Four pylon floodlights aimed at the deck; the warm diagonal pair casts
    // shadows (two of the eight shadow layers: each costs about a millisecond).
    for (i, [x, z]) in layout().lights.into_iter().enumerate() {
        let e = w.spawn_named(
            format!("flood-{i}"),
            (
                Transform::at(x * 0.97, 6.2, z * 0.97)
                    .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
                SpotLight {
                    color: if i % 3 == 0 {
                        [1.0, 0.82, 0.6]
                    } else {
                        [0.7, 0.85, 1.0]
                    },
                    intensity: if night { 2_500_000.0 } else { 600_000.0 },
                    range: 42.0,
                    inner: 0.28,
                    outer: 0.62,
                },
            ),
        );
        if i % 3 == 0 {
            w.insert(e, LightShadows);
        }
    }
}

/// Spawn the arena: one dressed model over invisible collision blocks.
pub fn build(w: &mut World, night: bool) {
    lighting(w, night);
    let arena = layout();
    let half = arena.half;
    let floor = Mesh::cuboid(Vec3::new(2.0 * half + 2.0, 1.0, 2.0 * half + 2.0));
    w.spawn_named(
        "floor",
        (Transform::at(0.0, -0.5, 0.0), Collider::of(&floor)),
    );
    for (i, b) in arena.blocks.iter().enumerate() {
        let mut pose = Transform::at(b.at[0], b.at[1], b.at[2]);
        pose.rotation = Quat::from_rotation_x(b.tilt);
        w.spawn_named(
            format!("block-{i}"),
            (pose, Collider::of(&Mesh::cuboid(Vec3::from(b.size)))),
        );
    }
    w.spawn_named("arena", (Transform::default(), Mesh::asset("arena.model")));
}
