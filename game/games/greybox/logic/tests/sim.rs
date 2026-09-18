use exact_game::{InputEvent, Sim, Transform, Vec3};
use greybox_logic::{Greybox, GreyboxArgs};

fn sim() -> Sim<Greybox> {
    Sim::new(GreyboxArgs {
        seed: 7,
        paused: false,
        restart: false,
    })
    .unwrap()
}

#[test]
fn forward_parity_and_seek_invariance() {
    let mut one = sim();
    let mut many = sim();
    let mut uneven = sim();
    for s in [&mut one, &mut many, &mut uneven] {
        s.key_down("KeyW");
    }
    one.run(1500.0);
    for _ in 1..=1500 {
        many.run(1.0);
    }
    for ms in [1.0, 2.0, 16.0, 88.0, 337.0, 456.0, 99.0, 500.0, 1.0] {
        uneven.run(ms);
    }
    assert_eq!(one.world().hash(), many.world().hash());
    assert_eq!(one.world().hash(), uneven.world().hash());
    println!(
        "GREYBOX position={:?} hash=0x{:016x}",
        one.world().get::<Transform>("player").unwrap().position,
        one.world().hash()
    );
    assert_eq!(
        one.world().get::<Transform>("player").unwrap().position,
        Vec3::new(0.0, 0.9, -5.3666644)
    );
    assert_eq!(one.world().hash(), 0x71f43e51a13cc49f);
}
#[test]
fn beacon_messages_journal_and_settle() {
    let mut s = sim();
    assert_eq!(s.take_published().as_deref(), Some("{\"beacons\":0}"));
    s.key_down("KeyW");
    s.run(1500.0);
    s.key_up("KeyW");
    s.tap("KeyE");
    s.run(17.0);
    assert_eq!(
        s.world().published("beacons").unwrap().as_number(),
        Some(1.0)
    );
    assert_eq!(s.take_published().as_deref(), Some("{\"beacons\":1}"));
    assert!(s.take_published().is_none());
    assert!(s
        .agent(r#"{"op":"logs","since":0}"#)
        .contains("tick=90 beacon-1 lit"));
    assert!(!s.quiescent());
    assert!(s.settle());
}
#[test]
fn save_mid_run_retains_clock_input_and_future_events() {
    let mut s = sim();
    s.key_down("KeyW");
    // Hosts can stamp future events; the relative API covers current input.
    s.input(InputEvent::Key {
        code: "Space".into(),
        down: true,
        at_ms: 1011.0,
    });
    s.input(InputEvent::Key {
        code: "Space".into(),
        down: false,
        at_ms: 1012.0,
    });
    s.run(713.123);
    let saved = s.save().unwrap();
    let mut restored = sim();
    restored.restore(&saved).unwrap();
    assert_eq!(s.world().hash(), restored.world().hash());
    assert_eq!(s.alpha(), restored.alpha());
    s.run(2000.0 - 713.123);
    restored.run(2000.0 - 713.123);
    assert_eq!(s.world().hash(), restored.world().hash());
    assert_eq!(
        s.world().get::<Transform>("player").unwrap().position,
        restored
            .world()
            .get::<Transform>("player")
            .unwrap()
            .position
    );
    assert_eq!(s.take_published(), restored.take_published());
    let hash = restored.world().hash();
    assert!(restored.restore(b"bad").is_err());
    assert_eq!(hash, restored.world().hash());
}
#[test]
fn agent_snapshots_and_pick() {
    let mut s = sim();
    s.run(0.0);
    let forms = [
        (
            "tree",
            include_str!("snapshots/tree.json"),
            r#"{"op":"tree","world":true}"#,
        ),
        (
            "state",
            include_str!("snapshots/state.json"),
            r#"{"op":"state"}"#,
        ),
        (
            "player",
            include_str!("snapshots/player.json"),
            r#"{"op":"state","entity":"player"}"#,
        ),
        (
            "layout",
            include_str!("snapshots/layout.json"),
            r#"{"op":"layout","entity":"player","width":800,"height":600,"scale":2}"#,
        ),
    ];
    for (name, expected, request) in forms {
        let got = s.agent(request);
        assert_eq!(got, expected.trim(), "{name}");
    }
    let screen = s.layout("player").unwrap().screen;
    let hit = s.pick(screen.center()).unwrap();
    assert_eq!(s.world().name(hit.entity), Some("player"));
    assert_eq!(
        s.agent(r#"{"op":"state","entity":"missing"}"#),
        r#"{"tick":0,"error":"no entity named `missing`"}"#
    );
    assert_eq!(
        s.agent(r#"{"op":"wat"}"#),
        r#"{"tick":0,"error":"unknown op `wat`"}"#
    );
    assert!(s
        .agent(r#"{"op":"layout","entity":"player","now":1000}"#)
        .starts_with("{\"tick\":60,"));
}
#[test]
fn headless_throughput() {
    let mut s = sim();
    s.key_down("KeyW");
    let start = std::time::Instant::now();
    s.run(60000.0);
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "GREYBOX 60s in {:.6}s = {:.1}x real time",
        elapsed,
        60.0 / elapsed
    );
    assert_eq!(s.world().tick(), 3600);
    // Timing is a diagnostic, not a flaky scheduling-dependent gate.
}

#[test]
fn a21_shorter_setup_preserves_materials_names_and_random_draws() {
    use exact_game::{Material, Rng};
    let s = sim();
    let w = s.world();
    let player = w.named("player").unwrap();
    assert_eq!(
        *w.get::<Material>(player).unwrap(),
        Material::rgb(0.8, 0.45, 0.15)
    );
    let mut rng = Rng::new(7);
    for i in 1..=3 {
        let e = w.named(&format!("crate-{i}")).unwrap();
        assert_eq!(
            w.get::<Transform>(e).unwrap().position,
            Vec3::new(rng.range(3.0..12.0), 0.5, rng.range(-12.0..8.0))
        );
    }
}

#[test]
fn soundscape_journals_ground_distance_beacon_and_camera_wind() {
    use exact_game::audio::{AudioSource, Voices};
    let mut s = sim();
    s.hold("KeyW", 1500.0);
    assert_eq!(
        s.world()
            .journal()
            .iter()
            .filter(|e| e.line.contains("sfx footstep at player "))
            .count(),
        11
    );
    assert!(s
        .world()
        .journal()
        .iter()
        .any(|e| e.line.contains("loop wind on ")));
    assert_eq!(s.get::<AudioSource>("camera").unwrap().sound, "wind");
    s.tap("KeyE");
    s.run(17.0);
    assert!(s
        .world()
        .journal()
        .iter()
        .any(|e| e.line.contains("sfx chime at beacon-1 ")));
    assert!(s
        .world()
        .resource::<Voices>()
        .voices
        .iter()
        .any(|v| v.sound == "chime"));
    let before = s
        .world()
        .journal()
        .iter()
        .filter(|e| e.line.contains("sfx footstep "))
        .count();
    s.key_down("KeyW");
    s.tap("Space");
    s.run(300.0);
    assert_eq!(
        s.world()
            .journal()
            .iter()
            .filter(|e| e.line.contains("sfx footstep "))
            .count(),
        before
    );
}

#[path = "../../../../physics/tests/compare.rs"]
mod paranoid;
#[test]
fn shared_paranoid_continuation() {
    paranoid::compare(sim, |sim| {
        sim.hold("KeyW", 1500.0);
        sim.tap("KeyE");
        sim.run(1000.0);
    });
}
