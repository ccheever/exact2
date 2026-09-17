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
impl Game for Counter {
    const ARGS: &'static [&'static str] = &["paused"];
    fn setup(w: &mut World, args: &Args) -> Result<(), String> {
        args.flag(0, "paused")?;
        w.insert_resource(Counts::default());
        w.spawn_named("counter", Transform::default());
        Ok(())
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
    fn paused(args: &Args) -> bool {
        args.flag(0, "paused").unwrap()
    }
    fn tick(w: &mut World, i: &Input) {
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
    let mut s = Sim::new(&[Value::Bool(false)]).unwrap();
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
    assert_eq!(s.world().resource::<Counts>().pressed, 0);
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
    before.advance(33.334, Clock::Seekable);
    assert_eq!(before.world().resource::<Counts>().pressed, 1);
    let mut after = sim();
    key(&mut after, "KeyE", true, 16.667);
    after.advance(33.334, Clock::Seekable);
    assert_eq!(after.world().resource::<Counts>().pressed, 0);
    after.advance(50.0, Clock::Seekable);
    assert_eq!(after.world().resource::<Counts>().pressed, 1);
}
#[test]
fn epoch_pause_live_gap_and_backwards_clock() {
    let mut s = Sim::<Counter>::new(&[Value::Bool(false)]).unwrap();
    assert_eq!(s.advance(5000.0, Clock::Seekable), 0);
    key(&mut s, "KeyE", true, 5100.0);
    assert_eq!(s.advance(6000.0, Clock::Live), 15);
    assert_eq!(s.advance(5500.0, Clock::Seekable), 0);
    assert_eq!(s.advance(6000.0, Clock::Seekable), 0);
    s.bind(&[Value::Bool(true)]).unwrap();
    key(&mut s, "KeyE", false, 6500.0);
    s.advance(7000.0, Clock::Seekable);
    assert_eq!(s.world().tick(), 15);
    s.bind(&[Value::Bool(false)]).unwrap();
    s.advance(7100.0, Clock::Seekable);
    let c = s.world().resource::<Counts>();
    assert_eq!(c.released, 1);
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
    s.agent(r#"{"op":"clock","settle":true}"#);
    assert!(s.quiescent());
    s.world_mut().busy("one step");
    assert!(!s.quiescent());
    s.agent(r#"{"op":"clock","settle":true}"#);
    assert!(s.quiescent());
}
#[test]
fn publication_drain_is_full_and_journal_is_an_indexed_ring() {
    let mut s = sim();
    s.world().publish("a", 2);
    s.world().publish("b", false);
    assert_eq!(s.take_messages(), ["{\"a\":2,\"b\":false}"]);
    s.world().publish("a", 2);
    assert!(s.take_messages().is_empty());
    s.world().publish("b", true);
    assert_eq!(s.take_messages(), ["{\"a\":2,\"b\":true}"]);
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
    assert!(Sim::<Counter>::new(&[]).err().unwrap().contains("paused"));
    assert!(Sim::<Counter>::new(&[Value::Number(1.0)])
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
#[cfg(debug_assertions)]
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
        assert_eq!((c.pressed, c.released), (1, 0));
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
    assert_eq!(s.world().tick(), 600);
    assert!(reply.contains("\"quiescent\":false"));
}
