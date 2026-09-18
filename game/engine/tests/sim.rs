use exact_game::*;

#[derive(Default, Resource)]
struct Counts {
    held: u32,
    pressed: u32,
    released: u32,
    distance: Vec2,
    pointer: Vec2,
    wheel: Vec2,
}
struct Counter;
#[derive(Default, exact_game::Args)]
struct CounterArgs {
    /// Canvas live argument.
    #[live]
    pub paused: bool,
}
impl Game for Counter {
    const ID: &'static str = "Counter";
    type Args = CounterArgs;

    fn setup(w: &mut World, _: &Self::Args) {
        w.insert_resource(Counts::default());
        w.spawn_named("counter", (Transform::default(), Ambient));
    }
    fn actions() -> Actions {
        Actions::new()
            .button("act", &["KeyE", "Enter"])
            .button_touch("act", Region::Right)
            .stick(
                "move",
                Stick::keys("KeyW", "KeyS", "KeyA", "KeyD").or_touch(Region::Left),
            )
    }
    fn paused(args: &Self::Args) -> bool {
        args.paused
    }
    fn tick(w: &mut World, i: &Input, _: &Self::Args) {
        let mut c = w.resource_mut::<Counts>();
        c.held += u32::from(i.held("act"));
        c.pressed += u32::from(i.pressed("act"));
        c.released += u32::from(i.released("act"));
        c.distance += i.stick("move");
        c.wheel += i.wheel();
        if let Some(p) = i.pointer() {
            c.pointer += p.delta;
        }
        w.get_mut::<Transform>(w.named("counter").unwrap())
            .unwrap()
            .position
            .x += 1.0;
    }
}
fn sim() -> Sim<Counter> {
    let mut s = Sim::new(CounterArgs::default()).unwrap();
    s.advance(0.0, Clock::Seekable);
    s
}
fn key(s: &mut Sim<Counter>, code: &str, down: bool, at_ms: f64) {
    s.input(InputEvent::Key {
        code: code.into(),
        down,
        at_ms,
    });
}
#[test]
fn integer_seek_partitions_and_fraction() {
    let mut a = sim();
    let mut b = sim();
    let mut c = sim();
    for s in [&mut a, &mut b, &mut c] {
        key(s, "KeyW", true, 0.0);
        key(s, "KeyE", true, 134.123);
        key(s, "KeyE", false, 721.987);
    }
    a.advance(1000.0, Clock::Seekable);
    for ms in 1..=1000 {
        b.advance(ms as f64, Clock::Seekable);
    }
    for ms in [0.123, 2.007, 15.1, 100.0, 501.3, 789.999, 999.001, 1000.0] {
        c.advance(ms, Clock::Seekable);
    }
    assert_eq!(a.world().hash(), b.world().hash());
    assert_eq!(a.world().hash(), c.world().hash());
    assert_eq!(a.world().tick(), 60);
    assert_eq!(a.alpha(), 0.0);
    a.advance(1008.333, Clock::Seekable);
    assert!((a.alpha() - 0.49998).abs() < 1e-5);
    let e = a.world().named("counter").unwrap();
    assert_eq!(a.world().global(e).unwrap().translation.x, 60.0);
}
#[test]
fn exact_event_boundary_tap_repeat_alias_and_blur() {
    let mut s = sim();
    key(&mut s, "KeyE", true, 1.0);
    key(&mut s, "KeyE", false, 2.0);
    s.advance(16.667, Clock::Seekable);
    assert_eq!(s.world().resource::<Counts>().pressed, 1);
    s.advance(33.334, Clock::Seekable);
    {
        let c = s.world().resource::<Counts>();
        assert_eq!((c.held, c.pressed, c.released), (0, 1, 1));
    }
    key(&mut s, "KeyE", true, 40.0);
    key(&mut s, "KeyE", true, 41.0);
    key(&mut s, "Enter", true, 42.0);
    key(&mut s, "KeyE", false, 43.0);
    s.advance(100.0, Clock::Seekable);
    {
        let c = s.world().resource::<Counts>();
        assert_eq!((c.pressed, c.released), (2, 1));
    }
    s.input(InputEvent::Blur { at_ms: 100.0 });
    s.advance(116.667, Clock::Seekable);
    assert_eq!(s.world().resource::<Counts>().released, 2);
    // 60 Hz's rational boundary at 16,666 2/3 us is not rounded down.
    let mut before = sim();
    key(&mut before, "KeyE", true, 16.666);
    before.advance(16.667, Clock::Seekable);
    assert_eq!(before.world().resource::<Counts>().pressed, 1);
    let mut after = sim();
    key(&mut after, "KeyE", true, 16.667);
    after.advance(16.667, Clock::Seekable);
    assert_eq!(after.world().resource::<Counts>().pressed, 0);
    after.advance(33.334, Clock::Seekable);
    assert_eq!(after.world().resource::<Counts>().pressed, 1);
}
#[test]
fn epoch_pause_live_gap_and_backwards_clock() {
    let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
    assert_eq!(s.advance(5000.0, Clock::Seekable), 0);
    key(&mut s, "KeyE", true, 5100.0);
    assert_eq!(s.advance(6000.0, Clock::Live), 15);
    assert_eq!(s.advance(5500.0, Clock::Seekable), 0);
    assert_eq!(s.advance(6000.0, Clock::Seekable), 0);
    s.bind(&CounterArgs { paused: true }.values(), None)
        .unwrap();
    key(&mut s, "KeyE", false, 6500.0);
    s.advance(7000.0, Clock::Seekable);
    assert_eq!(s.world().tick(), 15);
    s.bind(&CounterArgs::default().values(), None).unwrap();
    s.advance(7100.0, Clock::Seekable);
    let c = s.world().resource::<Counts>();
    assert_eq!(c.released, 0);
    assert_eq!(s.world().tick(), 21);
}

#[test]
fn controlled_restore_anchors_destination_clock_before_first_advance() {
    for bound in [false, true] {
        let mut original = sim();
        key(&mut original, "KeyW", true, 0.0);
        original.run(500.0);
        key(&mut original, "KeyE", true, 750.0);
        let saved = original.save();
        original.run(500.0);

        let mut restored = sim();
        // The destination's epoch differs from the checkpoint's host epoch.
        restored.agent(r#"{"op":"clock","owner":"agent","now":9000}"#);
        if bound {
            restored.restore_bound(&saved).unwrap();
        } else {
            restored.restore(&saved).unwrap();
        }
        assert_eq!(restored.world().tick(), 30);
        assert_eq!(restored.save(), saved);
        let state = restored.agent(r#"{"op":"state"}"#);
        assert!(state.contains(r#""hostMicros":9000000"#), "{state}");
        assert!(state.contains(r#""owner":"agent""#), "{state}");
        for op in ["state", "tree", "logs", "layout"] {
            let entity = if op == "layout" {
                r#", "entity":"counter""#
            } else {
                ""
            };
            let reply = restored.agent(&format!(r#"{{"op":"{op}","now":999999{entity}}}"#));
            assert!(!reply.contains("error"), "{reply}");
            assert_eq!(restored.save(), saved, "inspection must remain read-only");
        }
        assert_eq!(restored.advance(9500.0, Clock::Seekable), 30);
        assert_eq!(restored.world().tick(), 60);
        assert_eq!(restored.world().resource::<Counts>().pressed, 1);
        assert_eq!(
            restored.save(),
            original.save(),
            "held and future input survive"
        );
        let before = restored.save();
        assert!(restored.restore(b"invalid").is_err());
        assert_eq!(restored.save(), before);
        assert_eq!(restored.advance(9600.0, Clock::Seekable), 6);
    }
}

#[test]
fn live_restore_rebases_without_counting_paused_wall_time() {
    for bound in [false, true] {
        let mut original = sim();
        original.run(500.0);
        key(&mut original, "KeyE", true, 550.0);
        let saved = original.save();
        original.run(100.0);

        let mut restored = sim();
        restored.advance(100.0, Clock::Live);
        if bound {
            restored.restore_bound(&saved).unwrap();
        } else {
            restored.restore(&saved).unwrap();
        }
        let state = restored.agent(r#"{"op":"state","now":999999}"#);
        assert!(state.contains(r#""hostMicros":null"#), "{state}");
        assert!(state.contains(r#""owner":"human""#), "{state}");
        assert_eq!(restored.world().tick(), 30);
        assert_eq!(restored.save(), saved);
        assert_eq!(restored.advance(900_000.0, Clock::Live), 0);
        assert_eq!(restored.world().tick(), 30, "paused wall time is discarded");
        assert_eq!(restored.advance(900_100.0, Clock::Live), 6);
        assert_eq!(restored.world().tick(), 36);
        assert_eq!(restored.save(), original.save());
    }
}
#[test]
fn touch_is_data_and_pointer_wheel_deltas_expire() {
    let mut s = sim();
    s.viewport(800.0, 600.0);
    for (id, phase, x, y) in [
        (1, PointerPhase::Down, 100.0, 300.0),
        (1, PointerPhase::Move, 160.0, 240.0),
        (2, PointerPhase::Down, 700.0, 300.0),
    ] {
        s.input(InputEvent::Pointer {
            id,
            phase,
            x,
            y,
            at_ms: 0.0,
        });
    }
    s.input(InputEvent::Wheel {
        dx: 2.0,
        dy: -3.0,
        at_ms: 0.0,
    });
    s.advance(16.667, Clock::Seekable);
    {
        let c = s.world().resource::<Counts>();
        assert!((c.distance.length() - 1.0).abs() < 1e-6);
        assert_eq!(c.pointer, Vec2::new(60.0, -60.0));
        assert_eq!(c.wheel, Vec2::new(2.0, -3.0));
        assert_eq!(c.pressed, 1);
    }
    s.advance(33.334, Clock::Seekable);
    assert_eq!(
        s.world().resource::<Counts>().pointer,
        Vec2::new(60.0, -60.0)
    );
    assert!(s.agent(r#"{"op":"state"}"#).contains("Right"));
    s.input(InputEvent::Blur { at_ms: 34.0 });
    s.advance(100.0, Clock::Seekable);
    assert_eq!(s.world().resource::<Counts>().released, 1);
}
#[derive(Default, Component)]
struct Nested {
    springs: Vec<Option<Box<Spring>>>,
}
#[test]
fn settle_finds_nested_springs_and_busy_is_per_tick() {
    let mut s = sim();
    let mut spring = Spring::new(0.0);
    spring.set_target(s.world().now(), 1.0);
    s.world_mut().spawn(Nested {
        springs: vec![Some(Box::new(spring))],
    });
    assert!(!s.quiescent());
    settle_host(&mut s);
    assert!(s.quiescent());
    s.world_mut().busy("one step");
    assert!(!s.quiescent());
    settle_host(&mut s);
    assert!(s.quiescent());
}
#[test]
fn publication_drain_is_full_and_journal_is_an_indexed_ring() {
    let mut s = sim();
    s.world().publish("a", 2);
    s.world().publish("b", false);
    assert_eq!(s.take_published().as_deref(), Some("{\"a\":2,\"b\":false}"));
    s.world().publish("a", 2);
    assert!(s.take_published().is_none());
    s.world().publish("b", true);
    assert_eq!(s.take_published().as_deref(), Some("{\"a\":2,\"b\":true}"));
    for i in 0..5000 {
        s.world().log(format_args!("line {i}"));
    }
    let next = s.world().journal_next();
    assert_eq!(s.world().journal().len(), 4096);
    let reply = s.agent(r#"{"op":"logs","since":0}"#);
    assert!(reply.contains(&format!("\"from\":{}", next - 4096)));
    assert_eq!(
        s.agent(&format!("{{\"op\":\"logs\",\"since\":{next}}}")),
        format!("{{\"tick\":0,\"next\":{next},\"from\":{next},\"lines\":[]}}")
    );
}
#[test]
fn named_refusals_and_reads_do_not_change_the_hash() {
    assert!(Sim::<Counter>::from_values(&[])
        .err()
        .unwrap()
        .contains("paused"));
    assert!(Sim::<Counter>::from_values(&[Value::Number(1.0)])
        .err()
        .unwrap()
        .contains("paused"));
    let mut s = sim();
    let hash = s.world().hash();
    for q in [
        r#"{"op":"tree"}"#,
        r#"{"op":"state","world":true}"#,
        r#"{"op":"layout","entity":"counter"}"#,
        r#"{"op":"missing"}"#,
        r#"{"op":"state","entity":"nope"}"#,
    ] {
        s.agent(q);
    }
    assert_eq!(s.world().hash(), hash);
    assert!(s
        .agent(r#"{"op":"layout","entity":"counter"}"#)
        .contains("\"unavailable\":true"));
}
#[test]
#[should_panic(expected = "unknown action `typo`")]
fn misspelled_actions_are_not_silent() {
    Input::default().held("typo");
}

#[test]
fn a_late_event_cannot_block_an_earlier_pending_boundary() {
    let mut s = sim();
    key(&mut s, "KeyE", true, 10.0);
    s.advance(24.0, Clock::Seekable);
    key(&mut s, "KeyE", false, 0.0); // late delivery maps to world 24 ms
    s.advance(33.334, Clock::Seekable);
    {
        let c = s.world().resource::<Counts>();
        assert_eq!((c.pressed, c.released), (1, 1));
    }
    s.advance(50.0, Clock::Seekable);
    assert_eq!(s.world().resource::<Counts>().released, 1);
}
#[test]
fn settle_is_bounded_for_continuous_game_work() {
    let mut s = sim();
    #[derive(Default, Component)]
    struct Never {
        spring: Spring,
    }
    let mut spring = Spring {
        config: SpringConfig {
            stiffness: 1.0,
            damping: 0.0,
            mass: 1.0,
        },
        ..Spring::new(0.0)
    };
    spring.set_target(s.world().now(), 1.0);
    s.world_mut().spawn(Never { spring });
    let reply = s.agent(r#"{"op":"clock","settle":true}"#);
    assert_eq!(s.world().tick(), 0);
    assert!(reply.contains("\"quiescent\":false"));
}

fn settle_host(s: &mut Sim<Counter>) {
    #[allow(non_snake_case)]
    #[derive(Default, Data)]
    struct Reply {
        quiescent: bool,
        settleAt: f64,
    }
    for _ in 0..16 {
        let reply: Reply = json::from_str(&s.agent(r#"{"op":"clock","settle":true}"#)).unwrap();
        if reply.quiescent {
            return;
        }
        s.advance(reply.settleAt, Clock::Seekable);
    }
    panic!("world did not settle");
}

#[test]
fn explicit_events_are_saved_in_order_and_publication_is_separate() {
    let mut s = sim();
    let empty_save = s.world().save();
    let empty_hash = s.world().hash();
    assert_eq!(s.take_published().as_deref(), Some("{}"));
    s.world().publish("count", 4);
    s.world().emit("first");
    s.world().emit("second");
    assert_eq!(s.world().hash(), empty_hash);
    let saved = s.save();
    let mut restored = sim();
    restored.restore(&saved).unwrap();
    assert_eq!(s.take_messages(), ["first", "second"]);
    assert_eq!(restored.take_messages(), ["first", "second"]);
    assert!(restored.take_messages().is_empty());
    assert_eq!(
        s.world().save(),
        empty_save,
        "empty event queue adds no save bytes"
    );
    assert_eq!(s.world().hash(), empty_hash);
    assert_eq!(restored.take_published().as_deref(), Some("{\"count\":4}"));
    assert_eq!(s.take_published(), Some("{\"count\":4}".into()));
    let saved = s.save(); // publication was already delivered before this save
    restored.restore(&saved).unwrap();
    assert_eq!(restored.take_published().as_deref(), Some("{\"count\":4}"));
    assert!(restored.take_published().is_none());
    assert_eq!(
        restored.save(),
        saved,
        "delivery cursor is not simulation state"
    );
}

#[test]
fn overflow_warning_is_saved_behavior() {
    let mut a = sim();
    for i in 0..1030 {
        key(&mut a, "KeyE", i % 2 == 0, 0.0);
    }
    let mut b = sim();
    b.restore(&a.save()).unwrap();
    b.advance(0.0, Clock::Seekable);
    for i in 1030..1040 {
        key(&mut a, "KeyE", i % 2 == 0, 0.0);
        key(&mut b, "KeyE", i % 2 == 0, 0.0);
    }
    assert_eq!(a.save(), b.save());
}

#[test]
fn older_saves_default_the_overflow_warning_flag() {
    let s = sim();
    let mut bytes = s.save();
    let field = b"\x01\x00\x0foverflow_logged";
    let at = bytes.windows(field.len()).position(|v| v == field).unwrap();
    bytes.drain(at..at + field.len() + 1);
    let mut restored = sim();
    restored.restore(&bytes).unwrap();
    assert_eq!(s.save(), restored.save());
}

#[test]
fn forwarded_keys_include_pending_downs_and_releases_without_changing_tick_held_state() {
    let mut s = sim();
    key(&mut s, "KeyE", true, 0.0);
    let state = s.agent(r#"{"op":"state"}"#);
    assert!(state.contains(r#""held":[],"forwarded":["KeyE"]"#));
    key(&mut s, "KeyE", false, 0.0);
    assert!(s.agent(r#"{"op":"state"}"#).contains(r#""forwarded":[]"#));
}

#[test]
fn delivery_is_saved_but_never_changes_simulation_hash() {
    let mut a = sim();
    let before = a.world().hash();
    let epoch = a.world().mutation_epoch();
    a.world().emit("first");
    a.world().emit("second");
    assert_eq!(before, a.world().hash());
    assert_eq!(epoch, a.world().mutation_epoch());
    let saved = a.save();
    let mut b = sim();
    b.restore(&saved).unwrap();
    assert_eq!(b.take_messages(), ["first", "second"]);
    assert_eq!(a.world().hash(), b.world().hash());
    a.run(100.0);
    b.run(100.0);
    assert_eq!(a.world().hash(), b.world().hash()); // observation-prefix hash path too
    let epoch = a.world().mutation_epoch();
    assert_eq!(a.take_messages(), ["first", "second"]);
    assert_eq!(epoch, a.world().mutation_epoch());
    assert_eq!(a.world().hash(), b.world().hash());
}

#[test]
fn restore_touch_viewport_continues_headless_and_resize_replaces_it() {
    let mut a = sim();
    a.viewport(800.0, 600.0);
    a.input(InputEvent::Pointer {
        id: 7,
        phase: PointerPhase::Down,
        x: 700.0,
        y: 300.0,
        at_ms: 0.0,
    });
    a.run(100.0);
    let mut b = sim();
    b.restore(&a.save()).unwrap();
    a.run(100.0);
    b.run(100.0);
    assert_eq!(a.world().hash(), b.world().hash());
    let before = b.world().resource::<Counts>().held;
    b.viewport(1600.0, 600.0); // contact now lies in the left half
    b.run(100.0);
    assert_eq!(b.world().resource::<Counts>().held, before);
}

#[test]
fn old_save_containers_are_refused_by_name_atomically() {
    let mut s = sim();
    let saved = s.save();
    let mut old = saved.clone();
    assert!(saved.starts_with(b"EXSIM\0\x05"));
    old[6] = 4;
    assert!(s
        .restore(&old)
        .unwrap_err()
        .to_string()
        .contains("EXSIM v5"));
    assert_eq!(s.save(), saved);
    let saved = s.world().save();
    let mut old = saved.clone();
    assert!(saved.starts_with(b"EXGAME\0\x03"));
    old[7] = 2;
    assert!(s
        .world_mut()
        .load(&old)
        .unwrap_err()
        .to_string()
        .contains("EXGAME v3"));
    assert_eq!(s.world().save(), saved);
}

#[test]
fn wrong_magic_reports_actual_bytes_and_expected_format() {
    let mut s = sim();
    let saved = s.save();
    for bytes in [
        b"random!!".to_vec(),
        b"EXGAME\0\x02".to_vec(),
        b"EXSIM\0\x04!".to_vec(),
        vec![0xff; 8],
        b"short".to_vec(),
    ] {
        let seen = format!("{:02x?}", &bytes[..bytes.len().min(8)]);
        let error = s.restore(&bytes).unwrap_err().to_string();
        assert!(
            error.contains(&seen) && error.contains("EXSIM v5"),
            "{error}"
        );
        let error = s.world_mut().load(&bytes).unwrap_err().to_string();
        assert!(
            error.contains(&seen) && error.contains("EXGAME v3"),
            "{error}"
        );
        assert_eq!(s.save(), saved);
    }
}
