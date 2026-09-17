//! Run with cargo run --release -p exact-game --example churn.
use exact_game::{Component, Transform, Vec3, World};
use std::time::Instant;

#[derive(Default, Component)]
struct Velocity(Vec3);
fn main() {
    const ENTITIES: usize = 10_000;
    const TICKS: usize = 1_000;
    let mut world = World::new(60, 42);
    for i in 0..ENTITIES {
        world.spawn((
            Transform::at(i as f32, 0.0, 0.0),
            Velocity(Vec3::new(1.0, 2.0, 3.0)),
        ));
    }
    let dt = world.dt();
    let start = Instant::now();
    // Sim will own step_clock in the next brief; these are the same fixed-step
    // integrations, with setup and hashing outside the measured interval.
    for _ in 0..TICKS {
        for (_, (mut transform, velocity)) in world.query::<(&mut Transform, &Velocity)>() {
            transform.position += velocity.0 * dt;
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    std::hint::black_box(world.hash());
    println!(
        "{ENTITIES} entities, {TICKS} ticks: {:.0} ticks/s, {:.2} ns/entity-tick",
        TICKS as f64 / elapsed,
        elapsed * 1e9 / (ENTITIES * TICKS) as f64
    );
}
