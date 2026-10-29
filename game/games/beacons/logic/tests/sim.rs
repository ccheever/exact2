use beacons_logic::{Beacons, Options};
use exact_game::{character::Character, Sim, Transform};

fn sim(seed: u64) -> Sim<Beacons> {
    let mut sim = Sim::new(Options {
        seed,
        paused: false,
        restart: false,
    })
    .unwrap();
    tables(&mut sim);
    sim
}
fn tables(sim: &mut Sim<Beacons>) {
    let rows = std::collections::BTreeMap::from([(
        "movement.json".to_owned(),
        include_str!("../../movement.json").to_owned(),
    )]);
    let bytes = exact_game::json::to_string(&rows).unwrap();
    sim.asset(exact_game::asset::TABLE_ASSET, Some(bytes.as_bytes()))
        .unwrap();
}
#[test]
fn movement_seed_jump_and_save() {
    let mut a = sim(7);
    let mut b = sim(7);
    let c = sim(8);
    assert_eq!(a.world().hash(), b.world().hash());
    assert_ne!(
        a.get::<Transform>("crate-1").unwrap().position,
        c.get::<Transform>("crate-1").unwrap().position
    );
    a.hold("KeyW", 1500.0);
    for _ in 0..90 {
        b.key_down("KeyW");
        b.run(1000.0 / 60.0);
    }
    b.key_up("KeyW");
    let p = a.get::<Transform>("player").unwrap().position;
    println!("W 1500 ms: {p:?}");
    assert!((p.z - -5.3666644).abs() < 0.001);
    assert_eq!(a.world().hash(), b.world().hash());
    a.assert_pin(include_str!("../../pins.json"));
    a.settle();
    a.tap("Space");
    a.run(250.0);
    let saved = a.save().unwrap();
    let mut restored = sim(7);
    restored.restore(&saved).unwrap();
    a.tap("Space"); // Airborne press must not become another jump.
    a.run(250.0);
    restored.run(250.0);
    assert_eq!(
        a.get::<Transform>("player").unwrap().position,
        restored.get::<Transform>("player").unwrap().position
    );
    let apex = a.get::<Transform>("player").unwrap().position.y;
    assert!((apex - 2.1).abs() < 0.02);
    a.run(1000.0);
    assert_eq!(a.get::<Transform>("player").unwrap().position.y, 0.9);
    assert_eq!(a.get::<Character>("player").unwrap().velocity.y, 0.0);
}

#[test]
fn rust_defaults_and_positional_binding_have_identical_bytes() {
    use exact_game::Value;
    let mut omitted = Sim::<Beacons>::new(Options {
        seed: 7,
        ..Options::default()
    })
    .unwrap();
    let mut positional =
        Sim::<Beacons>::from_values(&[Value::Number(7.), Value::Bool(false), Value::Bool(false)])
            .unwrap();
    tables(&mut omitted);
    tables(&mut positional);
    assert_eq!(omitted.save().unwrap(), positional.save().unwrap());
    omitted.assert_pin(include_str!("../../pins.json"));
}

/// Sprint released, or its pad gone, while paused: the player walks on
/// resume, slowing from the first tick.
#[test]
fn a_sprint_ended_while_paused_walks_on_resume() {
    use exact_game::Args;
    let pause = |s: &mut Sim<Beacons>, paused: bool| {
        let args = Options {
            seed: 7,
            paused,
            restart: false,
        };
        s.bind(&args.values(), None).unwrap();
    };
    let character = |s: &Sim<Beacons>| {
        let c = s.get::<Character>("player").unwrap();
        (c.speed, c.velocity.x)
    };
    for pad in [false, true] {
        let mut s = sim(7);
        s.settle();
        s.key_down("KeyD");
        if pad {
            s.pad_button(0, "GamepadL1", 1.);
        } else {
            s.key_down("ShiftLeft");
        }
        s.run(1500.);
        assert_eq!(
            character(&s),
            (7., 7.),
            "the hold matured and the player runs"
        );
        pause(&mut s, true);
        if pad {
            s.pad_disconnect(0);
        } else {
            s.key_up("ShiftLeft");
        }
        s.run(100.);
        pause(&mut s, false);
        let end = if pad {
            "a pad disconnected"
        } else {
            "Shift released"
        };
        let mut ticks = 0;
        while ticks == 0 {
            ticks = s.run(1.);
        }
        assert_eq!(ticks, 1);
        let (speed, velocity) = character(&s);
        assert_eq!(speed, 4., "{end} while paused");
        assert!(velocity < 7., "{end}: the first tick slows ({velocity})");
    }
}

/// Tilt to move (LLP 1133.013 §10): the level the app posts, a held tilt at
/// 60 Hz walking the player along x at the stick the engine's map gives, and
/// the input's end bringing the player to rest.
#[test]
fn tilt_walks_from_the_level_and_stops_at_the_end() {
    use exact_game::{tilt_stick, InputEvent, SensorKind, Tilt, Vec3};
    let mut a = sim(7);
    a.run(0.0);
    a.post("tilt 0 4.9 8.49");
    a.run(1000.0 / 60.0);
    let start = a.get::<Transform>("player").unwrap().position;
    let mut at = a.world().tick() as f64 * 1000.0 / 60.0;
    for _ in 0..60 {
        a.input(InputEvent::Sensor {
            sensor: SensorKind::Gravity,
            values: [-3.0, 4.9, 7.9, 0.0],
            dropped: 0,
            at_ms: at,
            player: 0,
        });
        at += 1000.0 / 60.0;
        a.run(1000.0 / 60.0);
    }
    let stick = tilt_stick(
        Vec3::new(-3.0, 4.9, 7.9),
        Vec3::new(0.0, 4.9, 8.49),
        Tilt {
            max_degrees: 20.0,
            dead_zone: 2.0,
        },
    );
    assert!((stick.x - 0.88).abs() < 0.01, "{stick}");
    let walked = a.get::<Transform>("player").unwrap().position - start;
    assert!(walked.x > 1.0 && walked.z.abs() < 0.05, "{walked}");
    a.input(InputEvent::SensorEnded {
        sensor: SensorKind::Gravity,
        at_ms: at,
        player: 0,
    });
    a.run(500.0);
    let rest = a.get::<Transform>("player").unwrap().position;
    a.run(500.0);
    assert_eq!(a.get::<Transform>("player").unwrap().position, rest);
    assert!(rest.x - start.x - walked.x < 0.3, "{rest}");
}
