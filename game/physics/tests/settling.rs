use exact_game::*;
use exact_game_physics::{self as physics, Body, Collider, Physics};
struct Awake;
impl Game for Awake {
    const ID: &'static str = "unchanged-awake";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        physics::register(w);
        w.resource_mut::<Physics>().gravity = Vec3::ZERO;
        w.spawn((Transform::at(0., -1., 0.), Collider::default()));
        w.spawn_named(
            "supported",
            (Transform::default(), Collider::default(), Body::default()),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        physics::step(w);
    }
}
#[test]
fn unchanged_awake_body_holds_settle_until_sleep() {
    let mut s = Sim::<Awake>::new(()).unwrap();
    s.run(100.);
    assert!(!physics::quiescent(s.world()));
    assert!(!s.quiescent(), "unchanged awake Body falsely settled");
    assert!(s.settle());
    assert!(physics::quiescent(s.world()));
    assert_eq!(s.local_position("supported").unwrap(), Vec3::ZERO);
}

#[path = "../examples/minimal.rs"]
mod minimal;
#[test]
fn exphys_v2_minimal_card_matches_in_every_reconstruction_mode() {
    let pins: std::collections::BTreeMap<String, String> =
        json::from_str(include_str!("pins.json")).unwrap();
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let hash = minimal::simulate_with_restore(120, mode);
        assert_eq!(format!("0x{hash:016x}"), pins["minimal-120"], "{mode:?}");
    }
}
