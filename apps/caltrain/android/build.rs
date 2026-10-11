//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, with
//! the compatibility id beside it, for `exact_linux::canvas_jni!` to embed.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // The library is empty off Android (src/lib.rs): nothing to bake.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked =
        contract::bake(plan, caltrain_data::Caltrain).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    // The compatibility id (LLP 1030 D3a) as `../linux/build.rs` computes it
    // for this target: the Canvas host is the Linux host's presenter under
    // Android's own Canvas. That host has no update store and replaces no
    // Rust module, whatever the record says of either (host/android/README.md).
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let grants = caltrain_data::Caltrain.grants();
    let compat = exact_bake::compatibility_id(&app_dir, "linux", &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
}
