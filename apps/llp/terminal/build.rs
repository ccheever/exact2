//! Compile the terminal entry once, when the executable is built.

fn main() {
    let entry = std::path::Path::new("../terminal.contract");
    contract::rerun_if_changed(entry);
    let plan = contract::compile_path_terminal(entry)
        .unwrap_or_else(|errors| panic!("terminal.contract: {errors:?}"));
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out.join("terminal.plan"), plan.encode()).expect("write terminal plan");
}
