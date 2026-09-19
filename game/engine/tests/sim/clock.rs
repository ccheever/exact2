use super::*;
// This incoming suite schedules future events explicitly; engine input() keeps
// its existing delivered-event semantics on Live clocks.
fn key<G: Game>(s: &mut Sim<G>, code: &str, down: bool, at_ms: f64) {
    s.scheduled_input(InputEvent::Key {
        code: code.into(),
        down,
        at_ms,
    });
}

#[test]
fn controlled_restore_anchors_destination_clock_before_first_advance() {
    for bound in [false, true] {
        let mut original = sim();
        key(&mut original, "KeyW", true, 0.0);
        original.run(500.0);
        key(&mut original, "KeyE", true, 750.0);
        let saved = original.save().unwrap();
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
        assert_eq!(restored.save().unwrap(), saved);
        let state = restored.agent(r#"{"op":"state","clockState":true}"#);
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
            assert_eq!(
                restored.save().unwrap(),
                saved,
                "inspection must remain read-only"
            );
        }
        assert_eq!(restored.advance(9500.0, Clock::Seekable), 30);
        assert_eq!(restored.world().tick(), 60);
        assert_eq!(restored.alpha(), 0.0, "seekable has no lookahead");
        assert_eq!(restored.world().resource::<Counts>().pressed, 1);
        assert_eq!(
            restored.save().unwrap(),
            original.save().unwrap(),
            "held and future input survive"
        );
        let before = restored.save().unwrap();
        assert!(restored.restore(b"invalid").is_err());
        assert_eq!(restored.save().unwrap(), before);
        assert_eq!(restored.advance(9600.0, Clock::Seekable), 6);
    }
}

#[test]
fn live_restore_rebases_without_counting_paused_wall_time() {
    for bound in [false, true] {
        let mut original = sim();
        original.run(500.0);
        key(&mut original, "KeyE", true, 550.0);
        let saved = original.save().unwrap();
        // Compare like clocks: live accepts input exactly on its deadline;
        // seekable keeps the strict tick-stamped rule.
        original.frame_period(1000.0 / 60.0);
        original.advance(600.0, Clock::Live);

        let mut restored = sim();
        restored.advance(100.0, Clock::Live);
        if bound {
            restored.restore_bound(&saved).unwrap();
        } else {
            restored.restore(&saved).unwrap();
        }
        let state = restored.agent(r#"{"op":"state","now":999999,"clockState":true}"#);
        assert!(state.contains(r#""hostMicros":null"#), "{state}");
        assert!(state.contains(r#""owner":"human""#), "{state}");
        assert_eq!(restored.world().tick(), 30);
        assert_eq!(restored.save().unwrap(), saved);
        restored.frame_period(1000.0 / 60.0);
        assert_eq!(restored.advance(900_000.0, Clock::Live), 0);
        assert_eq!(restored.world().tick(), 30, "paused wall time is discarded");
        assert_eq!(restored.advance(900_100.0, Clock::Live), 6);
        assert_eq!(restored.world().tick(), 36);
        assert_eq!(restored.alpha(), 1.0, "F2c renders at T + L - step");
        assert_eq!(restored.save().unwrap(), original.save().unwrap());
    }
}

#[test]
fn live_frame_history_does_not_cross_handoff_or_rebase() {
    for release in [false, true] {
        let mut s = sim();
        s.advance(0.0, Clock::Live);
        s.advance(20.0, Clock::Live);
        let saved = s.world().save();
        s.rebase(900_000.0, release).unwrap();
        // Releasing input logs the handoff; persistent world state is unchanged.
        assert_eq!(s.world().save(), saved);
        assert_eq!(s.ticks_due(900_000.0, Clock::Live), 0);
        assert_eq!(s.advance(900_000.0, Clock::Live), 0);
        assert_eq!(s.advance(900_100.0, Clock::Live), 6);
        assert_eq!(s.world().tick(), 7); // Unknown period has no early tick.
        assert!((s.alpha() - 0.2).abs() < 1e-6);
    }
    let mut s = sim();
    s.advance(0.0, Clock::Live);
    s.advance(20.0, Clock::Live);
    s.handoff(true);
    s.rebase(500.0, false).unwrap();
    assert_eq!(s.advance(600.0, Clock::Seekable), 6);
    s.handoff(false);
    assert_eq!(s.ticks_due(900_000.0, Clock::Live), 0);
    assert_eq!(s.advance(900_000.0, Clock::Live), 0);
    assert_eq!(s.advance(900_100.0, Clock::Live), 6);
    assert_eq!(s.world().tick(), 13);
}

#[test]
fn live_lookahead_uses_strict_next_frame_and_horizon_alpha() {
    let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
    s.frame_period(10.0);
    assert_eq!(s.advance(1000.0, Clock::Live), 0);
    assert_eq!(s.alpha(), 0.0);
    assert_eq!(s.advance(1010.0, Clock::Live), 1);
    assert!((s.alpha() - 0.2).abs() < 1e-6);
    assert_eq!(s.ticks_due(1020.0, Clock::Live), 0);
    assert_eq!(s.advance(1020.0, Clock::Live), 0);
    assert!((s.alpha() - 0.8).abs() < 1e-6);
    // The fixed 10 ms lookahead has not reached tick 3 yet.
    assert_eq!(s.advance(1035.0, Clock::Live), 1);
    assert_eq!(s.world().tick(), 2);
    assert_eq!(s.advance(1035.001, Clock::Live), 0);
    assert_eq!(s.ticks_due(1030.0, Clock::Live), 0);
    assert_eq!(s.advance(1030.0, Clock::Live), 0);
    assert_eq!(s.advance(1040.0, Clock::Live), 0); // T + L == 50 ms: strict horizon.
    assert_eq!(s.advance(1040.001, Clock::Live), 1);
}

#[test]
fn live_inputs_follow_stamps_and_future_stamps_wait() {
    let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
    s.frame_period(1000.0 / 60.0);
    s.advance(0.0, Clock::Live);
    key(&mut s, "KeyE", true, 18.0); // After deadline, before the frame.
    key(&mut s, "KeyE", false, 25.0); // Beyond this frame's host stamp.
    assert_eq!(s.advance(20.0, Clock::Live), 2);
    let c = s.world().resource::<Counts>();
    assert_eq!((c.held, c.pressed, c.released), (1, 1, 0));
    drop(c);
    assert_eq!(s.advance(30.0, Clock::Live), 0);
    assert_eq!(s.advance(40.0, Clock::Live), 1); // Fixed period reaches tick 3.
    assert_eq!(s.advance(50.0, Clock::Live), 0);
    assert_eq!(s.world().resource::<Counts>().released, 1);
}

#[test]
fn live_catchup_pause_save_and_seekable_transition() {
    let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
    s.frame_period(10.0);
    s.advance(0.0, Clock::Live);
    s.advance(10.0, Clock::Live);
    let saved = s.save().unwrap(); // F2d saves at tick 1's deadline, rounded up to a microsecond.
    let mut restored = sim();
    restored.restore(&saved).unwrap();
    assert_eq!(restored.save().unwrap(), saved);
    // Presentation lookahead is not saved; restore has no live history yet.
    assert!((restored.alpha() - 0.00002).abs() < 1e-7);
    assert!((s.alpha() - 0.2).abs() < 1e-6);
    assert_eq!(restored.advance(5000.0, Clock::Live), 0);
    assert_eq!(s.advance(10_010.0, Clock::Live), 15); // 250 ms, plus bounded lookahead.
    s.bind(&CounterArgs { paused: true }.values(), None)
        .unwrap();
    assert_eq!(s.advance(20_000.0, Clock::Live), 0);
    s.bind(&CounterArgs::default().values(), None).unwrap();
    let tick = s.world().tick();
    assert_eq!(s.advance(20_016.0, Clock::Seekable), 0);
    assert_eq!(s.world().tick(), tick);
    assert_eq!(s.advance(20_032.0, Clock::Seekable), 1);
    let mut exact = sim();
    assert_eq!(exact.advance(16.0, Clock::Seekable), 0);
    assert_eq!(exact.advance(32.0, Clock::Seekable), 1);
}

#[test]
fn live_half_step_frames_draw_equal_deltas_at_every_phase() {
    let step = 1000.0 / 60.0;
    for phase in [0.0, 0.1, 0.25, 0.4] {
        let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
        s.frame_period(step / 2.0);
        s.advance(0.0, Clock::Live);
        let mut previous = 0.0;
        for frame in 0..600 {
            let now = (2.0 + phase + frame as f64 * 0.5) * step;
            s.advance(now, Clock::Live);
            let drawn = drawn_counter(&s);
            if frame >= 480 {
                assert!(
                    (drawn - previous - 0.5).abs() < 1e-6,
                    "phase {phase}, frame {frame}: {}",
                    drawn - previous
                );
                // After acquisition, F2d holds the nearest frame-aligned origin.
                let origin = if phase <= 0.25 { -phase } else { 0.5 - phase };
                assert!((drawn - (now / step - 0.5 + origin)).abs() < 1e-6);
            }
            previous = drawn;
        }
    }
}

#[test]
fn live_stall_returns_to_steady_deltas_after_one_recovery_frame() {
    for hz in [60.0, 120.0] {
        let delta = 1000.0 / hz;
        let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
        s.frame_period(delta);
        for frame in 0..=10 {
            s.advance(frame as f64 * delta, Clock::Live);
        }
        let now = 10.0 * delta + 40.0;
        s.advance(now, Clock::Live);
        let mut previous = drawn_counter(&s);
        for frame in 1..=10 {
            s.advance(now + frame as f64 * delta, Clock::Live);
            let drawn = drawn_counter(&s);
            // The fixed period keeps even the first recovery interval steady.
            if frame >= 1 {
                assert!((drawn - previous - 60.0 / hz).abs() < 1e-6);
            }
            previous = drawn;
        }
    }
}

#[test]
fn live_three_tick_catchup_spreads_input_by_stamp() {
    let mut s = Sim::<Counter>::new(CounterArgs::default()).unwrap();
    s.frame_period(1000.0 / 60.0);
    s.advance(0.0, Clock::Live);
    key(&mut s, "KeyE", true, 18.0);
    key(&mut s, "KeyE", false, 35.0);
    let mut seen = Vec::new();
    assert_eq!(
        s.advance_with(40.0, Clock::Live, |w, _| {
            let c = w.resource::<Counts>();
            seen.push((w.tick(), c.held, c.pressed, c.released));
        }),
        3
    );
    assert_eq!(seen, [(1, 0, 0, 0), (2, 1, 1, 0), (3, 1, 1, 1)]);
}

#[test]
fn live_input_after_deadline_is_drawn_one_frame_before_zero_lookahead() {
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
        fn tick(w: &mut World, input: &Input, _: &()) {
            if input.held("move") {
                w.get_mut::<Transform>("player").unwrap().position.x += 1.0;
            }
        }
    }
    let step = 1000.0 / 60.0;
    let mut first = Vec::new();
    for clock in [Clock::Live, Clock::Seekable] {
        let mut s = Sim::<Mover>::new(()).unwrap();
        s.frame_period(step / 2.0);
        s.advance(0.0, clock);
        s.scheduled_input(InputEvent::Key {
            code: "KeyE".into(),
            down: true,
            at_ms: step + 0.01,
        });
        let (mut prev, mut curr) = (0.0, 0.0);
        let mut landed = None;
        let mut drawn_at = None;
        for frame in 0..8 {
            let now = (0.25 + frame as f64 * 0.5) * step;
            s.advance_with(now, clock, |w, _| {
                prev = curr;
                curr = w.get::<Transform>("player").unwrap().position.x;
                if curr > 0.0 && landed.is_none() {
                    landed = Some(w.tick());
                }
            });
            let drawn = prev + (curr - prev) * s.alpha();
            if drawn > 0.0 && drawn_at.is_none() {
                // F2d slews at most 0.25% while acquiring its initial origin.
                let slew = if clock == Clock::Live {
                    now / step * 0.0025
                } else {
                    0.0
                };
                assert!((drawn - 0.25).abs() <= slew as f32 + 1e-6);
                drawn_at = Some(frame);
            }
        }
        assert_eq!(
            landed,
            Some(2),
            "{clock:?}: stamp is after tick 1's deadline"
        );
        first.push(drawn_at.unwrap());
    }
    assert_eq!(first, [3, 4]); // T = 1.75 / 2.25 steps, R = 1.25 steps for both.
}
