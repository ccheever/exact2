use exact_game::Sim;
use lanterns_logic::{scene_types, Lanterns, Options};
use std::time::Instant;

fn game() -> Sim<Lanterns> {
    use exact_game::Game;
    Sim::new(Options {
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scene.json"),
            &scene_types(),
            Lanterns::assets(),
        )
        .unwrap()
        .content,
        seed: 1_041_003,
        started: true,
        sound: true,
        ..Options::default()
    })
    .unwrap()
}

// Include the normal clock, physics, follow, audio and observation work. Each
// sample starts fresh, with the same held movement and 10,000 active ticks.
#[test]
#[ignore = "manual 10k-tick timing; median of five"]
fn tick_10k_median() {
    let mut samples = Vec::new();
    for _ in 0..5 {
        let mut sim = game();
        sim.key_down("KeyW");
        let start = Instant::now();
        sim.run(10_000.0 * 1000.0 / 60.0);
        samples.push(start.elapsed().as_nanos() / 10_000);
        assert_eq!(sim.world().tick(), 10_000);
        std::hint::black_box(sim.world().hash());
    }
    samples.sort_unstable();
    println!("Lanterns ns/tick: {samples:?}; median {}", samples[2]);
}

#[test]
fn typed_facade_preserves_original_world_bytes() {
    let mut sim = game();
    for (ms, expected) in [
        (0.0, 0xaa5115299d8598d3),
        (1000.0, 0x99071d4692d75e6f),
        (2000.0, 0xbb79c1986b61792a),
    ] {
        sim.key_down("KeyW");
        sim.run(ms);
        assert_eq!(sim.world().hash(), expected);
        // Optional local byte comparison against the pre-facade capture.
        if let Ok(dir) = std::env::var("EXACT_KIND_BASELINE") {
            let bytes =
                std::fs::read(format!("{dir}/lanterns-{}.world", sim.world().tick())).unwrap();
            assert_eq!(sim.world().save(), bytes);
        }
    }
}

#[path = "../../../../paranoid-test.rs"]
mod paranoid;
#[test]
fn every_tick_save_matches_normal_script() {
    paranoid::compare(game, |sim| {
        sim.hold("KeyW", 713.123);
        sim.tap("Space");
        sim.run(286.877);
        sim.tap("KeyE");
        sim.run(1000.0);
    });
}
