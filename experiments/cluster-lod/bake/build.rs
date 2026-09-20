fn main() {
    let vendor = "../vendor/meshoptimizer";
    let mut sources: Vec<_> = std::fs::read_dir(format!("{vendor}/src"))
        .expect("vendored sources")
        .map(|entry| entry.expect("source entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cpp"))
        .collect();
    sources.sort();
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .opt_level(2)
        .debug(false)
        .include(format!("{vendor}/src"))
        .include(format!("{vendor}/demo"))
        .files(&sources)
        .file("src/shim.cpp")
        .compile("clod_vendor");
    println!("cargo:rerun-if-changed=src/shim.cpp");
    println!("cargo:rerun-if-changed={vendor}");
}
