use super::*;
use lanterns_logic::{Lanterns, Options};
use std::{path::PathBuf, time::Instant};

// Measures the real feed algorithm with CPU copies at its Writes seam. Sampling
// is timed inside Model::sample, then subtracted per tick (not median-minus-median).
#[test]
#[ignore = "I3 120-tick CPU simulation / animation / feed medians, no GPU"]
fn difficult_moment_cost() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../games/lanterns");
    let bytes = std::fs::read(root.join("fixtures/difficult-moment.sim")).unwrap();
    let mut sim = Sim::<Lanterns>::new(Options {
        scene: exact_game_scene::bake::compile(
            root.join("scene.json"),
            &lanterns_logic::scene_types(),
            Lanterns::assets(),
        )
        .unwrap()
        .content,
        seed: 1_041_003,
        started: true,
        sound: true,
        ..Default::default()
    })
    .unwrap();
    sim.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    sim.restore_bound(&bytes).unwrap();
    let begin = sim.world().tick();
    let mut feed = Feed::with_assets(Lanterns::assets()).unwrap();
    let mut writes = Recording {
        record: false,
        ..Default::default()
    };
    feed.feed_to(sim.world(), &mut writes).unwrap();
    let mut samples = [Vec::new(), Vec::new(), Vec::new()];
    for tick in 1..=120 {
        let start = Instant::now();
        sim.advance(tick as f64 * 1000.0 / 60.0 + 0.001, Clock::Seekable);
        samples[0].push(start.elapsed().as_nanos());
        crate::assets::sample_timing::NANOS.with(|n| n.set(Some(0)));
        let start = Instant::now();
        feed.feed_to(sim.world(), &mut writes).unwrap();
        let total = start.elapsed().as_nanos();
        let animation = crate::assets::sample_timing::NANOS.with(|n| n.replace(None).unwrap());
        samples[1].push(animation);
        samples[2].push(total - animation);
    }
    assert_eq!(sim.world().tick(), begin + 120);
    for (name, values) in ["simulation", "animation", "feed_excluding_animation"]
        .into_iter()
        .zip(&mut samples)
    {
        values.sort_unstable();
        println!("I3 {name} median_ns={}", (values[59] + values[60]) / 2);
    }
    println!("I3 final_hash=0x{:016x}", sim.world().hash());
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    println!(
        "I3 process_peak_memory {}",
        status.lines().find(|l| l.starts_with("VmHWM:")).unwrap()
    );
}

#[test]
fn replacement_running_clip_has_different_cpu_skinning() {
    let model = crate::assets::Model::parse(Lanterns::assets()[0].bytes).unwrap();
    let run = model.sample("Run", 1.0 / 60.0, true).unwrap();
    let walk = model.sample("Walk", 1.0 / 60.0, true).unwrap();
    assert_eq!(run.len(), walk.len());
    assert!(run.iter().zip(&walk).any(|(a, b)| a.0 != b.0));
}

#[test]
fn carried_initializer_edits_reach_real_feed_on_next_tick() {
    struct Edited;
    impl Game for Edited {
        const ID: &'static str = Lanterns::ID;
        type Args = Options;
        fn actions() -> exact_game::Actions {
            Lanterns::actions()
        }
        fn assets() -> &'static [exact_game::Asset] {
            Lanterns::assets()
        }
        fn setup(w: &mut World, args: &Options) {
            Lanterns::setup(w, args);
            w.get_mut::<Transform>("ledge").unwrap().position.x += 1.0;
            w.get_mut::<Material>("lantern-12/bulb").unwrap().color = [0.1, 0.35, 0.8, 1.0];
        }
        fn tick(w: &mut World, input: &Input, args: &Options) {
            Lanterns::tick(w, input, args);
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../games/lanterns");
    let bytes = std::fs::read(root.join("fixtures/difficult-moment.sim")).unwrap();
    let mut old = Sim::<Lanterns>::from_save(&bytes).unwrap();
    let mut sim = Sim::<Edited>::new(Options {
        scene: exact_game_scene::bake::compile(
            root.join("scene.json"),
            &lanterns_logic::scene_types(),
            Lanterns::assets(),
        )
        .unwrap()
        .content,
        seed: 1_041_003,
        started: true,
        sound: true,
        ..Default::default()
    })
    .unwrap();
    let mut feed = Feed::with_assets(Lanterns::assets()).unwrap();
    let mut writes = Recording::default();
    feed.feed_to(old.world(), &mut writes).unwrap();
    let ledge = old.world().named("ledge").unwrap();
    assert_eq!(writes.position(ledge, false).x, 10.0);
    sim.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    sim.restore_bound(&bytes).unwrap();
    sim.agent(r#"{"op":"clock","ticks":1}"#);
    feed.feed_to(sim.world(), &mut writes).unwrap();
    assert_eq!(writes.position(ledge, false).x, 11.0);
    let bulb = sim.world().named("lantern-12/bulb").unwrap();
    let material = &writes.materials[bulb.index() as usize * 12..];
    assert_eq!(&material[..4], &[0.1, 0.35, 0.8, 1.0]);
    old.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    old.agent(r#"{"op":"clock","ticks":1}"#);
    assert_eq!(
        old.world()
            .get::<lanterns_logic::Lantern>("lantern-12")
            .unwrap()
            .glow
            .value(old.world().now()),
        sim.world()
            .get::<lanterns_logic::Lantern>("lantern-12")
            .unwrap()
            .glow
            .value(sim.world().now())
    );
}
