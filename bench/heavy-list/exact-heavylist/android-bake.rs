//! Bake the original Heavy List Contract/data for an embedded Android owner.
use contract::DataSource;

pub fn main() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for path in [
        "app.contract",
        "app.json",
        "data",
        "android-bake.rs",
        "Cargo.toml",
    ] {
        println!("cargo:rerun-if-changed={}", app.join(path).display());
    }
    println!("cargo:rerun-if-env-changed=BENCH_LIVE");
    let manifest = contract::Manifest::read(&app).expect("Heavy List manifest");
    let source = exact_heavylist_data::Heavy::default();
    let plan = contract::compile_path(&app.join("app.contract")).expect("Heavy List Contract");
    let baked = contract::bake(plan, source).expect("Heavy List baked resources");
    let target = std::env::var("TARGET").expect("Cargo target");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("Cargo output"));
    std::fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let source = exact_heavylist_data::Heavy::default();
    let compat =
        exact_bake::compatibility_id(&app, "android", &target, &manifest, Some(source.grants()))
            .expect("Heavy List Android compatibility receipt");
    assert_eq!(
        compat.inputs["store"]["L"], "0",
        "benchmark has no delivery store"
    );
    std::fs::write(out.join("compat.json"), compat.to_json()).unwrap();
}
