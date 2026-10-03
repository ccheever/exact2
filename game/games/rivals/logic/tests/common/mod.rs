//! The baked art every test world loads before setup (the bake writes ../assets).
use exact_game::{Game, Sim};

pub fn loaded<G: Game>(mut sim: Sim<G>) -> Sim<G> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    sim.load_assets(|name| std::fs::read(dir.join(name)))
        .expect("baked assets: run the art bake (shells.mjs --test does)");
    sim
}
