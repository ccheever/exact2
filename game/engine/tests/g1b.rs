use exact_game::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Default, Resource)]
struct Countdown(u32);
#[derive(Default, Component)]
struct Envelope {
    spring: Spring,
    tween: Tween,
}
struct Motion;
impl Game for Motion {
    type Args = ();
    const ID: &'static str = "g1b-motion";
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", Transform::default());
        w.insert_resource(Countdown(0));
    }
    fn actions() -> Actions {
        Actions::new()
            .button("jump", &["Space"])
            .stick("move", Stick::wasd().or_arrows())
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut t = w.get_mut::<Transform>("player").unwrap();
        if w.tick() == 0 || input.pressed("jump") {
            t.position.y = 4.0;
        } else {
            t.position.y = (t.position.y - 0.1).max(0.0);
        }
        t.position += input.stick_xz("move");
        let mut r = w.resource_mut::<Countdown>();
        r.0 = r.0.saturating_sub(1);
    }
}
#[derive(Default, Data)]
#[allow(non_snake_case)]
struct Reply {
    quiescent: bool,
    settleAt: f64,
    changing: Vec<String>,
}
fn clock<G: Game>(s: &mut Sim<G>, settle: bool) -> Reply {
    json::from_str(&s.agent(&format!(r#"{{"op":"clock","settle":{settle}}}"#))).unwrap()
}
#[test]
fn new_live_restored_and_queued_worlds_require_an_observation() {
    let mut s = Sim::<Motion>::new(()).unwrap();
    assert!(!s.quiescent());
    assert!(!clock(&mut s, false).changing.is_empty());
    assert!(s.settle());
    assert!(s.world().tick() > 40);
    s.tap("Space");
    let now = s.world().seconds() * 1000.0;
    s.advance(now, Clock::Seekable); // zero due, the queued edge cannot be rest
    assert!(!s.quiescent());
    assert!(s.settle());
    assert_eq!(
        s.world().get::<Transform>("player").unwrap().position.y,
        0.0
    );
    s.tap("Space");
    s.advance(s.world().seconds() * 1000.0 + 200.0, Clock::Live);
    assert!(s.world().get::<Transform>("player").unwrap().position.y > 0.0);
    let saved = s.save().unwrap();
    let mut restored = Sim::<Motion>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert!(!restored.quiescent());
    assert!(restored.settle());
    assert_eq!(
        restored
            .world()
            .get::<Transform>("player")
            .unwrap()
            .position
            .y,
        0.0
    );
}
#[test]
fn observation_is_not_saved_and_resources_are_observed() {
    let mut seek = Sim::<Motion>::new(()).unwrap();
    let mut live = Sim::<Motion>::new(()).unwrap();
    seek.run(100.0);
    live.advance(0.0, Clock::Live);
    live.advance(100.0, Clock::Live);
    assert_eq!(seek.save().unwrap(), live.save().unwrap());
    assert!(seek.settle());
    seek.world_mut().resource_mut::<Countdown>().0 = 60;
    seek.run(17.0);
    let reply = clock(&mut seek, false);
    assert!(!reply.quiescent && reply.changing.iter().any(|r| r == "resource.Countdown"));
    assert!(seek.settle());
    assert_eq!(seek.world().resource::<Countdown>().0, 0);
}
#[test]
fn current_and_future_input_prevent_rest_and_bound_the_next_jump() {
    let mut s = Sim::<Motion>::new(()).unwrap();
    assert!(s.settle());
    let now: f64 = s.world().seconds() * 1000.0;
    s.input(InputEvent::Key {
        code: "Space".into(),
        down: true,
        at_ms: now + 200.0,
    });
    s.input(InputEvent::Key {
        code: "Space".into(),
        down: false,
        at_ms: now + 201.0,
    });
    assert!(!s.quiescent());
    let r = clock(&mut s, true);
    assert!(r.settleAt <= now + 217.0);
    assert!(s.settle());
    assert!(s.world().seconds() * 1000.0 > now + 700.0);
}
#[test]
fn clock_reads_do_not_back_off_and_input_bind_restore_restart_reset() {
    #[derive(Default, Args)]
    struct Options {
        seed: u32,
        #[live]
        gain: f32,
    }
    struct Busy;
    impl Game for Busy {
        type Args = Options;
        const ID: &'static str = "busy-backoff";
        fn setup(_: &mut World, _: &Options) {}
        fn tick(w: &mut World, _: &Input, _: &Options) {
            w.busy("work pending");
        }
    }
    let mut s = Sim::<Busy>::new(Options::default()).unwrap();
    for _ in 0..4 {
        assert_eq!(clock(&mut s, false).settleAt, 100.0);
    }
    assert_eq!(clock(&mut s, true).settleAt, 100.0);
    assert_eq!(clock(&mut s, true).settleAt, 200.0);
    s.tap("KeyE");
    assert!(clock(&mut s, false).settleAt <= 100.0);
    s.run(100.0);
    let r = clock(&mut s, true);
    assert!(r.changing.iter().any(|r| r == "work pending"));
    s.bind(
        &Options {
            gain: 1.0,
            ..Default::default()
        }
        .values(),
        None,
    )
    .unwrap();
    assert_eq!(clock(&mut s, false).settleAt, 200.0);
    clock(&mut s, true);
    s.bind(&Options { seed: 1, gain: 1.0 }.values(), None)
        .unwrap();
    assert_eq!(clock(&mut s, false).settleAt, 200.0);
    clock(&mut s, true);
    s.restore(&s.save().unwrap()).unwrap();
    assert_eq!(clock(&mut s, false).settleAt, 100.0);
    assert!(!s.settle());
}
#[test]
fn global_poses_and_componentless_existence_are_observed() {
    struct Scene;
    impl Game for Scene {
        type Args = ();
        const ID: &'static str = "global-observe";
        fn setup(w: &mut World, _: &()) {
            let parent = w.spawn_named("ambient-parent", (Transform::default(), Ambient));
            w.spawn_named("visible-child", (Transform::default(), Parent(parent)));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            if w.tick() == 0 {
                w.get_mut::<Transform>("ambient-parent").unwrap().position.x += 1.0;
            }
            if w.tick() == 1 {
                w.spawn_named("empty", ());
            }
            if w.tick() == 2 {
                w.despawn(w.named("empty").unwrap());
            }
        }
    }
    let mut s = Sim::<Scene>::new(()).unwrap();
    for expected in ["visible-child.global", "empty.exists", "#2.exists"] {
        s.run(16.667);
        let r = clock(&mut s, false);
        assert!(
            !r.quiescent && r.changing.iter().any(|r| r == expected),
            "{:?}",
            r.changing
        );
    }
    assert!(s.settle());
}
#[test]
fn springs_and_tweens_explain_rest_and_deadlines() {
    let mut s = Sim::<Motion>::new(()).unwrap();
    s.settle();
    let now = s.world().now();
    let mut envelope = Envelope::default();
    envelope.spring.set_target(now, 1.0);
    envelope.tween.to(now, 1.0, 0.5);
    s.world_mut().spawn_named("glow", envelope);
    let r = clock(&mut s, false);
    assert!(!r.quiescent && r.changing.iter().any(|r| r == "glow.Envelope"));
    assert!(s.settle());
    for hz in [30, 60, 120] {
        let now = Now { tick: 10, hz };
        let mut t = Tween::new(-1.0);
        t.to(now, 1.0, 0.5);
        assert_eq!(t.value(now), -1.0);
        assert!(t.moving(now));
        let deadline = t.settle_tick(now).unwrap();
        assert_eq!(deadline, 10 + hz as u64 / 2);
        assert_eq!(t.value(Now { tick: deadline, hz }), 1.0);
        let mid = Now {
            tick: 10 + hz as u64 / 4,
            hz,
        };
        let value = t.value(mid);
        t.to(mid, 0.5, 1.0);
        assert_eq!(t.value(mid), value);
        let loaded: Tween = bin::from_slice(&bin::to_vec(&t)).unwrap();
        assert_eq!(loaded.value(mid), value);
    }
}
#[test]
fn followers_handle_top_down_zero_offsets_current_parents_and_reincarnation() {
    let mut w = World::new(60, 0);
    let parent = w.spawn(Transform::default());
    let target = w.spawn_named("player", (Transform::at(2.0, 0.0, 0.0), Parent(parent)));
    w.spawn_named(
        "overhead",
        (
            Transform::default(),
            Follow::new(target).offset(0.0, 10.0, 0.0),
        ),
    );
    w.spawn_named(
        "zero",
        (Transform::default(), Follow::new("player").lag(10.0)),
    );
    w.propagate();
    w.get_mut::<Transform>(parent).unwrap().position.x = 7.0;
    scene::follow(&w);
    assert_eq!(
        w.get::<Transform>("overhead").unwrap().position,
        Vec3::new(9.0, 10.0, 0.0)
    );
    assert!(w.get::<Transform>("overhead").unwrap().rotation.is_finite());
    assert_eq!(w.get::<Transform>("zero").unwrap().position.x, 9.0);
    assert_eq!(w.get::<Transform>("zero").unwrap().rotation, Quat::IDENTITY);
    w.despawn(target);
    w.spawn_named("player", Transform::at(50.0, 0.0, 0.0));
    scene::follow(&w);
    assert_eq!(w.get::<Transform>("zero").unwrap().position.x, 50.0);
    assert!(math::ease(-f32::MAX, f32::MAX, 1.0, 1.0).is_finite());
    assert!(math::ease(Vec3::splat(-f32::MAX), Vec3::splat(f32::MAX), 1.0, 1.0).is_finite());
}
#[test]
fn named_access_guarded_rows_and_release_singleton_refusal() {
    let mut w = World::new(60, 0);
    let e = w.spawn_named("player", Transform::default());
    w.get_mut::<Transform>("player").unwrap().position.x = 2.0;
    assert_eq!(w.get::<Transform>(e).unwrap().position.x, 2.0);
    w.spawn(Transform::default());
    for mut t in w.query::<&mut Transform>() {
        t.position.y = 3.0;
    }
    let rows: Vec<_> = w.query::<&mut Transform>().into_iter().collect();
    assert_eq!(rows.len(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| w.get_mut::<Transform>(e))).is_err());
    drop(rows);
    let err = catch_unwind(AssertUnwindSafe(|| {
        w.query::<&Transform>().one();
    }))
    .unwrap_err();
    assert!(err
        .downcast_ref::<String>()
        .unwrap()
        .contains("expected one Transform, found 2"));
    assert_eq!(w.query::<&Transform>().iter().next().unwrap().0, e);
}
#[test]
fn input_sugar_and_relative_sentences_match_absolute_host_input() {
    let mut a = Sim::<Motion>::new(()).unwrap();
    let mut b = Sim::<Motion>::new(()).unwrap();
    a.hold("KeyW", 200.0);
    a.tap("Space");
    a.run(100.0);
    b.advance(0.0, Clock::Seekable);
    for (code, down, at_ms) in [
        ("KeyW", true, 0.0),
        ("KeyW", false, 200.0),
        ("Space", true, 200.0),
        ("Space", false, 200.0),
    ] {
        b.input(InputEvent::Key {
            code: code.into(),
            down,
            at_ms,
        });
    }
    b.advance(300.0, Clock::Seekable);
    assert_eq!(a.save().unwrap(), b.save().unwrap());
    a.hold("ArrowUp", 100.0);
    b.hold("KeyW", 100.0);
    assert_eq!(
        a.world().get::<Transform>("player").unwrap().position,
        b.world().get::<Transform>("player").unwrap().position
    );
}

#[test]
fn ambient_resources_are_never_serialized_by_observation() {
    #[derive(Default)]
    struct Executor;
    impl Resource for Executor {
        const NAME: &'static str = "Executor";
        const AMBIENT: bool = true;
    }
    impl Data for Executor {
        fn write(&self, _: &mut dyn Writer) {
            panic!("snapshotting an ambient executor during observation");
        }
        fn read(&mut self, _: &mut dyn Reader) -> Result<(), DataError> {
            Ok(())
        }
    }
    let mut s = Sim::<Motion>::new(()).unwrap();
    s.world_mut().insert_resource(Executor);
    assert!(s.settle());
}
