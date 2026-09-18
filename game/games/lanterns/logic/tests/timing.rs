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
fn saved_bindings_and_physics_v2_pins_match_continuous_and_every_tick_restore() {
    use exact_game::Paranoid;
    let mut sims = [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame]
        .map(|mode| (mode, game().paranoid(mode)));
    let pin_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pins.json");
    let expected: Vec<String> = serde_json::from_slice(&std::fs::read(&pin_path).unwrap()).unwrap();
    let mut actual = Vec::new();
    let mut saves = Vec::new();
    for ms in [0.0, 1000.0, 2000.0] {
        let mut continuous = None;
        for (mode, sim) in &mut sims {
            sim.key_down("KeyW");
            sim.run(ms);
            let hash = sim.world().hash();
            println!(
                "LANTERNS_PIN {mode:?} tick={} hash=0x{hash:016x}",
                sim.world().tick()
            );
            let observation = (hash, sim.world().tick(), sim.save().unwrap());
            if let Some(continuous) = &continuous {
                assert!(
                    continuous == &observation,
                    "{mode:?} differs from continuous at {}",
                    sim.world().tick()
                );
            } else {
                continuous = Some(observation);
            }
        }
        let (hash, tick, bytes) = continuous.unwrap();
        actual.push(format!("0x{hash:016x}"));
        saves.push((tick, bytes));
    }
    // Candidates are isolated by the driver; no test writes committed pins.
    // Every checkpoint must first agree byte-for-byte across all three modes.
    if let Some(dir) = std::env::var_os("EXACT_REPIN_OUT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (tick, bytes) in saves {
            std::fs::write(dir.join(format!("tick-{tick}.sim")), bytes).unwrap();
        }
        std::fs::write(
            dir.join("pins.json"),
            serde_json::to_string_pretty(&actual).unwrap() + "\n",
        )
        .unwrap();
    } else {
        assert_eq!(
            actual, expected,
            "pin mismatch: bun game/proof.mjs lanterns --repin"
        );
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

#[test]
fn respawned_cached_child_matches_continuous_and_every_tick_restore() {
    use exact_game::{Material, Mesh, Paranoid, Parent, PointLight, Transform};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let mut outcomes = Vec::new();
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = game().paranoid(mode);
        sim.run(17.0); // Advance with the saved bindings initialized during setup.
        let world = sim.world_mut();
        let old = world.named("lantern-1/bulb").unwrap();
        let replacement = (
            *world.get::<Transform>(old).unwrap(),
            world.get::<Mesh>(old).unwrap().clone(),
            *world.get::<Material>(old).unwrap(),
            *world.get::<PointLight>(old).unwrap(),
            *world.get::<Parent>(old).unwrap(),
        );
        assert!(world.despawn(old));
        let new = world.spawn_named("lantern-1/bulb", replacement);
        assert_ne!(old, new, "the cached entity generation must become stale");
        let result = catch_unwind(AssertUnwindSafe(|| {
            sim.run(100.0);
            (sim.world().hash(), sim.world().tick(), sim.save().unwrap())
        }))
        .map_err(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "unknown panic".into())
        });
        println!(
            "CHILD_RESPAWN {mode:?}: {:?}",
            result.as_ref().map(|(h, t, _)| (format!("0x{h:016x}"), t))
        );
        outcomes.push(result);
    }
    // Saved IDs deliberately refuse a replaced child; reconstruction must not
    // silently retarget the binding. Require the same precise refusal in all modes.
    for outcome in &outcomes {
        let error = outcome
            .as_ref()
            .expect_err("saved child replacement must refuse");
        for part in ["Lamp", "lantern-1", "lantern.bulb", "generation"] {
            assert!(error.contains(part), "{error}");
        }
    }
    assert!(
        outcomes[1] == outcomes[2],
        "both reconstructed runs must agree"
    );
    assert!(
        outcomes[0] == outcomes[1],
        "Lantern.bulb is stale after respawn; Session.actors prevents rebuilding it: {:?}",
        outcomes[0].as_ref().err()
    );
}
