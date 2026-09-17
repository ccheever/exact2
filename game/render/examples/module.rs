//! Compile proof for the one-line game GPU module (native and wasm).
use greybox_logic::Greybox;
exact_game_render::module!(Greybox);
fn main() {
    assert_eq!(REGISTRY.surfaces[0].0, "world");
    assert_eq!(REGISTRY.surfaces[0].1, 2);
    assert!(REGISTRY.shaders.is_empty());
}
