use exact_game::{Clock, InputEvent, Sim, Transform, Value, Vec3};
use greybox_logic::Greybox;

fn sim() -> Sim<Greybox> {
    let mut s = Sim::new(&[Value::Number(7.0), Value::Bool(false)]).unwrap();
    s.advance(0.0, Clock::Seekable);
    s
}
fn key(s: &mut Sim<Greybox>, code: &str, down: bool, at_ms: f64) {
    s.input(InputEvent::Key {
        code: code.into(),
        down,
        at_ms,
    });
}
fn position(s: &Sim<Greybox>) -> Vec3 {
    s.world()
        .get::<Transform>(s.world().named("player").unwrap())
        .unwrap()
        .position
}

#[test]
fn forward_parity_and_seek_invariance() {
    let mut one = sim();
    let mut many = sim();
    let mut uneven = sim();
    for s in [&mut one, &mut many, &mut uneven] {
        key(s, "KeyW", true, 0.0);
    }
    one.advance(1500.0, Clock::Seekable);
    for ms in 1..=1500 {
        many.advance(ms as f64, Clock::Seekable);
    }
    for ms in [1.0, 3.0, 19.0, 107.0, 444.0, 900.0, 999.0, 1499.0, 1500.0] {
        uneven.advance(ms, Clock::Seekable);
    }
    assert_eq!(one.world().hash(), many.world().hash());
    assert_eq!(one.world().hash(), uneven.world().hash());
    println!(
        "GREYBOX position={:?} hash=0x{:016x}",
        position(&one),
        one.world().hash()
    );
    assert!((position(&one) - Vec3::new(0.0, 0.9, -5.733332)).length() < 1e-4);
    assert_eq!(one.world().hash(), 0x70c17d4a69834418);
}
#[test]
fn beacon_messages_journal_and_settle() {
    let mut s = sim();
    assert_eq!(s.take_messages(), ["{\"beacons\":0}"]);
    key(&mut s, "KeyW", true, 0.0);
    s.advance(1500.0, Clock::Seekable);
    key(&mut s, "KeyW", false, 1500.0);
    key(&mut s, "KeyE", true, 1500.0);
    key(&mut s, "KeyE", false, 1500.0);
    s.advance(1517.0, Clock::Seekable);
    assert_eq!(s.world().published("beacons"), Some(Value::Number(1.0)));
    assert_eq!(s.take_messages(), ["{\"beacons\":1}"]);
    assert!(s.take_messages().is_empty());
    assert!(s
        .agent(r#"{"op":"logs","since":0}"#)
        .contains("tick=90 beacon-1 lit"));
    assert!(!s.quiescent());
    #[allow(non_snake_case)]
    #[derive(Default, exact_game::Data)]
    struct ClockReply {
        quiescent: bool,
        settleAt: f64,
    }
    for _ in 0..16 {
        let reply: ClockReply =
            exact_game::json::from_str(&s.agent(r#"{"op":"clock","settle":true}"#)).unwrap();
        if reply.quiescent {
            break;
        }
        s.advance(reply.settleAt, Clock::Seekable);
    }
    assert!(s.quiescent());
}
#[test]
fn save_mid_run_retains_clock_input_and_future_events() {
    let mut s = sim();
    key(&mut s, "KeyW", true, 0.0);
    key(&mut s, "Space", true, 1011.0);
    key(&mut s, "Space", false, 1012.0);
    s.advance(713.123, Clock::Seekable);
    let saved = s.save();
    let mut restored = sim();
    restored.restore(&saved).unwrap();
    assert_eq!(s.world().hash(), restored.world().hash());
    assert_eq!(s.alpha(), restored.alpha());
    s.advance(2000.0, Clock::Seekable);
    restored.advance(0.0, Clock::Seekable);
    restored.advance(2000.0 - 713.123, Clock::Seekable);
    assert_eq!(s.world().hash(), restored.world().hash());
    assert_eq!(position(&s), position(&restored));
    assert_eq!(s.take_messages(), restored.take_messages());
    let hash = restored.world().hash();
    assert!(restored.restore(b"bad").is_err());
    assert_eq!(hash, restored.world().hash());
}
#[test]
fn agent_snapshots_and_pick() {
    let mut s = sim();
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
    #[derive(Default, exact_game::Data)]
    struct Screen {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }
    #[derive(Default, exact_game::Data)]
    struct EntityLayout {
        screen: Screen,
    }
    #[derive(Default, exact_game::Data)]
    struct Layout {
        entity: EntityLayout,
    }
    let layout: Layout =
        exact_game::json::from_str(&s.agent(r#"{"op":"layout","entity":"player"}"#)).unwrap();
    let screen = layout.entity.screen;
    let hit = s.agent(&format!(
        "{{\"op\":\"layout\",\"x\":{},\"y\":{}}}",
        screen.x + screen.w * 0.5,
        screen.y + screen.h * 0.5
    ));
    assert!(hit.contains("\"name\":\"player\""), "{hit}");
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
    key(&mut s, "KeyW", true, 0.0);
    let start = std::time::Instant::now();
    s.advance(60000.0, Clock::Seekable);
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
    for seed in [-1.0, 1.5, 9_007_199_254_740_992.0] {
        assert!(
            Sim::<Greybox>::new(&[Value::Number(seed), Value::Bool(false)])
                .err()
                .unwrap()
                .contains("seed")
        );
    }
}
