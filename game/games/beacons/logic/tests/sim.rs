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
    let rows = std::collections::BTreeMap::from([("movement.json".to_owned(), include_str!("../../movement.json").to_owned())]);
    let bytes = exact_game::json::to_string(&rows).unwrap();
    sim.asset(exact_game::asset::TABLE_ASSET, Some(bytes.as_bytes())).unwrap();
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
