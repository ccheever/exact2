use exact_game::{Game, Sim};
use lanterns_logic::{scene_types, Lanterns, Options};
#[test]
fn before_change_save_bytes() {
    let mut sim = Sim::<Lanterns>::new(Options {
        seed: 1_041_003,
        started: true,
        sound: true,
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json"),
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
        assert_eq!(sim.world().save(), bytes);
    }
}
