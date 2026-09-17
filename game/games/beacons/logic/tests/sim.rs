use beacons_logic::{Beacon, Beacons, Player};
use exact_game::{Clock, InputEvent, Sim, Transform, Value, Vec3};
fn sim(seed: f64) -> Sim<Beacons> {
    let mut s = Sim::new(&[Value::Number(seed), Value::Number(0.0), Value::Bool(false)]).unwrap();
    s.advance(0.0, Clock::Seekable);
    s
}
fn key(s: &mut Sim<Beacons>, code: &str, down: bool, at_ms: f64) {
    s.input(InputEvent::Key {
        code: code.into(),
        down,
        at_ms,
    });
}
fn position(s: &Sim<Beacons>, name: &str) -> Vec3 {
    s.world()
        .get::<Transform>(s.world().named(name).unwrap())
        .unwrap()
        .position
}
#[test]
fn movement_seed_and_seek_invariance() {
    let mut a = sim(7.0);
    let mut b = sim(7.0);
    let c = sim(8.0);
    for n in 1..=6 {
        let name = format!("crate-{n}");
        assert_eq!(position(&a, &name), position(&b, &name));
        assert_ne!(position(&a, &name), position(&c, &name));
    }
    for s in [&mut a, &mut b] {
        key(s, "KeyW", true, 0.0);
    }
    a.advance(1500.0, Clock::Seekable);
    for ms in 1..=1500 {
        b.advance(ms as f64, Clock::Seekable);
    }
    assert_eq!(a.save(), b.save());
    assert!((position(&a, "player") - Vec3::new(0.0, 0.9, -5.733332)).length() < 0.001);
    let player = a
        .world()
        .get::<Player>(a.world().named("player").unwrap())
        .unwrap();
    assert!(player.velocity.length() <= 4.0);
    println!("W1500 hash=0x{:016x}", a.world().hash());
    assert_eq!(a.world().hash(), 0xf1bdfbe68b382647);
}
#[test]
fn jump_no_double_jump_and_camera_lags() {
    let mut a = sim(7.0);
    let mut b = sim(7.0);
    for s in [&mut a, &mut b] {
        key(s, "Space", true, 0.0);
        key(s, "Space", false, 0.0);
    }
    a.advance(250.0, Clock::Seekable);
    key(&mut a, "Space", true, 250.0);
    key(&mut a, "Space", false, 250.0);
    a.advance(500.0, Clock::Seekable);
    b.advance(500.0, Clock::Seekable);
    assert_eq!(position(&a, "player"), position(&b, "player"));
    assert!((position(&a, "player").y - 2.1).abs() < 0.002);
    a.advance(1100.0, Clock::Seekable);
    assert_eq!(position(&a, "player").y, 0.9);
    key(&mut a, "ArrowRight", true, 1100.0);
    a.advance(1200.0, Clock::Seekable);
    assert!(position(&a, "camera").x > 0.0);
    assert!(position(&a, "camera").x < position(&a, "player").x);
}
#[test]
fn beacon_range_glow_and_restart() {
    let mut s = sim(7.0);
    key(&mut s, "KeyE", true, 0.0);
    key(&mut s, "KeyE", false, 0.0);
    s.advance(100.0, Clock::Seekable);
    assert_eq!(s.world().published("beacons"), Some(Value::Number(0.0)));
    key(&mut s, "KeyD", true, 100.0);
    s.advance(1850.0, Clock::Seekable);
    key(&mut s, "KeyD", false, 1850.0);
    key(&mut s, "KeyE", true, 1850.0);
    key(&mut s, "KeyE", false, 1850.0);
    s.advance(1950.0, Clock::Seekable);
    let e = s.world().named("beacon-1").unwrap();
    let glow = s.world().get::<Beacon>(e).unwrap().glow;
    assert!(glow > 0.0 && glow < 1.0);
    assert_eq!(s.world().published("beacons"), Some(Value::Number(1.0)));
    s.advance(2850.0, Clock::Seekable);
    assert_eq!(s.world().get::<Beacon>(e).unwrap().glow, 1.0);
    s.bind(
        &[Value::Number(7.0), Value::Number(0.0), Value::Bool(true)],
        Some(2850.0),
    )
    .unwrap();
    let before = s.world().save();
    s.advance(4850.0, Clock::Seekable);
    assert_eq!(before, s.world().save());
    s.bind(
        &[Value::Number(7.0), Value::Number(1.0), Value::Bool(false)],
        Some(4850.0),
    )
    .unwrap();
    assert_eq!(s.world().tick(), 0);
    assert_eq!(position(&s, "player"), Vec3::new(0.0, 0.9, 0.0));
    assert_eq!(s.world().published("beacons"), Some(Value::Number(0.0)));
}
