//! The arena: a walled 44 m square with cover at three heights, a raised deck
//! reached by a ramp, and the spawn and cover points the bots read.
use exact_game::*;
use exact_game_physics::Collider;

/// Collision layer of everything static; fighters take one bit each above it.
pub const WORLD: u32 = 1;
/// Half the arena's side, inside the walls.
pub const HALF: f32 = 22.0;

struct Block {
    at: [f32; 3],
    size: [f32; 3],
    tilt: f32,
    color: [f32; 3],
}
const fn block(at: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Block {
    Block {
        at,
        size,
        tilt: 0.0,
        color,
    }
}
const WALL: [f32; 3] = [0.23, 0.26, 0.31];
const CRATE: [f32; 3] = [0.78, 0.52, 0.24];
const SLAB: [f32; 3] = [0.42, 0.47, 0.55];
const DECK: [f32; 3] = [0.30, 0.48, 0.62];

/// Positions are box centres; a crate 1 m tall can be jumped onto.
const BLOCKS: &[Block] = &[
    // Perimeter walls.
    block([0.0, 2.5, -HALF - 0.5], [46.0, 5.0, 1.0], WALL),
    block([0.0, 2.5, HALF + 0.5], [46.0, 5.0, 1.0], WALL),
    block([-HALF - 0.5, 2.5, 0.0], [1.0, 5.0, 44.0], WALL),
    block([HALF + 0.5, 2.5, 0.0], [1.0, 5.0, 44.0], WALL),
    // The centre: a 2.6 m block with a deck on top, reached by the ramp.
    block([0.0, 1.3, 0.0], [5.0, 2.6, 5.0], DECK),
    Block {
        at: [0.0, 1.3, 5.6],
        size: [3.0, 0.4, 6.8],
        tilt: 0.40,
        color: SLAB,
    },
    // Tall cover slabs that block sight lines across the middle.
    block([-9.0, 1.4, -4.0], [1.0, 2.8, 5.0], SLAB),
    block([9.0, 1.4, 4.0], [1.0, 2.8, 5.0], SLAB),
    block([-5.0, 1.4, 11.0], [5.0, 2.8, 1.0], SLAB),
    block([5.0, 1.4, -11.0], [5.0, 2.8, 1.0], SLAB),
    // Jumpable crates.
    block([-3.5, 0.5, 14.0], [1.6, 1.0, 1.6], CRATE),
    block([3.5, 0.5, -14.0], [1.6, 1.0, 1.6], CRATE),
    block([-14.0, 0.5, 3.0], [1.6, 1.0, 1.6], CRATE),
    block([14.0, 0.5, -3.0], [1.6, 1.0, 1.6], CRATE),
    block([-14.0, 0.9, -14.0], [2.4, 1.8, 2.4], CRATE),
    block([14.0, 0.9, 14.0], [2.4, 1.8, 2.4], CRATE),
    block([13.0, 0.5, 15.8], [1.2, 1.0, 1.2], CRATE),
    block([-13.0, 0.5, -15.8], [1.2, 1.0, 1.2], CRATE),
    // Low walls: crouch-height cover you can shoot over.
    block([0.0, 0.6, 9.5], [4.0, 1.2, 0.6], SLAB),
    block([0.0, 0.6, -9.5], [4.0, 1.2, 0.6], SLAB),
    block([-17.0, 0.6, 9.0], [0.6, 1.2, 4.0], SLAB),
    block([17.0, 0.6, -9.0], [0.6, 1.2, 4.0], SLAB),
];

/// Spawns face the middle; duels start at the first two. There is room for
/// every supported fighter without wrapping onto an occupied capsule.
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
    [-18.0, -12.0],
    [-18.0, -6.0],
    [-18.0, 6.0],
    [-18.0, 12.0],
    [18.0, -12.0],
    [18.0, -6.0],
    [18.0, 6.0],
    [18.0, 12.0],
    [-12.0, 18.0],
    [-6.0, 18.0],
    [6.0, 18.0],
    [12.0, 18.0],
    [-12.0, -18.0],
    [-6.0, -18.0],
    [6.0, -18.0],
    [12.0, -18.0],
];

/// Spots tucked against cover, where a hurt or reloading bot hides.
pub fn cover_points() -> Vec<Vec3> {
    let mut points = Vec::new();
    for b in &BLOCKS[4..] {
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

/// Spawn the floor, walls, cover, lights and fog.
pub fn build(w: &mut World) {
    w.insert_resource(Environment {
        background: Some([0.55, 0.70, 0.86]),
        fog: Some(Fog {
            color: Some([0.55, 0.70, 0.86]),
            ..Fog::new(0.004, 0.02)
        }),
        ..Environment::default()
    });
    let floor = Mesh::cuboid(Vec3::new(2.0 * HALF + 2.0, 1.0, 2.0 * HALF + 2.0));
    w.spawn_named(
        "floor",
        (
            Transform::at(0.0, -0.5, 0.0),
            Collider::of(&floor),
            floor,
            Material::grid([0.33, 0.36, 0.40], 2.0),
        ),
    );
    for (i, b) in BLOCKS.iter().enumerate() {
        let mesh = Mesh::cuboid(Vec3::from(b.size));
        let mut pose = Transform::at(b.at[0], b.at[1], b.at[2]);
        pose.rotation = Quat::from_rotation_x(b.tilt);
        let [r, g, bl] = b.color;
        w.spawn_named(
            format!("block-{i}"),
            (
                pose,
                Collider::of(&mesh),
                mesh,
                Material::rgb(r, g, bl).rough(0.8),
            ),
        );
    }
    w.spawn_named(
        "sun",
        (
            Transform::at(12.0, 30.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ),
    );
}
