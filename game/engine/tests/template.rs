#[path = "../../new/logic/src/lib.rs"]
mod template;
pub use template::*;
extern crate self as small_game_logic;
#[path = "../../new/logic/tests/sim.rs"]
mod tests;
