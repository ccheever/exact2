//! Run with cargo run --release -p exact-game --example churn.
use exact_game::{Component, Parent, Quat, Rng, Transform, Vec3, World};
use std::hint::black_box;
use std::time::Instant;

#[derive(Default, Component)]
struct Spin {
    step: Quat,
}
#[derive(Default, Component)]
struct Rare;

fn spin() -> Spin {
    Spin {
        step: Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalize(), 0.01),
    }
}
fn world(n: usize) -> World {
    let mut world = World::new(120, 42);
    for i in 0..n {
        world.spawn((Transform::at(i as f32, 0.0, 0.0), spin()));
    }
    world
}
fn rotate(world: &World) {
    for (_, (t, s)) in world.query::<(&mut Transform, &Spin)>().iter() {
        t.rotation = (s.step * t.rotation).normalize();
    }
}
fn rotations(n: usize) {
    const TICKS: usize = 200;
    let world = world(n);
    for _ in 0..10 {
        rotate(&world);
    }
    let mut samples = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..TICKS {
            rotate(black_box(&world));
        }
        samples.push(start.elapsed().as_secs_f64() * 1e9 / (n * TICKS) as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "rotation {n}: {:.2} ns/entity (median of 5 x {TICKS} ticks)",
        samples[2]
    );
    println!("rotation {n} hash: {:016x}", black_box(world.hash()));
}
fn churn() {
    let mut world = world(100_000);
    let mut entities: Vec<_> = world.entities().collect();
    let mut rng = Rng::new(42);
    let mut samples = Vec::new();
    for _ in 0..100 {
        // Selection is outside the measured interval; every batch has 1,000 unique indices.
        for i in 0..1000 {
            let j = rng.range(i as u32..entities.len() as u32) as usize;
            entities.swap(i, j);
        }
        let start = Instant::now();
        for &e in &entities[..1000] {
            assert!(world.despawn(e));
        }
        for e in &mut entities[..1000] {
            *e = world.spawn((Transform::default(), spin()));
        }
        samples.push(start.elapsed());
    }
    samples.sort();
    println!(
        "despawn + respawn 1,000 / 100,000: {:.3} ms (median; p95 {:.3} ms)",
        samples[50].as_secs_f64() * 1e3,
        samples[95].as_secs_f64() * 1e3
    );
    black_box(world.hash());
}
fn rare() {
    let mut world = world(100_000);
    let entities: Vec<_> = world.entities().collect();
    for i in 0..12 {
        world.insert(entities[i * 8_999], Rare);
    }
    const QUERIES: usize = 100_000;
    let start = Instant::now();
    for _ in 0..QUERIES {
        black_box(black_box(&world).query::<&Rare>());
    }
    println!(
        "12-row query construction / 100,000: {:.3} us/query",
        start.elapsed().as_secs_f64() * 1e6 / QUERIES as f64
    );
    let start = Instant::now();
    for _ in 0..QUERIES {
        black_box(black_box(&world).query::<&Rare>().iter().count());
    }
    println!(
        "12-row query construction + iteration: {:.3} us/query",
        start.elapsed().as_secs_f64() * 1e6 / QUERIES as f64
    );
}
fn half_churn() {
    let mut w = world(100_000);
    let entities: Vec<_> = w.entities().step_by(2).collect();
    let start = Instant::now();
    for &e in entities.iter().rev() {
        w.despawn(e);
    }
    for _ in &entities {
        w.spawn((Transform::default(), spin()));
    }
    println!(
        "despawn + respawn 50,000 / 100,000: {:.3} ms",
        start.elapsed().as_secs_f64() * 1e3
    );
    black_box(w.hash());
}
fn propagation() {
    let mut w = World::new(120, 42);
    let entities: Vec<_> = (0..500_000)
        .map(|_| w.spawn(Transform::at(1.0, 0.0, 0.0)))
        .collect();
    fn measure(w: &mut World, description: &str) {
        for _ in 0..10 {
            w.propagate();
        }
        let start = Instant::now();
        for _ in 0..100 {
            black_box(&mut *w).propagate();
        }
        println!(
            "propagate {description}: {:.3} us/call",
            start.elapsed().as_secs_f64() * 1e6 / 100.0
        );
    }
    measure(&mut w, "500,000 roots");
    // 10,000 independent chains, each with five parented entities and one root.
    for chain in 0..10_000 {
        let base = chain * 50;
        for depth in 1..=5 {
            w.insert(entities[base + depth], Parent(entities[base + depth - 1]));
        }
    }
    measure(&mut w, "500,000 entities / 50,000 parented, chains of 5");
    assert_eq!(w.global(entities[5]).unwrap().translation.x, 6.0);
    let start = Instant::now();
    for &e in entities.iter().step_by(50).take(1000) {
        w.despawn(e);
    }
    println!(
        "despawn 1,000 among 50,000 parented: {:.3} ms",
        start.elapsed().as_secs_f64() * 1e3
    );
    let start = Instant::now();
    w.reap_orphans();
    println!(
        "reap 5,000 orphaned descendants: {:.3} ms",
        start.elapsed().as_secs_f64() * 1e3
    );
}
fn proximity() {
    for (entities, every) in [(32, 4), (100_000, 8_999), (10_000, 1)] {
        let mut w = world(entities);
        let origin = w.spawn(Transform::default());
        let candidates: Vec<_> = w.entities().step_by(every).collect();
        for &e in &candidates {
            w.insert(e, Rare);
        }
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..1_000 {
                assert_eq!(
                    black_box(&w).nearest_xz::<Rare>(black_box(origin), 100_000.),
                    Some(candidates[0])
                );
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 1_000.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "nearest / {entities} entities, {} candidates: {:.3} us/query (median of 7 x 1,000)",
            candidates.iter().filter(|&&e| e != origin).count(),
            samples[3]
        );
    }
}
fn model_layout() {
    use exact_game::{asset::*, Camera, Game, Input, Mat4, Mesh, Sim, Vec2};
    struct Scene;
    impl Game for Scene {
        const ID: &'static str = "layout-bench";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("model", (Transform::default(), Mesh::asset("rig.model")));
            w.spawn((Transform::at(0., 0., 20.), Camera::default()));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    for vertices in [3, 30_000] {
        let model = Model {
            meshes: vec![MeshData {
                positions: [1., 0., 0.].repeat(vertices),
                normals: [0., 1., 0.].repeat(vertices),
                uvs: vec![0.; vertices * 2],
                joints: vec![0; vertices * 4],
                weights: [1., 0., 0., 0.].repeat(vertices),
                indices: (0..vertices as u32).collect(),
                bounds: [-1., -1., -1., 1., 1., 1.],
                ..Default::default()
            }],
            materials: vec![MaterialData::default()],
            nodes: vec![Node {
                mesh: Some(0),
                skin: Some(0),
                ..Default::default()
            }],
            skins: vec![Skin {
                joints: vec![0],
                inverse_binds: Mat4::IDENTITY.to_cols_array().to_vec(),
                ..Default::default()
            }],
            bounds: [-1., -1., -1., 1., 1., 1.],
            ..Default::default()
        };
        model.validate().unwrap();
        let mut sim = Sim::<Scene>::new(()).unwrap();
        sim.deliver_asset("rig.model", Ok(Content::Model(model)))
            .unwrap();
        sim.viewport(1280., 720.);
        let entity = sim.world().named("model").unwrap();
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..100 {
                black_box(black_box(&sim).layout(entity).unwrap());
                assert_eq!(
                    black_box(&sim).pick(Vec2::new(640., 360.)).unwrap().entity,
                    entity
                );
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 100.);
        }
        samples.sort_by(f64::total_cmp);
        println!("model layout + pick / {vertices} vertices: {:.3} us/pair (median of 7 x 100); hash {:016x}", samples[3], sim.world().hash());
    }
}
fn texture_delivery() {
    use exact_game::{
        asset::{Content, TextureData},
        Game, Input, Sim,
    };
    struct Textures;
    impl Game for Textures {
        const ID: &'static str = "texture-delivery-bench";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    for count in [8, 64, 256] {
        let names: Vec<_> = (0..count).rev().map(|i| format!("{i:03}.tex")).collect();
        let mut sim = Sim::<Textures>::new(()).unwrap();
        sim.defer_assets(true);
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..100 {
                for name in &names {
                    sim.deliver_asset(
                        name,
                        Ok(Content::Texture(TextureData {
                            width: 1,
                            height: 1,
                            mips: vec![vec![255; 4]],
                            ..Default::default()
                        })),
                    )
                    .unwrap();
                }
                assert_eq!(black_box(sim.take_textures()).len(), count);
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 100.);
        }
        samples.sort_by(f64::total_cmp);
        println!("texture delivery + drain / {count} names: {:.3} us/batch (median of 7 x 100); hash {:016x}", samples[3], sim.world().hash());
    }
}
fn asset_restore() {
    use exact_game::{
        asset::{Content, Model},
        Game, Input, Sim,
    };
    struct Assets;
    impl Game for Assets {
        const ID: &'static str = "asset-restore-bench";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    for count in [0, 8, 64, 256] {
        let mut sim = Sim::<Assets>::new(()).unwrap();
        for i in 0..count {
            sim.deliver_asset(
                &format!("{i:03}.model"),
                Ok(Content::Model(Model::default())),
            )
            .unwrap();
        }
        let world = sim.world().save();
        let simulation = sim.save().unwrap();
        for whole in [false, true] {
            let mut samples = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                for _ in 0..100 {
                    if whole {
                        black_box(&mut sim).restore(&simulation).unwrap();
                    } else {
                        black_box(sim.world_mut()).load(&world).unwrap();
                    }
                }
                samples.push(start.elapsed().as_secs_f64() * 1e6 / 100.);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{} restore / {count} assets: {:.3} us/restore (median of 7 x 100); hash {:016x}",
                if whole { "Sim" } else { "World" },
                samples[3],
                sim.world().hash()
            );
        }
    }
}
fn main() {
    println!(
        "{} / {}; release={}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        !cfg!(debug_assertions)
    );
    if std::env::args().any(|arg| arg == "--proximity") {
        proximity();
        return;
    }
    if std::env::args().any(|arg| arg == "--model-layout") {
        model_layout();
        return;
    }
    if std::env::args().any(|arg| arg == "--texture-delivery") {
        texture_delivery();
        return;
    }
    if std::env::args().any(|arg| arg == "--asset-restore") {
        asset_restore();
        return;
    }
    rotations(100_000);
    rotations(500_000);
    churn();
    half_churn();
    rare();
    propagation();
}
