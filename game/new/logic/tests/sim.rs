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
        game.assert_pin(pins);
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
    assert!(game.world().require::<Transform>("camera").position.y > 9.0);
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
    game.world().require_mut::<Transform>("beacon-2").position.x = 3.0;
    game.world().require_mut::<Transform>("player").position.x = 2.8;
    game.run(100.0);
    assert_eq!(
        game.world().published("near").unwrap().as_str(),
        Some("beacon-2")
    );
    game.world().require_mut::<Beacon>("beacon-2").lit = true;
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

#[test]
fn starter_keeps_grid_sky_fog_pads_and_saved_glow() {
    use exact_game::{Environment, Glow, Material};
    let game = Sim::<SmallGame>::new(Options::default()).unwrap();
    let w = game.world();
    let env = w.resource::<Environment>();
    assert_eq!(env.fog.unwrap().color, env.background);
    assert!(w.count::<Material>(|m| m.grid_spacing > 0.) > 0);
    assert_eq!(w.count::<Glow>(|_| true), 2);
    assert_eq!(w.require::<Material>("beacon-1").emissive, [3., 1.5, 0.3]);
}

#[test]
fn held_w_matches_the_closed_form_acceleration_series() {
    let mut game = Sim::<SmallGame>::new(Options::default()).unwrap();
    game.hold("KeyW", 1500.);
    // v_k = min(k*a/h, v), d_n = a*m*(m+1)/(2*h*h)+(n-m)*v/h.
    let (h, n, a, v): (f64, f64, f64, f64) = (120., 180., 12., 4.);
    let m = n.min((v * h / a).floor());
    let distance = a * m * (m + 1.) / (2. * h * h) + (n - m) * v / h;
    assert!((game.local_position("player").unwrap().z as f64 + distance).abs() < 0.001);
}
