//! Build an app's Swift native module (LLP 1067) from its build script:
//!
//! ```ignore
//! exact_js_bake::swift_native(&["swift/Native.swift"]).expect("native module");
//! ```
//!
//! The app's sources are compiled with exact2's `js/native/ExactNative.swift`
//! into an app-named static library. Cargo bundles it into the crate's archive;
//! the host's Swift link supplies the runtime and frameworks' autolink entries.
//! Nothing happens off Apple targets.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
#[path = "swift_tests.rs"]
mod tests;

fn bridge() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../native/ExactNative.swift")
}

fn names(package: &str) -> (String, String) {
    // Encoding the whole package name keeps '-' and '_' distinct, and both
    // Swift identifiers and C symbols valid without a process-wide registry.
    let name: String = package.bytes().map(|b| format!("{b:02x}")).collect();
    (
        format!("ExactNative_{name}"),
        format!("exact_native_{name}"),
    )
}

struct Toolchain {
    compiler: String,
    version: String,
    sdk: String,
    sdk_version: String,
}

fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|e| e.to_string())
}

fn toolchain(sdk: &str) -> Result<Toolchain, String> {
    let compiler = output("xcrun", &["--find", "swiftc"])?;
    let version = output(&compiler, &["--version"])?;
    let sdk_version = output("xcrun", &["--sdk", sdk, "--show-sdk-build-version"])?;
    let sdk = output("xcrun", &["--sdk", sdk, "--show-sdk-path"])?;
    // A replaced compiler/SDK or changed Xcode selection reruns Cargo's bake.
    println!("cargo:rerun-if-changed={compiler}");
    println!("cargo:rerun-if-changed={sdk}/SDKSettings.plist");
    Ok(Toolchain {
        compiler,
        version,
        sdk,
        sdk_version,
    })
}

fn fingerprint(inputs: &[PathBuf], settings: &[&str]) -> Result<String, String> {
    let mut hash = Sha256::new();
    let mut add = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    for setting in settings {
        add(setting.as_bytes());
    }
    for input in inputs {
        add(input.as_os_str().as_encoded_bytes());
        let bytes =
            std::fs::read(input).map_err(|e| format!("swift_native: {}: {e}", input.display()))?;
        add(&bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn compile(
    inputs: &[PathBuf],
    out: &Path,
    triple: &str,
    package: &str,
    tool: &Toolchain,
) -> Result<PathBuf, String> {
    let (module, prefix) = names(package);
    let library = out.join(format!("lib{prefix}.a"));
    let stamp = out.join(format!("{prefix}.sha256"));
    let flags = ["-parse-as-library", "-emit-library", "-static", "-O"];
    let fingerprint = fingerprint(
        inputs,
        &[
            &module,
            &prefix,
            triple,
            &flags.join(" "),
            &tool.compiler,
            &tool.version,
            &tool.sdk,
            &tool.sdk_version,
        ],
    )?;
    if library.is_file() && std::fs::read_to_string(&stamp).ok().as_deref() == Some(&fingerprint) {
        return Ok(library);
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let generated = out.join(format!("{module}.swift"));
    let bridge = std::fs::read_to_string(inputs.last().expect("the bridge is the final input"))
        .map_err(|e| e.to_string())?;
    std::fs::write(
        &generated,
        bridge.replace("@_cdecl(\"exact_native_", &format!("@_cdecl(\"{prefix}_")),
    )
    .map_err(|e| e.to_string())?;
    // A failed rebuild cannot make an earlier stamp bless a partial archive.
    if stamp.exists() {
        std::fs::remove_file(&stamp).map_err(|e| e.to_string())?;
    }
    let status = Command::new(&tool.compiler)
        .args(flags)
        .args([
            "-module-name",
            &module,
            "-target",
            triple,
            "-sdk",
            &tool.sdk,
        ])
        .arg("-o")
        .arg(&library)
        .args(&inputs[..inputs.len() - 1])
        .arg(&generated)
        .status()
        .map_err(|e| format!("swiftc: {e}"))?;
    if !status.success() {
        return Err(format!("swiftc failed for {triple}"));
    }
    std::fs::write(stamp, fingerprint).map_err(|e| e.to_string())?;
    Ok(library)
}

/// Compile and link `sources` (paths relative to the crate) as the app's
/// Swift native module. The Cargo package names both its Swift module and C ABI.
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
    for name in [
        "IPHONEOS_DEPLOYMENT_TARGET",
        "MACOSX_DEPLOYMENT_TARGET",
        "DEVELOPER_DIR",
        "TOOLCHAINS",
        "SDKROOT",
        "PATH",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // The host's manifest-selected deployment target, or its floor for cargo.
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
    let package = std::env::var("CARGO_PKG_NAME").map_err(|e| e.to_string())?;
    let out = PathBuf::from(std::env::var("OUT_DIR").map_err(|e| e.to_string())?).join(&triple);
    let tool = toolchain(sdk)?;
    compile(&inputs, &out, &triple, &package, &tool)?;
    let (_, prefix) = names(&package);
    println!("cargo:rustc-env=EXACT_SWIFT_SYMBOL_PREFIX={prefix}");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static={prefix}");
    Ok(())
}
