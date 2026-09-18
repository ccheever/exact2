use exact_game::{Sim, Transform};
use small_game_logic::{Beacon, Options, SmallGame};

#[test]
fn movement_and_light() {
    let mut game = Sim::<SmallGame>::new(Options {
        seed: 7,
        ..Options::default()
    })
    .unwrap();
    // A new game has no verified pins until its first three-mode proof.
    let pins = include_str!("../../pins.json");
    #[derive(Default, exact_game::Data)]
    struct Pins {
        ticks: std::collections::BTreeMap<String, String>,
    }
    if exact_game::json::from_str::<Pins>(pins)
        .unwrap()
        .ticks
        .contains_key("0")
    {
        exact_game::World::assert_pin(pins, "small-game", 0, game.world().hash());
    }
    game.viewport(800., 600.);
    let hit = game
        .pick(game.layout("player").unwrap().screen.center())
        .unwrap();
    assert_eq!(game.world().name(hit.entity), Some("player"));
    let saved = game.save().unwrap();
    game.restore(&saved).unwrap();
    game.hold("KeyD", 500.0);
    game.settle();
    let position = game.global_position("player").unwrap();
    game.tap("KeyE");
    game.run(100.0);
    let beacon = game.get::<Beacon>("beacon-1").unwrap();
    assert!(position.x > 0.5 && beacon.lit);
    assert_eq!(
        game.world().published("lit").unwrap().as_number(),
        Some(1.0)
    );
    assert!(game.world().get::<Transform>("camera").unwrap().position.y > 9.0);
}

#[test]
fn acceleration_braking_and_ballistic_jump() {
    let mut game = Sim::<SmallGame>::new(Options {
        seed: 7,
        ..Options::default()
    })
    .unwrap();
    game.hold("KeyW", 100.0);
    assert!(
        game.global_position("player").unwrap().z < 0.0
            && game.global_position("player").unwrap().z > -0.35,
        "movement accelerates toward four metres per second"
    );
    let released = game.global_position("player").unwrap().z;
    game.run(100.0);
    assert!(
        game.global_position("player").unwrap().z < released,
        "movement brakes after release"
    );
    assert!(game.settle());
    let stopped = game.global_position("player").unwrap();
    game.run(100.0);
    assert_eq!(game.global_position("player").unwrap(), stopped);
    game.tap("Space");
    game.run(400.0);
    assert!(
        game.global_position("player").unwrap().y > 1.8,
        "jump rises toward 1.2 metres above the floor"
    );
    game.run(1000.0);
    assert_eq!(
        game.global_position("player").unwrap().y,
        0.9,
        "gravity lands on the floor"
    );
}

#[test]
fn proximity_nearest_unlit_plinths_bounds_and_restart() {
    use exact_game::{Mesh, Value};
    let mut game = Sim::<SmallGame>::new(Options {
        seed: 7,
        ..Options::default()
    })
    .unwrap();
    assert_eq!(game.world().published("near").unwrap().as_str(), Some(""));
    assert_eq!(
        *game.get::<Mesh>("plinth-1").unwrap(),
        Mesh::cylinder(0.9, 0.2)
    );
    game.tap("KeyE");
    game.run(100.0);
    assert!(!game.get::<Beacon>("beacon-1").unwrap().lit);
    game.world()
        .get_mut::<Transform>("beacon-2")
        .unwrap()
        .position
        .x = 3.0;
    game.world()
        .get_mut::<Transform>("player")
        .unwrap()
        .position
        .x = 2.8;
    game.run(100.0);
    assert_eq!(
        game.world().published("near").unwrap().as_str(),
        Some("beacon-2")
    );
    game.world().get_mut::<Beacon>("beacon-2").unwrap().lit = true;
    game.run(100.0);
    assert_eq!(
        game.world().published("near").unwrap().as_str(),
        Some("beacon-1") // A lit closest beacon cannot hide another unlit candidate.
    );
    game.hold("KeyW", 1000.0);
    game.settle();
    assert_eq!(game.world().published("near").unwrap().as_str(), Some(""));
    game.hold("KeyD", 10000.0);
    game.settle();
    assert_eq!(game.global_position("player").unwrap().x, 19.6);
    game.bind(
        &[Value::Number(7.0), Value::Bool(false), Value::Bool(true)],
        None,
    )
    .unwrap();
    assert_eq!(
        game.global_position("player").unwrap(),
        exact_game::Vec3::new(0.0, 0.9, 0.0)
    );
    assert_eq!(
        game.world().published("lit").unwrap().as_number(),
        Some(0.0)
    );
    assert_eq!(game.world().published("near").unwrap().as_str(), Some(""));
}
