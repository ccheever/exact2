use super::*;
use exact_game_physics::BodyKind;
const LANTERNS: [(&str, Vec3); 12] = [
    ("lantern-1", Vec3::new(-12.0, 0.0, 10.0)),
    ("lantern-2", Vec3::new(-10.0, 0.0, 2.0)),
    ("lantern-3", Vec3::new(-12.0, 0.0, -8.0)),
    ("lantern-4", Vec3::new(-6.0, 0.0, -12.0)),
    ("lantern-5", Vec3::new(0.0, 0.0, -10.0)),
    ("lantern-6", Vec3::new(6.0, 0.0, -12.0)),
    ("lantern-7", Vec3::new(12.0, 0.0, -8.0)),
    ("lantern-8", Vec3::new(12.0, 0.0, 0.0)),
    ("lantern-9", Vec3::new(5.0, 0.0, 2.0)),
    ("lantern-10", Vec3::new(0.0, 0.0, 0.0)),
    ("lantern-11", Vec3::new(0.0, 0.0, 7.0)),
    ("lantern-12", Vec3::new(10.0, 2.4, 8.0)),
];

fn spawn_lantern(world: &mut World, name: &str, at: Vec3) {
    let root = world.spawn_named(name, (Transform::at(at.x, at.y, at.z), Lantern::default()));
    world.spawn_named(
        format!("{name}/post"),
        (
            Transform::at(0.0, 0.675, 0.0),
            Parent(root),
            Mesh::cylinder(0.06, 1.35),
            Material::rgb(0.12, 0.08, 0.06).metallic(0.35).rough(0.7),
        ),
    );
    world.spawn_named(
        format!("{name}/cap"),
        (
            Transform::at(0.0, 1.42, 0.0),
            Parent(root),
            Mesh::cylinder(0.24, 0.12),
            Material::rgb(0.12, 0.08, 0.06).metallic(0.35).rough(0.7),
        ),
    );
    world.spawn_named(
        format!("{name}/bulb"),
        (
            Transform::at(0.0, 1.16, 0.0),
            Parent(root),
            Mesh::sphere(0.19),
            Material::rgb(0.18, 0.16, 0.14),
            PointLight {
                color: [1.0, 0.42, 0.08],
                intensity: 0.0,
                range: 7.0,
            },
        ),
    );
}

fn legacy_scene(world: &mut World) {
    spawn_static_box(
        world,
        "ground",
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(36.0, 1.0, 36.0),
        Material::rgb(0.07, 0.18, 0.11).rough(0.92),
    );
    spawn_static_box(
        world,
        "ledge",
        Vec3::new(10.0, 1.2, 8.0),
        Vec3::new(4.0, 2.4, 4.0),
        Material::rgb(0.28, 0.27, 0.32).rough(0.85),
    );
    spawn_static_box(
        world,
        "wall",
        Vec3::new(-4.0, 1.0, 5.0),
        Vec3::new(1.0, 2.0, 5.0),
        Material::rgb(0.31, 0.29, 0.34).rough(0.88),
    );
    let crate_mesh = Mesh::cube(1.2);
    world.spawn_named(
        "crate",
        (
            Transform::at(6.0, 0.6, 8.0),
            crate_mesh.clone(),
            Material::rgb(0.45, 0.24, 0.09).rough(0.78),
            Collider::of(&crate_mesh),
            Body {
                kind: BodyKind::Dynamic,
                mass: 3.0,
                damping: 0.45,
                spin_damping: 8.0,
                ..Body::default()
            },
        ),
    );

    for (name, position) in LANTERNS {
        spawn_lantern(world, name, position);
    }
}

#[test]
fn authored_initial_world_matches_previous_rust_construction() {
    let baked = bake_scene();
    let types = scene_types();
    let mut authored = World::new(60, 1_041_003);
    types.register(&mut authored);
    types
        .prepare(&baked.content, Lanterns::assets())
        .unwrap()
        .instantiate(&mut authored)
        .unwrap();
    let mut old = World::new(60, 1_041_003);
    types.register(&mut old);
    legacy_scene(&mut old);
    old.insert_resource(exact_game_scene::SceneIdentity {
        digest: baked.digest,
        assets: [("Fox.glb".into(), exact_game_scene::digest_bytes(FOX_BYTES))].into(),
    });
    old.propagate();
    assert_eq!(
        authored.hash(),
        old.hash(),
        "same names, handles, parents and all typed component values"
    );
    assert_eq!(authored.len(), 52);
    for entity in authored.entities() {
        assert_eq!(authored.name(entity), old.name(entity));
        assert_eq!(authored.global(entity), old.global(entity));
    }
}
