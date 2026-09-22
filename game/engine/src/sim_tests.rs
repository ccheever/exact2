use super::*;
struct Ticker<const HZ: u32>;
impl<const HZ: u32> Game for Ticker<HZ> {
    const ID: &'static str = "alpha-guard";
    const HZ: u32 = HZ;
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn steady<const HZ: u32>() {
    for frame_hz in [60.0, 59.94, 120.0, 144.0, 240.0] {
        for epoch in [0.0, 1234.567, 1_000_000.123] {
            for phase in [0.0, 0.1, 0.25, 0.4] {
                let mut s = Sim::<Ticker<HZ>>::new(()).unwrap();
                s.frame_period(1000.0 / frame_hz);
                s.advance(epoch, Clock::Live);
                for frame in 1..=600 {
                    let now = epoch + frame as f64 * 1000.0 / frame_hz + phase * 1000.0 / HZ as f64;
                    let due = s.ticks_due(now, Clock::Live);
                    assert_eq!(s.advance(now, Clock::Live), due);
                    if HZ == 120 && frame_hz == 120.0 && phase == 0.0 {
                        assert_eq!(due, 1, "120/120 frame {frame}");
                    }
                    // Inspect the signed numerator BEFORE alpha's guard.
                    let raw = s.alpha_numerator();
                    assert!((0..=1_000_000).contains(&raw),
                        "world {HZ}, display {frame_hz}, epoch {epoch}, phase {phase}, frame {frame}: {raw}");
                    if s.world.tick() > 0 {
                        assert_eq!(s.alpha(), raw as f32 / 1_000_000.0);
                    }
                }
                // Seekable must never reuse the preceding live lookahead.
                s.advance(epoch + 602.0 * 1000.0 / frame_hz, Clock::Seekable);
                assert_eq!(s.lookahead_us_hz, 0);
                assert!(s.live_time.is_none());
            }
        }
    }
}
#[test]
fn alpha_guard_never_fires_on_600_steady_frames_at_both_world_rates() {
    steady::<60>();
    steady::<120>();
}
#[test]
fn integer_phase_survives_the_old_eighteen_hour_precision_limit() {
    let mut s = Sim::<Ticker<120>>::new(()).unwrap();
    // Just beyond 2^26 milliseconds, without running eight million ticks.
    let tick = 8_100_000;
    for _ in 0..tick {
        s.world.step_clock();
    }
    assert_eq!(s.world.tick(), tick);
    s.world_us = tick as i64 * 1_000_000 / 120;
    s.frame_period(1000.0 / 120.0);
    s.advance(0.0, Clock::Live);
    for frame in 1..=3600 {
        let now = frame as f64 * 1000.0 / 120.0;
        assert_eq!(s.ticks_due(now, Clock::Live), 1);
        assert_eq!(s.advance(now, Clock::Live), 1);
        let live = s.live_time.unwrap();
        assert!(live.remainder.abs() <= 0.5);
        assert_eq!(live.phase, (tick as i128 + frame as i128) * 1_000_000);
        assert_eq!(s.alpha(), 1.0);
    }
}
#[test]
fn live_restore_preserves_a_pending_future_stamp_on_the_epoch_sample() {
    let mut s = Sim::<Ticker<60>>::new(()).unwrap();
    s.advance(10.0, Clock::Live);
    s.queue.push_back(Queued {
        host_us: 12_000,
        ..Default::default()
    });
    let save = s.save().unwrap();
    s.restore(&save).unwrap();
    s.advance(100.0, Clock::Live);
    assert_eq!(s.queue.len(), 1);
    assert_eq!(s.queue[0].host_us, 102_000);
    assert!(s.queue[0].world_us.is_none());
}
#[test]
fn restore_refuses_a_one_tick_ahead_clock() {
    let mut s = Sim::<Ticker<60>>::new(()).unwrap();
    s.advance(0.0, Clock::Seekable);
    s.advance(17.0, Clock::Seekable);
    let good = s.save().unwrap();
    let mut saved: Saved = bin::from_slice(&good[7..]).unwrap();
    saved.world_us = 10_000;
    let mut bad = b"EXSIM\0\x05".to_vec();
    bad.extend(bin::to_vec(&saved));
    assert!(s
        .restore(&bad)
        .unwrap_err()
        .to_string()
        .contains("world and clock disagree"));
    assert_eq!(s.save().unwrap(), good);
}

#[test]
fn restore_preflights_input_and_offsets_without_setup() {
    thread_local! { static SETUPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; static REGISTERS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    struct Counter;
    impl Game for Counter {
        const ID: &'static str = "restore-preflight";
        type Args = ();
        fn register(_: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
            REGISTERS.with(|n| n.set(n.get() + 1));
        }
        fn setup(_: &mut World, _: &()) {
            SETUPS.with(|n| n.set(n.get() + 1));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Counter>::new(()).unwrap().paranoid(Paranoid::Off);
    sim.run(17.);
    let good = sim.save().unwrap();
    SETUPS.with(|n| n.set(0));
    REGISTERS.with(|n| n.set(0));
    for case in 0..3 {
        let mut saved: Saved = bin::from_slice(&good[7..]).unwrap();
        let invalid = InputEvent::Control {
            name: "unknown".into(),
            id: 7,
            phase: crate::PointerPhase::Down,
            x: 0.,
            y: 0.,
            at_ms: 0.,
        };
        match case {
            0 => {
                saved.input = crate::json::from_str(
                    r#"{"contacts":[{"id":7,"action":"unknown","origin":[0,0],"position":[0,0]}]}"#,
                )
                .unwrap()
            }
            1 => saved.queue.push(Queued {
                event: invalid,
                ..Default::default()
            }),
            _ => saved.queue.push(Queued {
                event: InputEvent::Blur { at_ms: 0. },
                world_us: Some(i64::MAX),
                ..Default::default()
            }),
        }
        let mut bad = b"EXSIM\0\x05".to_vec();
        bad.extend(bin::to_vec(&saved));
        assert!(sim.restore(&bad).is_err(), "case {case}");
        assert_eq!(SETUPS.with(|n| n.get()), 0, "case {case}");
        assert_eq!(REGISTERS.with(|n| n.get()), 0, "case {case}");
        assert_eq!(sim.save().unwrap(), good);
    }
}

#[test]
fn restore_constructs_current_bindings_once_without_setup() {
    thread_local! {
        static VALIDATIONS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static ACTIONS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static SETUPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    struct Checked;
    impl Game for Checked {
        const ID: &'static str = "restore-current-bindings";
        type Args = ();
        fn validate(_: &()) -> Result<(), String> {
            VALIDATIONS.with(|n| n.set(n.get() + 1));
            Ok(())
        }
        fn actions() -> Actions {
            ACTIONS.with(|n| n.set(n.get() + 1));
            Actions::new().button("jump", &["Space"])
        }
        fn setup(_: &mut World, _: &()) {
            SETUPS.with(|n| n.set(n.get() + 1));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    for retain_args in [false, true] {
        let mut sim = Sim::<Checked>::new(()).unwrap().paranoid(Paranoid::Off);
        sim.input(InputEvent::Control {
            name: "jump".into(),
            id: 7,
            phase: PointerPhase::Down,
            x: 0.,
            y: 0.,
            at_ms: 0.,
        });
        sim.run(34.);
        let saved = sim.save().unwrap();
        VALIDATIONS.with(|n| n.set(0));
        ACTIONS.with(|n| n.set(0));
        SETUPS.with(|n| n.set(0));
        sim.restore_into(&saved, retain_args).unwrap();
        assert!(sim.input.held("jump"));
        assert_eq!(sim.save().unwrap(), saved);
        assert_eq!(SETUPS.with(|n| n.get()), 0);
        assert_eq!(VALIDATIONS.with(|n| n.get()), 1);
        assert_eq!(ACTIONS.with(|n| n.get()), 1);
    }
}

#[test]
fn proof_profiles_disable_floating_point_contraction() {
    assert!(include_str!("../../.cargo/config.toml").contains("llvm-args=-fp-contract=off"));
}
#[test]
fn restore_validates_arguments_before_registration() {
    thread_local! { static REGISTERS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    #[derive(Default, crate::Args)]
    struct Options {
        invalid: bool,
    }
    struct Checked;
    impl Game for Checked {
        const ID: &'static str = "validate-before-register";
        type Args = Options;
        fn validate(args: &Options) -> Result<(), String> {
            if args.invalid {
                Err("invalid options".into())
            } else {
                Ok(())
            }
        }
        fn register(_: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
            REGISTERS.with(|n| n.set(n.get() + 1));
        }
        fn setup(_: &mut World, _: &Options) {}
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    let mut sim = Sim::<Checked>::new(Options::default()).unwrap();
    let good = sim.save().unwrap();
    let mut saved: Saved = bin::from_slice(&good[7..]).unwrap();
    saved.args = r#"{"invalid":true}"#.into();
    let mut bad = b"EXSIM\0\x05".to_vec();
    bad.extend(bin::to_vec(&saved));
    REGISTERS.with(|n| n.set(0));
    assert!(sim
        .restore(&bad)
        .unwrap_err()
        .to_string()
        .contains("invalid options"));
    assert_eq!(REGISTERS.with(|n| n.get()), 0);
    assert_eq!(sim.save().unwrap(), good);
}

#[cfg(test)]
mod e11_assets_restore {
    use super::*;
    struct Declared;
    impl Game for Declared {
        const ID: &'static str = "restore-declared-assets";
        const ASSETS: &'static [&'static str] = &["crate.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            assert!(w.model("crate.model").is_some());
            w.spawn((
                crate::Transform::default(),
                crate::Mesh::asset("crate.model"),
            ));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            assert!(w.model("crate.model").is_some());
        }
    }
    #[test]
    fn repeated_restore_keeps_declared_assets_and_pending_restore_refuses() {
        assert!(crate::asset::AssetStore::default().ready());
        let mut sim = Sim::<Declared>::new(()).unwrap();
        sim.asset(
            "crate.model",
            Some(&crate::bin::to_vec(&crate::asset::Model::default())),
        )
        .unwrap();
        let saved = sim.save().unwrap();
        for _ in 0..2 {
            sim.restore(&saved).unwrap();
            assert!(!sim.is_loading());
            assert!(sim.world().model("crate.model").is_some());
            sim.run(100.);
        }
        let mut pending = Sim::<Declared>::new(()).unwrap();
        let before = pending.world().hash();
        assert!(pending
            .restore(&saved)
            .unwrap_err()
            .to_string()
            .contains("awaits declared assets"));
        assert!(pending.is_loading());
        assert_eq!(pending.world().hash(), before);
        assert_eq!(pending.take_assets(), ["crate.model"]);
    }
}
