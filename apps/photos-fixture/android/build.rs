//! Compile and bake `app.contract` into `OUT_DIR/app.plan`, with the
//! compatibility id beside it (as the Linux crate computes it: the Canvas
//! host is that host's presenter) and what `contract::native::android_entry`
//! writes, for `exact_linux::canvas_jni!` to embed.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    let plan = contract::compile_path(std::path::Path::new("../app.contract"))
        .unwrap_or_else(|e| panic!("app.contract:{e}"));
    let baked = contract::bake(plan, photos_fixture_data::Fixture::default())
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let fixture = photos_fixture_data::Fixture::default();
    let grants = fixture.grants();
    let compat = exact_bake::compatibility_id(&app_dir, "linux", &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    contract::native::android_entry(&app_dir, &manifest, &baked, &out_dir)
        .unwrap_or_else(|e| panic!("android: {e}"));
}
