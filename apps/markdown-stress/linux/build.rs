//! Compile and bake `app.contract` into `OUT_DIR/app.plan` at build time, so
//! the library carries its plan and the app links exactly one archive.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=../app.contract");
    println!("cargo:rerun-if-changed=build.rs");
    let plan = match contract::compile_path(std::path::Path::new("../app.contract")) {
        Ok(p) => p,
        Err(e) => panic!("app.contract:{e}"),
    };
    let baked = contract::bake(plan, markdown_stress_data::MarkdownStress::default())
        .unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    // The compatibility id (LLP 1030 D3a) for this platform, beside the
    // plan: what a bundle may depend on and this binary cannot replace.
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../../markdown/parse");
    println!("cargo:rerun-if-changed=../../../Cargo.lock");
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let platform = "linux";
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let source = markdown_stress_data::MarkdownStress::default();
    let grants = source.grants();
    let compat = contract::compatibility_id(&app_dir, platform, &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    let host = if compat.inputs["store"]["L"] == "0" {
        "exact_linux"
    } else {
        "exact_linux_update"
    };
    std::fs::write(
        out_dir.join("entry.rs"),
        format!(
            "{}\nfn main() {{
    let region_args: Vec<_> = std::env::args().filter(|a| a == \"--content-region\" || a.starts_with(\"--content-region=\")).collect();
    if !region_args.is_empty() {{
        let activate = match region_args.as_slice() {{
            [one] if one == \"--content-region=1048576\" => \"launchParagraph1MiB\",
            [one] if one == \"--content-region=4194304\" => \"launchParagraph4MiB\",
            _ => {{
                eprintln!(\"content-region trial requires --content-region=1048576 or --content-region=4194304 (paragraph, explicit CPU); default mixed startup is unsupported\");
                std::process::exit(2);
            }}
        }};
        let region = exact_linux::content_region::ContentRegionRegistration {{
            activate: Some(activate), owner: \"markdown-region-owner\",
            content: \"markdown-region-content\", pending: \"markdown-region-pending\",
        }};
        std::process::exit(exact_linux::app::run_with_content_region::<markdown_stress_data::NativeMarkdownStress>(PLAN, COMPAT, region));
    }}
    std::process::exit({host}::run::<AppData>(PLAN, COMPAT));
}}\n",
            contract::rust_entry(
                "markdown_stress_data::MarkdownStress",
                "markdown_stress_data::MarkdownStress::default()",
                compat.inputs["rustMode"].as_str().unwrap()
            )
            .unwrap()
        ),
    )
    .unwrap();
}
