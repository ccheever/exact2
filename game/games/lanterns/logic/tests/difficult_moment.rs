//! I3 instrument. Compiled game edits, no mutation of the carried world.
// Each second build needs its own kinds bound to that build's component types.
#![allow(clippy::duplicate_mod)]
use exact_game::{Args, Game, InputEvent, Sim};
use lanterns_logic::{Lanterns, Options};
use serde_json::{json, Value};
use std::path::PathBuf;

#[allow(dead_code)]
mod physics_edit {
    include!(concat!(env!("OUT_DIR"), "/i3_physics.rs"));
}
// Preserve the literal Run-to-Walk edit, including identical fast/slow branches.
#[allow(dead_code, clippy::if_same_then_else)]
mod clip_edit {
    include!(concat!(env!("OUT_DIR"), "/i3_clip.rs"));
}
#[allow(dead_code)]
mod appearance_edit {
    include!(concat!(env!("OUT_DIR"), "/i3_appearance.rs"));
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
            &lanterns_logic::scene_types(),
            Lanterns::assets(),
        )
        .unwrap()
        .content,
        ..Default::default()
    }
}
fn ticks<G: Game>(s: &mut Sim<G>, n: u32) {
    let before = s.world().tick();
    let r = s.agent(&format!(r#"{{"op":"clock","ticks":{n}}}"#));
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
        "logs":read(s,json!({"op":"logs"}))})
}
fn route() -> (Sim<Lanterns>, Vec<u8>, Value) {
    let mut s = Sim::<Lanterns>::new(options()).unwrap();
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
                    moving = s.save();
                }
            }
            "key_down" | "key_up" => {
                let code = op["code"].as_str().unwrap();
                let down = op["op"] == "key_down";
                if let Some(after) = op["after_ticks"].as_u64() {
                    let state = read(&mut s, json!({"op":"state"}));
                    let now =
                        state["world"]["clockState"]["hostMicros"].as_i64().unwrap() as f64 / 1000.;
                    s.input(InputEvent::Key {
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
    let mut scene: Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("scene.json")).unwrap()).unwrap();
    let root = root().canonicalize().unwrap();
    for path in scene["fragments"].as_object_mut().unwrap().values_mut() {
        *path = json!(root.join(path.as_str().unwrap()));
    }
    scene["assets"]["fox"]["path"] = json!(root.join("assets/Fox.glb"));
    let rows = scene["entities"].as_array_mut().unwrap();
    if edit == "placement" {
        rows.iter_mut().find(|r| r["id"] == "ledge").unwrap()["parameters"]["position"][0] =
            json!(11);
    } else {
        rows.iter_mut().find(|r| r["id"] == "lantern-12").unwrap()["overrides"] =
            json!({"bulb":{"Material":{"color":[0.1,0.35,0.8,1]}}});
    }
    // Per-process temporary authored source, below Cargo's output, removed after bake.
    let path =
        PathBuf::from(env!("OUT_DIR")).join(format!("i3-{edit}-{}.json", std::process::id()));
    std::fs::write(&path, scene.to_string()).unwrap();
    let result =
        exact_game_scene::bake::compile(&path, &lanterns_logic::scene_types(), Lanterns::assets());
    std::fs::remove_file(path).unwrap();
    options.scene = result.unwrap().content;
    options
}
fn probe<G: Game>(bytes: &[u8], options: Options) -> Value {
    let mut s = Sim::<G>::from_values(&options.values()).unwrap();
    let fresh = snapshot(&mut s);
    read(&mut s, json!({"op":"clock","owner":"agent","now":9000}));
    s.restore_bound(bytes).unwrap();
    // Covers every serialized component/resource, cursor, timer, journal, held
    // input and future input queue, not merely the selected JSON fields below.
    assert_eq!(
        s.save(),
        bytes,
        "immediate carry must preserve every EXSIM byte"
    );
    let restored = snapshot(&mut s);
    let mut trajectory = Vec::new();
    for _ in 0..120 {
        ticks(&mut s, 1);
        trajectory.push(snapshot(&mut s));
    }
    json!({"fresh":fresh,"restored":restored,"immediate_save_byte_identical":true,"trajectory":trajectory})
}
fn artifact(name: &str, bytes: &[u8]) {
    let path = root().join("fixtures").join(name);
    if std::env::var_os("EXACT_I3_RECORD").is_some() {
        std::fs::write(path, bytes).unwrap();
    } else {
        assert_eq!(
            std::fs::read(path).unwrap(),
            bytes,
            "fixture {name} changed"
        );
    }
}
#[test]
fn difficult_moment_repeats_and_continues_under_four_compiled_edits() {
    let (mut a, moving, launch) = route();
    let (b, moving_again, _) = route();
    assert_eq!(a.save(), b.save());
    assert_eq!(moving, moving_again);
    let bytes = a.save();
    artifact("difficult-moment.sim", &bytes);
    artifact("moving-crate.sim", &moving);
    let before = snapshot(&mut a);
    let c = a
        .world()
        .get::<exact_game_physics::Character>("player")
        .unwrap();
    assert!(!c.grounded && c.velocity.y.abs() < 0.00001);
    let lamp = a
        .world()
        .get::<lanterns_logic::Lantern>("lantern-12")
        .unwrap();
    let glow = lamp.glow.value(a.world().now());
    let velocity = lamp.glow.velocity(a.world().now());
    assert!(glow > 0.0 && glow < 1.0 && velocity > 0.0);
    drop(c);
    drop(lamp);
    let facts = json!({"moment":before,"launch":launch,"reproduction_hashes":[format!("0x{:016x}",a.world().hash()),format!("0x{:016x}",b.world().hash())],
        "spring_sample":{"value":glow,"velocity":velocity},
        "animation_weights":null,"animation_limitation":"No blend state exists; airborne selects Run. Animation.play resets seconds on clip change.",
        "crate_limitation":"Crate sleeps before apex in all three route exploration rounds. Nonzero motion is tested at the supplementary pushed-crate checkpoint; no state was injected.",
        "pending_actions":[{"key":"Space","down":false,"after_ticks":10},{"key":"Space","down":true,"after_ticks":75},{"key":"Space","down":false,"after_ticks":76}]});
    artifact(
        "difficult-moment.json",
        serde_json::to_string_pretty(&facts).unwrap().as_bytes(),
    );
    let v1 = probe::<Lanterns>(&bytes, options());
    let placement = probe::<Lanterns>(&bytes, edited_scene("placement"));
    let physics = probe::<physics_edit::Lanterns>(&bytes, options());
    let clip = probe::<clip_edit::Lanterns>(&bytes, options());
    let appearance = probe::<appearance_edit::Lanterns>(&bytes, edited_scene("appearance"));
    for tick in 0..120 {
        ticks(&mut a, 1);
        assert_eq!(
            snapshot(&mut a),
            v1["trajectory"][tick],
            "unedited continuation tick {tick}"
        );
    }
    assert_eq!(v1["trajectory"], placement["trajectory"]);
    assert_eq!(
        placement["fresh"]["entities"]["ledge"]["Transform"]["position"][0],
        11.0
    );
    assert_eq!(
        placement["restored"]["entities"]["ledge"]["Transform"]["position"][0],
        10.0
    );
    assert_eq!(
        clip["trajectory"][0]["entities"]["fox"]["Animation"]["clip"],
        "Walk"
    );
    assert_eq!(physics["fresh"]["physics_gravity"], json!([0., -18., 0.]));
    assert_eq!(
        physics["restored"]["physics_gravity"],
        json!([0., -12., 0.])
    );
    assert_eq!(
        appearance["fresh"]["entities"]["lantern-12/bulb"]["Material"]["color"],
        json!([0.1, 0.35, 0.8, 1.0])
    );
    for state in appearance["trajectory"].as_array().unwrap() {
        assert_eq!(
            state["entities"]["lantern-12/bulb"]["Material"]["color"],
            json!([0.18, 0.16, 0.14, 1.0])
        );
    }
    assert_eq!(
        physics["trajectory"][75]["entities"]["player"]["Character"]["velocity"][1],
        8.0
    );
    assert_eq!(
        clip["trajectory"][0]["entities"]["fox"]["Animation"]["seconds"],
        json!(0.016666668)
    );
    // The supplementary moment really has both velocities, and tests their carry too.
    let moving_v1 = probe::<Lanterns>(&moving, options());
    let body = &moving_v1["restored"]["entities"]["crate"]["Body"];
    for field in ["velocity", "spin"] {
        assert!(body[field]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_f64().unwrap() != 0.0));
    }
    let moving_physics = probe::<physics_edit::Lanterns>(&moving, options());
    let moving_placement = probe::<Lanterns>(&moving, edited_scene("placement"));
    let moving_clip = probe::<clip_edit::Lanterns>(&moving, options());
    let moving_appearance = probe::<appearance_edit::Lanterns>(&moving, edited_scene("appearance"));
    assert_eq!(moving_v1["trajectory"], moving_placement["trajectory"]);
    let report = json!({"v1":v1,"placement":placement,"physics":physics,"clip":clip,"appearance":appearance,
        "supplementary_moving_crate":{"v1":moving_v1,"physics":moving_physics,"placement":moving_placement,"clip":moving_clip,"appearance":moving_appearance}});
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
                "player":e["player"]["Transform"]["position"],"velocity":e["player"]["Character"]["velocity"],
                "grounded":e["player"]["Character"]["grounded"],"crate":e["crate"]["Body"],
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
