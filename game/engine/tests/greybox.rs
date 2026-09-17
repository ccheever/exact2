#[path = "../../games/greybox/logic/src/lib.rs"]
mod greybox;
pub use greybox::*;
extern crate self as greybox_logic;
#[path = "../../games/greybox/logic/tests/sim.rs"]
mod tests;
