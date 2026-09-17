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
fn main() {
    println!(
        "{} / {}; release={}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        !cfg!(debug_assertions)
    );
    rotations(100_000);
    rotations(500_000);
    churn();
    half_churn();
    rare();
    propagation();
}
