//! Reflect the shader inventory the bake ships: the game bake writes every
//! shader under gpu.shaderRoots, after its gpu.shaderPreludes, to the directory
//! EXACT_GAME_SHADERS names (.shells/shaders), so `SHADERS` carries exactly the
//! interfaces the hosts register.
fn main() {
    println!("cargo:rerun-if-env-changed=EXACT_GAME_SHADERS");
    let dir = std::path::PathBuf::from(
        std::env::var_os("EXACT_GAME_SHADERS").expect("EXACT_GAME_SHADERS: build through the game bake (bun game/app/shells.mjs)"),
    );
    println!("cargo:rerun-if-changed={}", dir.display());
    let generated = exact_gpu_reflect::generate(&dir).unwrap_or_else(|e| panic!("{e}"));
    for source in &generated.sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("shaders.rs");
    std::fs::write(out, generated.rust).unwrap();
}
