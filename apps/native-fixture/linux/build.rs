//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the library carries its plan and the app links exactly one archive.

use contract::DataSource;

fn main() {
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    println!("cargo:rerun-if-changed=build.rs");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked = contract::bake(plan, native_fixture_data::Fixture)
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
    let source = native_fixture_data::Fixture;
    let grants = source.grants();
    let compat = exact_bake::compatibility_id(&app_dir, platform, &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let host = if compat.inputs["store"]["L"] == "0" {
        "exact_linux"
    } else {
        "exact_linux_update"
    };
    // The app's hatches (LLP 1075.003.000.001 §5): `modules/linux/*.rs`,
    // included here with the typed key of the words this platform handles.
    // The entry names only their type; the host makes the value.
    let hatches = contract::native::rust_hatches(&app_dir, &manifest, platform)
        .unwrap_or_else(|e| panic!("hatches: {e}"));
    let run = match hatches {
        Some(_) => {
            "run_with_hatches::<AppData, hatches::ExactHatches>(PLAN, COMPAT, HatchKey::WORDS)"
        }
        None => "run::<AppData>(PLAN, COMPAT)",
    };
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\n{}fn main() {{ std::process::exit({host}::{run}); }}\n",
            contract::rust_entry(
                "native_fixture_data::Fixture",
                "native_fixture_data::Fixture",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap(),
            hatches.as_deref().unwrap_or("")
        ),
    )
    .unwrap();
}
