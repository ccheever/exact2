//! Shared build-time Windows engine identity. Never linked into a shipped host.
//! @ref LLP 1027.006 — build.rs and the producer accept the same complete install.

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

#[cfg(test)]
#[path = "hermes_tests.rs"]
mod tests;

/// facebook/hermes. The Apple provisioner reads this declaration too.
pub const HERMES_PIN: &str = "6badada762121682b5481b6124e6c3a991ae6046";

pub const ICU_PIN: &str = "8eca245c7484ac6cc179e3e5f7c1ea7680810f39";
pub const ICU_SOURCE_SHA512: &str = "b702ab62fb37a1574d5f4a768326d0f8fa30d9db5b015605b5f8215b5d8547f83d84880c586d3dcc7b6c76f8d47ef34e04b0f51baa55908f737024dd79a42a6c";
pub const MSYS2_SHA256: &str = "ea2f31a0b6ade63914ce441ffb022f0f6aa96982bfefa2326460a26d5fb01322";
pub const MAKE_SHA256: &str = "af0bdba17f06fe037f0194069adaa31a8fe45f1a11381501896aea1fae37bd5d";
pub const ICU_DATA: &str = "icu-data/icudt76l.dat";
pub const PATCH_OLD: &str =
    "if ((NOT EMSCRIPTEN) AND target_type MATCHES \"EXECUTABLE|STATIC_LIBRARY\")";
pub const PATCH_NEW: &str =
    "if ((NOT EMSCRIPTEN) AND (NOT WIN32) AND target_type MATCHES \"EXECUTABLE|STATIC_LIBRARY\")";
pub const PATCH_BEFORE: &str = "ae124a8b50f14fece21cccc059b82cfb5d493248bf3e817c2f1321a7b0f20beb";
pub const PATCH_AFTER: &str = "c8777d23ad355f34b2ed0a74f93a185641693ca4fc4da9c74085bc4e9a91903c";

// The receipt binds this exact semantic source set, separately from the upstream
// pin and the CMake portability clause. A core-only schema2 cache cannot pass.
fn intl_patch() -> Result<serde_json::Value, String> {
    let policy: serde_json::Value = serde_json::from_str(include_str!("windows-intl.json"))
        .map_err(|e| format!("exact-js: invalid compiled Intl policy: {e}"))?;
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    if policy["patchSha256"] != digest(include_bytes!("windows-intl.patch"))
        || policy["probe"]["sourceSha256"] != digest(include_bytes!("windows-intl-probe.js"))
        || policy["probe"]["runnerSha256"] != digest(include_bytes!("windows-intl-probe.cc"))
    {
        return Err("exact-js: Intl patch/probe sources differ from compiled policy".into());
    }
    for (index, bytes) in [
        include_bytes!("windows-intl-case.inc").as_slice(),
        include_bytes!("windows-intl-date.inc").as_slice(),
        include_bytes!("windows-intl-number.inc").as_slice(),
    ]
    .iter()
    .enumerate()
    {
        if policy["fragments"][index]["sha256"] != digest(bytes) {
            return Err("exact-js: Intl fragment differs from compiled policy".into());
        }
    }
    Ok(policy)
}

pub const SYSTEM_LIBRARIES: &[&str] = &["dbghelp", "version", "psapi", "winmm", "advapi32"];
pub const ARCHIVES: &[&str] = &[
    "windows-static/hermesvmlean_a.lib",
    "windows-static/jsi.lib",
    "windows-static/boost_context.lib",
    "windows-static/icuuc.lib",
    "windows-static/icui18n.lib",
    "windows-static/icudata.lib",
];

/// Package JavaScript entries on Windows, where Bun writes .exe/.bunx shims.
/// The callers run their upstream shebang through Bun, preserving overrides.
pub fn package_tool(root: &Path, name: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(match name {
            "tsc" => "node_modules/typescript/bin/tsc",
            "rolldown" => "node_modules/rolldown/bin/cli.mjs",
            _ => unreachable!("the producer has only tsc and rolldown package tools"),
        })
    } else {
        root.join("node_modules/.bin").join(name)
    }
}

// The build script consumes every field; the producer needs the compiler and
// identity. Both include this source instead of maintaining two validators.
#[allow(dead_code)]
pub struct Install {
    pub root: PathBuf,
    pub headers: PathBuf,
    pub compiler: PathBuf,
    pub archives: Vec<PathBuf>,
    pub receipt_sha256: String,
    pub bytecode_version: u32,
    pub inputs: Vec<PathBuf>,
}

pub fn root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("EXACT_HERMES_DIR") {
        return Ok(PathBuf::from(path));
    }
    let local = std::env::var_os("LOCALAPPDATA")
        .ok_or("exact-js: LOCALAPPDATA is unset; name a complete install with EXACT_HERMES_DIR")?;
    Ok(PathBuf::from(local).join("Exact/hermes").join(format!(
        "{}-lean-windows-x64-icu76-intl1",
        &HERMES_PIN[..12]
    )))
}

pub fn compiler() -> Result<PathBuf, String> {
    Ok(std::env::var_os("EXACT_HERMESC")
        .map(PathBuf::from)
        .unwrap_or(root()?.join("hermesc.exe")))
}

pub fn resolve(target: &str, compiler: Option<&Path>) -> Result<Install, String> {
    validate(&root()?, target, compiler)
}

fn hash(path: &Path) -> Result<(u64, String), String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    let mut bytes = 0;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        digest.update(&buffer[..n]);
    }
    Ok((bytes, format!("{:x}", digest.finalize())))
}

fn headers(root: &Path, current: &Path, names: &mut BTreeSet<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(current).map_err(|e| format!("{}: {e}", current.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            headers(root, &entry.path(), names)?;
        } else if kind.is_file() {
            let path = entry.path();
            let name = path.strip_prefix(root).map_err(|e| e.to_string())?;
            names.insert(name.to_string_lossy().replace('\\', "/"));
        } else {
            return Err(format!(
                "unsupported engine header entry {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

/// Verify identity before accepting any archive or executing the compiler.
pub fn validate(root: &Path, target: &str, compiler: Option<&Path>) -> Result<Install, String> {
    if target != "x86_64-pc-windows-msvc" {
        return Err(format!(
            "exact-js: Windows lean Hermes does not support {target}"
        ));
    }
    let root = root.canonicalize().map_err(|e| {
        format!(
        "exact-js: no complete Windows lean Hermes at {}: {e}; run pwsh -File js/build-windows.ps1",
        root.display()
    )
    })?;
    let receipt_path = root.join("hermes-input-receipt.json");
    let receipt =
        std::fs::read(&receipt_path).map_err(|e| format!("{}: {e}", receipt_path.display()))?;
    let json: serde_json::Value =
        serde_json::from_slice(&receipt).map_err(|e| format!("{}: {e}", receipt_path.display()))?;
    for (key, expected) in [
        ("schema", "exact/hermes-windows-lean/3"),
        ("sourceCommit", HERMES_PIN),
        ("target", target),
        ("role", "lean"),
        ("crt", "MD"),
    ] {
        if json[key].as_str() != Some(expected) {
            return Err(format!(
                "exact-js: Windows Hermes receipt {key} must be {expected}"
            ));
        }
    }
    if json["iteratorDebugLevel"].as_u64() != Some(0)
        || json["debugger"].as_bool() != Some(false)
        || json["jit"].as_bool() != Some(false)
        || json["intl"].as_bool() != Some(true)
        || json["systemLibraries"] != serde_json::json!(SYSTEM_LIBRARIES)
    {
        return Err("exact-js: Windows Hermes ABI/build policy mismatch".into());
    }
    // include/hermes/BCGen/HBC/BytecodeVersion.h at HERMES_PIN.
    let version = 99;
    if json["bytecodeVersion"].as_u64() != Some(version as u64) {
        return Err("exact-js: Windows Hermes receipt bytecodeVersion must be 99".into());
    }
    let icu = &json["icu"];
    if icu["version"] != "76.1"
        || icu["sourceCommit"] != ICU_PIN
        || icu["sourceSha512"] != ICU_SOURCE_SHA512
        || icu["linkage"] != "static"
        || icu["extras"].as_bool() != Some(false)
        || icu["toolsEnabled"].as_bool() != Some(true)
        || icu["dataEnabled"].as_bool() != Some(true)
        || icu["dataPath"] != ICU_DATA
        || icu["tools"]["msys2Sha256"] != MSYS2_SHA256
        || icu["tools"]["makeSha256"] != MAKE_SHA256
    {
        return Err("exact-js: Windows static ICU receipt policy mismatch".into());
    }
    let patch = &json["build"]["sourcePatch"];
    let patch_sha256 = format!(
        "{:x}",
        Sha256::digest(format!("{PATCH_OLD}\n{PATCH_NEW}\n"))
    );
    if patch["path"] != "CMakeLists.txt"
        || patch["beforeSha256"] != PATCH_BEFORE
        || patch["afterSha256"] != PATCH_AFTER
        || patch["old"] != PATCH_OLD
        || patch["new"] != PATCH_NEW
        || patch["sha256"].as_str() != Some(patch_sha256.as_str())
    {
        return Err("exact-js: Windows static ICU source patch identity mismatch".into());
    }
    let intl = intl_patch()?;
    if json["build"]["intlPatch"] != intl
        || json["build"]["intlProbe"]["assertions"] != intl["probe"]["assertions"]
        || json["build"]["intlProbe"]["result"]["assertions"] != intl["probe"]["assertions"]
        || json["build"]["intlProbe"]["source"]["sha256"] != intl["probe"]["sourceSha256"]
        || json["build"]["intlProbe"]["runner"]["sha256"] != intl["probe"]["runnerSha256"]
    {
        return Err("exact-js: Windows Intl semantic patch/probe identity mismatch".into());
    }
    let entries = json["files"]
        .as_array()
        .ok_or("exact-js: Windows Hermes receipt has no files")?;
    let mut expected = BTreeSet::from(["hermesc.exe".to_owned(), ICU_DATA.to_owned()]);
    expected.extend(ARCHIVES.iter().map(|s| s.to_string()));
    headers(&root, &root.join("hermes-headers"), &mut expected)?;
    headers(&root, &root.join("icu-headers"), &mut expected)?;
    for required in [
        "hermes-headers/hermes/hermes.h",
        "hermes-headers/jsi/jsi.h",
        "icu-headers/unicode/utypes.h",
        "icu-headers/unicode/dtptngen.h",
        "icu-headers/unicode/timezone.h",
        "icu-headers/ICU-LICENSE",
    ] {
        if !expected.contains(required) {
            return Err(format!(
                "exact-js: missing Windows Hermes header {required}"
            ));
        }
    }
    let mut seen = BTreeSet::new();
    let mut inputs = vec![receipt_path];
    let mut compiler_identity = None;
    for entry in entries {
        let name = entry["path"]
            .as_str()
            .ok_or("exact-js: engine file has no path")?;
        if name.contains('\\')
            || name.split('/').any(|s| s.is_empty())
            || !Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            || !expected.contains(name)
            || !seen.insert(name.to_owned())
        {
            return Err(format!(
                "exact-js: unexpected or repeated engine input {name}"
            ));
        }
        let path = root.join(name);
        let identity = hash(&path)?;
        if entry["bytes"].as_u64() != Some(identity.0)
            || entry["sha256"].as_str() != Some(identity.1.as_str())
        {
            return Err(format!(
                "exact-js: Windows Hermes input mismatch: {}",
                path.display()
            ));
        }
        if name == "hermesc.exe" {
            if json["build"]["intlProbe"]["compiler"]["sha256"].as_str()
                != Some(identity.1.as_str())
                || json["build"]["intlProbe"]["compiler"]["bytes"].as_u64() != Some(identity.0)
            {
                return Err("exact-js: Intl probe compiler does not match the linked pack".into());
            }
            compiler_identity = Some(identity);
        } else if name == ICU_DATA
            && (icu["dataIdentity"]["bytes"].as_u64() != Some(identity.0)
                || icu["dataIdentity"]["sha256"].as_str() != Some(identity.1.as_str()))
        {
            return Err("exact-js: Windows static ICU data identity mismatch".into());
        }
        inputs.push(path);
    }
    if seen != expected {
        return Err(format!(
            "exact-js: incomplete Windows Hermes inventory: {:?}",
            expected.difference(&seen).collect::<Vec<_>>()
        ));
    }
    // Refuse an unrecorded archive/data/header or DLL, including one outside
    // the public header trees. All executable build inputs belong to this pack.
    let mut actual = BTreeSet::new();
    headers(&root, &root, &mut actual)?;
    actual.remove("hermes-input-receipt.json");
    if actual != expected {
        return Err("exact-js: unexpected Windows Hermes install payload".into());
    }
    let compiler = compiler
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("hermesc.exe"));
    if Some(hash(&compiler)?) != compiler_identity {
        return Err(format!(
            "exact-js: compiler {} does not match the linked Windows Hermes receipt",
            compiler.display()
        ));
    }
    if !inputs.contains(&compiler) {
        inputs.push(compiler.clone());
    }
    Ok(Install {
        headers: root.join("hermes-headers"),
        archives: ARCHIVES.iter().map(|p| root.join(p)).collect(),
        root,
        compiler,
        receipt_sha256: format!("{:x}", Sha256::digest(receipt)),
        bytecode_version: version,
        inputs,
    })
}
