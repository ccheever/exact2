//! Bake Fieldnotes as the Linux crate does (the Canvas host is that host's
//! presenter), then write what `contract::native::android_entry` adds: the
//! hatches' install and the plan's uses.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // The library is empty off Android (src/lib.rs): nothing to bake.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    exact_js_bake::build_mixed_with(
        std::path::Path::new(".."),
        "linux",
        &fieldnotes_data::Backup::default(),
        fieldnotes_data::RUST_SOURCES,
        |javascript| {
            Box::new(fieldnotes_data::mixed(
                javascript,
                fieldnotes_data::Placement::Main,
            ))
        },
    )
    .unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    println!("cargo:rerun-if-changed=../app.json");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let bytes = std::fs::read(out_dir.join("app.plan")).expect("the bake's plan");
    let plan =
        exact_plan::Plan::decode(&bytes).unwrap_or_else(|e| panic!("the bake's plan: {e:?}"));
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    contract::native::android_entry(&app_dir, &manifest, &plan, &out_dir)
        .unwrap_or_else(|e| panic!("android: {e}"));
}
