use super::*;
use lanterns_evidence_logic::{
    baseline::{Lantern, Lanterns},
    scene_assets, scene_types, simulation, Options,
};
use std::{path::PathBuf, time::Instant};

fn options() -> Options {
    Options {
        seed: 1_041_003,
        started: true,
        sound: true,
        scene: exact_game_scene::bake::compile(
            root().join("scene.json"),
            &scene_types(),
            &scene_assets(),
        )
        .unwrap()
        .content,
        ..Default::default()
    }
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../verification/lanterns")
}
fn difficult() -> Sim<Lanterns> {
    let mut sim = simulation::<Lanterns>(options());
    sim.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    let script: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../verification/lanterns/fixtures/difficult-moment.script.json"
    ))
    .unwrap();
    for op in script {
        match op["op"].as_str().unwrap() {
            "clock" => {
                let reply = sim.agent(&op.to_string());
                assert!(!reply.contains("error"), "{reply}");
            }
            "checkpoint" => {}
            "key_down" | "key_up" => {
                let code = op["code"].as_str().unwrap();
                let down = op["op"] == "key_down";
                if let Some(after) = op["after_ticks"].as_u64() {
                    let state: serde_json::Value =
                        serde_json::from_str(&sim.agent(r#"{"op":"state","clockState":true}"#))
                            .unwrap();
                    let now =
                        state["world"]["clockState"]["hostMicros"].as_i64().unwrap() as f64 / 1000.;
                    sim.scheduled_input(exact_game::InputEvent::Key {
                        code: code.into(),
                        down,
                        at_ms: now + after as f64 * 1000. / 60.,
                    });
                } else if down {
                    sim.key_down(code);
                } else {
                    sim.key_up(code);
                }
            }
            other => panic!("unknown I3 verb {other}"),
        }
    }
    assert_eq!(sim.world().tick(), 225);
    assert!(sim.world().get::<Lantern>("lantern-12").unwrap().lit);
    sim
}

// G executes animation inside Game::tick. These disjoint timings report the
// complete simulation (including animation) and actual primitive feed writes;
// they do not subtract medians or claim GPU/model-palette timings.
#[test]
#[ignore = "I3 bounded 120-tick CPU simulation and primitive feed medians"]
fn difficult_moment_cost() {
    let mut sim = difficult();
    let save = sim.save().unwrap();
    sim.restore_bound(&save).unwrap();
    let begin = sim.world().tick();
    let mut feed = Feed::default();
    let mut writes = Recording {
        record: false,
        ..Default::default()
    };
    feed.feed_to(sim.world(), &mut writes).unwrap();
    let mut samples = [Vec::new(), Vec::new()];
    for _ in 0..120 {
        let start = Instant::now();
        let reply = sim.agent(r#"{"op":"clock","ticks":1}"#);
        assert!(!reply.contains("error"), "{reply}");
        samples[0].push(start.elapsed().as_nanos());
        let start = Instant::now();
        feed.feed_to(sim.world(), &mut writes).unwrap();
        samples[1].push(start.elapsed().as_nanos());
    }
    assert_eq!(sim.world().tick(), begin + 120);
    assert!(!writes.meshes.is_empty());
    assert!(!sim
        .world()
        .get::<exact_game::animation::Pose>("fox")
        .unwrap()
        .local
        .is_empty());
    for (name, values) in ["simulation_including_animation", "primitive_feed"]
        .into_iter()
        .zip(&mut samples)
    {
        values.sort_unstable();
        println!("I3 {name} median_ns={}", (values[59] + values[60]) / 2);
    }
    println!("I3 final_hash=0x{:016x}", sim.world().hash());
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        println!(
            "I3 {}",
            status.lines().find(|l| l.starts_with("VmHWM:")).unwrap()
        );
    }
}

#[test]
fn replacement_running_clip_has_different_cpu_skinning() {
    let bytes = std::fs::read(root().join("assets/fox.model")).unwrap();
    let exact_game::asset::Content::Model(model) =
        exact_game::asset::Content::decode("fox.model", &bytes).unwrap()
    else {
        panic!("expected baked model")
    };
    let rest = exact_game::animation::bind_pose(&model);
    let sample = |name| {
        let clip = model.clips.iter().find(|c| c.name == name).unwrap();
        let mut local = Vec::new();
        exact_game::animation::sample(clip, 1. / 60., &rest, &mut local);
        model
            .skins
            .iter()
            .flat_map(|skin| {
                skin.joints
                    .iter()
                    .map(|joint| exact_game::animation::joint_matrix(&model, &local, *joint))
            })
            .collect::<Vec<_>>()
    };
    let run = sample("Run");
    let walk = sample("Walk");
    assert_eq!(run.len(), walk.len());
    assert!(!run.is_empty());
    assert!(run.iter().zip(&walk).any(|(a, b)| a != b));
}

#[test]
fn carried_initializer_edits_reach_real_feed_on_next_tick() {
    struct Edited;
    impl Game for Edited {
        const ID: &'static str = Lanterns::ID;
        const ASSETS: &'static [&'static str] = Lanterns::ASSETS;
        type Args = Options;
        fn actions() -> exact_game::Actions {
            Lanterns::actions()
        }
        fn setup(w: &mut World, args: &Options) {
            Lanterns::setup(w, args);
            w.get_mut::<Transform>("ledge").unwrap().position.x += 1.;
            w.get_mut::<Material>("lantern-12/bulb").unwrap().color = [0.1, 0.35, 0.8, 1.];
        }
        fn tick(w: &mut World, input: &Input, args: &Options) {
            Lanterns::tick(w, input, args);
        }
    }
    let mut old = difficult();
    let bytes = old.save().unwrap();
    let mut sim = simulation::<Edited>(options());
    let mut feed = Feed::default();
    let mut writes = Recording::default();
    feed.feed_to(old.world(), &mut writes).unwrap();
    let ledge = old.world().named("ledge").unwrap();
    assert_eq!(writes.position(ledge, false).x, 10.);
    sim.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    sim.restore_bound(&bytes).unwrap();
    sim.agent(r#"{"op":"clock","ticks":1}"#);
    feed.feed_to(sim.world(), &mut writes).unwrap();
    assert_eq!(writes.position(ledge, false).x, 11.);
    let bulb = sim.world().named("lantern-12/bulb").unwrap();
    assert_eq!(
        &writes.materials[bulb.index() as usize * 12..][..4],
        &[0.1, 0.35, 0.8, 1.]
    );
    old.agent(r#"{"op":"clock","ticks":1}"#);
    assert_eq!(
        old.world()
            .get::<Lantern>("lantern-12")
            .unwrap()
            .glow
            .value(old.world().now()),
        sim.world()
            .get::<Lantern>("lantern-12")
            .unwrap()
            .glow
            .value(sim.world().now())
    );
}
