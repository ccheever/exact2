//! Compile the rules program to the plan `rules::PLAN` embeds. A compile
//! error names the line and column in `rules/rules.contract`.
fn main() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rules/rules.contract");
    contract::rerun_if_changed(&src);
    let plan = contract::compile_path(&src).unwrap_or_else(|e| panic!("{e}"));
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("rules.plan");
    std::fs::write(out, plan.encode()).unwrap();
}
