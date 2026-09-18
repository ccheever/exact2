use super::*;
use exact_game::{bin, Sim};
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

#[test]
fn later_content_is_for_restart_and_never_reapplied_to_a_saved_world() {
    let baked = bake_scene();
    let mut running = Sim::<Lanterns>::new(Options {
        scene: baked.content.clone(),
        seed: 7,
        started: true,
        ..Default::default()
    })
    .unwrap();
    running.run(100.0);
    // Seed nondefault runtime state to test save semantics independently of input delivery.
    running
        .world_mut()
        .get_mut::<Transform>("player")
        .unwrap()
        .position = Vec3::new(-12.0, 0.65, 10.0);
    light_nearest(running.world_mut(), false);
    running
        .world_mut()
        .get_mut::<Transform>("crate")
        .unwrap()
        .position
        .x = 6.8;
    let before = running.world().hash();
    let save = running.save();

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json");
    let temp = path.with_file_name(format!("scene-test-{}.json", std::process::id()));
    // This scratch source is beside the original only to preserve relative fragment/asset paths.
    let original = std::fs::read_to_string(path).unwrap();
    let modified = original.replace("-12,", "-11,");
    std::fs::write(&temp, modified).unwrap();
    let replacement = exact_game_scene::bake::compile(&temp, &scene_types(), Lanterns::assets());
    std::fs::remove_file(temp).unwrap();
    let replacement = replacement.unwrap();
    assert_ne!(replacement.digest, baked.digest);
    let mut fresh = Sim::<Lanterns>::new(Options {
        scene: replacement.content,
        seed: 7,
        started: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        fresh
            .world()
            .get::<Transform>("lantern-1")
            .unwrap()
            .position
            .x,
        -11.0
    );
    fresh.restore_bound(&save).unwrap();
    assert_eq!(fresh.world().hash(), before);
    assert_eq!(fresh.world().resource::<Session>().elapsed, 6);
    assert!(fresh.world().get::<Lantern>("lantern-1").unwrap().lit);
    assert_eq!(
        fresh.world().get::<Transform>("crate").unwrap().position.x,
        6.8
    );
    assert_eq!(
        fresh
            .world()
            .get::<Transform>("lantern-1")
            .unwrap()
            .position
            .x,
        -12.0
    );
    assert_eq!(
        fresh
            .world()
            .resource::<exact_game_scene::SceneIdentity>()
            .digest,
        baked.digest
    );
    // The ordinary portable save retains its original initial-condition argument too.
    let mut restored = Sim::<Lanterns>::new(Options {
        scene: baked.content,
        ..Default::default()
    })
    .unwrap();
    restored.restore(&save).unwrap();
    assert_eq!(restored.world().hash(), before);
    assert_eq!(
        bin::to_vec(&*restored.world().resource::<Session>()),
        bin::to_vec(&*fresh.world().resource::<Session>())
    );
}

#[test]
fn procedural_entities_report_named_generator_with_seed() {
    let sim = Sim::<Lanterns>::new(Options {
        scene: bake_scene().content,
        seed: 41,
        ..Default::default()
    })
    .unwrap();
    let provenance = sim
        .world()
        .get::<exact_game_scene::GeneratedBy>("tree-0")
        .unwrap();
    assert_eq!(provenance.generator, "lanterns::spawn_decor");
    assert_eq!(provenance.parameters["seed"], "41");
}

#[test]
fn walking_fox_with_authored_bounds_behind_wall_changes_geometric_eyes_headlessly() {
    let mut sim = Sim::<Lanterns>::new(Options {
        scene: bake_scene().content,
        started: true,
        seed: 1_041_003,
        ..Default::default()
    })
    .unwrap();
    sim.viewport(800.0, 600.0);
    let player = sim.world().named("player").unwrap();
    let camera = sim.world().named("camera").unwrap();
    sim.world_mut().remove::<Follow>(camera);
    sim.world_mut().teleport(
        camera,
        Transform::at(-10.0, 1.0, 5.0).looking_at(Vec3::new(-2.0, 0.0, 5.0), Vec3::Y),
    );
    sim.world_mut()
        .teleport(player, Transform::at(-2.0, 0.65, 1.0));
    sim.world_mut().propagate();
    // The embedded cosmetic GLB has no declared simulation model. Give this
    // geometric probe explicit authored bounds instead of relying on a unit fallback.
    let fox = sim.world().named("fox").unwrap();
    sim.world_mut().insert(
        fox,
        exact_game::asset::ModelBounds([-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]),
    );
    let request = r#"{"op":"layout","entity":"fox","to":"lantern-2"}"#;
    let before = sim.agent(request);
    sim.hold("KeyS", 900.0);
    let epoch = sim.world().mutation_epoch();
    let after = sim.agent(request);
    assert_eq!(sim.world().mutation_epoch(), epoch);
    eprintln!("fox before: {before}\nfox after: {after}");
    #[derive(Default, exact_game::Data)]
    struct Visibility {
        occluded: f32,
    }
    // Parse only the sampled fraction; exact wire names are also checked below.
    #[derive(Default, exact_game::Data)]
    struct EntityEyes {
        visible: Visibility,
    }
    #[derive(Default, exact_game::Data)]
    struct Reply {
        entity: EntityEyes,
    }
    let a: Reply = exact_game::json::from_str(&before).unwrap();
    let b: Reply = exact_game::json::from_str(&after).unwrap();
    assert!(b.entity.visible.occluded > a.entity.visible.occluded);
    assert!(before.contains("\"lineOfSight\":true"), "{before}");
    assert!(after.contains("\"lineOfSight\":false"), "{after}");
    assert!(after.contains("\"wall\""), "{after}");
}
