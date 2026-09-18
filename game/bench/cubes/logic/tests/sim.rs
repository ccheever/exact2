use bench_cubes_logic::Cubes;
use exact_game::{Clock, Sim, Value};

#[test]
fn one_seek_and_sixty_ticks_agree() {
    let create = || Sim::<Cubes>::from_values(&[Value::Number(100.0)]).unwrap();
    let mut jump = create();
    let mut steps = create();
    jump.advance(0.0, Clock::Seekable);
    steps.advance(0.0, Clock::Seekable);
    jump.advance(1000.0, Clock::Seekable);
    for i in 1..=60 {
        steps.advance(i as f64 * 1000.0 / 60.0, Clock::Seekable);
    }
    assert_eq!(jump.world().hash(), steps.world().hash());
}

#[path = "../../../../paranoid-test.rs"]
mod paranoid;
#[test]
fn every_tick_save_matches_normal_script() {
    paranoid::compare(
        || Sim::<Cubes>::from_values(&[Value::Number(100.0)]).unwrap(),
        |sim| {
            sim.hold("KeyW", 713.123);
            sim.tap("Space");
            sim.run(286.877);
            sim.tap("KeyE");
            sim.run(1000.0);
        },
    );
}
