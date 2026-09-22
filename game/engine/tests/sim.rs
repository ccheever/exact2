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
    assert!(Sim::<Counter>::from_values(&[]).is_ok());
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
    let saved = s.save().unwrap();
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
    let saved = s.save().unwrap(); // publication was already delivered before this save
    restored.restore(&saved).unwrap();
    assert_eq!(restored.take_published().as_deref(), Some("{\"count\":4}"));
    assert!(restored.take_published().is_none());
    assert_eq!(
        restored.save().unwrap(),
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
    b.restore(&a.save().unwrap()).unwrap();
    b.advance(0.0, Clock::Seekable);
    for i in 1030..1040 {
        key(&mut a, "KeyE", i % 2 == 0, 0.0);
        key(&mut b, "KeyE", i % 2 == 0, 0.0);
    }
    assert_eq!(a.save().unwrap(), b.save().unwrap());
}

#[test]
fn older_saves_default_the_overflow_warning_flag() {
    let s = sim();
    let mut bytes = s.save().unwrap();
    let field = b"\x01\x00\x0foverflow_logged";
    let at = bytes.windows(field.len()).position(|v| v == field).unwrap();
    bytes.drain(at..at + field.len() + 1);
    let mut restored = sim();
    restored.restore(&bytes).unwrap();
    assert_eq!(s.save().unwrap(), restored.save().unwrap());
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
fn agent_input_projection_preserves_pending_contacts_pause_and_restore() {
    fn input_state(s: &mut Sim<Counter>) -> String {
        let saved = s.save().unwrap();
        let hash = s.world().hash();
        let tick = s.world().tick();
        let state = s.agent(r#"{"op":"state"}"#);
        assert_eq!(s.agent(r#"{"op":"state"}"#), state);
        assert_eq!(s.save().unwrap(), saved);
        assert_eq!(s.world().hash(), hash);
        assert_eq!(s.world().tick(), tick);
        state
            .split_once("\"input\":")
            .unwrap()
            .1
            .split_once(",\"published\":")
            .unwrap()
            .0
            .to_owned()
    }
    let mut s = sim();
    key(&mut s, "KeyZ", true, 0.);
    key(&mut s, "KeyA", true, 0.);
    s.advance(17., Clock::Seekable);
    key(&mut s, "KeyZ", false, 18.);
    key(&mut s, "KeyM", true, 20.);
    key(&mut s, "KeyA", true, 22.);
    for (id, phase, at_ms) in [
        (7, PointerPhase::Down, 23.),
        (8, PointerPhase::Down, 24.),
        (7, PointerPhase::Move, 25.),
        (8, PointerPhase::Cancel, 26.),
    ] {
        s.input(InputEvent::Control {
            name: "act".into(),
            id,
            phase,
            x: at_ms as f32,
            y: 10.,
            at_ms,
        });
    }
    let pending = input_state(&mut s);
    assert!(
        pending.contains(r#""held":["KeyA","KeyZ"],"forwarded":["KeyA","KeyM"]"#),
        "{pending}"
    );
    assert!(
        pending.contains(r#""controls":[],"forwardedControls":["act"]"#),
        "{pending}"
    );
    assert!(pending.contains(r#""id":7,"action":"act""#), "{pending}");
    assert!(!pending.contains(r#""id":8"#), "{pending}");
    s.input(InputEvent::Control {
        name: "missing".into(),
        id: 9,
        phase: PointerPhase::Down,
        x: 0.,
        y: 0.,
        at_ms: 27.,
    });
    assert_eq!(input_state(&mut s), pending);
    let mut restored = sim();
    restored.restore(&s.save().unwrap()).unwrap();
    assert_eq!(input_state(&mut restored), pending);
    s.input(InputEvent::Blur { at_ms: 30. });
    let blurred = input_state(&mut s);
    assert!(
        blurred.contains(r#""held":["KeyA","KeyZ"],"forwarded":[]"#),
        "{blurred}"
    );
    assert!(
        blurred.contains(r#""forwardedControls":[],"controlContacts":[]"#),
        "{blurred}"
    );
    s.advance(100., Clock::Seekable);
    s.bind(&CounterArgs { paused: true }.values(), None)
        .unwrap();
    key(&mut s, "KeyB", true, 40.);
    let paused = input_state(&mut s);
    assert!(
        paused.contains(r#""held":["KeyB"],"forwarded":["KeyB"]"#),
        "{paused}"
    );
    s.bind(&CounterArgs::default().values(), None).unwrap();
    assert_eq!(input_state(&mut s), paused);
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
    let saved = a.save().unwrap();
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
    b.restore(&a.save().unwrap()).unwrap();
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
    let saved = s.save().unwrap();
    let mut old = saved.clone();
    assert!(saved.starts_with(b"EXSIM\0\x05"));
    old[6] = 4;
    assert!(s
        .restore(&old)
        .unwrap_err()
        .to_string()
        .contains("EXSIM v5"));
    assert_eq!(s.save().unwrap(), saved);
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
    let saved = s.save().unwrap();
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
        assert_eq!(s.save().unwrap(), saved);
    }
}

fn live(hz: f64) -> Sim<Counter> {
    let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
    s.frame_period(1000.0 / hz);
    s.advance(0.0, Clock::Live);
    s
}
fn drawn_counter(s: &Sim<Counter>) -> f64 {
    if s.world().tick() == 0 {
        0.0
    } else {
        (s.world().tick() - 1) as f64 + f64::from(s.alpha())
    }
}
#[test]
fn unknown_period_has_no_lookahead_and_duplicates_preserve_the_pose() {
    let mut s = sim();
    s.advance(10.0, Clock::Live);
    assert_eq!(s.world().tick(), 0);
    s.frame_period(1000.0 / 120.0);
    s.advance(10.0, Clock::Live);
    assert_eq!(s.world().tick(), 0); // A new period cannot move a zero-delta pose.
    let pose = drawn_counter(&s);
    for _ in 0..10 {
        assert_eq!(s.ticks_due(10.0, Clock::Live), 0);
        assert_eq!(s.advance(10.0, Clock::Live), 0);
        assert_eq!(drawn_counter(&s), pose);
    }
}
#[test]
fn period_transitions_share_one_bounded_monotonic_render_slew() {
    for (old, new) in [(0.0, 60.0), (60.0, 120.0), (60.0, 144.0), (120.0, 60.0)] {
        for offset in [0.0, 3.1, 6.2] {
            let mut s = if old == 0.0 { sim() } else { live(old) };
            let before_period = 1000.0 / if old == 0.0 { new } else { old };
            let mut now = offset;
            for _ in 0..600 {
                now += before_period;
                s.advance(now, Clock::Live);
            }
            let period = 1000.0 / new;
            let mut previous = drawn_counter(&s);
            s.frame_period(period);
            assert_eq!(s.advance(now, Clock::Live), 0);
            assert_eq!(drawn_counter(&s), previous);
            for frame in 1..=1800 {
                now += period;
                assert_eq!(s.ticks_due(now, Clock::Live), s.advance(now, Clock::Live));
                let drawn = drawn_counter(&s);
                let delta = (drawn - previous) * 1000.0 / 60.0;
                assert!(delta > 0.0);
                assert!(
                    (delta - period).abs() <= period * 0.0025 + 0.00003,
                    "{old}->{new}, {offset}, frame {frame}: {delta}"
                );
                previous = drawn;
            }
            // L has actually arrived, rather than merely ignoring the new rate.
            // The grid has also settled: ticking frames have the new rate's alpha.
            let mut ticking = 0;
            for _ in 0..60 {
                now += period;
                if s.advance(now, Clock::Live) > 0 {
                    ticking += 1;
                }
                let horizon = drawn_counter(&s) + 1.0;
                let l = period.min(1000.0 / 60.0) * 60.0 / 1000.0;
                // The world time in a save is exact when this frame did not tick early.
                if s.alpha() as f64 >= l {
                    let mut restored = sim();
                    restored.restore(&s.save().unwrap()).unwrap();
                    assert!(
                        (horizon - l - restored.world().tick() as f64 - restored.alpha() as f64)
                            .abs()
                            < 0.0001
                    );
                }
            }
            assert!(ticking > 0);
        }
    }
}
#[test]
fn sub_half_percent_period_noise_cannot_restart_grid_slew() {
    let mut stable = live(144.0);
    let mut noisy = live(144.0);
    for frame in 1..=1500 {
        let period = 1000.0 / 144.0;
        noisy.frame_period(period * if frame % 2 == 0 { 1.004 } else { 0.996 });
        let now = 3.1 + frame as f64 * period;
        assert_eq!(
            stable.advance(now, Clock::Live),
            noisy.advance(now, Clock::Live)
        );
        assert_eq!(stable.alpha(), noisy.alpha());
        assert_eq!(stable.save().unwrap(), noisy.save().unwrap());
    }
}
#[test]
fn duplicate_stamp_with_negative_half_unit_remainder_cannot_retreat() {
    let mut s = sim();
    let now = 20.000025; // 1_200_001.5 units: round up and retain -0.5.
    s.advance(now, Clock::Live);
    let pose = drawn_counter(&s);
    let saved = s.save().unwrap();
    for _ in 0..100 {
        assert_eq!(s.ticks_due(now, Clock::Live), 0);
        assert_eq!(s.advance(now, Clock::Live), 0);
        assert_eq!(drawn_counter(&s), pose);
        assert_eq!(s.save().unwrap(), saved);
    }
}
#[test]
fn delivered_live_input_is_eligible_at_the_paced_frame_but_seekable_future_waits() {
    for clock in [Clock::Live, Clock::Seekable] {
        let mut s = live(60.0);
        key(&mut s, "KeyE", true, 16.8);
        assert_eq!(s.advance(1000.0 / 60.0, clock), 1);
        assert_eq!(
            s.world().resource::<Counts>().pressed,
            u32::from(clock == Clock::Live)
        );
        s.advance(33.334, clock); // Seekable rounds to integer µs, past tick 2.
        assert_eq!(s.world().resource::<Counts>().pressed, 1);
    }
}
#[test]
fn synthetic_live_frames_have_no_equal_rate_beats() {
    for hz in [60.0, 59.94, 120.0] {
        for epoch in [0.0, 1234.567, 1_000_000.123] {
            let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
            s.frame_period(1000.0 / hz);
            s.advance(epoch, Clock::Live);
            let mut extra = 0;
            for frame in 1..=3600 {
                let now = epoch + frame as f64 * 1000.0 / hz;
                let due = s.ticks_due(now, Clock::Live);
                let ticks = s.advance(now, Clock::Live);
                assert_eq!(due, ticks);
                if hz == 60.0 {
                    assert_eq!(ticks, 1, "frame {frame}, epoch {epoch}");
                } else if hz == 120.0 {
                    assert_eq!(ticks, u32::from(frame % 2 == 0));
                } else {
                    assert!((1..=2).contains(&ticks));
                    extra += ticks - 1;
                }
            }
            if hz == 59.94 {
                assert!((3..=4).contains(&extra));
            }
        }
    }
}
#[test]
fn jittered_sixty_hz_stays_within_one_tick_over_six_hundred_frames() {
    let mut s = live(60.0);
    let mut now = 0.0;
    // The five deltas total five nominal periods, without making individual
    // raw callbacks a lattice. Counts can straddle a boundary by at most one.
    for frame in 1..=600 {
        now += [16.0, 17.5, 16.4, 16.2, 1000.0 / 12.0 - 66.1][(frame - 1) % 5];
        let due = s.ticks_due(now, Clock::Live);
        assert_eq!(s.advance(now, Clock::Live), due);
        assert!((s.world().tick() as i64 - frame as i64).abs() <= 1);
    }
}
#[test]
fn live_stall_and_its_first_recovery_frame_keep_the_same_period() {
    for hz in [60.0, 120.0, 144.0] {
        let delta = 1000.0 / hz;
        let mut s = live(hz);
        for frame in 1..=10 {
            s.advance(frame as f64 * delta, Clock::Live);
        }
        let mut previous = drawn_counter(&s);
        let now = 10.0 * delta + 40.0;
        s.advance(now, Clock::Live);
        assert!((drawn_counter(&s) - previous - 40.0 * 60.0 / 1000.0).abs() < 1e-6);
        previous = drawn_counter(&s);
        for frame in 1..=10 {
            s.advance(now + frame as f64 * delta, Clock::Live);
            let drawn = drawn_counter(&s);
            assert!(
                (drawn - previous - 60.0 / hz).abs() < 1e-6,
                "{hz} recovery {frame}"
            );
            previous = drawn;
        }
    }
}
#[test]
fn every_starting_phase_aligns_within_four_seconds_and_holds() {
    for hz in [60.0, 120.0, 240.0] {
        let period = 1000.0 / hz;
        for phase in 0..=100 {
            let mut s = live(hz);
            let offset = phase as f64 / 100.0 * period;
            let mut previous = 0.0;
            for frame in 1..=(hz as usize * 6) {
                let now = offset + frame as f64 * period;
                let due = s.ticks_due(now, Clock::Live);
                let ticks = s.advance(now, Clock::Live);
                assert_eq!(due, ticks);
                let drawn = drawn_counter(&s);
                if frame > 3 {
                    let delta = (drawn - previous) * 1000.0 / 60.0;
                    assert!(
                        (delta - period).abs() <= period * 0.0025 + 0.00002,
                        "{hz}/{phase}/{frame}: {delta}"
                    );
                }
                if frame as f64 * period >= 4000.0 {
                    // T/step = drawn + 1 - L/step; the origin meets this lattice.
                    let time = (drawn + 1.0) * 1000.0 / 60.0 - period;
                    let error = (time / period - (time / period).round()).abs() * period;
                    assert!(error < 0.0001, "{hz}/{phase}/{frame}: {error}");
                    if ticks > 0 {
                        assert!((s.alpha() as f64 - 60.0 / hz).abs() < 1e-6);
                    }
                }
                previous = drawn;
            }
        }
    }
}
#[test]
fn aligned_ticks_consume_input_delivered_after_the_preceding_frame() {
    for hz in [60.0, 120.0, 240.0] {
        let period = 1000.0 / hz;
        let mut s = live(hz);
        let offset = period * 0.37;
        let mut last = 0.0;
        for frame in 1..=(hz as usize * 4) {
            last = offset + frame as f64 * period;
            s.advance(last, Clock::Live);
        }
        let mut edges = 0;
        for frame in 1..=hz as usize {
            let now = last + frame as f64 * period;
            let due = s.ticks_due(now, Clock::Live);
            if due > 0 {
                // Delivery really happens after the previous advance, including
                // an event exactly at this frame's stamp. No prequeued future input.
                key(
                    &mut s,
                    "KeyE",
                    edges % 2 == 0,
                    now - if edges % 3 == 0 { 0.0 } else { period / 3.0 },
                );
                edges += 1;
            }
            assert_eq!(s.advance(now, Clock::Live), due);
            let c = s.world().resource::<Counts>();
            assert_eq!(c.pressed + c.released, edges, "display {hz}, frame {frame}");
        }
    }
}
#[test]
fn tick_phase_is_one_at_sixty_and_half_at_one_twenty() {
    for hz in [60.0, 120.0] {
        let mut s = live(hz);
        let (mut total, mut count) = (0.0, 0);
        for frame in 1..=600 {
            if s.advance(frame as f64 * 1000.0 / hz, Clock::Live) > 0 {
                total += s.alpha() as f64;
                count += 1;
            }
        }
        assert!((total / count as f64 - 60.0 / hz).abs() < 1e-6);
    }
}
#[test]
fn submicrosecond_backwards_stamps_cannot_add_time_or_change_lookahead() {
    let mut s = live(120.0);
    s.advance(10.0004, Clock::Live);
    let pose = drawn_counter(&s);
    let saved = s.save().unwrap();
    for _ in 0..1000 {
        s.frame_period(1000.0 / 60.0);
        assert_eq!(s.ticks_due(9.9996, Clock::Live), 0);
        assert_eq!(s.advance(9.9996, Clock::Live), 0);
        assert_eq!(drawn_counter(&s), pose);
        s.frame_period(1000.0 / 120.0);
        assert_eq!(s.advance(10.0004, Clock::Live), 0);
        assert_eq!(drawn_counter(&s), pose);
        assert_eq!(s.save().unwrap(), saved);
    }
}
#[test]
fn early_live_save_and_switch_catch_the_exact_clock_up_without_a_tick() {
    let mut s = live(120.0);
    s.advance(10.0, Clock::Live);
    assert_eq!(s.world().tick(), 1);
    key(&mut s, "KeyE", true, 12.0); // A future input survives save and switch.
    let saved = s.save().unwrap();
    let hash = s.world().hash();
    let mut restored = sim();
    restored.restore(&saved).unwrap();
    assert_eq!(restored.save().unwrap(), saved);
    assert_eq!(restored.world().hash(), hash);
    assert!(restored.alpha() < 0.0001); // ceil(1e6/60) us, exact seekable convention.
    assert_eq!(s.ticks_due(10.0, Clock::Seekable), 0);
    assert_eq!(s.advance(10.0, Clock::Seekable), 0);
    assert_eq!(s.save().unwrap(), saved);
    assert_eq!(s.world().hash(), hash);
    restored.advance(10.0, Clock::Seekable);
    for now in [12.0, 26.667, 43.334, 100.0] {
        s.advance(now, Clock::Seekable);
        restored.advance(now, Clock::Seekable);
        assert_eq!(s.save().unwrap(), restored.save().unwrap());
        assert_eq!(s.world().hash(), restored.world().hash());
    }
    assert_eq!(s.world().resource::<Counts>().pressed, 1);
}
#[test]
fn unpause_seeds_live_time_and_drops_the_pause_gap() {
    let mut s = live(120.0);
    s.advance(10.0, Clock::Live);
    s.bind(&CounterArgs { paused: true }.values(), None)
        .unwrap();
    s.advance(2000.0, Clock::Live);
    s.bind(&CounterArgs::default().values(), None).unwrap();
    assert_eq!(s.ticks_due(50_000.0, Clock::Live), 0);
    assert_eq!(s.advance(50_000.0, Clock::Live), 0);
    assert_eq!(s.world().tick(), 1);
    assert_eq!(s.advance(50_000.0 + 1000.0 / 60.0, Clock::Live), 1);
}
#[test]
fn live_three_tick_catchup_spreads_input_by_stamp_and_caps_a_large_gap() {
    let mut s = live(60.0);
    // Acquire the origin on a normal frame before the stall.
    s.advance(1000.0 / 60.0, Clock::Live);
    key(&mut s, "KeyE", true, 35.0);
    key(&mut s, "KeyE", false, 52.0);
    let mut seen = Vec::new();
    assert_eq!(
        s.advance_with(1000.0 / 60.0 + 40.0, Clock::Live, |w, _| {
            let c = w.resource::<Counts>();
            seen.push((w.tick(), c.held, c.pressed, c.released));
        }),
        3
    );
    assert_eq!(seen, [(2, 0, 0, 0), (3, 1, 1, 0), (4, 1, 1, 1)]);
    assert_eq!(s.advance(10_056.667, Clock::Live), 15);
}
#[test]
fn slew_and_period_are_absent_from_save_state_snapshot_and_seekable_continuation() {
    let mut s = live(120.0);
    for frame in 1..=480 {
        s.advance(3.0 + frame as f64 * 1000.0 / 120.0, Clock::Live);
    }
    let mut plain = sim();
    plain.restore(&s.save().unwrap()).unwrap();
    assert_eq!(s.world().hash(), plain.world().hash());
    assert_eq!(s.world().save(), plain.world().save());
    let state = s.agent(r#"{"op":"state"}"#);
    let restored_state = plain.agent(r#"{"op":"state"}"#).replace(
        r#""restored":true,"restoredFrom":{"paused":false},"#,
        r#""restored":false,"#,
    );
    assert_eq!(state, restored_state);
    for forbidden in ["lookahead", "slew", "period", "live_time"] {
        assert!(!state.contains(forbidden));
    }
    let now = 4003.0;
    s.advance(now, Clock::Seekable);
    plain.advance(now, Clock::Seekable);
    // The restored host has never had a period or a slew. Continuation is exact.
    for delta in [0.0, 1.0, 16.667, 100.0, 1000.0] {
        s.advance(now + delta, Clock::Seekable);
        plain.advance(now + delta, Clock::Seekable);
        assert_eq!(s.save().unwrap(), plain.save().unwrap());
        assert_eq!(s.world().hash(), plain.world().hash());
    }
}

#[test]
fn aligned_live_input_is_drawn_one_frame_before_seekable_interpolation() {
    struct Mover;
    impl Game for Mover {
        const ID: &'static str = "latency";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("player", Transform::default());
        }
        fn actions() -> Actions {
            Actions::new().button("move", &["KeyE"])
        }
        fn tick(w: &mut World, i: &Input, _: &()) {
            if i.held("move") {
                w.get_mut::<Transform>("player").unwrap().position.x += 1.0;
            }
        }
    }
    let period = 1000.0 / 120.0;
    let mut first = Vec::new();
    for clock in [Clock::Live, Clock::Seekable] {
        let mut s = Sim::<Mover>::new(()).unwrap();
        s.frame_period(period);
        s.advance(0.0, clock);
        let (mut previous, mut current) = (0.0, 0.0);
        let mut first_frame = None;
        for frame in 1..=6 {
            if frame == 4 {
                // Delivered after frame 3 (25 ms), before tick 2's deadline.
                s.input(InputEvent::Key {
                    code: "KeyE".into(),
                    down: true,
                    at_ms: 30.0,
                });
            }
            s.advance_with(frame as f64 * period, clock, |w, _| {
                previous = current;
                current = w.get::<Transform>("player").unwrap().position.x;
                if current == 1.0 {
                    assert_eq!(w.tick(), 2);
                }
            });
            if previous + (current - previous) * s.alpha() > 0.01 && first_frame.is_none() {
                first_frame = Some(frame);
            }
        }
        first.push(first_frame.unwrap());
    }
    assert_eq!(first, [4, 5]);
}

#[test]
fn restore_runs_no_setup_and_failed_world_validation_is_atomic() {
    thread_local! {
        static SETUPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    struct Once;
    impl Game for Once {
        const ID: &'static str = "single-setup";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            SETUPS.with(|calls| calls.set(calls.get() + 1));
            w.spawn_named("player", Transform::default());
            w.emit("setup message");
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut s = Sim::<Once>::new(()).unwrap().paranoid(Paranoid::Off);
    assert_eq!(SETUPS.with(|calls| calls.replace(0)), 1);
    let saved = s.save().unwrap();
    for bound in [false, true] {
        if bound {
            s.restore_bound(&saved).unwrap();
        } else {
            s.restore(&saved).unwrap();
        }
        assert_eq!(SETUPS.with(|calls| calls.replace(0)), 0);
        assert_eq!(s.save().unwrap(), saved);
    }
    // Invalid world headers must be rejected before setup has any side effects.
    let mut bad = saved.clone();
    let world = bad.windows(8).position(|v| v == b"EXGAME\0\x03").unwrap();
    bad[world] = b'!';
    assert!(s.restore(&bad).is_err());
    assert_eq!(SETUPS.with(|calls| calls.replace(0)), 0);
    assert_eq!(s.save().unwrap(), saved);
    // Game::register extends the scratch schema for the chosen arguments;
    // malformed typed data is refused before gameplay setup has side effects.
    let mut bad = saved.clone();
    bad[world + 8] = 0xff;
    assert!(s.restore(&bad).is_err());
    assert_eq!(SETUPS.with(|calls| calls.replace(0)), 0);
    assert_eq!(s.save().unwrap(), saved);
    for mode in [Paranoid::Save, Paranoid::FreshGame] {
        s = s.paranoid(mode);
        s.run(17.0);
        assert_eq!(SETUPS.with(|calls| calls.replace(0)), 0, "{mode:?}");
    }
}

#[test]
fn r13_empty_and_short_calls_take_all_remaining_rust_defaults() {
    #[derive(Args)]
    struct Options {
        seed: u64,
        #[live]
        paused: bool,
        #[restart]
        restart: bool,
    }
    impl Default for Options {
        fn default() -> Self {
            Self {
                seed: 17,
                paused: true,
                restart: false,
            }
        }
    }
    struct Defaults;
    impl Game for Defaults {
        const ID: &'static str = "r13-defaults";
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            w.reseed(args.seed);
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    let mut empty = Sim::<Defaults>::from_values(&[]).unwrap();
    assert_eq!(
        empty.save().unwrap(),
        Sim::<Defaults>::new(Options::default())
            .unwrap()
            .save()
            .unwrap()
    );
    let short = Sim::<Defaults>::from_values(&[Value::Number(7.)]).unwrap();
    let full =
        Sim::<Defaults>::from_values(&[Value::Number(7.), Value::Bool(true), Value::Bool(false)])
            .unwrap();
    assert_eq!(short.save().unwrap(), full.save().unwrap());
    empty.bind(&[Value::Number(7.)], None).unwrap();
    assert_eq!(empty.world().hash(), full.world().hash());
    assert!(empty
        .agent(r#"{"op":"state"}"#)
        .contains(r#""paused":true"#));
    assert!(Sim::<Defaults>::from_values(&[
        Value::Number(7.),
        Value::Bool(true),
        Value::Bool(false),
        Value::Bool(false)
    ])
    .err()
    .unwrap()
    .contains("got 4"));
}

#[test]
fn restore_registers_argument_dependent_types_before_setup() {
    thread_local! { static SETUPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    #[derive(Default, Args)]
    struct Options {
        other: bool,
    }
    #[derive(Default, exact_game::Component)]
    struct Other {
        value: u32,
    }
    struct Conditional;
    impl Game for Conditional {
        const ID: &'static str = "conditional-registration";
        type Args = Options;
        fn register(w: &mut World, args: &std::collections::BTreeMap<&str, Value>) {
            if args["other"].as_bool() == Some(true) {
                w.register::<Other>();
            }
        }
        fn setup(w: &mut World, args: &Options) {
            SETUPS.with(|calls| calls.set(calls.get() + 1));
            if args.other {
                w.spawn(Other { value: 17 });
            }
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    let source = Sim::<Conditional>::new(Options { other: true }).unwrap();
    let mut target = Sim::<Conditional>::new(Options::default()).unwrap();
    target.restore(&source.save().unwrap()).unwrap();
    assert_eq!(source.world().hash(), target.world().hash());
    SETUPS.with(|calls| calls.set(0));
    let before = target.save().unwrap();
    let mut bad = before.clone();
    let world = bad.windows(8).position(|v| v == b"EXGAME\0\x03").unwrap();
    bad[world + 8] = 0xff;
    assert!(target.restore(&bad).is_err());
    assert_eq!(SETUPS.with(|calls| calls.get()), 0);
    assert_eq!(target.save().unwrap(), before);
}

#[test]
fn registration_filters_live_arguments_and_validates_before_register() {
    thread_local! { static REGISTERS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    #[derive(Default, Args)]
    struct Options {
        extra: bool,
        #[live]
        enabled: bool,
    }
    #[derive(Default, exact_game::Component)]
    struct Extra(u32);
    struct Conditional;
    impl Game for Conditional {
        const ID: &'static str = "filtered-registration";
        type Args = Options;
        fn validate(a: &Options) -> Result<(), String> {
            if !a.extra {
                Err("extra required".into())
            } else {
                Ok(())
            }
        }
        fn register(w: &mut World, args: &std::collections::BTreeMap<&str, Value>) {
            REGISTERS.with(|n| n.set(n.get() + 1));
            assert!(!args.contains_key("enabled"));
            if args["extra"].as_bool() == Some(true) {
                w.register::<Extra>();
            }
        }
        fn setup(w: &mut World, _: &Options) {
            w.spawn(Extra(17));
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    let mut source = Sim::<Conditional>::new(Options {
        extra: true,
        enabled: true,
    })
    .unwrap();
    source
        .bind(&[Value::Bool(true), Value::Bool(false)], None)
        .unwrap();
    let saved = source.save().unwrap();
    let mut target = Sim::<Conditional>::new(Options {
        extra: true,
        enabled: true,
    })
    .unwrap();
    target.restore(&saved).unwrap();
    assert_eq!(target.save().unwrap(), saved);
    REGISTERS.with(|n| n.set(0));
    assert!(Sim::<Conditional>::new(Options::default()).is_err());
    assert_eq!(REGISTERS.with(|n| n.get()), 0);
}

#[test]
fn saves_canonicalize_consumed_input_edges_and_preserve_pending_events() {
    let drive = |mode, epoch| {
        let mut s = Sim::<Counter>::new(CounterArgs::default())
            .unwrap()
            .paranoid(mode);
        s.viewport(800., 600.);
        s.advance(epoch, Clock::Seekable);
        s.input(InputEvent::Key {
            code: "KeyE".into(),
            down: true,
            at_ms: epoch + 5.,
        });
        s.input(InputEvent::Pointer {
            id: 7,
            phase: PointerPhase::Down,
            x: 100.,
            y: 100.,
            at_ms: epoch + 6.,
        });
        s.input(InputEvent::Pointer {
            id: 7,
            phase: PointerPhase::Move,
            x: 110.,
            y: 105.,
            at_ms: epoch + 7.,
        });
        s.input(InputEvent::Wheel {
            dx: 2.,
            dy: -3.,
            at_ms: epoch + 8.,
        });
        s.input(InputEvent::Key {
            code: "KeyE".into(),
            down: false,
            at_ms: epoch + 24.,
        });
        s.input(InputEvent::Control {
            name: "act".into(),
            id: 9,
            phase: PointerPhase::Down,
            x: 0.,
            y: 0.,
            at_ms: epoch + 40.,
        });
        s.input(InputEvent::Control {
            name: "act".into(),
            id: 9,
            phase: PointerPhase::Up,
            x: 0.,
            y: 0.,
            at_ms: epoch + 70.,
        });
        s.advance(epoch + 17., Clock::Seekable);
        let saved = s.save().unwrap();
        assert_eq!(s.save().unwrap(), saved, "save is a read");
        let mut restored = sim().paranoid(mode);
        restored.restore(&saved).unwrap();
        assert_eq!(restored.save().unwrap(), saved, "{mode:?} edge boundary");
        s.advance(epoch + 100., Clock::Seekable);
        restored.advance(500_000., Clock::Seekable);
        restored.advance(500_083., Clock::Seekable);
        assert_eq!(restored.save().unwrap(), s.save().unwrap());
        let counts = s.world().resource::<Counts>();
        assert_eq!(counts.pressed, 2);
        assert_eq!(counts.released, 2);
        assert_eq!(counts.wheel, Vec2::new(2., -3.));
        (saved, s.save().unwrap())
    };
    for epoch in [0., 100_000.] {
        let expected = drive(Paranoid::Off, epoch);
        for mode in [Paranoid::Save, Paranoid::FreshGame] {
            assert_eq!(drive(mode, epoch), expected, "{mode:?}");
        }
    }
}
