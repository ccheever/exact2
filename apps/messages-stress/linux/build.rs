//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the library carries its plan and the app links exactly one archive.

use contract::DataSource;

#[path = "../factory.rs"]
mod factory;

fn main() {
    let source_factory = factory::Source::selected();
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=build.rs");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked = contract::bake(plan, messages_stress_data::MessagesStress)
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    // The compatibility id (LLP 1030 D3a) for this platform, beside the
    // plan: what a bundle may depend on and this binary cannot replace.
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = "linux";
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let grants = messages_stress_data::MessagesStress.grants();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let host = if compat.inputs["store"]["L"] == "0" {
        "exact_linux"
    } else {
        "exact_linux_update"
    };
    // The app's hatches, when it has `modules/linux/*.rs` (LLP 1075.003.000.001 §5).
    let (hatches, run) = contract::native::rust_hatch_entry(&app_dir, &manifest, &target)
        .unwrap_or_else(|e| panic!("hatches: {e}"));
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\n{hatches}fn main() {{ std::process::exit({host}::{run}); }}\n",
            source_factory
                .entry(compat.inputs["rustMode"].as_str().unwrap())
                .unwrap()
        ),
    )
    .unwrap();
}
