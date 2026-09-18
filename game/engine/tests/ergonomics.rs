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
    restored.restore(&s.save()).unwrap();
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
    let saved = s.save();
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
fn seeks_reuse_observation_hashes_and_live_never_hashes() {
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
    let counted = s.world_mut().spawn(Counted);
    s.advance(0.0, Clock::Seekable);
    s.advance(60_000.0, Clock::Seekable);
    assert_eq!(WRITES.swap(0, Ordering::Relaxed), 2);
    s.advance(60_000.0, Clock::Seekable);
    assert_eq!(WRITES.load(Ordering::Relaxed), 0);
    s.advance(60_017.0, Clock::Seekable);
    // The warm observation reuses its digest; the canonical stream still
    // serializes this value once for the final seek sample.
    assert_eq!(WRITES.swap(0, Ordering::Relaxed), 1);
    drop(s.world().get_mut::<Counted>(counted));
    s.advance(60_034.0, Clock::Seekable);
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
    let saved = s.save();
    let snapshot = s.agent(r#"{"op":"state","entity":"*"}"#);
    let mut restored = Sim::<Following>::new(Options::default()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(snapshot, restored.agent(r#"{"op":"state","entity":"*"}"#));
    s.run(100.0);
    restored.run(100.0);
    assert_eq!(s.save(), restored.save());
    // A newly added or teleported follower can be unplaced in a carry.
    s.world_mut().spawn_named(
        "new-camera",
        (
            Transform::default(),
            Follow::new("player").offset(0.0, 5.0, 8.0),
        ),
    );
    restored.restore(&s.save()).unwrap();
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
