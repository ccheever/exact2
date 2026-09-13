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
    exact_js_bake::build(Path::new(".."), platform).expect("bake Messages");
}
