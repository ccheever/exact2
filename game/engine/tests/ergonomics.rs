use exact_game::*;

#[derive(Args, Default, Debug)]
struct Settings {
    seed: u64,
    run: u32,
    signed: i64,
    small: i32,
    gain: f32,
    precise: f64,
    title: String,
    #[live]
    paused: bool,
}
#[test]
fn typed_fields_decode_in_order_and_refuse_by_name() {
    let values = [
        Value::Number(7.0),
        Value::Number(2.0),
        Value::Number(-5.0),
        Value::Number(-3.0),
        Value::Number(0.5),
        Value::Number(0.25),
        Value::str("hello"),
        Value::Bool(false),
    ];
    let a = Settings::decode(&values).unwrap();
    assert_eq!(
        (
            a.seed,
            a.run,
            a.signed,
            a.small,
            a.gain,
            a.precise,
            a.title.as_str()
        ),
        (7, 2, -5, -3, 0.5, 0.25, "hello")
    );
    let mut b = Settings::decode(&values).unwrap();
    b.paused = true;
    assert!(!a.setup_changed(&b));
    b.run += 1;
    assert!(a.setup_changed(&b));
    assert_eq!(Settings::FIELDS[7], ("paused", ArgumentKind::Live));
    for (i, bad, expected) in [
        (0, Value::str("x"), "whole number ≥ 0"),
        (1, Value::Number(4294967296.0), "whole number"),
        (2, Value::Number(1.5), "whole number"),
        (3, Value::Number(-2147483649.0), "whole number"),
        (4, Value::Number(f64::MAX), "finite f32"),
        (5, Value::Bool(true), "finite number"),
        (6, Value::Bool(false), "text"),
        (7, Value::Number(0.0), "boolean"),
    ] {
        let mut bad_values = values.clone();
        bad_values[i] = bad;
        let error = Settings::decode(&bad_values).unwrap_err();
        assert!(
            error.starts_with(Settings::FIELDS[i].0)
                && error.contains(expected)
                && error.contains("got"),
            "{error}"
        );
        assert!(Settings::decode(&values[..i])
            .unwrap_err()
            .contains(Settings::FIELDS[i].0));
    }
    assert!(<() as Args>::decode(&[Value::Bool(false)]).is_err());
}
#[derive(Default, Data)]
struct Bookkeeping(u64);
impl Resource for Bookkeeping {
    const NAME: &'static str = "Bookkeeping";
    const AMBIENT: bool = true;
}
struct Moving;
impl Game for Moving {
    type Args = ();
    const ID: &'static str = "observed";
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", Transform::default());
        w.spawn_named("bird", (Transform::default(), Ambient));
        w.insert_resource(Bookkeeping::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        for (_, t) in w.query::<&mut Transform>().iter() {
            // Stops after tick 3; the ambient bird continues forever.
            if w.tick() < 3 || t.position.y == 1.0 {
                t.position.x += 1.0;
            }
        }
        w.resource_mut::<Bookkeeping>().0 += 1;
    }
}
#[test]
fn last_tick_observations_zero_ticks_ambient_resources_and_saves() {
    let mut s = Sim::<Moving>::new(()).unwrap();
    let bird = s.world().named("bird").unwrap();
    s.world().get_mut::<Transform>(bird).unwrap().position.y = 1.0;
    s.advance(0.0, Clock::Seekable);
    s.advance(17.0, Clock::Seekable);
    assert!(!s.quiescent());
    let reply = s.agent(r#"{"op":"clock"}"#);
    assert!(
        reply.contains(r#""changing":["player.Transform"]"#),
        "{reply}"
    );
    s.advance(17.0, Clock::Seekable);
    assert!(!s.quiescent());
    let mut restored = Sim::<Moving>::new(()).unwrap();
    restored.restore(&s.save().unwrap()).unwrap();
    assert!(!restored.quiescent());
    s.advance(1000.0, Clock::Seekable);
    assert!(s.quiescent(), "the last tick, not the whole seek, is still");
    assert!(s.agent(r#"{"op":"clock"}"#).contains(r#""changing":[]"#));
    // busy is legal while holding a component borrow.
    let pose = s.world().get_mut::<Transform>(bird).unwrap();
    s.world().busy("pending work");
    drop(pose);
    assert!(!s.quiescent());
    assert!(s.agent(r#"{"op":"clock"}"#).contains("1100"));
    assert!(s.agent(r#"{"op":"clock"}"#).contains("1100"));
}
#[test]
fn follow_arrives_snaps_teleports_and_survives_save() {
    let mut s = Sim::<Moving>::new(()).unwrap();
    let target = s.world().named("player").unwrap();
    let camera = s.world_mut().spawn_named(
        "camera",
        (
            Transform::default(),
            Camera::default(),
            Follow::new("player").offset(0.0, 9.0, 13.0).lag(0.15),
        ),
    );
    scene::follow(s.world());
    assert_eq!(
        s.world().get::<Transform>(camera).unwrap().position,
        Vec3::new(0.0, 9.0, 13.0)
    );
    s.world().get_mut::<Transform>(target).unwrap().position.x = 2.0;
    scene::follow(s.world());
    let x = s.world().get::<Transform>(camera).unwrap().position.x;
    assert!(x > 0.0 && x < 2.0);
    // A world load registers Follow even when the current setup has no camera.
    let saved = s.save().unwrap();
    let mut other = Sim::<Moving>::new(()).unwrap();
    other.restore(&saved).unwrap();
    for _ in 0..200 {
        scene::follow(s.world());
        scene::follow(other.world());
    }
    assert_eq!(s.world().hash(), other.world().hash());
    assert_eq!(s.world().get::<Transform>(camera).unwrap().position.x, 2.0);
    s.world_mut()
        .teleport(target, Transform::at(50.0, 0.0, 0.0));
    scene::follow(s.world());
    assert_eq!(
        s.world().get::<Transform>(camera).unwrap().position,
        Vec3::new(50.0, 9.0, 13.0)
    );
    let mut scalar = 0.0;
    for _ in 0..200 {
        scalar = math::ease(scalar, 1.0, 0.15, 1.0 / 60.0);
    }
    assert_eq!(scalar, 1.0);
}
#[test]
fn wildcard_state_is_complete_bounded_and_narrowable() {
    let mut s = Sim::<Moving>::new(()).unwrap();
    let parent = s.world().named("player").unwrap();
    s.world_mut()
        .spawn_named("child", (Parent(parent), Transform::at(1.0, 2.0, 3.0)));
    let reply = s.agent(r#"{"op":"state","entity":"*","under":"player"}"#);
    assert_eq!(reply.matches("\"components\":").count(), 2);
    assert!(reply.contains(&format!("\"hash\":\"0x{:016x}\"", s.world().hash())));
    assert!(reply.contains("child") && !reply.contains("bird"));
    for _ in 0..520 {
        s.world_mut().spawn(Transform::default());
    }
    let reply = s.agent(r#"{"op":"state","entity":"*"}"#);
    assert_eq!(reply.matches("\"components\":").count(), 512);
    assert!(reply.contains(r#""truncated":true"#));
}

#[test]
fn seeks_hash_only_twice_and_live_never_hashes() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static WRITES: AtomicUsize = AtomicUsize::new(0);
    #[derive(Default)]
    struct Counted;
    impl Data for Counted {
        fn write(&self, w: &mut dyn Writer) {
            WRITES.fetch_add(1, Ordering::Relaxed);
            0u32.write(w);
        }
        fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
            0u32.read(r)
        }
    }
    impl Component for Counted {
        const NAME: &'static str = "Counted";
    }
    let mut s = Sim::<Moving>::new(()).unwrap();
    s.world_mut().spawn(Counted);
    s.advance(0.0, Clock::Seekable);
    s.advance(60_000.0, Clock::Seekable);
    assert_eq!(WRITES.swap(0, Ordering::Relaxed), 2);
    s.advance(60_000.0, Clock::Seekable);
    assert_eq!(WRITES.load(Ordering::Relaxed), 0);
    s.advance(60_017.0, Clock::Seekable);
    assert_eq!(WRITES.swap(0, Ordering::Relaxed), 2);
    s.advance(60_100.0, Clock::Live);
    assert_eq!(WRITES.load(Ordering::Relaxed), 0);
}

#[test]
fn engine_places_followers_after_setup_rebuild_and_restore_without_an_extra_tick() {
    #[derive(Default, Args)]
    struct Options {
        x: f32,
    }
    struct Following;
    impl Game for Following {
        const ID: &'static str = "placement";
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            w.spawn_named("player", Transform::at(args.x, 2.0, 3.0));
            w.spawn_named(
                "camera",
                (
                    Transform::default(),
                    Follow::new("player").offset(0.0, 9.0, 13.0).lag(0.15),
                ),
            );
        }
        fn tick(w: &mut World, _: &Input, _: &Options) {
            w.get_mut::<Transform>("player").unwrap().position.x += 1.0;
            scene::follow(w);
        }
    }
    let mut s = Sim::<Following>::new(Options { x: 4.0 }).unwrap();
    assert_eq!(
        s.world().get::<Transform>("camera").unwrap().position,
        Vec3::new(4.0, 11.0, 16.0)
    );
    s.bind(&[Value::Number(8.0)], None).unwrap();
    assert_eq!(
        s.world().get::<Transform>("camera").unwrap().position,
        Vec3::new(8.0, 11.0, 16.0)
    );
    s.run(100.0);
    let saved = s.save().unwrap();
    let snapshot = s.agent(r#"{"op":"state","entity":"*"}"#);
    let mut restored = Sim::<Following>::new(Options::default()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(snapshot, restored.agent(r#"{"op":"state","entity":"*"}"#));
    s.run(100.0);
    restored.run(100.0);
    assert_eq!(s.save().unwrap(), restored.save().unwrap());
    // A newly added or teleported follower can be unplaced in a carry.
    s.world_mut().spawn_named(
        "new-camera",
        (
            Transform::default(),
            Follow::new("player").offset(0.0, 5.0, 8.0),
        ),
    );
    restored.restore(&s.save().unwrap()).unwrap();
    let player = restored
        .world()
        .get::<Transform>("player")
        .unwrap()
        .position;
    assert_eq!(
        restored
            .world()
            .get::<Transform>("new-camera")
            .unwrap()
            .position,
        player + Vec3::new(0.0, 5.0, 8.0)
    );
}

#[test]
fn record_publication_keeps_names_nested_values_and_saved_state() {
    #[derive(Default, Data)]
    struct Detail {
        label: String,
        active: bool,
    }
    #[derive(Default, Data)]
    struct Hud {
        beacons: u32,
        detail: Detail,
        rows: Vec<Detail>,
        optional: Option<u32>,
        values: Vec<u32>,
    }
    let hud = Hud {
        beacons: 2,
        detail: Detail {
            label: "two".into(),
            active: true,
        },
        rows: vec![Detail::default()],
        optional: Some(4),
        values: vec![1, 3],
    };
    let mut sim = Sim::<Moving>::new(()).unwrap();
    sim.world().publish_record(&hud);
    let expected = r#"{"beacons":2,"detail":{"active":true,"label":"two"},"optional":4,"rows":[{"active":false,"label":""}],"values":[1,3]}"#;
    assert_eq!(sim.take_published().as_deref(), Some(expected));
    sim.world().publish_record(&hud);
    assert!(sim.take_published().is_none());
    sim.world().publish("beacons", 3);
    assert_eq!(
        sim.world().published("beacons").unwrap().as_number(),
        Some(3.0)
    );
    let saved = sim.save().unwrap();
    let mut restored = Sim::<Moving>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(sim.take_published(), restored.take_published());
    assert_eq!(saved, restored.save().unwrap());
}

#[test]
fn proximity_uses_both_parent_chains_and_returns_mutable_global_rows() {
    #[derive(Default, Component)]
    struct Beacon {
        lit: bool,
    }
    let mut w = World::new(60, 0);
    let parent = w.spawn(Transform::at(10., 4., 0.));
    let player = w.spawn_named("player", (Parent(parent), Transform::at(2., 0., 0.)));
    let other_parent = w.spawn(Transform::at(11., 4., 0.));
    let close = w.spawn((
        Parent(other_parent),
        Transform::at(2., 0., 0.),
        Beacon::default(),
    ));
    let high = w.spawn((Transform::at(12., 100., 0.), Beacon::default()));
    w.spawn((Transform::at(2., 0., 0.), Beacon::default()));
    let rows: Vec<_> = w.near::<Beacon>(player, 1.).collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, close);
    assert_eq!(rows[0].1.position, Vec3::new(13., 4., 0.));
    let mut ids = Vec::new();
    for (e, _) in w.near_xz::<Beacon>("player", 1.) {
        w.get_mut::<Beacon>(e).unwrap().lit = true;
        ids.push(e);
    }
    assert_eq!(ids, [close, high]);
    assert_eq!(w.near::<Beacon>("missing", 1.).count(), 0);
    w.get_mut::<Transform>(parent).unwrap().position.x = 50.;
    assert_eq!(w.near_xz::<Beacon>(player, 1.).count(), 0);
}
