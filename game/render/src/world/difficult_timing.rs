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
