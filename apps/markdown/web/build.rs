//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the wasm carries its plan and the page fetches exactly one file.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=../app.contract");
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../parse");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    // The bake asks every resource for its first frame. This source's only
    // source reaches a filesystem, and a bake has none: `open("")` is the
    // welcome document, compiled in (LLP 1027 D4).
    let baked = contract::bake(plan, markdown_data::Markdown::new())
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = "web";
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = markdown_data::Markdown::new();
    let grants = source.grants();
    let compat = contract::compatibility_id(&app_dir, platform, &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
}
