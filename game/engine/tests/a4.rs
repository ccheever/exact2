use exact_game::*;

#[derive(Default, Resource)]
struct Observed {
    held: u32,
    pressed: u32,
    released: u32,
    wheel: Vec2,
    pointer: Option<PointerState>,
    volume: f64,
}
#[derive(Default, Component)]
struct Motion(Vec<Option<Box<Spring>>>);
struct Probe;
#[derive(Default, exact_game::Args)]
struct ProbeArgs {
    /// Canvas setup argument.
    pub seed: u64,
    /// Canvas setup argument.
    pub run: u32,
    /// Canvas live argument.
    #[live]
    pub paused: bool,
    /// Canvas live argument.
    #[live]
    pub volume: f64,
}
impl Game for Probe {
    const ID: &'static str = "a4-probe";
    type Args = ProbeArgs;

    fn setup(w: &mut World, args: &Self::Args) {
        w.reseed(args.seed);
        w.insert_resource(Observed::default());
        w.spawn_named("probe", (Transform::default(), Ambient));
    }
    fn paused(args: &Self::Args) -> bool {
        args.paused
    }
    fn actions() -> Actions {
        Actions::new().button("act", &["KeyE"])
    }
    fn tick(w: &mut World, input: &Input, args: &Self::Args) {
        let mut r = w.resource_mut::<Observed>();
        r.held += u32::from(input.held("act"));
        r.pressed += u32::from(input.pressed("act"));
        r.released += u32::from(input.released("act"));
        r.wheel += input.wheel();
        r.pointer = input.pointer();
        r.volume = args.volume;
        w.get_mut::<Transform>(w.named("probe").unwrap())
            .unwrap()
            .position
            .x += 1.0;
    }
}
fn args(seed: f64, run: f64, paused: bool, volume: f64) -> [Value; 4] {
    [
        Value::Number(seed),
        Value::Number(run),
        Value::Bool(paused),
        Value::Number(volume),
    ]
}
fn sim() -> Sim<Probe> {
    let mut s = Sim::new(&args(7.0, 0.0, false, 1.0)).unwrap();
    s.advance(0.0, Clock::Seekable);
    s
}
fn key<G: Game>(s: &mut Sim<G>, down: bool, at_ms: f64) {
    s.input(InputEvent::Key {
        code: "KeyE".into(),
        down,
        at_ms,
    });
}
fn pointer(s: &mut Sim<Probe>, phase: PointerPhase, x: f32, at_ms: f64) {
    s.input(InputEvent::Pointer {
        id: 1,
        phase,
        x,
        y: 0.0,
        at_ms,
    });
}
#[allow(non_snake_case)]
#[derive(Default, Data)]
struct ClockReply {
    tick: u64,
    quiescent: bool,
    settleAt: f64,
}
fn ask(s: &mut Sim<Probe>) -> ClockReply {
    json::from_str(&s.agent(r#"{"op":"clock","settle":true}"#)).unwrap()
}

#[test]
fn a4_1_host_loop_reaches_the_spring_deadline_without_the_world_running_ahead() {
    let mut s = sim();
    let mut spring = Spring::new(0.0);
    spring.set_target(s.world().now(), 1.0);
    let expected_tick = (spring.config.settle_time(-1.0, 0.0) * 60.0).ceil() as u64;
    let expected_ms = (expected_tick * 1_000_000).div_ceil(60) as f64 / 1000.0;
    s.world_mut().spawn(Motion(vec![Some(Box::new(spring))]));
    let mut host = 0.0;
    let mut rounds = 0;
    loop {
        let reply = ask(&mut s);
        assert_eq!(reply.tick, s.world().tick());
        assert!(s.world().seconds() * 1000.0 <= host + 1e-6);
        if reply.quiescent {
            break;
        }
        assert_eq!(reply.settleAt, expected_ms);
        assert_eq!(s.world().tick(), 0, "asking must never step privately");
        host = reply.settleAt;
        s.advance(host, Clock::Seekable);
        rounds += 1;
        assert!(rounds <= 16);
    }
    assert_eq!(s.world().tick(), expected_tick);
    assert_eq!(rounds, 1);

    struct Busy;
    impl Game for Busy {
        type Args = ();
        const ID: &'static str = "busy";
        fn setup(w: &mut World, _: &Self::Args) {
            w.busy("forever");
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            w.busy("forever");
        }
    }
    let mut never = Sim::<Busy>::new(&[]).unwrap();
    never.advance(0.0, Clock::Seekable);
    let mut host = 0.0;
    for round in 0..16 {
        let reply: ClockReply =
            json::from_str(&never.agent(r#"{"op":"clock","settle":true}"#)).unwrap();
        assert!(!reply.quiescent);
        assert_eq!(
            reply.settleAt,
            host + (100u32.saturating_mul(1 << round)).min(2000) as f64
        );
        assert!(never.world().seconds() * 1000.0 <= host);
        host = reply.settleAt;
        never.advance(host, Clock::Seekable);
    }
    assert!(
        !never.quiescent(),
        "host exhausts its rounds with reason world"
    );
}

#[test]
fn a4_2_stamps_land_in_the_containing_step_and_pause_and_queue_do_not_lose_releases() {
    let mut s = sim();
    key(&mut s, true, 1.0);
    s.advance(16.667, Clock::Seekable);
    assert_eq!(s.world().resource::<Observed>().held, 1);
    // Exact 50 ms boundary belongs to the next step, not the one ending there.
    key(&mut s, false, 50.0);
    s.advance(50.0, Clock::Seekable);
    assert_eq!(s.world().resource::<Observed>().released, 0);
    s.advance(66.667, Clock::Seekable);
    assert_eq!(s.world().resource::<Observed>().released, 1);
    s.bind(&args(7.0, 0.0, true, 1.0), Some(100.0)).unwrap();
    for n in 0..5000 {
        key(&mut s, n % 2 == 0, 100.0 + n as f64);
        pointer(&mut s, PointerPhase::Move, n as f32, 100.0 + n as f64);
        s.input(InputEvent::Wheel {
            dx: 1.0,
            dy: 1.0,
            at_ms: 100.0 + n as f64,
        });
    }
    s.bind(&args(7.0, 0.0, false, 1.0), Some(6000.0)).unwrap();
    s.advance(6017.0, Clock::Seekable);
    let r = s.world().resource::<Observed>();
    assert_eq!((r.pressed, r.released), (1, 1), "no paused edges survive");
    assert_eq!(r.pointer.unwrap().position.x, 4999.0);
    assert_eq!(r.pointer.unwrap().delta, Vec2::ZERO);
    assert_eq!(r.wheel, Vec2::ZERO);
    drop(r);
    assert!(!s
        .world()
        .journal()
        .iter()
        .any(|e| e.line.contains("overflow")));

    let mut s = sim();
    pointer(&mut s, PointerPhase::Down, 0.0, 0.0);
    for n in 0..5000 {
        pointer(&mut s, PointerPhase::Move, n as f32, 1.0);
        key(&mut s, true, 1.0); // duplicate key repeats do not fill the queue
        s.input(InputEvent::Wheel {
            dx: 1.0,
            dy: 2.0,
            at_ms: 1.0,
        });
    }
    s.advance(16.667, Clock::Seekable);
    let r = s.world().resource::<Observed>();
    assert_eq!((r.held, r.pressed), (1, 1));
    assert_eq!(r.pointer.unwrap().position.x, 4999.0);
    assert_eq!(r.wheel, Vec2::new(5000.0, 10000.0));
    drop(r);
    assert!(!s
        .world()
        .journal()
        .iter()
        .any(|e| e.line.contains("overflow")));

    // Different boundaries cannot coalesce. Overflow prefers a move over the
    // oldest key press, and logs only once even when thousands are dropped.
    let mut s = sim();
    key(&mut s, true, 0.0);
    for n in 1..5000 {
        pointer(&mut s, PointerPhase::Move, n as f32, n as f64 * 20.0);
    }
    key(&mut s, false, 100_000.0);
    #[derive(Default, Data)]
    struct Pending {
        host_us: i64,
    }
    #[derive(Default, Data)]
    struct SaveQueue {
        queue: Vec<Pending>,
    }
    let saved: SaveQueue = bin::from_slice(&s.save()[7..]).unwrap();
    assert_eq!(saved.queue.len(), 1024);
    s.advance(100_017.0, Clock::Seekable);
    let r = s.world().resource::<Observed>();
    assert_eq!((r.pressed, r.released), (1, 1));
    assert_eq!(r.pointer.unwrap().position.x, 4999.0);
    let overflow: Vec<_> = s
        .world()
        .journal()
        .into_iter()
        .filter(|e| e.line.contains("overflow"))
        .collect();
    assert_eq!(overflow.len(), 1);
    assert!(overflow[0].line.contains("dropped oldest move"));
}

#[test]
fn a4_3_pause_at_500_inside_one_seek_matches_two_seeks_restart_and_refusals_are_atomic() {
    let mut one = sim();
    let mut two = sim();
    one.bind(&args(7.0, 0.0, true, 2.0), Some(500.0)).unwrap();
    one.advance(1000.0, Clock::Seekable);
    two.advance(500.0, Clock::Seekable);
    two.bind(&args(7.0, 0.0, true, 2.0), Some(500.0)).unwrap();
    two.advance(1000.0, Clock::Seekable);
    assert_eq!(one.save(), two.save());
    assert_eq!(one.world().tick(), 30);
    assert_eq!(
        one.world().resource::<Observed>().volume,
        1.0,
        "old args up to the bind"
    );
    let before = one.save();
    for bad in [args(7.0, -1.0, false, 1.0), args(-1.0, 0.0, false, 1.0)] {
        assert!(one.bind(&bad, Some(2000.0)).is_err());
        assert_eq!(
            one.save(),
            before,
            "refusal cannot even advance the clock or journal"
        );
    }
    assert!(one.bind(&[], Some(2000.0)).is_err());
    assert_eq!(one.save(), before);
    one.bind(&args(7.0, 1.0, false, 2.0), Some(1000.0)).unwrap();
    assert_eq!(one.world().tick(), 0);
    assert!(one
        .world()
        .journal()
        .last()
        .unwrap()
        .line
        .contains("world restarted: run 0 → 1"));
    one.advance(1017.0, Clock::Seekable);
    assert_eq!(one.world().resource::<Observed>().volume, 2.0);
    one.bind(&args(8.0, 1.0, false, 2.0), Some(1017.0)).unwrap();
    assert_eq!(one.world().seed(), 8);
    assert!(one
        .world()
        .journal()
        .last()
        .unwrap()
        .line
        .contains("world restarted: seed 7 → 8"));
}

#[test]
fn a4_4_save_resumes_at_zero_or_a_billion_ms_and_uses_this_games_identity_and_bindings() {
    let mut original = sim();
    key(&mut original, true, 1.0);
    key(&mut original, false, 1234.0);
    original.advance(713.123, Clock::Seekable);
    // A mapped event inside the partially completed step, plus an unmapped future one.
    pointer(&mut original, PointerPhase::Move, 9.0, 714.0);
    original.advance(715.0, Clock::Seekable);
    let save = original.save();
    original.advance(2000.0, Clock::Seekable);
    for epoch in [0.0, 1_000_000_000.0] {
        let mut restored = sim();
        restored.restore(&save).unwrap();
        assert_eq!(restored.advance(epoch, Clock::Seekable), 0);
        restored.advance(epoch + 1285.0, Clock::Seekable);
        assert_eq!(original.world().hash(), restored.world().hash());
        assert_eq!(original.alpha(), restored.alpha());
    }
    struct Other;
    impl Game for Other {
        type Args = ();
        const ID: &'static str = "other";
        fn setup(_: &mut World, _: &Self::Args) {}
        fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
    }
    let mut other = Sim::<Other>::new(&[]).unwrap();
    let error = other.restore(&save).unwrap_err().to_string();
    assert!(error.contains("a4-probe") && error.contains("other"));
    struct Updated;
    impl Game for Updated {
        const ID: &'static str = Probe::ID;
        type Args = ProbeArgs;
        const SAVE_VERSION: u32 = 2;
        fn setup(w: &mut World, a: &Self::Args) {
            Probe::setup(w, a)
        }
        fn actions() -> Actions {
            Actions::new().button("new-action", &["KeyE"])
        }
        fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
        fn migrate(w: &mut World, from: u32) {
            w.resource_mut::<Observed>().volume = from as f64 + 100.0;
        }
    }
    let mut updated = Sim::<Updated>::new(&args(7.0, 0.0, false, 1.0)).unwrap();
    updated.restore(&save).unwrap();
    assert_eq!(updated.world().resource::<Observed>().volume, 101.0);
    let state = updated.agent(r#"{"op":"state"}"#);
    assert!(state.contains("new-action") && !state.contains("\"act\""));
    assert!(state.contains("\"held\":[\"KeyE\"]"));
    let before = original.save();
    assert!(original
        .restore(&updated.save())
        .unwrap_err()
        .to_string()
        .contains("newer version 2"));
    assert_eq!(original.save(), before);
}

#[test]
fn a4_5_a_coordinate_question_is_layout_and_the_old_ninth_op_is_refused() {
    let mut s = sim();
    s.world_mut()
        .spawn((Transform::at(0.0, 0.0, 10.0), Camera::default()));
    let probe = s.world().named("probe").unwrap();
    s.world_mut().insert(probe, Mesh::cube(1.0));
    s.world_mut().propagate();
    let reply = s.agent(r#"{"op":"layout","x":400,"y":300,"width":800,"height":600}"#);
    assert!(
        reply.contains("\"hit\":{\"id\":0,\"name\":\"probe\""),
        "{reply}"
    );
    let old = ["pi", "ck"].concat();
    assert!(s
        .agent(&format!(r#"{{"op":"{old}","x":400,"y":300}}"#))
        .contains("unknown op"));
}

#[test]
fn a4_6_names_schema_journal_and_cylinder_hits_follow_the_declared_contract() {
    let mut s = sim();
    let named = s.world_mut().spawn_named("phase#boss", ());
    let hash_name = s.world_mut().spawn_named("probe#0", ());
    assert_eq!(s.world().resolve("phase#boss"), Some(named));
    assert_eq!(s.world().resolve("probe#0"), Some(hash_name));
    assert_eq!(s.world().resolve("#0"), s.world().named("probe"));
    assert!(s.agent(r#"{"op":"tree"}"#).contains("\"tags\":[]"));
    let state = s.agent(r#"{"op":"state"}"#);
    assert!(state.contains("\"audio\":{\"voices\":[],\"sources\":[]}"));
    assert!(state.contains("\"args\":{\"seed\":7,\"run\":0,\"paused\":false,\"volume\":1}"));
    s.advance(1500.0, Clock::Seekable);
    s.world().log("test event");
    assert!(s
        .agent(r#"{"op":"logs"}"#)
        .contains("t=1500 tick=90 test event"));
    let camera = s.world_mut().spawn((
        Transform::at(0.9, 10.0, 0.9).looking_at(Vec3::new(0.9, 0.0, 0.9), Vec3::Z),
        Camera::default(),
    ));
    s.world_mut()
        .spawn_named("cylinder", (Transform::default(), Mesh::cylinder(1.0, 1.0)));
    s.world_mut().propagate();
    let request = r#"{"op":"layout","x":400,"y":300,"width":800,"height":600}"#;
    assert!(
        s.agent(request).contains("\"hit\":null"),
        "box corner is outside the circular cap"
    );
    for (pose, distance) in [
        (
            Transform::at(0.0, 10.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
            "9.5",
        ),
        (Transform::at(0.0, 0.0, 10.0), "9"),
        (Transform::default().looking_at(Vec3::Y, Vec3::Z), "0.5"),
    ] {
        s.world_mut().teleport(camera, pose);
        assert!(s
            .agent(request)
            .contains(&format!("\"distance\":{distance}")));
    }
}

#[test]
fn a4_7_blur_removes_hover_paused_springs_settle_and_a_hitch_collapses_input() {
    let mut s = sim();
    pointer(&mut s, PointerPhase::Down, 2.0, 0.0);
    key(&mut s, true, 0.0);
    s.advance(17.0, Clock::Seekable);
    s.input(InputEvent::Blur { at_ms: 18.0 });
    s.advance(34.0, Clock::Seekable);
    assert!(s.world().resource::<Observed>().pointer.is_none());
    assert_eq!(s.world().resource::<Observed>().released, 1);
    let mut spring = Spring::new(0.0);
    spring.set_target(s.world().now(), 1.0);
    s.world_mut().spawn(Motion(vec![Some(Box::new(spring))]));
    s.bind(&args(7.0, 0.0, true, 1.0), None).unwrap();
    assert!(ask(&mut s).quiescent);
    let mut s = sim();
    s.advance(1.0, Clock::Seekable); // nonzero fractional remainder at the hitch
    key(&mut s, true, 900.0);
    let mut first = None;
    assert_eq!(
        s.advance_with(1001.0, Clock::Live, |w, _| {
            first.get_or_insert(w.resource::<Observed>().held);
        }),
        15
    );
    assert_eq!(
        first,
        Some(1),
        "tail input is present in the first step after a hitch"
    );
    assert_eq!(s.world().resource::<Observed>().held, 15);
}

#[test]
fn a4_8_renderer_observes_completed_propagated_ticks_and_can_upload_only_the_last_two() {
    let mut s = sim();
    let mut uploads = vec![];
    let mut callbacks = 0;
    let count = s.advance_with(100.0, Clock::Seekable, |w, left| {
        callbacks += 1;
        assert_eq!(w.tick() + left as u64, 6);
        let e = w.named("probe").unwrap();
        assert_eq!(w.global(e).unwrap().translation.x, w.tick() as f32);
        if left < 2 {
            uploads.push(w.tick());
        }
    });
    assert_eq!((count, callbacks), (6, 6));
    assert_eq!(uploads, [5, 6]);
    s.advance_with(100.0, Clock::Seekable, |_, _| panic!("no completed tick"));
}

#[test]
fn setup_rebuild_clears_restored_status() {
    let mut s = sim();
    s.restore(&s.save()).unwrap();
    assert!(s.agent(r#"{"op":"state"}"#).contains("\"restored\":true"));
    let values = args(999.0, 1.0, false, 1.0);
    s.bind(&values, None).unwrap();
    assert!(s.agent(r#"{"op":"state"}"#).contains("\"restored\":false"));
}
