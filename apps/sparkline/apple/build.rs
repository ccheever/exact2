//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the library carries its plan and the app links exactly one archive.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=../app.contract");
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked =
        contract::bake(plan, sparkline_data::Spark).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => "ios",
        _ => "macos",
    };
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let grants = sparkline_data::Spark.grants();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let host = if compat.inputs["store"]["L"] == "0" {
        "exact_apple"
    } else {
        "exact_apple_update"
    };
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\n{host}::host!(AppData, PLAN, COMPAT);\n",
            contract::rust_entry(
                "sparkline_data::Spark",
                "sparkline_data::Spark",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap()
        ),
    )
    .unwrap();
}
