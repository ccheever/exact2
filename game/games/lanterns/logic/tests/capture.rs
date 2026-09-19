//! Simulation reproduction of the public native route; normalized inputs, not a UI claim.
use exact_game::{Args, Capture, CaptureLimits, Clock, Paranoid, Sim, Transform, Value};
use exact_game_physics::CapsuleController;
use lanterns_logic::{Lantern, Lanterns, Options, Session};

fn ticks(sim: &mut Sim<Lanterns>, count: u32) {
    let before = sim.world().tick();
    let reply = sim.agent(&format!(r#"{{"op":"clock","ticks":{count}}}"#));
    assert!(!reply.contains("error"), "{reply}");
    assert_eq!(sim.world().tick(), before + count as u64);
}
fn hold(sim: &mut Sim<Lanterns>, code: &str, count: u32) {
    sim.key_down(code);
    ticks(sim, count);
    sim.key_up(code);
}
fn press(sim: &mut Sim<Lanterns>, name: &str) {
    let index = <Options as Args>::FIELDS
        .iter()
        .position(|(field, _)| *field == name)
        .unwrap();
    let mut values = sim.args().values();
    let Value::Number(value) = &mut values[index] else {
        panic!("counter")
    };
    *value += 1.0;
    sim.bind(&values, None).unwrap();
}
fn steer(sim: &mut Sim<Lanterns>, x: f32, z: f32, tolerance: f32) {
    sim.move_to("player", exact_game::Vec3::new(x, 0.0, z), tolerance, 4.5)
        .unwrap();
}

fn land(sim: &mut Sim<Lanterns>, floor: f32) {
    for _ in 0..72 {
        if sim
            .world()
            .get::<CapsuleController>("player")
            .unwrap()
            .grounded
            && sim.global_position("player").unwrap().y - 0.65 > floor
        {
            return;
        }
        ticks(sim, 1);
    }
    panic!("landing failed at {:?}", sim.global_position("player"));
}
#[test]
fn crate_checkpoint_finishes_route_twice_and_restores_terminal_state() {
    let mut sim = game();
    ticks(&mut sim, 2);
    steer(&mut sim, -8.0, 12.0, 0.18);
    for index in 1..=11 {
        let name = format!("lantern-{index}");
        let at = sim
            .world()
            .get::<Transform>(name.as_str())
            .unwrap()
            .position;
        steer(&mut sim, at.x, at.z, 0.18);
        press(&mut sim, "light_press");
        ticks(&mut sim, 2);
        assert!(sim.world().get::<Lantern>(name.as_str()).unwrap().lit);
    }
    steer(&mut sim, 4.8, 8.0, 0.14);
    let before = sim.global_position("crate").unwrap();
    hold(&mut sim, "KeyD", 65);
    ticks(&mut sim, 15);
    let at = sim.global_position("crate").unwrap();
    assert!(
        at.x > before.x + 0.25 && at.x < 7.55 && (at.z - 8.0).abs() < 0.22,
        "{at:?}"
    );
    sim.start_capture(
        "test-executable:lanterns-route",
        CaptureLimits {
            bytes: 8 * 1024 * 1024,
            ..Default::default()
        },
    )
    .unwrap();
    steer(&mut sim, at.x - 1.5, at.z, 0.11);
    press(&mut sim, "jump_press");
    hold(&mut sim, "KeyD", 20);
    land(&mut sim, 0.9);
    assert!(
        (sim.global_position("player").unwrap().y
            - 0.65
            - (sim.global_position("crate").unwrap().y + 0.6))
            .abs()
            < 0.18
    );
    press(&mut sim, "jump_press");
    hold(&mut sim, "KeyD", 35);
    land(&mut sim, 2.2);
    steer(&mut sim, 10.0, 8.0, 0.18);
    press(&mut sim, "light_press");
    ticks(&mut sim, 2);
    assert_eq!(sim.world().resource::<Session>().phase, 2);
    assert_eq!(
        sim.world()
            .query::<&Lantern>()
            .iter()
            .filter(|(_, l)| l.lit)
            .count(),
        12
    );
    let hash = sim.world().hash();
    let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
    for _ in 0..2 {
        let replay = game()
            .replay_capture_using(&capture, "test-executable:lanterns-route", None)
            .unwrap();
        assert_eq!(replay.world().hash(), hash);
        assert_eq!(replay.world().resource::<Session>().phase, 2);
    }
    let terminal = sim.save().unwrap();
    let mut restored = game();
    restored.open_bound(&terminal).unwrap();
    assert_eq!(restored.world().hash(), hash);
    sim.start_capture("test-executable:lanterns-route", CaptureLimits::default())
        .unwrap();
    ticks(&mut sim, 10);
    let terminal_capture = sim.stop_capture().unwrap();
    let replay = game()
        .replay_capture_using(terminal_capture, "test-executable:lanterns-route", None)
        .unwrap();
    assert_eq!(replay.world().resource::<Session>().phase, 2);
    // Reuse the actual won instance; no save/open may normalize its live clock.
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        sim = sim.paranoid(mode);
        for clock in [Clock::Seekable, Clock::Live] {
            sim.handoff(false);
            sim.frame_period(1000.0 / 144.0);
            let mut at = 9_876_543.123_456;
            sim.advance(at, clock);
            sim.key_down("KeyE");
            at += 12.0;
            sim.advance(at, clock);
            let before = (sim.world().tick(), sim.world().hash());
            let elapsed = sim.world().resource::<Session>().elapsed;
            sim.start_capture("lanterns-won-live", CaptureLimits::default())
                .unwrap();
            assert_eq!((sim.world().tick(), sim.world().hash()), before);
            sim.key_up("KeyE");
            for period in [1000.0 / 144.0, 1000.0 / 60.0, 1000.0 / 120.0] {
                sim.frame_period(period);
                at += period;
                sim.advance(at, clock);
            }
            let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
            assert!(capture.records() > 0);
            let checkpoint = game()
                .replay_capture_using(&capture, "lanterns-won-live", Some(0))
                .unwrap();
            assert_eq!(
                (checkpoint.world().tick(), checkpoint.world().hash()),
                before
            );
            let replay = game()
                .replay_capture_using(&capture, "lanterns-won-live", None)
                .unwrap();
            assert!(replay.world().tick() > before.0);
            assert_eq!(replay.world().resource::<Session>().phase, 2);
            assert_eq!(replay.world().resource::<Session>().elapsed, elapsed);
            assert_eq!(replay.world().hash(), sim.world().hash());
            let actual = replay.save().unwrap();
            let expected = sim.save().unwrap();
            assert!(
                actual == expected,
                "won bytes differ in {mode:?}/{clock:?}; first byte {:?}, lengths {}/{}",
                actual.iter().zip(&expected).position(|(a, b)| a != b),
                actual.len(),
                expected.len()
            );
        }
    }
}

fn game() -> Sim<Lanterns> {
    game_sim(Options {
        seed: 1_041_003,
        started: true,
        sound: true,
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json"),
            &lanterns_logic::scene_types(),
            &lanterns_logic::scene_assets(),
        )
        .unwrap()
        .content,
        ..Default::default()
    })
}

#[test]
fn direct_crate_stall_names_sign_board_and_clear_side() {
    let mut sim = game();
    ticks(&mut sim, 2);
    let error = sim
        .move_to("player", exact_game::Vec3::new(4.8, 0.0, 8.0), 0.18, 4.5)
        .unwrap_err();
    for part in [
        "sign-board",
        "bounds",
        "nearestClearSide",
        "position",
        "160 bursts",
    ] {
        assert!(error.contains(part), "{error}");
    }
    assert!(!error.contains("nearestClearSide\":null"), "{error}");
    assert!(sim.world().tick() <= 1762);
    println!("{error}");
    let from = exact_game::Vec3::new(0.0, 1.3, 12.0);
    let to = exact_game::Vec3::new(4.8, 1.3, 8.0);
    // Removal is the negative control: a fabricated/empty diagnostic fails above.
    let sign = sim.world().named("sign-board").unwrap();
    sim.world_mut().despawn(sign);
    let clear = sim.route_diagnostic("player", from, to).unwrap();
    assert!(!clear.contains("sign-board"), "{clear}");
    assert!(sim
        .move_to("player", to, 0.0, 4.5)
        .unwrap_err()
        .contains("positive"));
}

// Terminal gameplay stops stepping physics, but host time and input still advance.
// Keep the actual live instance: restoring it before capture hid the trial failure.
#[test]
fn live_terminal_recapture_preserves_input_and_fractional_clock() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        for clock in [Clock::Seekable, Clock::Live] {
            let mut sim = game().paranoid(Paranoid::Off);
            sim.key_down("KeyW");
            ticks(&mut sim, 180 * 60);
            assert_eq!(sim.world().resource::<Session>().phase, 3);
            assert!(sim.world().get::<CapsuleController>("player").is_some());
            // Exercise reconstruction at the capture boundaries, without rebuilding
            // the entire three-minute setup in each diagnostic mode.
            let mut sim = sim.paranoid(mode);
            sim.handoff(false);
            sim.frame_period(1000.0 / 144.0);
            let mut at = 1_234_567_895.123_456;
            sim.advance(at, clock);
            sim.key_down("KeyE");
            at += 12.0; // Live lookahead executes a tick before its saved deadline.
            sim.advance(at, clock);
            let before = (sim.world().tick(), sim.world().hash());
            let elapsed = sim.world().resource::<Session>().elapsed;
            sim.start_capture("lanterns-terminal", CaptureLimits::default())
                .unwrap();
            assert_eq!((sim.world().tick(), sim.world().hash()), before);
            sim.key_up("KeyE");
            for period in [1000.0 / 144.0, 1000.0 / 60.0, 1000.0 / 120.0] {
                sim.frame_period(period);
                at += period;
                sim.advance(at, clock);
            }
            let capture = Capture::from_bytes(&sim.stop_capture().unwrap().to_bytes()).unwrap();
            assert!(capture.records() > 0);
            let checkpoint = game()
                .replay_capture_using(&capture, "lanterns-terminal", Some(0))
                .unwrap();
            assert_eq!(
                (checkpoint.world().tick(), checkpoint.world().hash()),
                before
            );
            let replay = game()
                .replay_capture_using(&capture, "lanterns-terminal", None)
                .unwrap();
            assert!(replay.world().tick() > before.0);
            assert_eq!(replay.world().resource::<Session>().phase, 3);
            assert_eq!(replay.world().resource::<Session>().elapsed, elapsed);
            assert_eq!(replay.world().hash(), sim.world().hash());
            assert_eq!(replay.save().unwrap(), sim.save().unwrap());
        }
    }
}

fn game_sim(args: Options) -> Sim<Lanterns> {
    Sim::with_assets(args, |name| {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets")
                .join(name),
        )
    })
    .unwrap()
}
