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
