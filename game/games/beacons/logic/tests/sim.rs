use beacons_logic::{Beacons, Options, Player};
use exact_game::{Sim, Transform, Vec3};
fn sim(seed: u64) -> Sim<Beacons> {
    Sim::new(Options {
        seed,
        ..Options::default()
    })
    .unwrap()
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
    assert_eq!(a.world().hash(), 0x7379ac5210e92317);
    assert_eq!(a.position("player"), Some(Vec3::new(0.0, 0.9, -5.3666644)));
    println!(
        "1500ms position={:?}, hash={:016x}",
        a.position("player").unwrap(),
        a.world().hash()
    );
    a.key_up("KeyW");
    a.run(100.0);
    assert!(a.position("player").unwrap().z < -5.3666644);
    assert!(a.settle());
    assert_eq!(
        a.world()
            .get::<Player>("player")
            .unwrap()
            .character
            .velocity,
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
    assert_eq!(a.position("player").unwrap(), b.position("player").unwrap());
    assert!((a.position("player").unwrap().y - 2.1).abs() < 0.005);
    a.run(1000.0);
    assert_eq!(a.position("player").unwrap().y, 0.9);
    b.key_down("ArrowRight");
    b.run(713.0);
    a.restore(&b.save()).unwrap();
    a.run(827.0);
    b.run(827.0);
    assert_eq!(a.save(), b.save());
}
