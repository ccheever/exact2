#[path = "../../games/beacons/logic/src/lib.rs"]
mod beacons;
pub use beacons::*;
extern crate self as beacons_logic;
#[path = "../../games/beacons/logic/tests/sim.rs"]
mod tests;
