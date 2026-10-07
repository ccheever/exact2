//! Bake the same core Contract into the Android archive and the web artifact.

pub fn main() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for path in ["app.contract", "app.json", "data", "build.rs"] {
        println!("cargo:rerun-if-changed={}", app.join(path).display());
    }
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let platform = if env!("CARGO_PKG_NAME").ends_with("-web") {
        "web"
    } else {
        "android"
    };
    let manifest = contract::Manifest::read(&app).expect("core app manifest");
    let development = std::env::var("EXACT_UPDATE_TRUST").as_deref() != Ok("production");
    assert_eq!(
        manifest.store(platform),
        "0",
        "the core example is binary-only"
    );
    assert_eq!(manifest.rust_mode(platform, development).unwrap(), "off");
    let plan = contract::compile_path(&app.join("app.contract")).expect("core Contract");
    let baked = contract::bake(plan, android_core_data::Core).expect("bake core rows");
    let target = std::env::var("TARGET").unwrap();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let compat = exact_bake::compatibility_id(&app, platform, &target, &manifest, Some(""))
        .expect("core compatibility receipt");
    std::fs::write(out.join("compat.json"), compat.to_json()).unwrap();
    if platform == "web" {
        std::fs::write(
            out.join("linked.rs"),
            contract::web_linked(&baked, &compat.inputs),
        )
        .unwrap();
    }
}
