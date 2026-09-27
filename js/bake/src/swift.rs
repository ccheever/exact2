//! Build an app's Swift native module (LLP 1067) from its build script:
//!
//! ```ignore
//! exact_js_bake::swift_native(&["swift/Native.swift"]).expect("native module");
//! ```
//!
//! The app's sources are compiled with exact2's `js/native/ExactNative.swift`
//! (the C seam `exact_js::swift_native_module!` links) into one static
//! library for the target Cargo is building and the deployment target the
//! host build chose from the manifest's `minimumOS`. Cargo bundles it into
//! the crate's archive; the host's Swift link supplies the Swift runtime and
//! the frameworks' autolink entries. Nothing happens off Apple targets.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The bridge every app's Swift module is compiled with.
fn bridge() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../native/ExactNative.swift")
}

/// Compile and link `sources` (paths relative to the crate) as the app's
/// Swift native module.
pub fn swift_native(sources: &[&str]) -> Result<(), String> {
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    if var("CARGO_CFG_TARGET_VENDOR") != "apple" {
        return Ok(());
    }
    let mut inputs: Vec<PathBuf> = sources.iter().map(PathBuf::from).collect();
    inputs.push(bridge());
    for input in &inputs {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    // The host build sets these from app.json's `minimumOS`, so this library
    // and the host link at one deployment target; the defaults are the
    // Apple host's own floor, for a bare `cargo check`.
    println!("cargo:rerun-if-env-changed=IPHONEOS_DEPLOYMENT_TARGET");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let ios = std::env::var("IPHONEOS_DEPLOYMENT_TARGET").unwrap_or_else(|_| "17.0".into());
    let macos = std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "14.0".into());
    let (triple, sdk) = match var("TARGET").as_str() {
        "aarch64-apple-ios-sim" => (format!("arm64-apple-ios{ios}-simulator"), "iphonesimulator"),
        "x86_64-apple-ios" => (
            format!("x86_64-apple-ios{ios}-simulator"),
            "iphonesimulator",
        ),
        "aarch64-apple-ios" => (format!("arm64-apple-ios{ios}"), "iphoneos"),
        "x86_64-apple-darwin" => (format!("x86_64-apple-macos{macos}"), "macosx"),
        "aarch64-apple-darwin" => (format!("arm64-apple-macos{macos}"), "macosx"),
        other => return Err(format!("swift_native: no Swift target for {other}")),
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").map_err(|e| e.to_string())?).join(&triple);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let library = out.join("libexact_native.a");
    // Cargo reruns this script for the bake's own inputs too; the Swift is
    // compiled only when a source is newer than the library built for this
    // triple (the triple names the directory, so a new target rebuilds).
    let built = library.metadata().and_then(|m| m.modified()).ok();
    let stale = built.is_none()
        || inputs
            .iter()
            .any(|input| input.metadata().and_then(|m| m.modified()).ok() > built);
    if stale {
        let sdk_path = Command::new("xcrun")
            .args(["--sdk", sdk, "--show-sdk-path"])
            .output()
            .map_err(|e| format!("xcrun: {e}"))?;
        let sdk_path = String::from_utf8(sdk_path.stdout).map_err(|e| e.to_string())?;
        let status = Command::new("xcrun")
            .args(["swiftc", "-parse-as-library", "-emit-library", "-static"])
            .args(["-module-name", "ExactNative", "-O", "-target", &triple])
            .args(["-sdk", sdk_path.trim()])
            .arg("-o")
            .arg(&library)
            .args(&inputs)
            .status()
            .map_err(|e| format!("swiftc: {e}"))?;
        if !status.success() {
            return Err(format!("swiftc failed for {triple}"));
        }
    }
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=exact_native");
    Ok(())
}
