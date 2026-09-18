use beacons_logic::{Beacons, Options, Player};
use exact_game::{Sim, Transform, Vec3};
fn sim(seed: u64) -> Sim<Beacons> {
    Sim::new(Options {
        seed,
        scene: exact_game_scene::bake::compile(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .find(|p| p.join("games/beacons/scene.json").is_file())
                .unwrap()
                .join("games/beacons/scene.json"),
            &beacons_logic::scene_types(),
            &[],
        )
        .unwrap()
        .content,
        ..Options::default()
    })
    .unwrap()
}
#[test]
fn movement_seed_and_partitioning() {
    let mut a = sim(7);
    let mut b = sim(7);
    assert_eq!(a.save(), b.save());
    assert_ne!(
        a.world().get::<Transform>("crate-1").unwrap().position,
        sim(8).world().get::<Transform>("crate-1").unwrap().position
    );
    a.key_down("KeyW");
    b.key_down("KeyW");
    a.run(1500.0);
    for _ in 0..1500 {
        b.run(1.0);
    }
    assert_eq!(a.save(), b.save());
    assert_eq!(a.world().hash(), b.world().hash());
    assert_eq!(a.world().hash(), 0x58d5d637a36c8365);
    assert_eq!(a.position("player"), Some(Vec3::new(0.0, 0.9, -5.3666644)));
    println!(
        "1500ms position={:?}, hash={:016x}",
        a.position("player").unwrap(),
        a.world().hash()
    );
    a.key_up("KeyW");
    a.run(100.0);
    assert!(a.position("player").unwrap().z < -5.3666644);
    assert!(a.settle());
    assert_eq!(
        a.world()
            .get::<Player>("player")
            .unwrap()
            .character
            .velocity,
        Vec3::ZERO
    );
}
#[test]
fn jump_no_double_jump_and_restore() {
    let mut a = sim(7);
    a.tap("Space");
    a.run(250.0);
    let mut b = sim(7);
    b.restore(&a.save()).unwrap();
    a.tap("Space");
    a.run(250.0);
    b.run(250.0);
    assert_eq!(a.position("player").unwrap(), b.position("player").unwrap());
    assert!((a.position("player").unwrap().y - 2.1).abs() < 0.005);
    a.run(1000.0);
    assert_eq!(a.position("player").unwrap().y, 0.9);
    b.key_down("ArrowRight");
    b.run(713.0);
    a.restore(&b.save()).unwrap();
    a.run(827.0);
    b.run(827.0);
    assert_eq!(a.save(), b.save());
}

#[test]
fn idle_tick_preserves_beacon_pages_and_partial_entities_keep_original_joins() {
    use beacons_logic::Beacon;
    use exact_game::{Camera, DirectionalLight, Material, Mesh};
    fn pages<C: exact_game::Component>(w: &exact_game::World) -> Vec<u64> {
        w.pages::<C>().iter().map(|p| p.generation).collect()
    }
    let mut s = sim(7);
    let w = s.world();
    let before = (
        pages::<Beacon>(w),
        pages::<Mesh>(w),
        pages::<Camera>(w),
        pages::<DirectionalLight>(w),
    );
    s.run(1000.0 / 60.0);
    assert_eq!(s.world().tick(), 1);
    let w = s.world();
    assert_eq!(
        before,
        (
            pages::<Beacon>(w),
            pages::<Mesh>(w),
            pages::<Camera>(w),
            pages::<DirectionalLight>(w)
        )
    );
    let w = s.world_mut();
    let partial = w.spawn_named(
        "no-material",
        (Transform::at(0.0, 0.9, 0.0), Beacon::default()),
    );
    let mut glow = Beacon {
        lit: true,
        ..Default::default()
    };
    glow.glow.to(w.now(), 1.0, 0.01);
    let no_pose = w.spawn_named("no-pose", (glow, Material::default()));
    s.tap("KeyE");
    s.run(100.0);
    assert!(s.world().get::<Beacon>(partial).unwrap().lit);
    assert!(s.world().get::<Material>(no_pose).unwrap().emissive[1] > 0.0);
}
#[test]
fn save_load_every_tick_matches_uninterrupted() {
    let mut a = sim(7);
    let mut b = sim(7);
    a.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    b.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    a.key_down("KeyW");
    b.key_down("KeyW");
    for tick in 0..180 {
        if tick == 45 {
            a.tap("Space");
            b.tap("Space");
        }
        if tick == 90 {
            a.tap("KeyE");
            b.tap("KeyE");
        }
        a.agent(r#"{"op":"clock","ticks":1}"#);
        b.agent(r#"{"op":"clock","ticks":1}"#);
        assert!(
            a.save() == b.save(),
            "continuation save mismatch at tick {tick}"
        );
        b.restore(&b.save()).unwrap();
        // Restore clears consumed input edges; compare complete world bytes immediately.
        assert!(
            a.world().save() == b.world().save(),
            "world mismatch at tick {tick}"
        );
    }
    assert!(a.position("player").unwrap().z < -5.0);
}
#[test]
#[ignore = "manual 10k-tick timing; median of five"]
fn tick_10k_median() {
    let mut samples = Vec::new();
    for _ in 0..5 {
        let mut s = sim(7);
        s.key_down("KeyW");
        let start = std::time::Instant::now();
        s.run(10_000.0 * 1000.0 / 60.0);
        samples.push(start.elapsed().as_nanos() / 10_000);
        assert_eq!(s.world().tick(), 10_000);
        std::hint::black_box(s.world().hash());
    }
    samples.sort_unstable();
    println!("Beacons ns/tick: {samples:?}; median {}", samples[2]);
}
