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
fn movement_seed_jump_and_save() {
    let mut a = sim(7);
    let mut b = sim(7);
    let c = sim(8);
    assert_ne!(
        a.get::<Transform>("crate-1").unwrap().position,
        c.get::<Transform>("crate-1").unwrap().position
    );
    a.hold("KeyW", 1500.0);
    b.key_down("ArrowUp");
    for _ in 0..1500 {
        b.run(1.0);
    }
    b.key_up("ArrowUp");
    assert_eq!(a.position("player"), b.position("player"));
    assert!((a.position("player").unwrap() - Vec3::new(0.0, 0.9, -5.3666644)).length() < 0.001);
    assert_eq!(
        a.get::<Player>("player").unwrap().character.velocity.z,
        -4.0
    );
    a.run(100.0);
    assert!(a.position("player").unwrap().z < -5.3666644);
    assert!(a.settle());
    assert_eq!(
        a.get::<Player>("player").unwrap().character.velocity,
        Vec3::ZERO
    );
    a.tap("Space");
    a.run(300.0);
    let bytes = a.save().unwrap();
    b.restore(&bytes).unwrap();
    a.tap("Space"); // Second jump must not change the ballistic arc.
    let mut peak = 0.0_f32;
    for _ in 0..60 {
        a.run(1000.0 / 60.0);
        b.run(1000.0 / 60.0);
        assert_eq!(a.position("player"), b.position("player"));
        peak = peak.max(a.position("player").unwrap().y);
    }
    assert!((peak - 2.1).abs() < 0.002);
    assert_eq!(a.position("player").unwrap().y, 0.9);
}
