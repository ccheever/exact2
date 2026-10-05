//! The tables (`rivals.level.json`): what the check refuses, and a
//! development reload replacing them in a running match (LLP 1046.009 G2).
use exact_game::asset::Checked;
use exact_game::{Sim, Transform};
use rivals_logic::fighter::Fighter;
use rivals_logic::tables::{self, Tables};
use rivals_logic::{Options, Rivals};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn game_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
fn source() -> String {
    std::fs::read_to_string(game_dir().join(tables::NAME)).unwrap()
}
fn parse(text: &str) -> Tables {
    exact_game::json::from_str(text).unwrap()
}

#[test]
fn the_tables_pass_their_check() {
    parse(&source()).check().unwrap();
    tables::LEVEL.decode(&source()).unwrap();
}

#[test]
fn the_check_names_what_the_types_cannot() {
    let refused = |edit: &dyn Fn(&mut Tables), says: &str| {
        let mut t = parse(&source());
        edit(&mut t);
        let error = t.check().unwrap_err();
        assert!(error.contains(says), "{error}");
    };
    refused(&|t| t.quick_reload = [0.65, 0.45], "quick_reload");
    refused(&|t| t.fighter.walk = 0.0, "fighter.walk");
    refused(&|t| t.arena.spawns.truncate(24), "needs one each");
    // The ramp: a dummy placed there slid off it (diary 006, round 1).
    refused(
        &|t| t.arena.spawns[3] = [0.0, 5.6],
        "stands in a ramp block",
    );
    refused(&|t| t.arena.spawns[4] = t.arena.spawns[0], "repeats");
    refused(
        &|t| t.arena.blocks[6].kind = "pillar".into(),
        "not a block kind",
    );
    // A missing number reads as 0, which the check refuses by name.
    let mut text = source();
    text = text.replace("\"gravity\": 22.0,", "");
    let Err(error) = tables::LEVEL.decode(&text) else {
        panic!("a level without fighter.gravity decoded");
    };
    assert!(error.to_string().contains("fighter.gravity"), "{error}");
}

/// A match whose level bytes a test can replace, as the dev loop does.
fn live(level: Arc<Mutex<String>>) -> Sim<Rivals> {
    let assets = game_dir().join("assets");
    let mut sim = Sim::<Rivals>::with_assets(
        Options {
            seed: 7,
            bots: 1,
            ..Options::default()
        },
        move |name: &str| -> Result<Vec<u8>, String> {
            if name == tables::NAME {
                Ok(level.lock().unwrap().clone().into_bytes())
            } else {
                std::fs::read(assets.join(name)).map_err(|e| format!("{name}: {e}"))
            }
        },
    )
    .unwrap();
    sim.viewport(1280.0, 720.0);
    sim
}

#[test]
fn a_reload_replaces_the_tables_in_a_running_match() {
    let level = Arc::new(Mutex::new(source()));
    let mut sim = live(level.clone());
    sim.run(2000.0);
    let tick = sim.world().tick();
    let kills = sim.world().require::<Fighter>("player").kills;
    let crate_at = |sim: &Sim<Rivals>| sim.world().require::<Transform>("block-10").position;
    assert_eq!(crate_at(&sim).x, -3.5);

    // Move a crate two metres and slow walking down, then reload the tables.
    let edited = source()
        .replace("\"at\": [-3.5, 0.5, 14.0]", "\"at\": [-1.5, 0.5, 14.0]")
        .replace("\"walk\": 7.0,", "\"walk\": 3.0,");
    *level.lock().unwrap() = edited;
    assert_eq!(sim.assets_changed([tables::NAME]), [tables::NAME]);
    // The bytes land at the end of an advance (as a host's arrive between
    // frames); the next tick rebuilds what was derived from them.
    sim.run(100.0);
    sim.run(100.0);

    // The same match, on: the clock moved on and nothing restarted.
    assert!(sim.world().tick() > tick);
    assert_eq!(sim.world().require::<Fighter>("player").kills, kills);
    assert_eq!(tables::of(sim.world()).fighter.walk, 3.0);
    assert_eq!(
        crate_at(&sim).x,
        -1.5,
        "the arena was rebuilt from the new layout"
    );
    // One floor and one entity per block: the old ones are gone.
    let blocks = tables::of(sim.world()).arena.blocks.len();
    assert!(sim.world().named(&format!("block-{blocks}")).is_none());

    // A save now carries the new tables' identity and continues identically.
    let saved = sim.save().unwrap();
    let mut again = live(level.clone());
    again.restore(&saved).unwrap();
    sim.run(500.0);
    again.run(500.0);
    assert_eq!(sim.world().hash(), again.world().hash());
}

#[test]
fn a_refused_reload_keeps_the_tables_the_match_runs_on() {
    let level = Arc::new(Mutex::new(source()));
    let mut sim = live(level);
    sim.run(500.0);
    sim.assets_changed([tables::NAME]);
    // As a host delivers it: the refusal is the host's to show.
    assert_eq!(sim.take_assets(), [tables::NAME]);
    let bad = source().replace(
        "\"quick_reload\": [0.45, 0.65]",
        "\"quick_reload\": [0.9, 0.1]",
    );
    let error = sim.asset(tables::NAME, Some(bad.as_bytes())).unwrap_err();
    assert!(
        error.contains("replacement refused") && error.contains("quick_reload"),
        "{error}"
    );
    // The match keeps running on the last good tables, and nothing restarts.
    let tick = sim.world().tick();
    sim.run(100.0);
    assert!(sim.world().tick() > tick);
    assert_eq!(tables::of(sim.world()).quick_reload, [0.45, 0.65]);
    assert!(sim.world().named("block-10").is_some());
}
