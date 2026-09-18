use beacons_logic::{scene_types, Beacons, Options};
use exact_game::{Game, Sim};
#[test]
fn before_change_save_bytes() {
    let mut sim = Sim::<Beacons>::new(Options {
        seed: 1_041_003,
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json"),
            &scene_types(),
            Beacons::assets(),
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
