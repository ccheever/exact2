//! Bake the shared core Contract for the web oracle.

fn main() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for path in ["app.contract", "app.json", "data", "web/build.rs"] {
        println!("cargo:rerun-if-changed={}", app.join(path).display());
    }
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let manifest = contract::Manifest::read(&app).expect("core app manifest");
    let plan = contract::compile_path(&app.join("app.contract")).expect("core Contract");
    let baked = contract::bake(plan, android_core_data::Core).expect("bake core rows");
    let target = std::env::var("TARGET").unwrap();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let compat = exact_bake::compatibility_id(&app, "web", &target, &manifest, Some(""))
        .expect("core web receipt");
    std::fs::write(out.join("compat.json"), compat.to_json()).unwrap();
    std::fs::write(
        out.join("linked.rs"),
        contract::web_linked(&baked, &compat.inputs),
    )
    .unwrap();
}
