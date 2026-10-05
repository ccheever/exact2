//! Reflect the shader inventory exactly as the bake ships it: every shader under
//! `shaders/` (gpu.shaderRoots), each after its declared gpu.shaderPreludes in
//! app.json, so `SHADERS` carries the interfaces the hosts register. The
//! assembled sources land in OUT_DIR/shaders for tests that register them.
use std::path::{Path, PathBuf};

fn main() {
    let render = Path::new(env!("CARGO_MANIFEST_DIR"));
    let app = render.join("..");
    let manifest = app.join("app.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    let preludes = &json["gpu"]["shaderPreludes"];
    let dir = render.join("shaders");
    println!("cargo:rerun-if-changed={}", dir.display());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let assembled = out.join("shaders");
    std::fs::create_dir_all(&assembled).unwrap();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|x| x != "wgsl") {
            continue;
        }
        // The bake's join (scripts/app.mjs shaderFiles): each prelude, a newline, then the shader.
        let mut text = String::new();
        for prelude in preludes[stem].as_array().into_iter().flatten() {
            let file = app.join(prelude.as_str().unwrap());
            println!("cargo:rerun-if-changed={}", file.display());
            text.push_str(&std::fs::read_to_string(&file).unwrap());
            text.push('\n');
        }
        text.push_str(&std::fs::read_to_string(&path).unwrap());
        std::fs::write(assembled.join(format!("{stem}.wgsl")), text).unwrap();
    }
    let generated = exact_gpu_reflect::generate(&assembled).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(out.join("shaders.rs"), generated.rust).unwrap();
}
