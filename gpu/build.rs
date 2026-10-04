//! On Apple this crate presents through exact2's patched wgpu-hal
//! (`vendor/wgpu-hal/EXACT-PATCHES.md`: no empty Metal command buffers, and a
//! frame's presentations on its one submit). Cargo applies a `[patch]` only
//! from the workspace root, so an app in a workspace of its own has to carry
//! the line; without it Cargo links the published wgpu-hal, which lacks what
//! `frame.rs` calls. This says so, with the line, before the compiler fails
//! on a missing method.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DEP_EXACT_WGPU_HAL_PATCHES");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !matches!(os.as_str(), "macos" | "ios" | "tvos") {
        return;
    }
    // Set by the patched crate's build script (its `links` name), for the
    // crates that depend on it directly.
    let patches = std::env::var("DEP_EXACT_WGPU_HAL_PATCHES")
        .ok()
        .and_then(|n| n.parse::<u32>().ok());
    if patches.is_some_and(|n| n >= 2) {
        return;
    }
    let exact2 = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<exact2>".into());
    eprintln!("exact-gpu: this workspace links the published wgpu-hal, not exact2's patched copy.");
    eprintln!("Add this line under [patch.crates-io] in the workspace root's Cargo.toml, where");
    eprintln!("taffy's is (a path relative to that file works too), and build again:");
    eprintln!();
    eprintln!("    wgpu-hal = {{ path = \"{exact2}/vendor/wgpu-hal\" }}");
    eprintln!();
    eprintln!("What the patches are: {exact2}/vendor/wgpu-hal/EXACT-PATCHES.md");
    std::process::exit(1);
}
