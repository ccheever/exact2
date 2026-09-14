fn main() {
    println!(
        "cargo:rustc-env=EXACT_LOGIC_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
