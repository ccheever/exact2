use beacons_logic::{Beacons, Options, Player};
use exact_game::{Sim, Transform, Vec3};
fn sim(seed: u64) -> Sim<Beacons> {
    Sim::new(Options {
        seed,
        paused: false,
        round: 0,
    })
    .unwrap()
}
#[test]
fn movement_jump_seed_and_save() {
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
    let position = a.world().get::<Transform>("player").unwrap().position;
    println!(
        "W1500 position={position:?} hash=0x{:016x}",
        a.world().hash()
    );
    assert!((position - Vec3::new(0.0, 0.9, -5.7333384)).length() < 0.001);
    assert_eq!(a.save(), b.save());
    a.key_up("KeyW");
    a.tap("Space");
    a.run(250.0);
    let before = a.world().get::<Player>("player").unwrap().velocity.y;
    a.tap("Space");
    a.run(1000.0 / 60.0);
    assert!(a.world().get::<Player>("player").unwrap().velocity.y < before);
    a.run(233.3333333333);
    let height = a.world().get::<Transform>("player").unwrap().position.y;
    assert!((height - 2.1).abs() < 0.01, "height={height}");
    let save = a.save();
    let mut restored = sim(7);
    restored.restore(&save).unwrap();
    a.run(1000.0);
    restored.run(1000.0);
    assert_eq!(a.save(), restored.save());
    assert!(a.settle());
    assert_eq!(
        a.world().get::<Transform>("player").unwrap().position.y,
        0.9
    );
}
