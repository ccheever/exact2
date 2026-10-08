//! Compile and bake `app.contract` into `OUT_DIR/app.plan` (as `apple/build.rs`), plus the web
//! compatibility id.

use contract::DataSource;

fn main() {
    for p in ["../app.contract", "../svgs.contract", "../lottie.contract", "../rings.contract", "../app.json", "../data", "../assets", "build.rs"] {
        println!("cargo:rerun-if-changed={p}");
    }
    let plan = contract::compile_path(std::path::Path::new("../app.contract")).unwrap_or_else(|e| panic!("app.contract:{e}"));
    let baked = contract::bake(plan, exact_xheavy_data::XHeavy::for_bake()).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = exact_xheavy_data::XHeavy::for_bake();
    let compat = exact_bake::compatibility_id(&app_dir, "web", &target, &manifest, Some(source.grants()))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    // The host entry (as exact2's apps/caltrain/web/build.rs): the data type and the capabilities the plan links.
    let entry = contract::rust_entry("exact_xheavy_data::XHeavy", "exact_xheavy_data::XHeavy::default()", contract::web_rust_mode(&compat.inputs)).unwrap();
    std::fs::write(
        out_dir.join("entry.rs"),
        format!("{entry}\n{}exact_web::host!(AppData, PLAN, COMPAT, app_data);\n", contract::web_linked(&baked, &compat.inputs)),
    )
    .unwrap();
}
