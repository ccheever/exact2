use crate::*;
#[derive(Default, Component)]
struct Tuning {
    speed: f32,
    label: String,
    direction: [f32; 3],
}
#[derive(Default, Args)]
struct EditOptions {
    #[restart]
    restart: bool,
}
struct Editable;
impl Game for Editable {
    const ID: &'static str = "set-oracle";
    type Args = EditOptions;
    fn setup(w: &mut World, _: &EditOptions) {
        w.spawn_named(
            "player",
            Tuning {
                speed: 4.,
                label: "player".into(),
                direction: [0.; 3],
            },
        );
    }
    fn tick(w: &mut World, input: &Input, _: &EditOptions) {
        for message in input.messages() {
            w.log(format_args!("gameplay: {message}"));
        }
    }
}
#[test]
fn live_edits_are_stamped_saved_replayed_validated_and_refused_in_production() {
    let mut sim = Sim::<Editable>::new(EditOptions::default()).unwrap();
    sim.advance(0., Clock::Seekable);
    let before = sim.world().hash();
    sim.post("set player Tuning.speed 8");
    assert_eq!(sim.world().hash(), before);
    assert!(!sim.world.set_seen.get());
    assert_eq!(sim.world().require::<Tuning>("player").speed, 4.);
    let queued = sim.save().unwrap();
    let mut replay = Sim::<Editable>::new(EditOptions::default()).unwrap();
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
        "set player Tuning.direction [1,2,3]",
        "set player Tuning.direction [1,2]",
        r#"set player Tuning.direction {"speeed":8}"#,
        "set player Tuning.direction [1,1e100,3]",
        r#"set player Tuning.direction [1,"bad",3]"#,
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
    let mut production = Sim::<Editable>::new(EditOptions::default()).unwrap();
    production.development_edits(false);
    production.restore(&queued).unwrap();
    production.advance(0., Clock::Seekable);
    production.advance(17., Clock::Seekable);
    assert_eq!(production.world().require::<Tuning>("player").speed, 4.);
    let mut pristine = Sim::<Editable>::new(EditOptions::default()).unwrap();
    pristine.advance(0., Clock::Seekable);
    pristine.advance(17., Clock::Seekable);
    assert_eq!(production.world().hash(), pristine.world().hash());
    assert!(production
        .world()
        .journal()
        .iter()
        .any(|e| e.line.contains("production bake")));
    assert!(!production.world.set_seen.get());
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

#[test]
fn only_admitted_edits_poison_pins_and_clean_restore_cannot_clear_them() {
    let mut sim = Sim::<Editable>::new(EditOptions::default()).unwrap();
    let clean = sim.save().unwrap();
    sim.advance(0., Clock::Seekable);
    for text in [
        "set difficulty hard",
        "set missing Tuning.speed 8",
        "set player Tuning.speed false",
    ] {
        sim.post(text);
    }
    sim.advance(17., Clock::Seekable);
    assert!(!sim.world.set_seen.get());
    let lines = sim.world().journal();
    for text in [
        "set difficulty hard",
        "set missing Tuning.speed 8",
        "set player Tuning.speed false",
    ] {
        assert!(lines
            .iter()
            .any(|e| e.line.contains(&format!("gameplay: {text}"))));
    }
    sim.post("set player Tuning.speed 8");
    sim.advance(34., Clock::Seekable);
    assert!(sim.edits_admitted);
    sim.bind(&[Value::Bool(true)], None).unwrap();
    assert!(sim.world.set_seen.get());
    sim.restore(&clean).unwrap();
    assert!(sim.world.set_seen.get());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.assert_pin("{}"))).is_err()
    );
    let mut old = sim.world().save();
    old[7] = 4;
    assert!(sim
        .world()
        .validate_saved(&old)
        .err()
        .unwrap()
        .to_string()
        .contains("EXGAME v4 predates v5"));
}
