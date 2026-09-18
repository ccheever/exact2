use beacons_logic::{Beacon, Beacons, BeaconsArgs, Player};
use exact_game::{Args, Sim, Transform, Vec3};
fn sim(seed: u64) -> Sim<Beacons> {
    Sim::new(BeaconsArgs {
        seed,
        ..Default::default()
    })
    .unwrap()
}
#[test]
fn movement_seed_and_seek_invariance() {
    let (mut a, mut b, c) = (sim(7), sim(7), sim(8));
    for n in 1..=6 {
        let name = format!("crate-{n}");
        assert_eq!(
            a.world().get::<Transform>(name.as_str()).unwrap().position,
            b.world().get::<Transform>(name.as_str()).unwrap().position
        );
        assert_ne!(
            a.world().get::<Transform>(name.as_str()).unwrap().position,
            c.world().get::<Transform>(name.as_str()).unwrap().position
        );
    }
    for s in [&mut a, &mut b] {
        s.key_down("KeyW");
    }
    a.run(1500.0);
    for _ in 0..1500 {
        b.run(1.0);
    }
    assert_eq!(a.save(), b.save());
    assert!(
        (a.world().get::<Transform>("player").unwrap().position - Vec3::new(0.0, 0.9, -5.733332))
            .length()
            < 0.001
    );
    assert!(a.world().get::<Player>("player").unwrap().velocity.length() <= 4.0);
    println!("W1500 hash=0x{:016x}", a.world().hash());
    assert_eq!(a.world().hash(), 0xd17e623e56fb8dc9);
}
#[test]
fn jump_no_double_jump_and_camera_lags() {
    let (mut a, mut b) = (sim(7), sim(7));
    for s in [&mut a, &mut b] {
        s.tap("Space");
    }
    a.run(250.0);
    a.tap("Space");
    a.run(250.0);
    b.run(500.0);
    assert_eq!(
        a.world().get::<Transform>("player").unwrap().position,
        b.world().get::<Transform>("player").unwrap().position
    );
    assert!((a.world().get::<Transform>("player").unwrap().position.y - 2.1).abs() < 0.002);
    assert!(a.settle());
    assert_eq!(
        a.world().get::<Transform>("player").unwrap().position.y,
        0.9
    );
    a.hold("ArrowRight", 100.0);
    let camera = a.world().get::<Transform>("camera").unwrap().position;
    assert!(camera.x > 0.0 && camera.x < a.world().get::<Transform>("player").unwrap().position.x);
}
#[test]
fn beacon_range_glow_and_restart() {
    let mut s = sim(7);
    s.tap("KeyE");
    s.run(100.0);
    assert_eq!(
        s.world().published("beacons").unwrap().as_number(),
        Some(0.0)
    );
    s.hold("KeyD", 1750.0);
    s.tap("KeyE");
    s.run(100.0);
    let glow = s
        .world()
        .get::<Beacon>("beacon-1")
        .unwrap()
        .glow
        .value(s.world().now());
    assert!(glow > 0.0 && glow < 1.0);
    assert_eq!(
        s.world().published("beacons").unwrap().as_number(),
        Some(1.0)
    );
    assert!(s.settle());
    assert_eq!(
        s.world()
            .get::<Beacon>("beacon-1")
            .unwrap()
            .glow
            .value(s.world().now()),
        1.0
    );
    s.bind(
        &BeaconsArgs {
            seed: 7,
            paused: true,
            ..Default::default()
        }
        .values(),
        None,
    )
    .unwrap();
    let before = s.world().save();
    s.run(2000.0);
    assert_eq!(before, s.world().save());
    s.bind(
        &BeaconsArgs {
            seed: 7,
            run: 1,
            paused: false,
        }
        .values(),
        None,
    )
    .unwrap();
    assert_eq!(s.world().tick(), 0);
    assert_eq!(
        s.world().get::<Transform>("player").unwrap().position,
        Vec3::new(0.0, 0.9, 0.0)
    );
    assert_eq!(
        s.world().published("beacons").unwrap().as_number(),
        Some(0.0)
    );
}
