use exact_game::*;
use std::collections::BTreeMap;

#[derive(Default, Data)]
struct Tuning {
    speed: f32,
    recoil: asset::Curve,
}
#[derive(Default, Data)]
struct Labels {
    title: String,
}
#[derive(Default, Component)]
struct Seen {
    speed: f32,
    title: String,
}
struct Tables;
impl Game for Tables {
    const ID: &'static str = "tables";
    const HZ: u32 = 1000;
    const TABLES: &'static [asset::Table] = &[
        asset::Table::of::<Tuning>("weapons.json"),
        asset::Table::of::<Labels>("labels.json"),
    ];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", (Seen::default(),));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let tuning = w.table::<Tuning>("weapons.json").unwrap();
        let title = w.table::<Labels>("labels.json").unwrap().title.clone();
        *w.require_mut::<Seen>("player") = Seen {
            speed: tuning.speed,
            title,
        };
    }
}
fn rows(speed: f32, title: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "weapons.json".into(),
            format!(r#"{{"speed":{speed},"recoil":[[0,0],[1,2]]}}"#),
        ),
        ("labels.json".into(), format!(r#"{{"title":"{title}"}}"#)),
    ])
}
fn deliver(sim: &mut Sim<Tables>, rows: BTreeMap<String, String>) -> Result<(), String> {
    sim.asset(
        asset::TABLE_ASSET,
        Some(json::to_string(&rows).unwrap().as_bytes()),
    )
}
fn sim(mode: Paranoid) -> Sim<Tables> {
    let mut sim = Sim::new(()).unwrap().paranoid(mode);
    deliver(&mut sim, rows(4.0, "first")).unwrap();
    sim
}
#[test]
fn cached_reads_and_whole_transactions_activate_only_on_a_tick() {
    let mut sim = sim(Paranoid::Off);
    let before = sim.world().hash();
    let first = sim.world().table::<Tuning>("weapons.json").unwrap() as *const _;
    for _ in 0..1000 {
        assert_eq!(
            first,
            sim.world().table::<Tuning>("weapons.json").unwrap() as *const _
        );
    }
    assert_eq!(
        sim.world()
            .table::<Tuning>("weapons.json")
            .unwrap()
            .recoil
            .at(0.25),
        0.5
    );
    assert!(sim.world().table::<Labels>("weapons.json").is_err());
    deliver(&mut sim, rows(8.0, "second")).unwrap();
    assert_eq!(
        sim.world().hash(),
        before,
        "admission does not advance simulation"
    );
    assert_eq!(
        sim.world().table::<Tuning>("weapons.json").unwrap().speed,
        4.0
    );
    assert!(!sim.quiescent());
    sim.run(1.0);
    assert_eq!(sim.get::<Seen>("player").unwrap().speed, 8.0);
    assert_eq!(sim.get::<Seen>("player").unwrap().title, "second");
    assert_ne!(sim.world().hash(), before);
    let pins = format!(
        r#"{{"game":"tables","ticks":{{"1":"0x{:016x}"}}}}"#,
        sim.world().hash()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.assert_pin(&pins))).is_err()
    );
    let mut bad = rows(12.0, "bad");
    bad.insert("labels.json".into(), r#"{"title":123}"#.into());
    assert!(deliver(&mut sim, bad).unwrap_err().contains("labels.json"));
    let mut missing = rows(12.0, "bad");
    missing.insert("weapons.json".into(), r#"{"recoil":[[0,0]]}"#.into());
    assert!(json::from_complete_str::<Tuning>(&missing["weapons.json"]).is_err());
    let error = deliver(&mut sim, missing).unwrap_err();
    assert!(
        error.contains("speed") && error.contains("weapons.json"),
        "{error}"
    );
    sim.run(1.0);
    assert_eq!(sim.get::<Seen>("player").unwrap().speed, 8.0);
    assert_eq!(sim.get::<Seen>("player").unwrap().title, "second");
    assert!(format!("{:?}", sim.world().journal()).contains("missing field"));
    assert!(sim
        .agent(r#"{"op":"state","resources":true}"#)
        .contains("TableValues"));
}
#[test]
fn two_swaps_replay_retained_bytes_and_survive_saves_in_every_mode() {
    let mut expected = None;
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut driven = sim(mode);
        let mut hashes = Vec::new();
        for tick in 1..=64 {
            if tick == 5 {
                deliver(&mut driven, rows(8.0, "second")).unwrap();
            }
            if tick == 20 {
                deliver(&mut driven, rows(2.0, "third")).unwrap();
            }
            driven.run(1.0);
            hashes.push(driven.world().hash());
        }
        if let Some(expected) = &expected {
            assert_eq!(&hashes, expected);
        } else {
            expected = Some(hashes.clone());
        }
        let saved = driven.save().unwrap();
        let mut restored = Sim::<Tables>::new(()).unwrap();
        deliver(&mut restored, rows(2.0, "third")).unwrap();
        restored.restore(&saved).unwrap();
        assert_eq!(restored.world().hash(), driven.world().hash());
        let retained = restored.table_inputs();
        assert_eq!(
            retained.iter().map(|i| i.tick).collect::<Vec<_>>(),
            [0, 5, 20]
        );
        let serialized = bin::to_vec(&retained);
        let retained = bin::from_slice::<Vec<asset::TableInput>>(&serialized).unwrap();
        let mut replay = sim(mode);
        replay.replay_tables(&retained).unwrap();
        for expected in hashes {
            replay.run(1.0);
            assert_eq!(replay.world().hash(), expected);
        }
        let mut changed = sim(mode);
        let before = changed.world().hash();
        let error = changed.restore(&saved).unwrap_err().to_string();
        assert!(
            error.contains("weapons.json") || error.contains("labels.json"),
            "{error}"
        );
        assert_eq!(changed.world().hash(), before);
    }
}
#[test]
fn pending_transaction_is_saved_and_a_reverted_edit_is_not_lost() {
    let mut driven = sim(Paranoid::Off);
    deliver(&mut driven, rows(8.0, "second")).unwrap();
    let saved = driven.save().unwrap();
    let mut restored = sim(Paranoid::Off);
    restored.restore(&saved).unwrap();
    driven.run(1.0);
    restored.run(1.0);
    assert_eq!(driven.world().hash(), restored.world().hash());
    deliver(&mut driven, rows(2.0, "third")).unwrap();
    deliver(&mut driven, rows(8.0, "second")).unwrap();
    driven.run(1.0);
    assert_eq!(driven.get::<Seen>("player").unwrap().speed, 8.0);
}
#[test]
fn curve_rejects_bad_points_and_clamps_endpoints() {
    for text in ["[[1,0],[0,1]]", "[[1,0],[1,1]]"] {
        assert!(json::from_str::<asset::Curve>(text).is_err());
    }
    let curve: asset::Curve = json::from_str("[[0,1],[2,5]]").unwrap();
    assert_eq!(curve.at(-1.0), 1.0);
    assert_eq!(curve.at(1.0), 3.0);
    assert_eq!(curve.at(3.0), 5.0);
}

#[derive(Default, Args)]
struct Options {
    #[live]
    paused: bool,
    #[restart]
    restart: bool,
}
struct SetupTables;
impl Game for SetupTables {
    const ID: &'static str = "setup-tables";
    const HZ: u32 = 1000;
    const TABLES: &'static [asset::Table] = Tables::TABLES;
    const ASSETS: &'static [&'static str] = &["barrier.model"];
    type Args = Options;
    fn setup(w: &mut World, _: &Options) {
        let speed = w.table::<Tuning>("weapons.json").unwrap().speed;
        w.spawn_named(
            "player",
            (Seen {
                speed,
                title: String::new(),
            },),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &Options) {}
    fn paused(args: &Options) -> bool {
        args.paused
    }
}
fn setup_delivery(s: &mut Sim<SetupTables>, speed: f32) {
    s.asset(
        asset::TABLE_ASSET,
        Some(json::to_string(&rows(speed, "first")).unwrap().as_bytes()),
    )
    .unwrap();
}
#[test]
fn restart_and_initial_barrier_admit_the_latest_pending_transaction() {
    let mut s = Sim::<SetupTables>::new(Options::default()).unwrap();
    setup_delivery(&mut s, 4.0);
    setup_delivery(&mut s, 8.0);
    assert!(s.is_loading());
    s.asset(
        "barrier.model",
        Some(&bin::to_vec(&asset::Model::default())),
    )
    .unwrap();
    assert_eq!(s.get::<Seen>("player").unwrap().speed, 8.0);
    assert_eq!(s.table_inputs().len(), 1);
    s.bind(&[Value::Bool(true), Value::Bool(false)], None)
        .unwrap();
    setup_delivery(&mut s, 12.0);
    setup_delivery(&mut s, 16.0);
    s.bind(&[Value::Bool(true), Value::Bool(true)], None)
        .unwrap();
    assert_eq!(s.get::<Seen>("player").unwrap().speed, 16.0);
    assert_eq!(s.world().tick(), 0);
    assert_eq!(s.table_inputs().len(), 1);
}
#[test]
fn paused_table_delivery_settles_without_a_tick_and_resumes_on_the_first_tick() {
    let mut s = Sim::<SetupTables>::new(Options::default()).unwrap();
    setup_delivery(&mut s, 4.0);
    s.asset(
        "barrier.model",
        Some(&bin::to_vec(&asset::Model::default())),
    )
    .unwrap();
    s.bind(&[Value::Bool(true), Value::Bool(false)], None)
        .unwrap();
    let hash = s.world().hash();
    setup_delivery(&mut s, 8.0);
    assert!(s.quiescent());
    let state = s.agent(r#"{"op":"clock","settle":true}"#);
    assert!(
        state.contains("awaiting resume") && state.contains("weapons.json"),
        "{state}"
    );
    s.run(100.0);
    assert_eq!(s.world().tick(), 0);
    assert_eq!(s.world().hash(), hash);
    s.bind(&[Value::Bool(false), Value::Bool(false)], None)
        .unwrap();
    s.run(1.0);
    assert_eq!(s.world().tick(), 1);
    assert_eq!(
        s.world().table::<Tuning>("weapons.json").unwrap().speed,
        8.0
    );
    // No component changes in tick: this isolates the table resource's hash contribution.
    let mut old = Sim::<SetupTables>::new(Options::default()).unwrap();
    setup_delivery(&mut old, 4.0);
    old.asset(
        "barrier.model",
        Some(&bin::to_vec(&asset::Model::default())),
    )
    .unwrap();
    old.run(1.0);
    assert_ne!(s.world().hash(), old.world().hash());
}
#[test]
fn restore_refuses_a_different_declared_type_even_with_identical_json() {
    #[derive(Default, Data)]
    struct Renamed {
        speed: f32,
        recoil: asset::Curve,
    }
    struct Changed;
    impl Game for Changed {
        const ID: &'static str = "tables";
        const HZ: u32 = 1000;
        const TABLES: &'static [asset::Table] = &[
            asset::Table::of::<Renamed>("weapons.json"),
            asset::Table::of::<Labels>("labels.json"),
        ];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            Tables::setup(w, &());
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let saved = sim(Paranoid::Off).save().unwrap();
    let mut changed = Sim::<Changed>::new(()).unwrap();
    changed
        .asset(
            asset::TABLE_ASSET,
            Some(json::to_string(&rows(4.0, "first")).unwrap().as_bytes()),
        )
        .unwrap();
    let before = changed.save().unwrap();
    let error = changed.restore(&saved).unwrap_err().to_string();
    for name in ["weapons.json", "Tuning", "Renamed", "type identity"] {
        assert!(error.contains(name), "{error}");
    }
    assert_eq!(changed.save().unwrap(), before);
}
#[test]
fn replay_export_includes_pending_transactions_and_curve_errors_name_the_point() {
    let mut driven = sim(Paranoid::Off);
    deliver(&mut driven, rows(8.0, "second")).unwrap();
    let mut replay = sim(Paranoid::Off);
    replay.replay_tables(&driven.table_inputs()).unwrap();
    driven.run(1.0);
    replay.run(1.0);
    assert_eq!(driven.world().hash(), replay.world().hash());
    for point in ["[0]", "[0,1,2]", "[]"] {
        let mut bad = rows(8.0, "second");
        bad.insert(
            "weapons.json".into(),
            format!(r#"{{"speed":8,"recoil":[{point},[1,2]]}}"#),
        );
        let error = deliver(&mut driven, bad).unwrap_err();
        assert!(
            error.contains("weapons.json.Tuning.recoil.0") && error.contains("exactly two"),
            "{error}"
        );
    }
}

#[test]
fn live_edits_interleave_with_replay_and_restore_in_every_mode() {
    let mut expected = None;
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut driven = sim(mode);
        let mut inputs = driven.table_inputs();
        inputs.extend([
            asset::TableInput {
                tick: 5,
                rows: rows(5.0, "five"),
            },
            asset::TableInput {
                tick: 20,
                rows: rows(20.0, "twenty"),
            },
        ]);
        driven.replay_tables(&inputs).unwrap();
        // Equal to the last future input: it must still be admitted at tick 1.
        deliver(&mut driven, rows(20.0, "twenty")).unwrap();
        deliver(&mut driven, rows(2.0, "live last")).unwrap();
        assert_eq!(
            driven
                .table_inputs()
                .iter()
                .map(|i| i.tick)
                .collect::<Vec<_>>(),
            [0, 1, 1, 5, 20]
        );
        let saved = driven.save().unwrap();
        let mut restored = sim(mode);
        restored.restore(&saved).unwrap();
        assert_eq!(restored.save().unwrap(), saved);
        let mut hashes = Vec::new();
        for tick in 1..=24 {
            if tick == 5 {
                // The replay row at the same tick runs first; live arrival runs last.
                for s in [&mut driven, &mut restored] {
                    deliver(s, rows(6.0, "live five")).unwrap();
                }
            }
            driven.run(1.0);
            restored.run(1.0);
            assert_eq!(driven.world().hash(), restored.world().hash());
            assert_eq!(
                driven.get::<Seen>("player").unwrap().speed,
                if tick < 5 {
                    2.0
                } else if tick < 20 {
                    6.0
                } else {
                    20.0
                }
            );
            hashes.push(driven.world().hash());
        }
        if let Some(expected) = &expected {
            assert_eq!(&hashes, expected);
        } else {
            expected = Some(hashes);
        }
    }
}

#[test]
fn restore_distinguishes_same_named_types_in_different_modules() {
    mod movement {
        use exact_game::*;
        #[derive(Default, Data)]
        pub struct Tuning {
            speed: f32,
            recoil: asset::Curve,
        }
    }
    struct Changed;
    impl Game for Changed {
        const ID: &'static str = "tables";
        const TABLES: &'static [asset::Table] = &[
            asset::Table::of::<movement::Tuning>("weapons.json"),
            asset::Table::of::<Labels>("labels.json"),
        ];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            Tables::setup(w, &());
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let saved = sim(Paranoid::Off).save().unwrap();
    let mut changed = Sim::<Changed>::new(()).unwrap();
    changed
        .asset(
            asset::TABLE_ASSET,
            Some(json::to_string(&rows(4.0, "first")).unwrap().as_bytes()),
        )
        .unwrap();
    let before = changed.save().unwrap();
    let error = changed.restore(&saved).unwrap_err().to_string();
    for name in [
        "weapons.json",
        "tables::Tuning",
        "movement::Tuning",
        "type identity",
    ] {
        assert!(error.contains(name), "{error}");
    }
    assert_eq!(changed.save().unwrap(), before);
}
