use super::*;
use exact_game::{Game, Sim};
#[test]
fn before_change_save_bytes() {
    let mut sim = Sim::<Lanterns>::new(Options {
        seed: 1_041_003,
        started: true,
        sound: true,
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../verification/lanterns/scene.json"),
            &scene_types(),
            Lanterns::assets(),
        )
        .unwrap()
        .content,
        ..Default::default()
    })
    .unwrap();
    for (ms, bytes) in [
        (0.0, include_bytes!("fixtures/before-0.world").as_slice()),
        (
            1000.0,
            include_bytes!("fixtures/before-60.world").as_slice(),
        ),
        (
            2000.0,
            include_bytes!("fixtures/before-180.world").as_slice(),
        ),
    ] {
        sim.key_down("KeyW");
        sim.run(ms);
        let mut expected = Sim::<Lanterns>::new(Options {
            scene: bake_scene().content,
            seed: 1_041_003,
            started: true,
            sound: true,
            ..Default::default()
        })
        .unwrap();
        // The approved schema addition changes only saved bindings and the scene
        // digest (the baked Lantern default now includes its saved Entity).
        let scene_identity = exact_game::bin::to_vec(
            &*expected
                .world()
                .resource::<exact_game_scene::SceneIdentity>(),
        );
        expected.world_mut().load(bytes).unwrap();
        bind_scene(expected.world());
        exact_game::bin::read_into(
            &scene_identity,
            &mut *expected
                .world()
                .resource_mut::<exact_game_scene::SceneIdentity>(),
        )
        .unwrap();
        assert_eq!(
            sim.world().save(),
            expected.world().save(),
            "old bytes plus only declared binding state"
        );
    }
}
