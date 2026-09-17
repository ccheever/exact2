//! Run with cargo run --release -p exact-game --example churn.
use exact_game::{Component, Quat, Rng, Transform, Vec3, World};
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
    rare();
}
