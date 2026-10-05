use exact_game::*;
use wind_fixture_logic::{WindGame, SIDE};

#[test]
fn the_field_is_still_in_the_simulation() {
    let mut sim = Sim::<WindGame>::new(()).unwrap();
    sim.run(1000.);
    assert_eq!(sim.world().query::<&Mesh>().iter().count(), SIDE * SIDE + 1);
    // The wind is drawn, never simulated: nothing moves between ticks.
    assert_eq!(sim.world().hash(), {
        let mut fresh = Sim::<WindGame>::new(()).unwrap();
        fresh.run(1000.);
        fresh.world().hash()
    });
    assert!(sim.settle());
}

#[test]
fn the_gust_is_drawn_state_outside_saves_and_hashes() {
    let mut sim = Sim::<WindGame>::new(()).unwrap();
    sim.run(1000.);
    let gust = sim
        .world()
        .get::<wind_fixture_logic::Gust>("wind")
        .unwrap()
        .0;
    assert!((0.4..=1.1).contains(&gust), "{gust}");
    // A restored world rebuilds the same gust from its tick.
    let mut restored = Sim::<WindGame>::new(()).unwrap();
    restored.restore(&sim.save().unwrap()).unwrap();
    assert_eq!(
        restored
            .world()
            .get::<wind_fixture_logic::Gust>("wind")
            .unwrap()
            .0,
        gust
    );
    assert_eq!(restored.world().hash(), sim.world().hash());
}
