//! The Messages schema is compiled beside its Contract and TypeScript, never at boot.
pub fn build(platform: &str) {
    use std::path::Path;
    println!("cargo:rerun-if-changed=../bake.rs");
    println!("cargo:rerun-if-changed=../snapback/schema.q");
    let schema = std::fs::read_to_string("../snapback/schema.q").expect("Messages schema");
    let bound = snapback4_lang::compile(&[("schema.q".into(), schema)]);
    assert!(
        bound.diagnostics.is_empty(),
        "Messages schema: {:?}",
        bound.diagnostics
    );
    let backend = serde_json::json!({
        "generation": 1,
        "schema": bound.schema,
        "programs": bound.programs.iter().map(|p| &p.program).collect::<Vec<_>>(),
    });
    let source = format!(
        "// @generated from schema.q at bake; do not edit.\nexport const backend = {backend};\n"
    );
    let output = Path::new("../snapback/backend.ts");
    if std::fs::read_to_string(output).ok().as_deref() != Some(&source) {
        let temporary = output.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temporary, source).expect("write Messages backend");
        std::fs::rename(temporary, output).expect("install Messages backend");
    }
    let license = std::fs::read("../vendor/snapback4/LICENSE").expect("Snapback4 MIT license");
    std::fs::create_dir_all("../assets").unwrap();
    if std::fs::read("../assets/snapback4-LICENSE.txt")
        .ok()
        .as_deref()
        != Some(license.as_slice())
    {
        std::fs::write("../assets/snapback4-LICENSE.txt", license).unwrap();
    }
    // Published browser device, loaded after first pixel; never host boot code.
    let package = Path::new("../../../node_modules/snapback4");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(package.join("package.json"))
            .expect("bun install --frozen-lockfile"),
    )
    .unwrap();
    assert_eq!(manifest["version"], "0.2.30", "Messages device release");
    for (input, output, prefix) in [
        (
            "dist/wasm/snapback4_device.js",
            "../snapback/generated/device.ts",
            "// @ts-nocheck\n// @generated from snapback4 0.2.30; MIT license.\n",
        ),
        (
            "dist/wasm/snapback4_device_bg.wasm",
            "../assets/snapback4-device.wasm",
            "",
        ),
    ] {
        let input = package.join(input);
        println!("cargo:rerun-if-changed={}", input.display());
        let mut bytes = prefix.as_bytes().to_vec();
        bytes.extend(std::fs::read(input).expect("published Snapback4 web device"));
        let output = Path::new(output);
        if std::fs::read(output).ok().as_deref() != Some(bytes.as_slice()) {
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();
            let temporary = output.with_extension(format!("{}.tmp", std::process::id()));
            std::fs::write(&temporary, bytes).unwrap();
            std::fs::rename(temporary, output).unwrap();
        }
    }
    exact_js_bake::build(Path::new(".."), platform).expect("bake Messages");
}
