//! Reflect every shader under `shaders/` at build (LLP 1009 D5): naga
//! validates each one — a bad shader fails the build with its line and
//! column, never a canvas at runtime — and what it declares (bindings,
//! layouts, vertex inputs, entry points) is generated as Rust into
//! `OUT_DIR/shaders.rs`, which `src/lib.rs` includes as `shaders`. The WGSL is
//! the one declaration authority; see `exact-gpu-reflect`.

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
    println!("cargo:rerun-if-changed={}", dir.display());
    let generated = exact_gpu_reflect::generate(&dir).unwrap_or_else(|e| panic!("{e}"));
    for source in &generated.sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("shaders.rs");
    std::fs::write(&out, generated.rust).unwrap();
}
