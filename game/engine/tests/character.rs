use exact_game::{bin, character::Character, hash, Component, Sim, Transform, Vec3, World};

fn character() -> Character {
    Character::new()
        .speed(4.0)
        .accel(12.0)
        .brake(20.0)
        .jump(1.2)
        .gravity(9.81)
        .ground(0.9)
        .bounds_xz(-19.6..=19.6)
}
#[test]
fn ballistic_height_contacts_and_no_double_jump() {
    let mut c = character();
    let mut pose = Transform::at(0.0, 0.9, 0.0);
    let first = c.step(&mut pose, Vec3::ZERO, true, 1.0 / 60.0);
    assert!(first.jumped && !first.grounded && !first.landed);
    let mut apex = pose.position.y;
    let mut landings = 0;
    for tick in 1..120 {
        let contact = c.step(&mut pose, Vec3::ZERO, tick == 15, 1.0 / 60.0);
        assert!(!contact.jumped);
        if contact.landed {
            landings += 1;
            assert!(contact.grounded);
        }
        apex = apex.max(pose.position.y);
    }
    assert!((apex - 0.9 - 1.2).abs() < 0.002, "{apex}");
    assert_eq!(landings, 1);
    assert_eq!(pose.position.y, 0.9);
    assert_eq!(c.velocity, Vec3::ZERO);
    assert!(c.step(&mut pose, Vec3::ZERO, true, 1.0 / 60.0).jumped);
}
#[test]
fn exact_arrival_bounds_and_saved_determinism() {
    let mut c = character();
    let mut pose = Transform::at(0.0, 0.9, 0.0);
    for _ in 0..90 {
        c.step(&mut pose, Vec3::NEG_Z, false, 1.0 / 60.0);
    }
    assert_eq!(pose.position, Vec3::new(0.0, 0.9, -5.3666644));
    for _ in 0..20 {
        c.step(&mut pose, Vec3::ZERO, false, 1.0 / 60.0);
    }
    assert_eq!(c.velocity, Vec3::ZERO);
    let stopped = pose.position;
    c.step(&mut pose, Vec3::ZERO, false, 1.0);
    assert_eq!(pose.position, stopped);
    c.step(&mut pose, Vec3::X, true, 0.2);
    let saved = bin::to_vec(&c);
    let mut copy: Character = bin::from_slice(&saved).unwrap();
    let mut other = pose;
    for _ in 0..900 {
        assert_eq!(
            c.step(&mut pose, Vec3::new(1.0, 0.0, -1.0), false, 1.0 / 60.0),
            copy.step(&mut other, Vec3::new(1.0, 0.0, -1.0), false, 1.0 / 60.0)
        );
        assert_eq!(bin::to_vec(&c), bin::to_vec(&copy));
        assert_eq!(pose, other);
    }
    assert_eq!(pose.position, Vec3::new(19.6, 0.9, -19.6));
    assert_eq!(c.velocity, Vec3::ZERO);
    let before = hash::of(&c);
    c.step(&mut pose, Vec3::NEG_X, false, 1.0 / 60.0);
    assert_ne!(hash::of(&c), before);
    assert!(pose.position.x < 19.6);
    let before = bin::to_vec(&c);
    let p = pose;
    assert!(!c.step(&mut pose, Vec3::X, true, 0.0).jumped);
    assert_eq!(before, bin::to_vec(&c));
    assert_eq!(pose, p);
}
#[derive(Default, Component)]
struct Beacon {
    lit: bool,
}
#[test]
fn proximity_is_inclusive_ordered_named_and_planar() {
    let mut w = World::new(60, 0);
    let player = w.spawn_named("player", Transform::default());
    let far = w.spawn((Beacon::default(), Transform::at(3.0, 0.0, 0.0)));
    let edge = w.spawn((Beacon::default(), Transform::at(1.5, 2.0, 0.0)));
    let close = w.spawn((Beacon::default(), Transform::at(0.5, 0.0, 0.0)));
    w.spawn(Transform::default()); // no Beacon
    let ids = |v: Vec<_>| v;
    assert_eq!(
        ids(w.near::<Beacon>(player, 1.5).map(|(e, _)| e).collect()),
        vec![close]
    );
    assert_eq!(
        ids(w.near_xz::<Beacon>("player", 1.5).map(|(e, _)| e).collect()),
        vec![edge, close]
    );
    assert_eq!(
        ids(w.near::<Beacon>("player", 3.0).map(|(e, _)| e).collect()),
        vec![far, edge, close]
    );
    for (e, _) in w.near_xz::<Beacon>("player", 1.5) {
        w.get_mut::<Beacon>(e).unwrap().lit = true;
    }
    assert!(w.get::<Beacon>(edge).unwrap().lit);
    assert!(!w.get::<Beacon>(far).unwrap().lit);
    assert_eq!(w.near::<Beacon>("missing", 9.0).count(), 0);
    w.despawn(player);
    assert_eq!(w.near::<Beacon>(player, 9.0).count(), 0);
}
#[test]
fn sim_reads_match_world_reads_and_missing_entities() {
    use exact_game::{Args, Game, Input};
    #[derive(Default, Args)]
    struct Options {}
    struct GameUnderTest;
    impl Game for GameUnderTest {
        const ID: &'static str = "character-test";
        type Args = Options;
        fn setup(w: &mut World, _: &Options) {
            w.spawn_named("player", Transform::at(1.0, 2.0, 3.0));
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    let s = Sim::<GameUnderTest>::new(Options {}).unwrap();
    assert_eq!(s.position("player"), Some(Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(
        s.get::<Transform>("player").unwrap().position,
        s.position("player").unwrap()
    );
    assert!(s.position("missing").is_none());
    assert!(s.get::<Beacon>("player").is_none());
}

#[test]
fn proximity_excludes_resolved_origin() {
    let mut w = World::new(60, 0);
    let origin = w.spawn_named("origin", (Beacon::default(), Transform::default()));
    let other = w.spawn((Beacon::default(), Transform::default()));
    assert_eq!(
        w.near::<Beacon>(origin, 0.)
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        [other]
    );
    assert_eq!(
        w.near_xz::<Beacon>("origin", 0.)
            .map(|(e, _)| e)
            .collect::<Vec<_>>(),
        [other]
    );
}
#[test]
fn proximity_returns_unborrowed_poses() {
    let mut w = World::new(60, 0);
    w.spawn_named("origin", Transform::default());
    w.spawn((Beacon::default(), Transform::at(1., 0., 0.)));
    for (e, pose) in w.near::<Beacon>("origin", 2.) {
        w.get_mut::<Transform>(e).unwrap().position.y = pose.position.x;
    }
    for (e, pose) in w.near_xz::<Transform>("origin", 2.) {
        w.get_mut::<Transform>(e).unwrap().position.z = pose.position.x;
    }
}
#[test]
fn character_external_contact_is_an_event_once() {
    for vy in [0., -1.] {
        let mut c = character();
        let mut pose = Transform::at(0., 5., 0.);
        c.step(&mut pose, Vec3::ZERO, false, 0.01);
        pose.position.y = 0.9;
        c.velocity.y = vy;
        assert!(c.step(&mut pose, Vec3::ZERO, false, 0.01).landed);
        assert!(!c.step(&mut pose, Vec3::ZERO, false, 0.01).landed);
    }
    let mut c = character();
    let mut pose = Transform::at(0., 0.9, 0.);
    c.step(&mut pose, Vec3::ZERO, false, 0.01);
    c = c.ground(2.);
    assert!(c.step(&mut pose, Vec3::ZERO, false, 0.01).landed);
    assert_eq!(pose.position.y, 2.);
}
#[test]
fn character_upward_penetration_preserves_jump() {
    let mut c = character();
    let mut pose = Transform::at(0., 0.8, 0.);
    c.velocity.y = 1.;
    let contact = c.step(&mut pose, Vec3::ZERO, false, 0.01);
    assert!(!contact.landed && !contact.grounded);
    assert!(pose.position.y > 0.9 && c.velocity.y > 0.);
}
#[test]
fn character_zero_dt_queries_actual_contact_without_mutation() {
    for (y, vy, grounded) in [(0.8, 0., false), (0.9, -1., false), (0.9, 0., true)] {
        let mut c = character();
        c.velocity.y = vy;
        let mut pose = Transform::at(0., y, 0.);
        let before = (bin::to_vec(&c), pose);
        let contact = c.step(&mut pose, Vec3::X, true, 0.);
        assert_eq!(contact.grounded, grounded);
        assert!(!contact.landed && !contact.jumped);
        assert_eq!((bin::to_vec(&c), pose), before);
    }
}
