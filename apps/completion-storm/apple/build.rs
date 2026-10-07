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
    // Every Contract file the plan reads, used files and packages too.
    contract::rerun_if_changed(&app.join("app.contract"));
    let plan = contract::compile_path(&app.join("app.contract")).expect("compile Completion Storm");
    let baked =
        contract::bake(plan, completion_storm_data::Storm::default()).expect("offline bake");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("app.plan"), baked.encode()).unwrap();
    let manifest = contract::Manifest::read(&app).expect("app manifest");
    // tvOS bakes the iOS host's plan.
    let platform = if matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("ios" | "tvos")
    ) {
        "ios"
    } else {
        "macos"
    };
    let mut compat = exact_bake::compatibility_id(
        &app,
        platform,
        &std::env::var("TARGET").unwrap(),
        &manifest,
        Some(completion_storm_data::Storm::default().grants()),
    )
    .expect("compatibility receipt");
    // What the archive links, into its compatibility id too (LLP 1047.001 D2).
    let linked = exact_bake::apple_link(&mut compat, &baked, &manifest, "exact_apple")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(out.join("compat.json"), compat.to_json()).unwrap();
    std::fs::write(out.join("linked.rs"), linked).unwrap();
}
