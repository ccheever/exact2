//! Retained physics entry points for wasm size and cross-host continuation parity.
use exact_game::{Transform, World};
use exact_game_physics::{self as physics, Body, Collider};
#[no_mangle]
pub extern "C" fn simulate(ticks: u32) -> u64 {
    simulate_with_restore(ticks, false)
}
pub(crate) fn simulate_with_restore(ticks: u32, every_tick: bool) -> u64 {
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
        if every_tick {
            let hash = w.hash();
            w.load(&w.save()).unwrap();
            assert_eq!(w.hash(), hash);
        }
    }
    w.hash()
}

#[path = "../tests/common/mod.rs"]
pub(crate) mod common;
/// Full saved pile continuation card, also callable directly from browser wasm.
#[no_mangle]
pub extern "C" fn pile_hash() -> u64 {
    let mut sim = common::scene("pile");
    for tick in 1..=90 {
        common::tick(&mut sim, tick);
    }
    let mut restored = common::scene("pile");
    restored.restore(&sim.save()).unwrap();
    common::tick(&mut restored, 90);
    for tick in 91..=600 {
        common::tick(&mut sim, tick);
        common::tick(&mut restored, tick);
        assert_eq!(sim.world().hash(), restored.world().hash());
    }
    let hash = sim.world().hash();
    assert_eq!(hash, 0x129ba6d92f9ac217);
    hash
}
