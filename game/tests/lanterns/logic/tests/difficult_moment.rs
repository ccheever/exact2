use exact_game::{Args, Game, InputEvent, Sim};
use historical::Lanterns;
use lanterns_evidence_logic::{
    appearance_edit, baseline as historical, clip_edit, physics_edit, Options,
};
use serde_json::{json, Value};
use std::path::PathBuf;
thread_local! { static MODE: std::cell::Cell<exact_game::Paranoid> = const { std::cell::Cell::new(exact_game::Paranoid::Off) }; }
fn simulation<G: Game>(args: G::Args) -> Sim<G> {
    let mut sim = lanterns_evidence_logic::simulation::<G>(args).paranoid(MODE.with(|m| m.get()));
    sim.viewport(1280., 720.);
    sim
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
fn options() -> Options {
    Options {
        seed: 1_041_003,
        started: true,
        sound: true,
        scene: exact_game_scene::bake::compile(
            root().join("scene.json"),
            &historical::scene_types(),
            &historical::scene_assets(),
        )
        .unwrap()
        .content,
        ..Default::default()
    }
}
fn ticks<G: Game>(s: &mut Sim<G>, n: u32) {
    let before = s.world().tick();
    // Host clocks are integral milliseconds. Match the public driver's lattice
    // exactly; the saved fractional world/queue phase is part of the byte oracle.
    let state = read(s, json!({"op":"state","clockState":true}));
    let clock = &state["world"]["clockState"];
    let due = ((before + u64::from(n)) * 1_000_000).div_ceil(60) as i64;
    let host = clock["hostMicros"].as_i64().unwrap();
    let elapsed = clock["worldMicros"].as_i64().unwrap();
    let now = (host + (due - elapsed).max(0) + 999) / 1000;
    let r = s.agent(&json!({"op":"clock","now":now}).to_string());
    assert!(!r.contains("error"), "{r}");
    assert_eq!(s.world().tick(), before + n as u64);
}
fn read<G: Game>(s: &mut Sim<G>, request: Value) -> Value {
    serde_json::from_str(&s.agent(&request.to_string())).unwrap()
}
fn snapshot<G: Game>(s: &mut Sim<G>) -> Value {
    let state = read(s, json!({"op":"state"}));
    let w = &state["world"];
    let mut entities = serde_json::Map::new();
    for name in [
        "player",
        "fox",
        "crate",
        "ledge",
        "lantern-12",
        "lantern-12/bulb",
    ] {
        entities.insert(
            name.into(),
            read(s, json!({"op":"state","entity":name}))["entity"]["components"].clone(),
        );
    }
    json!({"tick":w["tick"], "hash":w["hash"], "restored":w["restored"],
        "input":w["input"], "timer":w["resources"]["Session"],
        "physics_gravity":w["resources"]["Physics"]["gravity"],
        "scene_identity":w["resources"]["SceneIdentity"], "entities":entities,
        "reload":w["reload"], "logs":read(s,json!({"op":"logs"}))})
}
fn route() -> (Sim<Lanterns>, Vec<u8>, Value) {
    let mut s = simulation::<Lanterns>(options());
    read(&mut s, json!({"op":"clock","owner":"agent","now":0}));
    let script: Vec<Value> =
        serde_json::from_str(include_str!("../../fixtures/difficult-moment.script.json")).unwrap();
    let mut moving = Vec::new();
    let mut launch = Value::Null;
    for op in script {
        match op["op"].as_str().unwrap() {
            "clock" => ticks(&mut s, op["ticks"].as_u64().unwrap() as u32),
            "checkpoint" => {
                if op["name"] == "launch" {
                    launch = snapshot(&mut s);
                } else {
                    moving = s.save().unwrap();
                }
            }
            "key_down" | "key_up" => {
                let code = op["code"].as_str().unwrap();
                let down = op["op"] == "key_down";
                if let Some(after) = op["after_ticks"].as_u64() {
                    let state = read(&mut s, json!({"op":"state","clockState":true}));
                    let now =
                        state["world"]["clockState"]["hostMicros"].as_i64().unwrap() as f64 / 1000.;
                    s.scheduled_input(InputEvent::Key {
                        code: code.into(),
                        down,
                        at_ms: now + after as f64 * 1000. / 60.,
                    });
                } else if down {
                    s.key_down(code);
                } else {
                    s.key_up(code);
                }
            }
            other => panic!("unknown script verb {other}"),
        }
    }
    (s, moving, launch)
}
fn edited_scene(edit: &str) -> Options {
    let mut options = options();
    options.variant = if edit == "placement" { 1 } else { 4 };
    let mut scene: Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("scene.json")).unwrap()).unwrap();
    let root = root().canonicalize().unwrap();
    for path in scene["fragments"].as_object_mut().unwrap().values_mut() {
        *path = json!(root.join(path.as_str().unwrap()));
    }
    scene["assets"]["fox"]["path"] = json!(root.join("assets/fox.model"));
    let rows = scene["entities"].as_array_mut().unwrap();
    if edit == "placement" {
        rows.iter_mut().find(|r| r["id"] == "ledge").unwrap()["parameters"]["position"][0] =
            json!(11);
    } else {
        rows.iter_mut().find(|r| r["id"] == "lantern-12").unwrap()["overrides"] = json!({"bulb":{"Material":{"color":[0.1,0.35,0.8,1]},"PointLight":{"color":[1,0.42,0.08],"intensity":2,"range":7}}});
    }
    // Per-process temporary authored source, below Cargo's output, removed after bake.
    let path =
        PathBuf::from(env!("OUT_DIR")).join(format!("i3-{edit}-{}.json", std::process::id()));
    std::fs::write(&path, scene.to_string()).unwrap();
    let result = exact_game_scene::bake::compile(
        &path,
        &historical::scene_types(),
        &historical::scene_assets(),
    );
    std::fs::remove_file(path).unwrap();
    options.scene = result.unwrap().content;
    options
}
#[derive(Default, exact_game::Data)]
struct PendingInput {
    host_us: i64,
    world_us: Option<i64>,
    event: InputEvent,
    delivered: bool,
}
#[derive(Default, exact_game::Data)]
struct ControlState {
    input: exact_game::Input,
    queue: Vec<PendingInput>,
    world_us: i64,
}
fn control_bytes<G: Game>(sim: &Sim<G>) -> Vec<u8> {
    let save = sim.save().unwrap();
    let controls: ControlState = exact_game::bin::from_slice(&save[7..]).unwrap();
    exact_game::bin::to_vec(&controls)
}
fn probe<G: Game>(bytes: &[u8], options: Options, unedited: bool) -> Value {
    plan(&options);
    let args = G::Args::decode(&options.values()).unwrap();
    let mut s = simulation::<G>(args);
    let fresh = snapshot(&mut s);
    read(&mut s, json!({"op":"clock","owner":"agent","now":9000}));
    s.restore_bound(bytes).unwrap();
    // The unchanged row still checks every EXSIM byte, including executor and queue.
    let identical = s.save().unwrap() == bytes;
    if unedited {
        assert!(identical, "unedited carry must preserve every EXSIM byte");
    }
    let restored = snapshot(&mut s);
    let mut controls = vec![control_bytes(&s)];
    let mut trajectory = Vec::new();
    let key = format!("{}-{}", options.variant, s.world().tick());
    artifact(&format!("{key}-0.sim"), &s.save().unwrap());
    for offset in 1..=120 {
        ticks(&mut s, 1);
        if s.world()
            .get::<exact_game::Transform>("ledge")
            .unwrap()
            .position
            .x
            == 11.0
        {
            let ledge = s.world().named("ledge").unwrap();
            let hit = exact_game_physics::raycast(
                s.world(),
                exact_game::Vec3::new(12.5, 5.0, 8.0),
                -exact_game::Vec3::Y,
                10.0,
                u32::MAX,
            )
            .unwrap();
            assert_eq!(
                hit.entity, ledge,
                "new ledge edge must reach collision queries"
            );
            let old = exact_game_physics::raycast(
                s.world(),
                exact_game::Vec3::new(8.5, 5.0, 8.0),
                -exact_game::Vec3::Y,
                10.0,
                u32::MAX,
            )
            .unwrap();
            assert_ne!(old.entity, ledge, "old ledge edge must disappear");
        }
        trajectory.push(snapshot(&mut s));
        controls.push(control_bytes(&s));
        artifact(&format!("{key}-{offset}.sim"), &s.save().unwrap());
    }
    json!({"fresh":fresh,"restored":restored,"immediate_save_byte_identical":identical,"trajectory":trajectory,"controls":controls,"save_key":key})
}
fn assert_report(row: &Value, kind: &str, entity: &str, component: &str, field: &str) {
    let items = row["restored"]["reload"][kind].as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|i| i["entity"] == entity && i["component"] == component && i["field"] == field),
        "missing {kind} {entity}.{component}.{field}: {items:?}"
    );
    assert!(row["restored"]["logs"]
        .to_string()
        .contains(&format!("reload {kind}: {entity}.{component}.{field}")));
    assert_eq!(row["restored"]["reload"], row["trajectory"][119]["reload"]);
}
fn plan(options: &Options) {
    if let Some(dir) = std::env::var_os("EXACT_I3_OUT") {
        use exact_runner::{DataError, DataSource, Value as RunnerValue};
        struct NoData;
        impl DataSource for NoData {
            fn query(&mut self, source: &str, _: &[RunnerValue]) -> Result<RunnerValue, DataError> {
                Err(DataError::UnknownSource(source.into()))
            }
        }
        let args: Value =
            serde_json::from_str(&exact_game::json::to_string(options).unwrap()).unwrap();
        let bindings = args
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        let path = PathBuf::from(&dir).join(format!("{}.contract", options.variant));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, format!("component Evidence\n  view\n    canvas surface=world({bindings}) testId=\"world\" width=\"100%\" height=\"100%\"\n")).unwrap();
        let compiled = contract::compile_path(&path).unwrap();
        let baked = contract::bake_with_surface_arguments(compiled, NoData, |_| {
            Some(Options::FIELDS.iter().map(|f| f.0.to_owned()).collect())
        })
        .unwrap();
        std::fs::write(
            PathBuf::from(dir).join(format!("{}.plan", options.variant)),
            baked.encode(),
        )
        .unwrap();
    }
}
thread_local! { static ARTIFACTS: std::cell::RefCell<std::collections::BTreeMap<String,Vec<u8>>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) }; }
fn artifact(name: &str, bytes: &[u8]) {
    ARTIFACTS.with_borrow_mut(|items| {
        assert!(items.insert(name.into(), bytes.to_vec()).is_none());
    });
    if let Some(dir) = std::env::var_os("EXACT_I3_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}
#[test]
fn difficult_moment_repeats_and_continues_under_four_compiled_edits() {
    let mut baseline = None;
    for mode in [
        exact_game::Paranoid::Off,
        exact_game::Paranoid::Save,
        exact_game::Paranoid::FreshGame,
    ] {
        MODE.with(|m| m.set(mode));
        ARTIFACTS.with_borrow_mut(|items| items.clear());
        exercise();
        let artifacts = ARTIFACTS.with_borrow_mut(std::mem::take);
        if let Some(previous) = &baseline {
            assert_eq!(
                previous, &artifacts,
                "every checkpoint and continued control/state byte must agree in {mode:?}"
            );
        } else {
            baseline = Some(artifacts);
        }
    }
}
fn exercise() {
    let (mut a, moving, launch) = route();
    let (b, moving_again, _) = route();
    assert_eq!(a.save().unwrap(), b.save().unwrap());
    assert_eq!(moving, moving_again);
    let bytes = a.save().unwrap();
    artifact("difficult-moment.sim", &bytes);
    artifact("moving-crate.sim", &moving);
    let before = snapshot(&mut a);
    let c = a
        .world()
        .get::<exact_game_physics::CapsuleController>("player")
        .unwrap();
    assert!(!c.grounded && c.velocity.y.abs() < 0.00001);
    let lamp = a.world().get::<historical::Lantern>("lantern-12").unwrap();
    let glow = lamp.glow.value(a.world().now());
    let velocity = lamp.glow.velocity(a.world().now());
    assert!(glow > 0.0 && glow < 1.0 && velocity > 0.0);
    drop(c);
    drop(lamp);
    let facts = json!({"moment":before,"launch":launch,"reproduction_hashes":[format!("0x{:016x}",a.world().hash()),format!("0x{:016x}",b.world().hash())],
        "spring_sample":{"value":glow,"velocity":velocity},
        "animation_weights":null,"animation_limitation":"This fixed consumer has no active blend; airborne selects Run. Animation::play resets time on clip change.",
        "crate_limitation":"Crate sleeps before apex in all three route exploration rounds. Nonzero motion is tested at the supplementary pushed-crate checkpoint; no state was injected.",
        "pending_actions":[{"key":"Space","down":false,"after_ticks":10},{"key":"Space","down":true,"after_ticks":75},{"key":"Space","down":false,"after_ticks":76}]});
    artifact(
        "difficult-moment.json",
        serde_json::to_string_pretty(&facts).unwrap().as_bytes(),
    );
    let v1 = probe::<Lanterns>(&bytes, options(), true);
    let placement = probe::<Lanterns>(&bytes, edited_scene("placement"), false);
    let physics = probe::<physics_edit::Lanterns>(
        &bytes,
        Options {
            variant: 2,
            ..options()
        },
        false,
    );
    let clip = probe::<clip_edit::Lanterns>(
        &bytes,
        Options {
            variant: 3,
            ..options()
        },
        false,
    );
    let appearance = probe::<appearance_edit::Lanterns>(&bytes, edited_scene("appearance"), false);
    for tick in 0..120 {
        ticks(&mut a, 1);
        assert_eq!(
            snapshot(&mut a),
            v1["trajectory"][tick],
            "unedited continuation tick {tick}"
        );
    }

    assert_eq!(
        placement["fresh"]["entities"]["ledge"]["Transform"]["position"][0],
        11.0
    );
    assert_eq!(
        placement["restored"]["entities"]["ledge"]["Transform"]["position"][0],
        11.0
    );
    assert_eq!(
        clip["trajectory"][0]["entities"]["fox"]["Animation"]["clip"],
        "Walk"
    );
    assert_eq!(physics["fresh"]["physics_gravity"], json!([0., -18., 0.]));
    assert_eq!(
        physics["restored"]["physics_gravity"],
        json!([0., -18., 0.])
    );
    assert_eq!(
        appearance["fresh"]["entities"]["lantern-12/bulb"]["Material"]["color"],
        json!([0.1, 0.35, 0.8, 1.0])
    );
    for state in appearance["trajectory"].as_array().unwrap() {
        assert_eq!(
            state["entities"]["lantern-12/bulb"]["Material"]["color"],
            json!([0.1, 0.35, 0.8, 1.0])
        );
    }
    assert_eq!(
        physics["trajectory"][75]["entities"]["player"]["CapsuleController"]["velocity"][1],
        8.0
    );
    assert_eq!(
        clip["trajectory"][0]["entities"]["fox"]["Animation"]["time"],
        json!(0.016666668)
    );
    // The supplementary moment really has both velocities, and tests their carry too.
    let moving_v1 = probe::<Lanterns>(&moving, options(), true);
    let body = &moving_v1["restored"]["entities"]["crate"]["Body"];
    for field in ["velocity", "spin"] {
        assert!(body[field]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_f64().unwrap() != 0.0));
    }
    let moving_physics = probe::<physics_edit::Lanterns>(
        &moving,
        Options {
            variant: 2,
            ..options()
        },
        false,
    );
    let moving_placement = probe::<Lanterns>(&moving, edited_scene("placement"), false);
    let moving_clip = probe::<clip_edit::Lanterns>(
        &moving,
        Options {
            variant: 3,
            ..options()
        },
        false,
    );
    let moving_appearance =
        probe::<appearance_edit::Lanterns>(&moving, edited_scene("appearance"), false);
    for (baseline, edited, name) in [
        (&v1, &placement, "placement"),
        (&v1, &physics, "physics"),
        (&v1, &clip, "clip"),
        (&v1, &appearance, "appearance"),
        (&moving_v1, &moving_placement, "placement"),
        (&moving_v1, &moving_physics, "physics"),
        (&moving_v1, &moving_clip, "clip"),
        (&moving_v1, &moving_appearance, "appearance"),
    ] {
        // Full binary dynamic input, pending queue and relative world clock at
        // the restored boundary and every continued tick, not only held keys.
        assert_eq!(
            baseline["controls"], edited["controls"],
            "{name} input/queue bytes"
        );
        // Exact state at carry; report/journal and authored fields may change.
        for field in ["input", "timer"] {
            assert_eq!(
                baseline["restored"][field], edited["restored"][field],
                "{name} {field}"
            );
        }
        for (entity, component) in [
            ("player", "CapsuleController"),
            ("crate", "Body"),
            ("fox", "Animation"),
            ("lantern-12", "Lantern"),
        ] {
            assert_eq!(
                baseline["restored"]["entities"][entity][component],
                edited["restored"]["entities"][entity][component],
                "{name} {entity}.{component}"
            );
        }
        for i in 0..120 {
            let a = &baseline["trajectory"][i];
            let b = &edited["trajectory"][i];
            assert_eq!(a["input"], b["input"], "{name} input {i}");
            assert_eq!(a["timer"], b["timer"], "{name} timer {i}");
            // Gravity intentionally changes rigid-body integration. Placement can
            // affect the player/ledge contact; appearance and clip cannot.
            if name != "physics" {
                assert_eq!(
                    a["entities"]["crate"]["Body"], b["entities"]["crate"]["Body"],
                    "{name} crate {i}"
                );
            }
            if matches!(name, "clip" | "appearance") {
                for (entity, component) in [
                    ("player", "CapsuleController"),
                    ("player", "Transform"),
                    ("lantern-12", "Lantern"),
                ] {
                    assert_eq!(
                        a["entities"][entity][component], b["entities"][entity][component],
                        "{name} {entity}.{component} {i}"
                    );
                }
            }
        }
    }
    assert_report(&placement, "applied", "ledge", "Transform", "position");
    assert_report(&physics, "applied", "resource", "Physics", "gravity");
    assert_report(&clip, "kept", "fox", "Animation", "clip");
    assert_report(
        &appearance,
        "applied",
        "lantern-12/bulb",
        "Material",
        "color",
    );
    assert_report(
        &appearance,
        "kept",
        "lantern-12/bulb",
        "PointLight",
        "intensity",
    );
    assert!(v1["restored"]["reload"]["kept"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!clip["restored"]["reload"]["kept"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["component"] == "PointLight"));
    for row in placement["trajectory"].as_array().unwrap() {
        assert_eq!(row["entities"]["ledge"]["Transform"]["position"][0], 11.0);
    }
    // The fox remains supported past the old edge, then leaves the new edge.
    assert_eq!(
        placement["trajectory"][45]["entities"]["player"]["CapsuleController"]["grounded"],
        true
    );
    assert_eq!(
        v1["trajectory"][45]["entities"]["player"]["CapsuleController"]["grounded"],
        false
    );
    // A genuine landing on the shifted ledge, through the restored physics executor.
    let landing = placement["trajectory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s["entities"]["player"]["CapsuleController"]["grounded"] == true
                && s["entities"]["player"]["Transform"]["position"][1]
                    .as_f64()
                    .unwrap()
                    > 2.0
        })
        .expect("fox must land on the moved ledge");
    assert!(
        landing["entities"]["player"]["Transform"]["position"][0]
            .as_f64()
            .unwrap()
            >= 9.0
    );

    let report = json!({"v1":v1,"placement":placement,"physics":physics,"clip":clip,"appearance":appearance,
        "supplementary_moving_crate":{"v1":moving_v1,"physics":moving_physics,"placement":moving_placement,"clip":moving_clip,"appearance":moving_appearance}});
    artifact(
        "all-continuations.json",
        &serde_json::to_vec(&report).unwrap(),
    );
    // Full 120-tick per-edit records are written only when explicitly requested.
    if let Some(dir) = std::env::var_os("EXACT_I3_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            PathBuf::from(dir).join("continuations.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .unwrap();
    }
    let mut records = String::new();
    for name in ["v1", "placement", "physics", "clip", "appearance"] {
        for state in std::iter::once(&report[name]["restored"])
            .chain(report[name]["trajectory"].as_array().unwrap())
        {
            let e = &state["entities"];
            records.push_str(&json!({"edit":name,"tick":state["tick"],"hash":state["hash"],
                "player":e["player"]["Transform"]["position"],"velocity":e["player"]["CapsuleController"]["velocity"],
                "grounded":e["player"]["CapsuleController"]["grounded"],"crate":e["crate"]["Body"],
                "animation":e["fox"]["Animation"],"spring":e["lantern-12"]["Lantern"]["glow"],
                "material":e["lantern-12/bulb"]["Material"],"light":e["lantern-12/bulb"]["PointLight"]["intensity"],
                "elapsed":state["timer"]["elapsed"],"held":state["input"]["held"]}).to_string());
            records.push('\n');
        }
    }
    artifact("difficult-moment.continuations.jsonl", records.as_bytes());
    for name in ["v1", "placement", "physics", "clip", "appearance"] {
        println!(
            "I3 {name}: restored={} tick+1={} tick+120={}",
            report[name]["restored"]["hash"],
            report[name]["trajectory"][0]["hash"],
            report[name]["trajectory"][119]["hash"]
        );
    }
}
