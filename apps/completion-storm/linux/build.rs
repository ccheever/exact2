use contract::DataSource;

fn main() {
    for path in [
        "../app.contract",
        "../app.json",
        "../data",
        "build.rs",
        "../../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let plan = contract::compile_path(&app.join("app.contract")).expect("compile Completion Storm");
    let baked =
        contract::bake(plan, completion_storm_data::Storm::default()).expect("offline bake");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let manifest = contract::Manifest::read(&app).expect("app manifest");
    let platform = "linux";
    let compat = exact_bake::compatibility_id(
        &app,
        platform,
        &std::env::var("TARGET").unwrap(),
        &manifest,
        Some(completion_storm_data::Storm::default().grants()),
    )
    .expect("compatibility receipt");
    std::fs::write(out.join("compat.json"), compat.to_json()).unwrap();
}
