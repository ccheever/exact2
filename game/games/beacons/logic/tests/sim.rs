use beacons_logic::{Beacons, Options, Player};
use exact_game::{Sim, Transform, Vec3};
fn sim(seed: u64) -> Sim<Beacons> {
    Sim::new(Options {
        seed,
        ..Options::default()
    })
    .unwrap()
}
fn position(s: &Sim<Beacons>) -> Vec3 {
    s.world().get::<Transform>("player").unwrap().position
}
#[test]
fn movement_seed_and_partitioning() {
    let mut a = sim(7);
    let mut b = sim(7);
    assert_eq!(a.save(), b.save());
    assert_ne!(
        a.world().get::<Transform>("crate-1").unwrap().position,
        sim(8).world().get::<Transform>("crate-1").unwrap().position
    );
    a.key_down("KeyW");
    b.key_down("KeyW");
    a.run(1500.0);
    for _ in 0..1500 {
        b.run(1.0);
    }
    assert_eq!(a.save(), b.save());
    assert!((position(&a).z + 5.3666644).abs() < 0.001);
    println!(
        "1500ms position={:?}, hash={:016x}",
        position(&a),
        a.world().hash()
    );
    a.key_up("KeyW");
    a.run(100.0);
    assert!(position(&a).z < -5.3666644);
    assert!(a.settle());
    assert_eq!(
        a.world().get::<Player>("player").unwrap().velocity,
        Vec3::ZERO
    );
}
#[test]
fn jump_no_double_jump_and_restore() {
    let mut a = sim(7);
    a.tap("Space");
    a.run(250.0);
    let mut b = sim(7);
    b.restore(&a.save()).unwrap();
    a.tap("Space");
    a.run(250.0);
    b.run(250.0);
    assert_eq!(position(&a), position(&b));
    assert!((position(&a).y - 2.1).abs() < 0.005);
    a.run(1000.0);
    assert_eq!(position(&a).y, 0.9);
    b.key_down("ArrowRight");
    b.run(713.0);
    a.restore(&b.save()).unwrap();
    a.run(827.0);
    b.run(827.0);
    assert_eq!(a.save(), b.save());
}
