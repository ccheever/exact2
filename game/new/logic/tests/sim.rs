use exact_game::{Sim, Transform};
use small_game_logic::{Options, SmallGame};

#[test]
fn movement_and_light() {
    let mut game = Sim::<SmallGame>::new(Options { seed: 7, paused: false }).unwrap();
    game.hold("KeyW", 250.0);
    assert!(game.world().get::<Transform>("player").unwrap().position.z < 0.0);
    game.tap("KeyE");
    assert!(game.settle());
    assert_eq!(game.world().published("lit").unwrap().as_number(), Some(1.0));
    assert!(game.world().get::<Transform>("camera").unwrap().position.y > 9.0);
}

#[test]
fn acceleration_braking_and_ballistic_jump() {
    let mut game = Sim::<SmallGame>::new(Options { seed: 7, paused: false }).unwrap();
    let position = |s: &Sim<SmallGame>| s.world().get::<Transform>("player").unwrap().position;
    game.hold("KeyW", 100.0);
    assert!(position(&game).z < 0.0 && position(&game).z > -0.35, "movement accelerates toward four metres per second");
    let released = position(&game).z;
    game.run(100.0);
    assert!(position(&game).z < released, "movement brakes after release");
    assert!(game.settle());
    let stopped = position(&game);
    game.run(100.0);
    assert_eq!(position(&game), stopped);
    game.tap("Space");
    game.run(400.0);
    assert!(position(&game).y > 1.8, "jump rises toward 1.2 metres above the floor");
    game.run(1000.0);
    assert_eq!(position(&game).y, 0.9, "gravity lands on the floor");
}
