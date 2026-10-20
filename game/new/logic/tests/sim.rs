use exact_game::{Sim, Transform};

// Apart, so rustfmt's order does not depend on the name a new game gives these.
use small_game_logic::SmallGame;
use small_game_logic::{shop_rows, Beacon, Critter, Options, Shop, FEED_PRICE};

/// The world every test here starts from.
fn start(options: Options) -> Sim<SmallGame> {
    Sim::new(options).unwrap()
}

#[test]
fn movement_and_light() {
    let mut game = start(Options {
        seed: 7,
        ..Options::default()
    });
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
    const GROUND_HEIGHT: f32 = 0.9;
    const JUMP_HEIGHT: f32 = 1.2;
    const TICK_MS: f64 = 8.334;
    const APEX_TOLERANCE: f32 = 0.000_1;

    let mut game = start(Options {
        seed: 7,
        ..Options::default()
    });
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

    let saved = game.save().unwrap();
    let mut reference = start(Options {
        seed: 7,
        ..Options::default()
    });
    reference.restore(&saved).unwrap();
    game.tap("Space");
    reference.tap("Space");
    let mut maximum_height = GROUND_HEIGHT;
    // 8.334 ms rounds to 8,334 us, advancing exactly one 120 Hz tick here.
    // Sampling can miss the continuous apex by at most g * dt^2 / 8 = 0.085 mm.
    for step in 0..150 {
        if step == 20 {
            assert!(game.global_position("player").unwrap().y > GROUND_HEIGHT);
            game.tap("Space");
        }
        assert_eq!(game.run(TICK_MS), 1);
        assert_eq!(reference.run(TICK_MS), 1);
        let height = game.global_position("player").unwrap().y;
        maximum_height = maximum_height.max(height);
        assert_eq!(
            height,
            reference.global_position("player").unwrap().y,
            "an airborne press must not add another jump"
        );
    }
    assert!(
        (maximum_height - (GROUND_HEIGHT + JUMP_HEIGHT)).abs() < APEX_TOLERANCE,
        "sampled apex {maximum_height} should be {JUMP_HEIGHT} m above {GROUND_HEIGHT} m ground"
    );
    assert_eq!(
        game.global_position("player").unwrap().y,
        GROUND_HEIGHT,
        "gravity lands on the floor"
    );
    game.tap("Space");
    assert_eq!(game.run(TICK_MS), 1);
    assert!(
        game.global_position("player").unwrap().y > GROUND_HEIGHT,
        "a later grounded press starts another jump"
    );
}

#[test]
fn proximity_nearest_unlit_plinths_bounds_and_restart() {
    use exact_game::{Mesh, Value};
    let mut game = start(Options {
        seed: 7,
        ..Options::default()
    });
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
    let game = start(Options::default());
    let w = game.world();
    let env = w.resource::<Environment>();
    assert_eq!(env.fog.unwrap().color, env.background);
    assert!(w.count::<Material>(|m| m.grid_spacing > 0.) > 0);
    assert_eq!(w.count::<Glow>(|_| true), 2);
    assert_eq!(
        w.require::<Material>("beacon-1").emissive,
        [3., 0.45, 0.025]
    );
}

#[test]
fn held_w_matches_the_closed_form_acceleration_series() {
    let mut game = start(Options::default());
    game.hold("KeyW", 1500.);
    // v_k = min(k*a/h, v), d_n = a*m*(m+1)/(2*h*h)+(n-m)*v/h.
    let (h, n, a, v): (f64, f64, f64, f64) = (120., 180., 12., 4.);
    let m = n.min((v * h / a).floor());
    let distance = a * m * (m + 1.) / (2. * h * h) + (n - m) * v / h;
    assert!((game.local_position("player").unwrap().z as f64 + distance).abs() < 0.001);
}

/// A scripted player plays five minutes on each of three seeds in about a
/// second: it walks to the critter, stays by it, feeds it whenever it may and
/// buys shoes with what is left over. The starter's prices and income keep the
/// critter fed and pay for all three pairs; a change that breaks that loop
/// fails here before anyone plays it.
#[test]
fn a_bot_plays_five_minutes_and_the_loop_pays() {
    for seed in [7, 8, 9] {
        let mut game = start(Options {
            seed,
            ..Options::default()
        });
        game.hold("KeyA", 600.0);
        let mut feeds = 0;
        for _ in 0..5 * 60 * 4 {
            let rows = shop_rows(game.world());
            let coins = game.world().resource::<Shop>().coins;
            if rows[0].reason.is_empty() {
                game.tap("Digit1");
                feeds += 1;
            } else if rows[1].reason.is_empty() && coins >= rows[1].price + FEED_PRICE {
                game.tap("Digit2");
            }
            game.run(250.0);
        }
        let critter = game.get::<Critter>("critter").unwrap();
        let shop = game.world().resource::<Shop>();
        let (earned, shoes) = (shop.earned, shop.shoes);
        assert!(
            critter.awake && !critter.starved,
            "seed {seed}: the critter starved"
        );
        assert!(
            earned >= 140,
            "seed {seed}: earned {earned} coins in five minutes"
        );
        assert_eq!(
            shoes, 3,
            "seed {seed}: {earned} coins earned, {feeds} feeds bought, shoes {shoes}"
        );
    }
}
