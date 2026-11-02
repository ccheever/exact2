#[path = "../../new/logic/src/lib.rs"]
mod template;
pub use template::*;
extern crate self as small_game_logic;
#[path = "../../new/logic/tests/sim.rs"]
mod tests;

/// A save from the starter before its critter and shop (save contract 1) is
/// refused by name at restore, not run into a missing resource on the next tick.
#[test]
fn a_pre_kit_starter_save_is_refused_by_name() {
    let old = include_bytes!("fixtures/starter-v1.world");
    let mut game = exact_game::Sim::<SmallGame>::new(Options::default()).unwrap();
    let refused = game.restore(old).unwrap_err().to_string();
    assert!(
        refused.contains("refuses this save: its save contract is absent (version 1); this build reads version 2"),
        "{refused}"
    );
    // The running world is untouched and still ticks.
    game.run(100.0);
    let saved = game.save().unwrap();
    assert!(game.restore(&saved).is_ok());
}
