//! Compile and bake `app.contract` into `OUT_DIR/app.plan` (the 10,000 messages
//! are the resource's compiled boot value), plus the compatibility id.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=../app.contract");
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../data/messages.json");
    println!("cargo:rerun-if-changed=build.rs");
    let plan = contract::compile_path(std::path::Path::new("../app.contract"))
        .unwrap_or_else(|e| panic!("app.contract:{e}"));
    let baked = contract::bake(plan, exact_bounded_data::Heavy::default())
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = exact_bounded_data::Heavy::default();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(source.grants()))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    assert_eq!(compat.inputs["store"]["L"], "0", "app.json deploy.store must stay 0 (no update store)");
}
