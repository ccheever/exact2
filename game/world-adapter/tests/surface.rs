use exact_gpu::{InputEvent, Restore, Surface, Value};
use exact_world::{Game, Paranoid, Sim};
use exact_world_adapter::WorldSurface;
#[path = "../../games/tally/logic/src/lib.rs"]
mod tally;
// Reuse the frozen kernel's allocator implementation; add no handwritten unsafe.
#[path = "../../../world/src/storage/counting.rs"]
mod counting;
fn clock(s: &mut WorldSurface<tally::Tally>, now: f64) {
    let reply = s
        .agent(&format!(r#"{{"op":"clock","now":{now}}}"#))
        .unwrap();
    assert!(!reply.contains("error"), "{reply}");
}
fn key(s: &mut WorldSurface<tally::Tally>, at_ms: f64, down: bool) {
    s.input(&InputEvent::Key {
        code: "KeyD".into(),
        key: "d".into(),
        down,
        repeat: false,
        at_ms,
    });
}
#[test]
fn no_device_ticks_publishes_and_restores_pending_input_exactly() {
    let mut s = WorldSurface::<tally::Tally>::default();
    s.bind(&[Value::Number(7.)], Some(0.)).unwrap();
    let first = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(first.contains(r#""ready":true"#), "{first}");
    assert!(!first.contains("no device"));
    let before = s.published().unwrap();
    clock(&mut s, 1000.);
    assert_ne!(
        before,
        s.published().unwrap(),
        "negative control: ticking must publish"
    );
    key(&mut s, 1001., true);
    key(&mut s, 1002., false);
    let saved = s.carry().unwrap().unwrap();
    clock(&mut s, 1100.);
    let expected = s.sim().unwrap().world().hash().unwrap();
    let mut t = WorldSurface::<tally::Tally>::default();
    t.bind(&[], Some(0.)).unwrap();
    t.restore(&saved, Restore::Open).unwrap();
    clock(&mut t, 100.);
    assert_eq!(expected, t.sim().unwrap().world().hash().unwrap());
    assert_eq!(s.carry().unwrap(), t.carry().unwrap());
    assert!(t.agent(r#"{"op":"tree"}"#).unwrap().contains("card-11"));
    assert!(t.agent(r#"{"op":"logs"}"#).unwrap().contains("lines"));
}
#[test]
fn queue_and_transport_refuse_at_explicit_bounds() {
    let mut s = WorldSurface::<tally::Tally>::default();
    s.bind(&[], Some(0.)).unwrap();
    for i in 0..1024 {
        key(&mut s, 1001., i % 2 == 0);
    }
    assert!(s.take_error().is_none());
    key(&mut s, 1001., true);
    assert!(s.take_error().unwrap().0.contains("1024"));
    clock(&mut s, 1100.);
    let before = s.carry().unwrap();
    assert!(s
        .agent(r#"{"op":"clock","ticks":216001}"#)
        .unwrap()
        .contains("216000"));
    assert!(s.agent(&" ".repeat(16385)).unwrap().contains("16384"));
    assert_eq!(before, s.carry().unwrap());
}
#[test]
fn tally_continuation_in_all_paranoid_modes() {
    let mut hashes = Vec::new();
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = Sim::<tally::Tally>::new(tally::Options { seed: 7 })
            .unwrap()
            .paranoid(mode);
        for i in 0..30 {
            for down in [true, false] {
                sim.input(exact_world::InputEvent::Action {
                    name: if i % 3 == 2 { "hold" } else { "draw" }.into(),
                    down,
                    at_ms: i as f64 * 100.,
                })
                .unwrap();
            }
            sim.run(100.).unwrap();
        }
        hashes.push(sim.world().hash().unwrap());
    }
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(hashes[1], hashes[2]);
}
#[test]
fn counted_tally_construction_first_tick_restore() {
    let (mut sim, construction) =
        counting::measure(|| Sim::<tally::Tally>::new(tally::Options { seed: 7 }).unwrap());
    let (_, tick) = counting::measure(|| sim.run(17.).unwrap());
    let save = sim.save().unwrap();
    let (restored, restore, histogram) =
        counting::histogram(|| Sim::<tally::Tally>::from_save(&save).unwrap());
    assert_eq!(sim.world().hash(), restored.world().hash());
    println!("Tally allocations/bytes: construction={construction:?}, first_tick={tick:?}, restore={restore:?}; save={} bytes; restore histogram={histogram:?}",save.len());
    assert!(construction.0 < 200 && construction.1 < 100_000);
    assert!(tick.0 < 100 && tick.1 < 30_000);
    assert!(restore.0 < 500 && restore.1 < 200_000);
    assert_eq!(tally::Tally::HZ, 60);
}

#[path = "../../tests/world-failure/logic/src/lib.rs"]
mod failing;
#[test]
fn tick_failure_reports_once_and_remains_inspectable_until_restore() {
    let mut s = WorldSurface::<failing::Fails>::default();
    s.bind(&[], Some(0.)).unwrap();
    let saved = s.carry().unwrap().unwrap();
    let reply = s.agent(r#"{"op":"clock","ticks":216000}"#).unwrap();
    assert!(reply.contains("fixture tick refused"), "{reply}");
    assert_eq!(s.sim().unwrap().world().tick(), 2);
    assert!(s.take_error().unwrap().0.contains("tick 3"));
    assert!(s.take_error().is_none());
    let state = s.agent(r#"{"op":"state"}"#).unwrap();
    assert!(
        state.contains(r#""failed":true"#) && state.contains("fixture tick refused"),
        "{state}"
    );
    assert!(state.contains(r#""ready":false"#));
    assert!(s.agent(r#"{"op":"tree"}"#).unwrap().contains("entities"));
    assert!(s
        .agent(r#"{"op":"logs"}"#)
        .unwrap()
        .contains("fixture tick refused"));
    for q in [
        r#"{"op":"clock","ticks":1}"#,
        r#"{"op":"clock","owner":"human"}"#,
        r#"{"op":"clock","reload":true}"#,
    ] {
        assert!(s.agent(q).unwrap().contains("fixture tick refused"));
        assert!(s.take_error().is_none());
    }
    assert!(s.carry().is_err());
    assert!(
        s.published().is_none(),
        "failed ticks must not publish partial state"
    );
    s.restore(&saved, Restore::Open).unwrap();
    assert!(s
        .agent(r#"{"op":"state"}"#)
        .unwrap()
        .contains(r#""failed":false"#));
    assert!(!s
        .agent(r#"{"op":"clock","ticks":1}"#)
        .unwrap()
        .contains("error"));
    assert_eq!(s.sim().unwrap().world().tick(), 2);
}

#[test]
fn shared_scalar_reader_admits_unicode_and_refuses_pressure_before_dispatch() {
    use exact_gpu::json::parse_fields;
    let text = r#"{"op":"state","entity":"escaped\n\"\uD83E\uDD8Aé","now":1.25e3,"reload":false,"extra":null}"#;
    let expected: serde_json::Value = serde_json::from_str(text).unwrap();
    let fields = parse_fields(text).unwrap();
    assert_eq!(fields[1].1.as_str(), expected["entity"].as_str());
    assert_eq!(fields[2].1.as_number(), Some(1250.));
    let maximum = format!(
        "{{{}}}",
        (0..64)
            .map(|i| format!(r#""k{i}":false"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(parse_fields(&maximum).unwrap().len(), 64);
    assert!(parse_fields(&maximum.replacen('{', "{\"overflow\":0,", 1))
        .unwrap_err()
        .contains("64 fields"));
    let mut s = WorldSurface::<tally::Tally>::default();
    s.bind(&[], Some(0.)).unwrap();
    let before = s.carry().unwrap();
    for text in [
        r#"{"op":"clock","ticks":1,"ticks":2}"#.into(),
        r#"{"op":"clock","ticks":+1}"#.into(),
        r#"{"op":"clock","ticks":01}"#.into(),
        r#"{"op":"clock","ticks":1.}"#.into(),
        r#"{"op":"clock","ticks":1}false"#.into(),
        r#"{"op":"clock","ticks":1,"x":"\uD800"}"#.into(),
        format!(
            r#"{{"op":"clock","ticks":1,"x":{}{}}}"#,
            "[".repeat(8000),
            "]".repeat(8000)
        ),
        " ".repeat(16385),
    ] {
        assert!(s.agent(&text).unwrap().contains("error"), "{text}");
        assert_eq!(s.carry().unwrap(), before);
        assert!(s.take_error().is_none());
    }
    assert!(!s
        .agent(r#"{"op":"clock","ticks":1}"#)
        .unwrap()
        .contains("error"));
    assert_ne!(
        s.carry().unwrap(),
        before,
        "negative control: accepted requests must drive"
    );
}

#[test]
fn oversized_returned_error_is_bounded_without_losing_failed_inspection() {
    struct LargeError;
    impl exact_world::Game for LargeError {
        const ID: &'static str = "large-error";
        type Args = ();
        fn setup(_: &mut exact_world::World, _: &()) -> Result<(), exact_world::DataError> {
            Ok(())
        }
        fn tick(
            _: &mut exact_world::World,
            _: &exact_world::Input,
            _: &(),
        ) -> Result<(), exact_world::DataError> {
            Err(exact_world::DataError::new("é".repeat(40_000)))
        }
    }
    let mut s = WorldSurface::<LargeError>::default();
    assert!(s.bind(&[], Some(0.)).unwrap_err().0.contains("truncated"));
    assert!(s.take_error().unwrap().0.len() < 4200);
    assert!(s.take_error().is_none());
    let state: serde_json::Value =
        serde_json::from_str(&s.agent(r#"{"op":"state"}"#).unwrap()).unwrap();
    assert_eq!(state["world"]["failed"], true);
    assert!(state["world"]["error"]
        .as_str()
        .unwrap()
        .contains("truncated"));
    assert!(s.agent(r#"{"op":"tree"}"#).unwrap().contains("entities"));
    assert!(s.agent(r#"{"op":"logs"}"#).unwrap().contains("lines"));
    assert!(s
        .agent(r#"{"op":"clock","ticks":1}"#)
        .unwrap()
        .contains("truncated"));
}

#[test]
fn checkpoint_is_sim_save_and_clock_is_canonical_after_fractional_settle_and_restore() {
    let mut surface = WorldSurface::<tally::Tally>::default();
    surface.bind(&[], Some(0.)).unwrap();
    clock(&mut surface, 0.0006);
    let save = surface.sim().unwrap().save().unwrap();
    assert_eq!(surface.carry().unwrap().unwrap(), save);
    // Tally's heartbeat never settles: the refusal must still retain the Sim's
    // exact microsecond clock after all 3,600 admitted ticks.
    let result = surface
        .agent(r#"{"op":"clock","settle":true,"now":0.0006}"#)
        .unwrap();
    assert!(result.contains("settle tick budget exhausted"), "{result}");
    let bytes = surface.carry().unwrap().unwrap();
    let sim = Sim::<tally::Tally>::from_save(&bytes).unwrap();
    assert_eq!(
        bytes,
        sim.save().unwrap(),
        "no surface envelope or shadow clock"
    );
    let mut restored = WorldSurface::<tally::Tally>::default();
    restored.bind(&[], Some(0.0006)).unwrap();
    restored.restore(&bytes, Restore::Open).unwrap();
    for target in [&mut surface, &mut restored] {
        key(target, 1.0006, true);
        key(target, 2.0006, false);
        clock(target, 100.0006);
    }
    assert_eq!(surface.carry().unwrap(), restored.carry().unwrap());
    assert_eq!(
        surface
            .sim()
            .unwrap()
            .world()
            .resource::<tally::Round>()
            .drawn,
        1
    );
    let before = restored.carry().unwrap();
    assert!(restored.restore(b"EXSURF\0\x01", Restore::Open).is_err());
    assert_eq!(restored.carry().unwrap(), before);
}

#[derive(Default, exact_world::Args)]
struct FailureArgs {
    restart: bool,
}
struct BrokenOwnership;
impl Game for BrokenOwnership {
    const ID: &'static str = "broken-ownership";
    type Args = FailureArgs;
    fn setup(w: &mut exact_world::World, _: &FailureArgs) -> Result<(), exact_world::DataError> {
        w.register::<tally::Owner>()?;
        let parent = w.spawn_named("owner", tally::Owner)?;
        let child = w.spawn_named("child", tally::Owner)?;
        w.set_parent(child, Some(parent))?;
        Ok(())
    }
    fn tick(
        w: &mut exact_world::World,
        _: &exact_world::Input,
        _: &FailureArgs,
    ) -> Result<(), exact_world::DataError> {
        if w.tick() > 0 {
            w.despawn(w.named("owner").unwrap())?;
            w.emit("must not escape")?;
            return Err(exact_world::DataError::new("original failure"));
        }
        Ok(())
    }
}
#[test]
fn broken_ownership_failure_is_inspectable_and_timestamped_restart_recovers() {
    let mut s = WorldSurface::<BrokenOwnership>::default();
    s.bind(&[], Some(0.)).unwrap();
    assert!(s
        .agent(r#"{"op":"clock","ticks":1}"#)
        .unwrap()
        .contains("original failure"));
    assert!(s.sim().unwrap().world().hash().is_err());
    for op in ["state", "tree"] {
        let text = s.agent(&format!(r#"{{"op":"{op}"}}"#)).unwrap();
        assert!(
            text.contains("original failure") && text.contains(r#""failed":true"#),
            "{text}"
        );
        assert!(
            text.contains("child") || text.contains("resources"),
            "{text}"
        );
    }
    assert!(s.messages().is_empty());
    s.bind(&[Value::Bool(true)], Some(1000.)).unwrap();
    assert_eq!(s.sim().unwrap().world().tick(), 1);
    assert!(s.sim().unwrap().world().hash().is_ok());
    assert!(s.take_error().is_none());
}
#[test]
fn reload_uses_plain_values_indices_and_releases_sim_input() {
    let mut s = WorldSurface::<tally::Tally>::default();
    s.bind(&[Value::Number(7.)], Some(0.)).unwrap();
    key(&mut s, 1., true);
    clock(&mut s, 17.);
    assert!(s.sim().unwrap().input_state().key("KeyD"));
    let text = s
        .agent(r#"{"op":"clock","reload":true,"releaseInput":true,"now":17}"#)
        .unwrap();
    let reply: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(reply["reload"]["values"], serde_json::json!([7.]));
    assert_eq!(reply["reload"]["setupIndices"], serde_json::json!([0]));
    assert!(!s.sim().unwrap().input_state().key("KeyD"));
}
#[test]
fn log_transport_repeats_since_and_returns_real_lines_and_cursors() {
    let mut s = WorldSurface::<tally::Tally>::default();
    s.bind(&[], Some(0.)).unwrap();
    let read = |s: &mut WorldSurface<tally::Tally>, since| -> serde_json::Value {
        serde_json::from_str(
            &s.agent(&format!(r#"{{"op":"logs","since":{since}}}"#))
                .unwrap(),
        )
        .unwrap()
    };
    let page = read(&mut s, 0);
    assert!(
        page["from"].is_number() && page["next"].is_number(),
        "{page}"
    );
    assert!(!page["lines"].as_array().unwrap().is_empty());
    assert!(page["lines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|line| line.is_string()));
    assert_eq!(
        page["next"].as_u64().unwrap() - page["from"].as_u64().unwrap(),
        page["lines"].as_array().unwrap().len() as u64
    );
    assert_eq!(page, read(&mut s, 0));
    assert_eq!(
        read(&mut s, page["next"].as_u64().unwrap())["lines"],
        serde_json::json!([])
    );
}
thread_local! { static WRITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[derive(Default)]
struct Counted(u32);
impl exact_world::Data for Counted {
    fn write(&self, w: &mut dyn exact_world::Writer) {
        WRITES.with(|n| n.set(n.get() + 1));
        self.0.write(w);
    }
    fn read(&mut self, r: &mut dyn exact_world::Reader) -> Result<(), exact_world::DataError> {
        self.0.read(r)
    }
}
#[derive(Default, exact_world::Component)]
struct IdleComponent(Counted);
struct Idle;
impl Game for Idle {
    const ID: &'static str = "idle";
    type Args = ();
    fn setup(w: &mut exact_world::World, _: &()) -> Result<(), exact_world::DataError> {
        w.register::<IdleComponent>()?;
        for _ in 0..1000 {
            w.spawn(IdleComponent::default())?;
        }
        Ok(())
    }
    fn tick(
        _: &mut exact_world::World,
        _: &exact_world::Input,
        _: &(),
    ) -> Result<(), exact_world::DataError> {
        Ok(())
    }
}
#[test]
fn thousand_idle_advances_allocate_and_observe_nothing_but_really_tick() {
    let mut s = WorldSurface::<Idle>::default();
    s.bind(&[], Some(0.)).unwrap();
    s.published();
    WRITES.with(|n| n.set(0));
    let (_, cost) = counting::measure(|| {
        for i in 1..=1000 {
            assert!(Surface::advance(&mut s, i as f64 * 17.));
        }
    });
    assert_eq!(cost, (0, 0));
    assert_eq!(WRITES.with(|n| n.get()), 0);
    assert!(s.sim().unwrap().world().tick() > 1000);
    assert!(!Surface::advance(&mut s, 17000.));
    s.agent(r#"{"op":"state"}"#);
    assert_eq!(
        WRITES.with(|n| n.get()),
        1000,
        "inspection is the positive observation control"
    );
}

#[test]
fn native_headless_abi_retains_failure_reply_inspection_and_restart() {
    use exact_gpu::native as abi;
    static REGISTRY: exact_gpu::Registry = exact_gpu::Registry {
        surfaces: &[("world", 1, || {
            Box::new(WorldSurface::<BrokenOwnership>::default())
        })],
        shaders: &[],
    };
    abi::load_headless(&REGISTRY);
    let id = abi::create_headless("world");
    assert_ne!(id, 0);
    assert_eq!(abi::bind_at(id, "[]", Some(0.)), 0);
    assert!(!abi::advance(id, 17.));
    assert!(abi::error().contains("original failure"));
    assert!(abi::agent(id, r#"{"op":"state"}"#).contains("original failure"));
    assert!(abi::agent(id, r#"{"op":"tree"}"#).contains("child"));
    assert!(abi::messages(id).is_none());
    assert_eq!(abi::bind_at(id, "[true]", Some(1000.)), 0);
    assert!(abi::agent(id, r#"{"op":"state"}"#).contains(r#""failed":false"#));
    abi::unload();
}
#[test]
fn host_input_with_nonfinite_stamp_refuses_without_mutation() {
    let mut s = WorldSurface::<tally::Tally>::default();
    s.bind(&[], Some(0.)).unwrap();
    let before = s.carry().unwrap();
    for stamp in [f64::NAN, f64::INFINITY, -1.] {
        key(&mut s, stamp, true);
        assert!(s.take_error().is_some(), "stamp={stamp}");
        assert_eq!(s.carry().unwrap(), before);
    }
}
