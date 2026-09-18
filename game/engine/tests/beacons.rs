#[path = "../../games/beacons/logic/src/lib.rs"]
mod beacons;
pub use beacons::*;
extern crate self as beacons_logic;
#[path = "../../games/beacons/logic/tests/sim.rs"]
mod tests;

#[test]
fn beacons_imported_capture_replays_with_scoped_checkpoint_budget() {
    use exact_game::{Capture, CaptureLimits, Clock, Sim};
    let scene = exact_game_scene::bake::compile(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../games/beacons/scene.json"),
        &scene_types(),
        &[],
    )
    .unwrap()
    .content;
    let mut sim = Sim::<Beacons>::new(Options {
        seed: 7,
        scene,
        ..Default::default()
    })
    .unwrap();
    sim.advance(0.0, Clock::Seekable);
    sim.run(100.0);
    sim.start_capture("beacons-test", CaptureLimits::default())
        .unwrap();
    sim.key_down("KeyW");
    sim.run(300.0);
    sim.key_up("KeyW");
    let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
    let replay = Sim::<Beacons>::replay_capture(&capture, "beacons-test", None).unwrap();
    assert_eq!(sim.world().hash(), replay.world().hash());
}
