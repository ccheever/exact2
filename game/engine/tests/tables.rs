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
