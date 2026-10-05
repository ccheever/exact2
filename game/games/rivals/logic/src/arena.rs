//! The arena: a walled square with cover at three heights, a raised deck
//! reached by a ramp, and the spawn and cover points the bots read. Its layout
//! is the tables' (`rivals.level.json`); this builds the world from it.
use crate::tables::{self, Arena};
use exact_game::*;
use exact_game_physics::Collider;

/// Collision layer of everything static; fighters take one bit each above it.
pub const WORLD: u32 = 1;

/// The classic look's colour for a block kind.
fn color(kind: &str) -> [f32; 3] {
    match kind {
        "wall" => [0.23, 0.26, 0.31],
        "crate" => [0.78, 0.52, 0.24],
        "deck" => [0.30, 0.48, 0.62],
        _ => [0.42, 0.47, 0.55],
    }
}

/// Spots tucked against cover, where a hurt or reloading bot hides.
pub fn cover_points(arena: &Arena) -> Vec<Vec3> {
    let half = arena.half;
    let mut points = Vec::new();
    for b in &arena.blocks[4..] {
        if b.tilt != 0.0 || b.size[1] < 1.0 {
            continue;
        }
        let [x, _, z] = b.at;
        let (hx, hz) = (b.size[0] / 2.0 + 0.8, b.size[2] / 2.0 + 0.8);
        for (dx, dz) in [(hx, 0.0), (-hx, 0.0), (0.0, hz), (0.0, -hz)] {
            let (px, pz) = (x + dx, z + dz);
            if px.abs() < half - 1.0 && pz.abs() < half - 1.0 {
                points.push(Vec3::new(px, 0.9, pz));
            }
        }
    }
    points
}

/// Spawn the floor, walls, cover, lights and fog. A `dressed` arena (the art
/// pass) keeps exactly these colliders but draws none of the boxes: one model
/// built from the same blocks is drawn over them instead.
pub fn build(w: &mut World, dressed: bool) {
    w.insert_resource(Environment {
        background: Some([0.55, 0.70, 0.86]),
        fog: Some(Fog {
            color: Some([0.55, 0.70, 0.86]),
            ..Fog::new(0.004, 0.02)
        }),
        ..Environment::default()
    });
    blocks(w, dressed);
    w.spawn_named(
        "sun",
        (
            Transform::at(12.0, 30.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ),
    );
}

/// The floor and blocks: what a layout edit replaces in a running match.
fn blocks(w: &mut World, dressed: bool) {
    let arena = tables::of(w).arena.clone();
    let half = arena.half;
    let floor = Mesh::cuboid(Vec3::new(2.0 * half + 2.0, 1.0, 2.0 * half + 2.0));
    let pose = Transform::at(0.0, -0.5, 0.0);
    if dressed {
        w.spawn_named("floor", (pose, Collider::of(&floor)));
    } else {
        let grid = Material::grid([0.33, 0.36, 0.40], 2.0);
        w.spawn_named("floor", (pose, Collider::of(&floor), floor, grid));
    }
    for (i, b) in arena.blocks.iter().enumerate() {
        let mesh = Mesh::cuboid(Vec3::from(b.size));
        let mut pose = Transform::at(b.at[0], b.at[1], b.at[2]);
        pose.rotation = Quat::from_rotation_x(b.tilt);
        let [r, g, bl] = color(&b.kind);
        let name = format!("block-{i}");
        if dressed {
            w.spawn_named(name, (pose, Collider::of(&mesh)));
        } else {
            let material = Material::rgb(r, g, bl).rough(0.8);
            w.spawn_named(name, (pose, Collider::of(&mesh), mesh, material));
        }
    }
}

/// A development reload replaced the tables: rebuild the floor and blocks in
/// place, as the new layout says. Fighters keep where they stand; one a new
/// block now overlaps is pushed out by its capsule. The art pass's arena model
/// is the generator's (`art-src/gen.mjs`): rerun it to redress the new layout.
pub fn rebuild(w: &mut World, dressed: bool) {
    let old: Vec<Entity> = std::iter::once("floor".to_owned())
        .chain((0..).map(|i| format!("block-{i}")))
        .map_while(|name| w.named(&name))
        .collect();
    for e in old {
        w.despawn(e);
    }
    blocks(w, dressed);
}
