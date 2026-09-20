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
