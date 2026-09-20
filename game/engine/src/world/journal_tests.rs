use crate::{character::Character, *};

struct Probe;
impl Game for Probe {
    const ID: &'static str = "session-journal";
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        w.register::<Transform>();
        w.register::<Character>();
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", (Transform::default(), Character::default()));
        w.publish("score", 0);
        w.log("game setup");
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.log("game tick");
    }
}

#[derive(Default, Data)]
struct Logs {
    next: u64,
    from: u64,
    lines: Vec<String>,
}
fn logs<G: Game>(s: &mut Sim<G>, since: u64) -> Logs {
    json::from_str(&s.agent(&format!(r#"{{"op":"logs","since":{since}}}"#))).unwrap()
}

#[test]
fn session_telemetry_preserves_save_hash_and_interleaved_log_order() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = Sim::<Probe>::new(()).unwrap().paranoid(mode);
        let initial = sim.save().unwrap();
        let hash = sim.world().hash();
        let cursor = sim.world().journal_next();
        sim.handoff(true);
        sim.handoff(false);
        for owner in ["agent", "human"] {
            let reply = sim.agent(&format!(
                r#"{{"op":"clock","owner":"{owner}","now":12000}}"#
            ));
            assert!(!reply.contains("error"), "{reply}");
        }
        let refusal = sim.restore(b"bad save").unwrap_err();
        // The host owns reporting restore failures; exercise its unsaved sink.
        sim.world().session_log(&refusal);
        let report: crate::world::reload::Report = json::from_str(r#"{"omitted":17}"#).unwrap();
        report.log(sim.world());
        assert_eq!(sim.save().unwrap(), initial);
        assert_eq!(sim.world().hash(), hash);
        let entries = logs(&mut sim, cursor);
        assert_eq!(entries.lines.len(), 6);
        for (line, expected) in entries.lines.iter().zip([
            "control: agent attached; controlled clock",
            "control: agent detached; human input and live clock",
            "control: agent attached; controlled clock",
            "control: agent detached; human input and live clock",
            "restore refused:",
            "reload: 17 additional items omitted",
        ]) {
            assert!(line.starts_with("t=0 tick=0 "), "{line}");
            assert!(line.contains(expected), "{line}");
        }
        let cursor = entries.next;
        sim.world().log("game before");
        sim.world().session_log("capture note");
        sim.world().log("game after");
        assert_eq!(
            logs(&mut sim, cursor).lines,
            [
                "t=0 tick=0 game before",
                "t=0 tick=0 capture note",
                "t=0 tick=0 game after"
            ]
        );
        let saved = sim.save().unwrap();
        assert_ne!(
            saved, initial,
            "negative control: game events must be saved"
        );
        let mut fresh = Sim::<Probe>::from_save(&saved).unwrap();
        let restored = logs(&mut fresh, 0);
        assert_eq!(restored.lines.len(), 5);
        assert!(restored.lines[0].contains("spawn"));
        assert!(restored.lines[1].contains("publish"));
        assert!(restored.lines[2].contains("game setup"));
        assert!(restored.lines[3].contains("game before"));
        assert!(restored.lines[4].contains("game after"));
        assert_eq!(fresh.save().unwrap(), saved);
        sim.agent(r#"{"op":"clock","ticks":2}"#);
        let entries = logs(&mut sim, cursor);
        assert_eq!(entries.lines.len(), 5, "{mode:?}: no loss or duplication");
        assert!(entries.lines[3].contains("game tick"));
        assert!(entries.lines[4].contains("game tick"));
    }
}

#[test]
fn session_churn_cannot_evict_or_renumber_saved_game_events() {
    let mut noisy = Sim::<Probe>::new(()).unwrap();
    let clean = Sim::<Probe>::new(()).unwrap();
    for i in 0..10_000 {
        noisy.world().session_log(format_args!("host before {i}"));
        noisy.world().log(format_args!("game {i}"));
        noisy.world().session_log(format_args!("host after {i}"));
        clean.world().log(format_args!("game {i}"));
    }
    assert_eq!(noisy.save().unwrap(), clean.save().unwrap());
    assert_eq!(noisy.world().hash(), clean.world().hash());
    let entries = logs(&mut noisy, 0);
    assert_eq!(entries.next, 30_003);
    assert_eq!(entries.from, entries.next - 4096);
    assert_eq!(entries.lines.len(), 4096);
    assert!(entries.lines[4093].ends_with("host before 9999"));
    assert!(entries.lines[4094].ends_with("game 9999"));
    assert!(entries.lines[4095].ends_with("host after 9999"));
    let before = noisy.save().unwrap();
    noisy.world().session_log("x".repeat(65_536));
    noisy.world().session_log("x".repeat(65_537));
    let tail = logs(&mut noisy, entries.next);
    assert_eq!(tail.lines.len(), 2);
    assert!(tail.lines[0].ends_with(&"x".repeat(65_536)));
    assert!(tail.lines[1].contains("session journal refused: line exceeds 65536-byte budget"));
    assert_eq!(noisy.save().unwrap(), before);
}

#[test]
fn session_attach_during_asset_loading_survives_setup_without_entering_save() {
    struct Loading;
    impl Game for Loading {
        const ID: &'static str = "session-loading";
        const ASSETS: &'static [&'static str] = &["pending.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            Probe::setup(w, &());
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut early = Sim::<Loading>::new(()).unwrap();
    let mut late = Sim::<Loading>::new(()).unwrap();
    assert!(early.is_loading());
    early.handoff(true);
    let bytes = bin::to_vec(&asset::Model::default());
    early.asset("pending.model", Some(&bytes)).unwrap();
    late.asset("pending.model", Some(&bytes)).unwrap();
    late.handoff(true);
    assert!(!early.is_loading());
    assert_eq!(early.save().unwrap(), late.save().unwrap());
    assert_eq!(early.world().hash(), late.world().hash());
    let entries: Logs = json::from_str(&early.agent(r#"{"op":"logs"}"#)).unwrap();
    assert_eq!(entries.lines.len(), 4);
    assert!(entries.lines[0].contains("agent attached"));
    assert!(entries.lines[1].contains("spawn"));
    assert!(entries.lines[2].contains("publish"));
    assert!(entries.lines[3].contains("game setup"));
}

#[test]
fn session_reload_report_is_visible_but_absent_from_the_next_save() {
    let old = Sim::<Probe>::new(()).unwrap();
    let saved = old.save().unwrap();
    struct Edited;
    impl Game for Edited {
        const ID: &'static str = Probe::ID;
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            Probe::setup(w, &());
            w.get_mut::<Transform>("player").unwrap().position.x = 2.0;
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut next = Sim::<Edited>::new(()).unwrap();
    next.handoff(true);
    next.restore(&saved).unwrap();
    assert_eq!(
        next.world().get::<Transform>("player").unwrap().position.x,
        2.0
    );
    let before = next.save().unwrap();
    let hash = next.world().hash();
    let cursor = next.world().journal_next();
    next.reload.log(next.world());
    assert_eq!(next.save().unwrap(), before);
    assert_eq!(next.world().hash(), hash);
    let entries = logs(&mut next, cursor);
    assert_eq!(entries.lines.len(), 1);
    assert!(entries.lines[0].contains("reload applied:"));
    let mut fresh = Sim::<Probe>::from_save(&before).unwrap();
    assert!(!logs(&mut fresh, 0)
        .lines
        .iter()
        .any(|l| l.contains("reload")));
}

#[test]
fn session_restore_reinstates_saved_game_history_and_merges_same_tick_diagnostics() {
    let donor = Sim::<Probe>::new(()).unwrap();
    donor.world().log("saved alpha");
    donor.world().log("saved beta");
    let saved = donor.save().unwrap();
    let mut destination = Sim::<Probe>::new(()).unwrap();
    destination.handoff(true);
    destination.world().session_log("restore pending");
    destination.restore(&saved).unwrap();
    assert_eq!(destination.save().unwrap(), saved);
    let entries = logs(&mut destination, 0);
    assert_eq!(entries.next, 7);
    assert_eq!(entries.lines.len(), 7);
    assert!(entries.lines[0].ends_with("spawn #0"));
    assert!(entries.lines[1].contains("publish score"));
    assert!(entries.lines[2].ends_with("game setup"));
    assert!(entries.lines[3].ends_with("control: agent attached; controlled clock"));
    assert!(entries.lines[4].ends_with("restore pending"));
    assert!(entries.lines[5].ends_with("saved alpha"));
    assert!(entries.lines[6].ends_with("saved beta"));
    destination.restore(&saved).unwrap();
    assert_eq!(logs(&mut destination, 0).lines, entries.lines);
    assert_eq!(destination.save().unwrap(), saved);
}
