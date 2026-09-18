use super::*;
use exact_game::{bin, Sim};
#[test]
fn changed_scene_authored_fields_apply_while_carried_simulation_state_survives() {
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
    light_nearest(running.world_mut(), false).unwrap();
    running
        .world_mut()
        .get_mut::<Transform>("crate")
        .unwrap()
        .position
        .x = 6.8;
    let before = running.world().hash();
    let save = running.save().unwrap();

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
    assert_ne!(fresh.world().hash(), before);
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
        -11.0
    );
    assert_eq!(
        fresh
            .world()
            .resource::<exact_game_scene::SceneIdentity>()
            .digest,
        replacement.digest
    );
    assert!(fresh
        .agent(r#"{"op":"state"}"#)
        .contains("authored initializer changed"));
    // A second save/restore in this build must not reconstruct the old scene
    // as its initializer and silently undo the applied edit.
    let carried = fresh.save().unwrap();
    let mut twice = Sim::<Lanterns>::from_save(&carried).unwrap();
    assert_eq!(twice.save().unwrap(), carried);
    twice.restore(&carried).unwrap();
    assert_eq!(twice.save().unwrap(), carried);
    assert_eq!(
        twice
            .world()
            .get::<Transform>("lantern-1")
            .unwrap()
            .position
            .x,
        -11.0
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
    sim.world_mut().insert(
        player,
        Mesh::Capsule {
            radius: 0.4,
            height: 1.3,
        },
    );
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
    #[derive(Default, exact_game::Data)]
    struct Visibility {
        occluded: f32,
        occluders: Vec<String>,
    }
    // Parse the occluder field, not arbitrary occurrences of a name in the reply.
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
    assert!(
        !a.entity.visible.occluders.iter().any(|n| n == "wall"),
        "{before}"
    );
    assert!(
        b.entity.visible.occluders.iter().any(|n| n == "wall"),
        "{after}"
    );
    assert!(
        !b.entity.visible.occluders.iter().any(|n| n == "player"),
        "{after}"
    );
}
