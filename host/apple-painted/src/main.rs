#![deny(unsafe_code)]
fn main() {
    let plan = std::env::var_os("EXACT_PLAN")
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_default();
    let compat = std::env::var_os("EXACT_COMPAT")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| "{}".into());
    if plan.is_empty() && !std::env::args().any(|s| s == "--help" || s == "-h") {
        eprintln!("exact-apple-painted: set EXACT_PLAN to a baked plan, or build the app's painted binary");
        std::process::exit(1);
    }
    std::process::exit(exact_apple_painted::run::<()>("Exact", &plan, &compat));
}
