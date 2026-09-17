//! Minimal retained physics entry point for raw/gzip wasm size measurements.
use exact_game::{Transform, World};
use exact_game_physics::{self as physics, Body, Collider};
#[no_mangle]
pub extern "C" fn simulate(ticks: u32) -> u64 {
    let mut w = World::new(60, 0);
    physics::register(&mut w);
    w.spawn((Transform::at(0.0, -0.5, 0.0), Collider::default()));
    w.spawn((
        Transform::at(0.0, 2.0, 0.0),
        Collider::default(),
        Body::default(),
    ));
    for _ in 0..ticks {
        physics::step(&mut w);
    }
    w.hash()
}
