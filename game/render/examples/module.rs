//! Compile proof for the one-line game GPU module (native and wasm).
#[path = "../tests/fixture/mod.rs"]
mod test_game;
use crate::test_game::Fixture;
exact_game_render::module!(Fixture);
fn main() {
    assert_eq!(REGISTRY.surfaces[0].0, "world");
    assert_eq!(REGISTRY.surfaces[0].1, 3);
    assert!(REGISTRY.shaders.is_empty());
}
