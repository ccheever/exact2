//! A world that shows a model saves and restores in a hostless test.
//! `sim_with_art!` bakes `art/` in memory, from any directory and with no
//! `assets/`; `Sim::new` alone would leave `crate.model` pending, and `save`
//! refuses while a shown asset is ("save refused: assets are not ready").
use exact_game::{Mesh, Sim};

// Apart, so rustfmt's order does not depend on the name a new game gives these.
use small_game_logic::Options;

use small_game_logic::SmallGame;

fn start() -> Sim<SmallGame> {
    exact_game::sim_with_art!(Options::default()).unwrap()
}

#[test]
fn a_world_with_a_model_saves_and_restores_byte_identically() {
    let mut game = start();
    assert_eq!(
        *game.get::<Mesh>("crate").unwrap(),
        Mesh::asset("crate.model")
    );
    assert!(game.world().model("crate.model").is_some());
    game.hold("KeyD", 500.0);
    game.settle();
    let saved = game.save().unwrap();
    game.run(500.0);
    let mut restored = start();
    restored.restore(&saved).unwrap();
    restored.run(500.0);
    assert_eq!(restored.world().hash(), game.world().hash());
    assert_eq!(restored.save().unwrap(), game.save().unwrap());
}
