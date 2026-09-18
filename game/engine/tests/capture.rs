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
    sim.input(InputEvent::Pointer {
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
    let first = Sim::<Fixture>::replay_capture(&capture, "sha256:loaded-artifacts", None).unwrap();
    let second = Sim::<Fixture>::replay_capture(&capture, "sha256:loaded-artifacts", None).unwrap();
    assert_eq!(sim.world().hash(), first.world().hash());
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
    let before = sim.save();
    sim.agent(r#"{"op":"state"}"#);
    assert_eq!(before, sim.save());
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
    let saved = sim.save();
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
    let before = sim.save();
    for op in ["state", "tree", "logs"] {
        let reply = sim.agent(&format!(
            r#"{{"op":"{op}","now":999999,"width":123,"height":456}}"#
        ));
        assert!(!reply.contains("error"), "{reply}");
        assert_eq!(sim.save(), before);
        assert_eq!(sim.capture().unwrap().records(), 0);
    }
    let hash = sim.world().hash();
    let refused = sim
        .agent(r#"{"op":"state","capture":"replay","data":"aabb","build":"wrong","now":999999}"#);
    assert!(refused.contains("error"));
    assert_eq!(sim.world().hash(), hash);
    assert_eq!(sim.world().tick(), 0);
}
