use crate::*;
#[derive(Default, Component)]
struct Tuning {
    speed: f32,
    label: String,
}
struct Editable;
impl Game for Editable {
    const ID: &'static str = "set-oracle";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "player",
            Tuning {
                speed: 4.,
                label: "player".into(),
            },
        );
    }
    fn tick(_: &mut World, input: &Input, _: &()) {
        assert!(input.messages().is_empty());
    }
}
#[test]
fn live_edits_are_stamped_saved_replayed_validated_and_refused_in_production() {
    let mut sim = Sim::<Editable>::new(()).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.post("set player Tuning.speed 8");
    assert_eq!(sim.world().require::<Tuning>("player").speed, 4.);
    let queued = sim.save().unwrap();
    let mut replay = Sim::<Editable>::new(()).unwrap();
    replay.restore(&queued).unwrap();
    replay.advance(0., Clock::Seekable);
    for s in [&mut sim, &mut replay] {
        s.advance(17., Clock::Seekable);
    }
    assert_eq!(sim.world().require::<Tuning>("player").speed, 8.);
    assert_eq!(sim.world().hash(), replay.world().hash());
    println!("set replay hash: 0x{:016x}", sim.world().hash());
    assert!(sim
        .agent(r#"{"op":"state"}"#)
        .contains("\"developmentEdits\":true"));
    let hash = sim.world().hash();
    for text in [
        "set missing Tuning.speed 1",
        "set player Missing.speed 1",
        "set player Tuning.unknown 1",
        "set player Tuning.speed 1e100",
        "set player Tuning.speed NaN",
        "set player Tuning.speed false",
    ] {
        sim.world.apply_set(text, true);
        assert_eq!(sim.world().hash(), hash, "{text}");
        assert!(sim
            .world()
            .journal()
            .last()
            .unwrap()
            .line
            .contains("set refused:"));
    }
    let mut production = Sim::<Editable>::new(()).unwrap();
    production.development_edits(false);
    production.restore(&queued).unwrap();
    production.advance(0., Clock::Seekable);
    production.advance(17., Clock::Seekable);
    assert_eq!(production.world().require::<Tuning>("player").speed, 4.);
    assert!(production
        .world()
        .journal()
        .last()
        .unwrap()
        .line
        .contains("production bake"));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.assert_pin("{}"))).is_err()
    );
    for _ in 0..4100 {
        sim.world().log("rollover");
    }
    let saved = sim.save().unwrap();
    replay.restore(&saved).unwrap();
    assert!(replay
        .agent(r#"{"op":"state"}"#)
        .contains("\"developmentEdits\":true"));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| replay.assert_pin("{}"))).is_err()
    );
}
