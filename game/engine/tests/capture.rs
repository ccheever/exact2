use exact_game::*;

#[derive(Default, exact_game::Args)]
struct Options {
    seed: u32,
    #[live]
    paused: bool,
    #[live]
    presses: u32,
}
#[derive(Default, Resource)]
struct Counts {
    held: u32,
    presses: u32,
    last_press: u32,
    touch: u32,
}
struct Fixture;
impl Game for Fixture {
    const ID: &'static str = "capture-fixture";
    const CAPTURE_SUPPORTED: bool = true;
    type Args = Options;
    fn register(w: &mut World, _: &Options) {
        w.register_resource::<Counts>();
    }
    fn setup(w: &mut World, _: &Options) {
        w.insert_resource(Counts::default());
    }
    fn actions() -> Actions {
        Actions::new()
            .button("move", &["KeyW"])
            .button_touch("touch", Region::Right)
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        let mut counts = w.resource_mut::<Counts>();
        counts.held += u32::from(input.held("move"));
        counts.touch += u32::from(input.held("touch"));
        counts.presses += args.presses.saturating_sub(counts.last_press);
        counts.last_press = args.presses;
    }
}
fn fixture() -> Sim<Fixture> {
    let mut sim = Sim::new(Options::default()).unwrap();
    sim.advance(9000.0, Clock::Seekable);
    sim.viewport(800.0, 600.0);
    sim
}
fn bind(sim: &mut Sim<Fixture>, paused: bool, presses: f64) {
    sim.bind(
        &[
            Value::Number(0.0),
            Value::Bool(paused),
            Value::Number(presses),
        ],
        None,
    )
    .unwrap();
}
fn recording() -> (Sim<Fixture>, Capture) {
    let mut sim = fixture();
    sim.key_down("KeyW"); // Intentional held input in checkpoint is preserved.
    sim.run(50.0);
    sim.start_capture("sha256:loaded-artifacts", CaptureLimits::default())
        .unwrap();
    sim.scheduled_input(InputEvent::Pointer {
        id: 7,
        phase: PointerPhase::Down,
        x: 700.0,
        y: 400.0,
        at_ms: 9050.0,
    });
    bind(&mut sim, false, 1.0);
    sim.run(100.0);
    bind(&mut sim, true, 1.0);
    sim.key_up("KeyW");
    sim.run(800.0);
    sim.viewport(600.0, 800.0);
    bind(&mut sim, false, 2.0);
    sim.run(100.0);
    let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
    (sim, capture)
}
#[test]
fn isolated_twice_interleaved_touch_bindings_pause_and_epoch() {
    let (sim, capture) = recording();
    let mut first =
        Sim::<Fixture>::replay_capture(&capture, "sha256:loaded-artifacts", None).unwrap();
    let second = Sim::<Fixture>::replay_capture(&capture, "sha256:loaded-artifacts", None).unwrap();
    assert_eq!(sim.world().hash(), first.world().hash());
    assert!(first
        .agent(r#"{"op":"state"}"#)
        .contains(r#""owner":"agent","clock":"controlled""#));
    assert_eq!(first.world().hash(), second.world().hash());
    assert_eq!(first.world().resource::<Counts>().presses, 2);
    assert!(first.world().resource::<Counts>().touch > 0);
    let seek =
        Sim::<Fixture>::replay_capture(&capture, "sha256:loaded-artifacts", Some(0)).unwrap();
    assert_eq!(seek.world().tick(), 3);
    assert!(seek.world().tick() < first.world().tick());
    assert!(capture.status().contains("world-only"));
}
#[test]
fn paused_checkpoint_and_pending_input_restore_without_losing_suffix() {
    let mut sim = fixture();
    bind(&mut sim, true, 0.0);
    sim.key_down("KeyW");
    sim.start_capture("build", CaptureLimits::default())
        .unwrap();
    sim.run(1000.0);
    bind(&mut sim, false, 1.0);
    sim.run(100.0);
    let capture = sim.stop_capture().unwrap();
    let replay = Sim::<Fixture>::replay_capture(capture, "build", None).unwrap();
    assert_eq!(replay.world().resource::<Counts>().held, 6);
}
#[test]
fn refused_capture_and_restore_keep_live_hash() {
    let (mut sim, capture) = recording();
    let before = sim.world().hash();
    assert!(Sim::<Fixture>::replay_capture(&capture, "other build", None).is_err());
    let mut corrupt = capture.to_bytes();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(Capture::from_bytes(&corrupt).is_err());
    assert!(sim.restore(b"broken world").is_err());
    assert_eq!(sim.world().hash(), before);
    assert!(sim
        .agent(r#"{"op":"state","capture":"start","build":"x","external":true}"#)
        .contains("unsupported external dependency"));
    assert_eq!(sim.world().hash(), before);
}
#[test]
fn budget_exhaustion_and_human_contamination_are_incomplete() {
    let mut sim = fixture();
    sim.start_capture(
        "build",
        CaptureLimits {
            events: 2,
            ..Default::default()
        },
    )
    .unwrap();
    sim.key_down("KeyW");
    sim.run(100.0);
    sim.key_up("KeyW");
    let capture = sim.stop_capture().unwrap();
    assert_eq!(capture.records(), 2);
    assert!(capture.status().contains("limit reached"));
    assert!(Sim::<Fixture>::replay_capture(capture, "build", None).is_err());
    sim.handoff(true);
    sim.advance(50_000.0, Clock::Seekable);
    sim.start_capture("build", CaptureLimits::default())
        .unwrap();
    sim.input_from(InputEvent::Blur { at_ms: 50_000.0 }, false);
    assert!(sim
        .stop_capture()
        .unwrap()
        .status()
        .contains("contaminated"));
}
#[test]
fn explicit_handoff_releases_held_input_and_discards_paused_wall_time() {
    let mut sim = fixture();
    let before = sim.save().unwrap();
    sim.agent(r#"{"op":"state"}"#);
    assert_eq!(before, sim.save().unwrap());
    sim.key_down("KeyW");
    sim.run(100.0);
    sim.handoff(true);
    sim.advance(90_000.0, Clock::Seekable);
    assert_eq!(sim.world().tick(), 6);
    sim.run(100.0);
    assert_eq!(sim.world().resource::<Counts>().held, 6);
    sim.handoff(false);
    sim.advance(900_000.0, Clock::Live);
    assert_eq!(sim.world().tick(), 12);
    let reply = sim.agent(r#"{"op":"state"}"#);
    assert!(reply.contains(r#""owner":"human""#));
}
#[test]
fn ticks_start_epoch_remain_exact_and_refuse_pause() {
    let mut sim = Sim::<Fixture>::new(Options::default()).unwrap();
    for tick in 1..=120 {
        let reply = sim.agent(r#"{"op":"clock","ticks":1}"#);
        assert!(!reply.contains("error"), "{reply}");
        assert_eq!(sim.world().tick(), tick);
    }
    let saved = sim.save().unwrap();
    bind(&mut sim, true, 0.0);
    let hash = sim.world().hash();
    assert!(sim.agent(r#"{"op":"clock","ticks":1}"#).contains("paused"));
    assert_eq!(sim.world().hash(), hash);
    sim.restore(&saved).unwrap();
    sim.agent(r#"{"op":"clock","ticks":7}"#);
    assert_eq!(sim.world().tick(), 127);
    let tick = sim.world().tick();
    sim.agent(r#"{"op":"clock","reload":true,"releaseInput":true,"now":900000}"#);
    assert_eq!(sim.world().tick(), tick);
    sim.agent(r#"{"op":"clock","ticks":1}"#);
    assert_eq!(sim.world().tick(), tick + 1);
}

#[test]
fn incidental_inspection_timestamp_and_viewport_do_not_step_or_record() {
    let mut sim = fixture();
    sim.key_down("KeyW");
    sim.start_capture("build", CaptureLimits::default())
        .unwrap();
    let before = sim.save().unwrap();
    for op in ["state", "tree", "logs"] {
        let reply = sim.agent(&format!(
            r#"{{"op":"{op}","now":999999,"width":123,"height":456}}"#
        ));
        assert!(!reply.contains("error"), "{reply}");
        assert_eq!(sim.save().unwrap(), before);
        assert_eq!(sim.capture().unwrap().records(), 0);
    }
    let hash = sim.world().hash();
    let refused = sim
        .agent(r#"{"op":"state","capture":"replay","data":"aabb","build":"wrong","now":999999}"#);
    assert!(refused.contains("error"));
    assert_eq!(sim.world().hash(), hash);
    assert_eq!(sim.world().tick(), 0);
}

#[test]
fn live_capture_after_lookahead_replays_checkpoint_and_continuation() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        for clock in [Clock::Live, Clock::Seekable] {
            let mut sim = Sim::<Fixture>::new(Options::default())
                .unwrap()
                .paranoid(mode);
            sim.frame_period(1000.0 / 60.0);
            for at in [0., 20., 40.] {
                sim.advance(at, clock);
            }
            let before = (sim.world().tick(), sim.world().hash());
            sim.start_capture("live-clock-regression", CaptureLimits::default())
                .unwrap();
            assert_eq!((sim.world().tick(), sim.world().hash()), before);
            for at in [60., 80., 100.] {
                sim.advance(at, clock);
            }
            let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
            let checkpoint =
                Sim::<Fixture>::replay_capture(&capture, "live-clock-regression", Some(0)).unwrap();
            assert_eq!(
                (checkpoint.world().tick(), checkpoint.world().hash()),
                before
            );
            let replay =
                Sim::<Fixture>::replay_capture(&capture, "live-clock-regression", None).unwrap();
            assert_eq!(replay.world().hash(), sim.world().hash());
            assert_eq!(replay.world().tick(), sim.world().tick());
            assert_eq!(replay.save().unwrap(), sim.save().unwrap());
            assert!(replay.world().tick() > before.0);
        }
    }
}

#[test]
fn live_capture_keeps_fractional_phase_period_changes_and_pending_device_input() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = Sim::<Fixture>::new(Options::default())
            .unwrap()
            .paranoid(mode);
        sim.viewport(800., 600.);
        let mut at = 1_234_567_895.123_456; // Host uptime beyond one day.
        sim.frame_period(1000. / 144.);
        sim.advance(at, Clock::Live);
        for _ in 0..7 {
            at += 1000. / 144.;
            sim.advance(at, Clock::Live);
        }
        sim.device_input(InputEvent::Key {
            code: "KeyW".into(),
            down: true,
            at_ms: at + 0.123456,
        });
        sim.start_capture("fractional-live-clock", CaptureLimits::default())
            .unwrap();
        for frame in 0..90 {
            let period = if frame < 30 {
                1000. / 144.
            } else if frame < 60 {
                1000. / 60.
            } else {
                1000. / 120.
            };
            sim.frame_period(period);
            at += period;
            if frame == 40 {
                sim.device_input(InputEvent::Key {
                    code: "KeyW".into(),
                    down: false,
                    at_ms: at + 0.12,
                });
            }
            sim.advance(at, Clock::Live);
        }
        let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
        let mut replay =
            Sim::<Fixture>::replay_capture(&capture, "fractional-live-clock", None).unwrap();
        assert!(sim.world().resource::<Counts>().held > 10);
        assert_eq!(replay.save().unwrap(), sim.save().unwrap());
        // Future live execution still uses the reconstructed display phase.
        for _ in 0..10 {
            at += 1000. / 120.;
            sim.advance(at, Clock::Live);
            replay.advance(at, Clock::Live);
        }
        assert_eq!(replay.save().unwrap(), sim.save().unwrap());
        let mut old = capture.to_bytes();
        old[6] = 1;
        assert!(Capture::from_bytes(&old)
            .err()
            .unwrap()
            .to_string()
            .contains("unsupported capture format"));
    }
}

#[test]
fn named_controls_replay_held_checkpoint_and_release_and_validate_before_recording() {
    let mut sim = fixture();
    let control = |name: &str, phase, at_ms| InputEvent::Control {
        name: name.into(),
        id: 47,
        phase,
        x: 1.,
        y: 2.,
        at_ms,
    };
    sim.scheduled_input(control("move", PointerPhase::Down, 9000.));
    sim.run(100.);
    assert_eq!(sim.held_controls(), ["move"]);
    sim.start_capture("named-controls", CaptureLimits::default())
        .unwrap();
    let before = sim.world().hash();
    sim.input_from(control("missing-action", PointerPhase::Down, 9100.), false);
    assert_eq!(sim.world().hash(), before);
    assert_eq!(sim.capture().unwrap().records(), 0);
    let validation = sim.stop_capture().unwrap().clone();
    assert_eq!(
        Sim::<Fixture>::replay_capture(&validation, "named-controls", None)
            .unwrap()
            .world()
            .hash(),
        before
    );
    // The refusal's diagnostic journal is outside the normalized accepted-input
    // suffix. Begin the byte-equality checkpoint after that diagnostic.
    sim.start_capture("named-controls", CaptureLimits::default())
        .unwrap();
    sim.scheduled_input(control("move", PointerPhase::Up, 9120.));
    sim.run(100.);
    assert!(sim.held_controls().is_empty());
    let capture = sim.stop_capture().unwrap().clone();
    let initial = Sim::<Fixture>::replay_capture(&capture, "named-controls", Some(0)).unwrap();
    assert_eq!(initial.held_controls(), ["move"]);
    let replay = Sim::<Fixture>::replay_capture(&capture, "named-controls", None).unwrap();
    assert!(replay.held_controls().is_empty());
    assert_eq!(replay.save().unwrap(), sim.save().unwrap());
    sim.scheduled_input(control("move", PointerPhase::Down, 9200.));
    assert_eq!(sim.held_controls(), ["move"]);
    sim.handoff(true);
    assert!(sim.held_controls().is_empty());
}
