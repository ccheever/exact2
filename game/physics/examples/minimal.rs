//! Retained physics entry points for wasm size and cross-host continuation parity.
use exact_game::{Paranoid, Transform, World};
use exact_game_physics::{self as physics, Body, Collider};
#[no_mangle]
pub extern "C" fn simulate(ticks: u32) -> u64 {
    simulate_with_restore(ticks, exact_game::Paranoid::Off)
}
/// The same two-body card reconstructed after every step, for browser parity.
#[no_mangle]
pub extern "C" fn simulate_restored(ticks: u32, fresh: bool) -> u64 {
    simulate_with_restore(
        ticks,
        if fresh {
            Paranoid::FreshGame
        } else {
            Paranoid::Save
        },
    )
}
pub(crate) fn simulate_with_restore(ticks: u32, mode: exact_game::Paranoid) -> u64 {
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
        if mode != exact_game::Paranoid::Off {
            let hash = w.hash();
            let bytes = w.save();
            if mode == exact_game::Paranoid::FreshGame {
                w = World::new(60, 0);
                physics::register(&mut w);
            }
            w.load(&bytes).unwrap();
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
    let mut sim = common::scene("pile").paranoid(Paranoid::Off);
    let mut saved = common::scene("pile").paranoid(Paranoid::Save);
    let mut fresh = common::scene("pile").paranoid(Paranoid::FreshGame);
    for tick in 1..=90 {
        common::tick(&mut sim, tick);
        common::tick(&mut saved, tick);
        common::tick(&mut fresh, tick);
        assert_eq!(sim.world().hash(), saved.world().hash());
        assert_eq!(sim.world().hash(), fresh.world().hash());
    }
    let mut restored = common::scene("pile");
    restored.restore(&sim.save().unwrap()).unwrap();
    common::tick(&mut restored, 90);
    for tick in 91..=600 {
        common::tick(&mut sim, tick);
        common::tick(&mut saved, tick);
        common::tick(&mut fresh, tick);
        assert_eq!(sim.world().hash(), saved.world().hash());
        assert_eq!(sim.world().hash(), fresh.world().hash());
        common::tick(&mut restored, tick);
        assert_eq!(sim.world().hash(), restored.world().hash());
    }
    let hash = sim.world().hash();
    assert_eq!(hash, 0x129ba6d92f9ac217);
    hash
}
