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
fn saved_binding_hashes_are_pinned() {
    let mut sim = game();
    for (ms, expected) in [
        (0.0, 0xa778d065d6cea372),
        (1000.0, 0x8f7cfe89cd32bdef),
        (2000.0, 0xecf7e7cab49ab213),
    ] {
        sim.key_down("KeyW");
        sim.run(ms);
        assert_eq!(sim.world().hash(), expected);
    }
}
