//! The compatibility id: the cohort a bundle is safe for (LLP 1030 D3a).
//!
//! @ref LLP 1030 D3a (the id and what enters it) / D2 (policy is not
//! identity) / D4 (the two axes, `L` and `E`); LLP 1030.000 stage 3
//!
//! A bundle may depend on things an installed binary cannot replace — the
//! kernel schema, the plan format, the ABI numbers, the executors linked,
//! the native data crate, the surfaces' shaders, the icons and host
//! capabilities a plan may select, the store's shape. Two binaries that
//! agree on all of them can take the same bundle; two that differ cannot,
//! however alike their version strings. The id is the digest of exactly
//! those inputs, computed by the bake per platform and written beside the
//! plan, so an update stream is named by it and a static origin serves each
//! cohort its own manifest with no negotiation.
//!
//! What must not move it (D3a, D2): `[deploy]` — schedules, channels, the
//! release policy — and a host change a bundle cannot observe. The inputs
//! ride along as JSON so two ids that differ can be explained field by
//! field (`dev.mjs`, `exact deploy`), and a field that has no value yet is
//! `null`, so the id is stable until the thing exists.

use contract::Manifest;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The C ABI's header, whose `EXACT_ABI_VERSION` is one of the numbers.
const ABI_HEADER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../host/apple/include/exact.h"
));
/// The GPU module's C ABI (LLP 1009 D2): unnumbered in the module today.
pub const GPU_MODULE_ABI: u32 = 1;
/// The separately linked Rust data-source request/outcome wire ABI.
pub(crate) const RUST_ABI: u32 = 3;
/// The domain separator over the canonical inputs.
const DOMAIN: &str = "exact2 compatibility id v1\n";

/// The id and the inputs it digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compat {
    /// 16 bytes of the domain-separated SHA-256 over the canonical inputs, lowercase hex.
    pub id: String,
    /// The inputs, one named field each, `null` where a thing does not exist yet.
    pub inputs: serde_json::Value,
    /// The channel this build bakes in and its origin (LLP 1030.000 D4) —
    /// beside the id, not in it: where the binary's update store checks.
    pub channel: String,
    /// The channel's origin URL, if the manifest names one.
    pub origin: Option<String>,
    /// When a staged bundle applies (`deploy.activate`): `next-launch` or
    /// `app-decides` — policy the binary's store follows (LLP 1030.000 D4).
    pub activate: String,
    /// The exact target the binary-producing Cargo bake received.
    pub target: String,
    /// Embedded bundle provenance and its complete static roster.
    pub embedded: serde_json::Value,
    /// What the app can reach and each host's spelling of it (LLP 1069.008
    /// D7): beside the id, not in it; `null` when the grants are unknown.
    pub reach: serde_json::Value,
}

impl Compat {
    /// `{"id":…,"inputs":{…},"delivery":{"activate":…,"channel":…,"origin":…}}`,
    /// canonical (sorted keys within each value), the id first, one line plus
    /// a newline. The runner reads `id`, `store.L`, and `executors` from this
    /// text by hand (`exact_runner::Delivery::with_compat`); a host's update
    /// store reads the rest (`exact_update::Baked::from_compat`).
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\"id\":");
        canonical(&serde_json::Value::String(self.id.clone()), &mut s);
        s.push_str(",\"inputs\":");
        canonical(&self.inputs, &mut s);
        s.push_str(",\"delivery\":");
        canonical(
            &serde_json::json!({
                "activate": self.activate,
                "channel": self.channel,
                "origin": self.origin.clone().map_or(serde_json::Value::Null, serde_json::Value::String),
            }),
            &mut s,
        );
        s.push_str(",\"target\":");
        canonical(&serde_json::json!(self.target), &mut s);
        s.push_str(",\"embedded\":");
        canonical(&self.embedded, &mut s);
        s.push_str(",\"reach\":");
        canonical(&self.reach, &mut s);
        s.push_str("}\n");
        s
    }
}

/// The compatibility id of the app at `app_dir` built for `platform`
/// (`ios`, `macos`, `linux`, `web`) and `target` (the triple), with the
/// data crate's grants when the caller has a `DataSource` to ask (the bake
/// does; `exact-bake compat` on the command line does not).
pub fn compatibility_id(
    app_dir: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    grants: Option<&str>,
) -> Result<Compat, String> {
    compatibility_id_sources(app_dir, platform, target, manifest, grants, None)
}

/// Compatibility for separately owned language sources. With `rust_grants`,
/// `grants` describes JavaScript; the host admits their union and hashes each
/// child's exact grant declaration independently.
pub fn compatibility_id_sources(
    app_dir: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    grants: Option<&str>,
    rust_grants: Option<&str>,
) -> Result<Compat, String> {
    // Cargo must rebake even when only the explicit trust selection changes.
    // External apps call this same entrypoint from their own build scripts.
    if std::env::var_os("OUT_DIR").is_some() {
        println!("cargo:rerun-if-env-changed=EXACT_UPDATE_TRUST");
        println!("cargo:rerun-if-env-changed=EXACT_TYPESCRIPT_PLACEMENT");
        println!("cargo:rerun-if-env-changed=EXACT_RUST_PLACEMENT");
    }
    let trust = match std::env::var("EXACT_UPDATE_TRUST") {
        Ok(value) => value,
        // Unset is development: such a binary checks only an origin the
        // developer names (EXACT_UPDATE_ORIGIN), never the manifest's.
        Err(std::env::VarError::NotPresent) => "development".into(),
        Err(_) => return Err("EXACT_UPDATE_TRUST is not UTF-8".into()),
    };
    let ceiling = rust_grants.map(|rust| grant_union(grants.unwrap_or(""), rust));
    let mut compat = compatibility_with_trust(
        app_dir,
        platform,
        target,
        manifest,
        ceiling.as_deref().or(grants),
        &trust,
    )?;
    if let Some(rust) = rust_grants {
        source_scopes(&mut compat, grants.unwrap_or(""), rust);
    }
    crate::reach::derive(app_dir, platform, &mut compat)?;
    if let Some(out) = std::env::var_os("OUT_DIR") {
        crate::receipt::emit(
            &mut compat,
            &trust,
            app_dir,
            platform,
            target,
            manifest,
            Path::new(&out),
        )?;
    }
    Ok(compat)
}

fn grant_union(javascript: &str, rust: &str) -> String {
    let union = [javascript, rust]
        .into_iter()
        .flat_map(str::lines)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    // Match Mixed::grants: retain the existing receipt spelling when Rust adds
    // no capability; otherwise the combined ceiling is deterministic.
    if union
        .lines()
        .all(|line| javascript.lines().map(str::trim).any(|grant| grant == line))
    {
        javascript.into()
    } else {
        union
    }
}

fn source_scopes(compat: &mut Compat, javascript: &str, rust: &str) {
    compat.inputs["grantCeiling"] = serde_json::Value::String(grant_union(javascript, rust));
    compat.inputs["javascriptGrants"] = serde_json::Value::String(javascript.into());
    compat.inputs["rustGrants"] = serde_json::Value::String(rust.into());
    compat.id = compatibility_digest(&compat.inputs);
}

pub(crate) fn compatibility_digest(inputs: &serde_json::Value) -> String {
    let mut canon = String::new();
    canonical(inputs, &mut canon);
    let mut h = Sha256::new();
    h.update(DOMAIN.as_bytes());
    h.update(canon.as_bytes());
    let digest = h.finalize();
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

fn compatibility_with_trust(
    app_dir: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    grants: Option<&str>,
    trust: &str,
) -> Result<Compat, String> {
    use serde_json::{json, Value};
    if !matches!(trust, "production" | "development") {
        return Err("EXACT_UPDATE_TRUST must be production or development".into());
    }
    if platform == "web"
        && manifest
            .json
            .pointer("/deploy/store/web")
            .is_some_and(|value| value.as_str() != Some("0"))
    {
        return Err("the web host links no updater; deploy.store.web must be \"0\"".into());
    }
    let store = manifest.store(platform);
    if !matches!(store.as_str(), "0" | "A") {
        return Err(format!("deploy.store.{platform} must be \"0\" or \"A\""));
    }
    let binary_only = store == "0";
    let keys = if binary_only {
        Value::Null
    } else {
        manifest.keys()?
    };
    let (channel, origin) = if binary_only {
        (String::new(), None)
    } else {
        manifest.channel()
    };
    if trust == "production"
        && (store != "0" || origin.is_some())
        && !binary_only
        && keys.as_object().is_none_or(|keys| keys.is_empty())
    {
        return Err("production updater requires deploy.signing.keys with at least one verification key; use EXACT_UPDATE_TRUST=development only for a development artifact".into());
    }
    let host = manifest.host(platform);
    let rust_mode = manifest.rust_mode(platform, trust == "development")?;
    let mut executors = executors(app_dir, platform);
    let rust_target = match rust_mode {
        "tiered" => {
            executors.push("native-dylib".into());
            executors.push("wasmi".into());
            Some(target)
        }
        "native" => {
            executors.push("native-dylib".into());
            Some(target)
        }
        "wasm" => {
            executors.push("wasmi".into());
            Some("wasm32-unknown-unknown")
        }
        "browser" => {
            executors.push("browser-wasm".into());
            Some("wasm32-unknown-unknown")
        }
        _ => None,
    };
    executors.sort();
    executors.dedup();
    let rust_module = manifest
        .json
        .pointer("/rust/module/package")
        .and_then(Value::as_str);
    let hermes = executors.iter().any(|e| e == "hermes");
    let wasmtime = executors.iter().any(|e| e == "wasmtime");
    let mut kinds = vec!["plan", "assets"];
    if hermes {
        kinds.push("bytecode");
    }
    if wasmtime {
        kinds.push("wasm");
    }
    let list = |key: &str| -> Value {
        match host.get(key).and_then(|v| v.as_array()) {
            Some(items) => {
                let mut v: Vec<String> = items
                    .iter()
                    .filter_map(|i| i.as_str().map(str::to_string))
                    .collect();
                v.sort();
                json!(v)
            }
            None => Value::Null,
        }
    };
    let mut icons: Vec<String> = manifest
        .json
        .get("icons")
        .and_then(|i| i.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get("src").and_then(|s| s.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    icons.sort();
    let mut inputs = json!({
        "kernelSchema": format!("{:016x}", exact_kernel::SCHEMA_DIGEST),
        "formatVersion": exact_plan::FORMAT_VERSION,
        "formatDigest": format!("{:016x}", exact_plan::FORMAT_DIGEST),
        "abi": { "c": abi_version()?, "gpuModule": GPU_MODULE_ABI, "storeCodec": if binary_only { Value::Null } else { json!(exact_update::STORE_CODEC) } },
        "executors": executors,
        "rustMode": rust_mode,
        "rustAbi": if rust_mode == "off" { Value::Null } else { json!(RUST_ABI) },
        "rustTarget": rust_target,
        "rustModule": rust_module,
        // Where each language's module runs (LLP 1027.002 §6): a host
        // without the executor refuses the build; a change is a new cohort.
        "typescriptPlacement": manifest.placement("typescript", platform)?,
        "rustPlacement": manifest.placement("rust", platform)?,
        "dataCrate": data_crate(app_dir)?,
        // Each shader's reflected interface digest (LLP 1030 D8) — entry
        // points, bindings, layouts, inputs, outputs, overrides, never the
        // text — so a colour edit does not move the id and a binding edit
        // does. Shaders are assets (1030.000 stage 1); the surface's Rust
        // binds this interface.
        "gpuSurfaces": gpu_surfaces(app_dir, manifest)?,
        // @ref LLP 1024 D3/D4 — the roster and the table's ABI: a new tag
        // is a binary change, never a bundle (LLP 1047 D8).
        "nativeModules": match contract::native::roster(manifest)? {
            tags if tags.is_empty() => Value::Null,
            tags => json!({ "abi": 1, "tags": tags }),
        },
        "icons": icons,
        "capabilities": {
            "backgroundModes": list("backgroundModes"),
            "urlSchemes": list("urlSchemes"),
            "associatedDomains": host.get("associatedDomains").and_then(|v| v.as_bool()).map_or(Value::Null, Value::Bool),
        },
        // The verification keys the binary carries (LLP 1026 D11), by id: a
        // rotation is a new cohort (1030 D3a).
        "keys": keys,
        "trust": if binary_only { Value::Null } else { json!(trust) },
        "grantCeiling": grants.map_or(Value::Null, |g| Value::String(g.to_string())),
        "javascriptGrants": Value::Null,
        "rustGrants": grants.map_or(Value::Null, |g| Value::String(g.to_string())),
        "platform": platform,
        "arch": target.split('-').next().unwrap_or(target),
        "minimumOS": host.get("minimumOS").and_then(|v| v.as_str()).map_or(Value::Null, |s| Value::String(s.to_string())),
        "store": { "L": store, "acceptedKinds": if binary_only { vec![] } else { kinds } },
        "app": manifest.id,
    });
    // Which GPU artifact owns which surface (LLP 1009 D6): the manifest's
    // modules, bound into the cohort — a routing change is a new binary. An
    // app with one artifact carries no key, so its id and bytes are as before.
    if let Some(modules) = manifest
        .json
        .pointer("/gpu/modules")
        .filter(|m| m.as_object().is_some_and(|m| !m.is_empty()))
    {
        inputs["gpuModules"] = modules.clone();
    }
    let id = compatibility_digest(&inputs);
    Ok(Compat {
        id,
        inputs,
        channel,
        origin,
        activate: manifest.activate(),
        target: target.into(),
        embedded: serde_json::Value::Null,
        reach: serde_json::Value::Null,
    })
}

/// `EXACT_ABI_VERSION` from the C header.
fn abi_version() -> Result<u32, String> {
    ABI_HEADER
        .lines()
        .find_map(|l| l.trim().strip_prefix("#define EXACT_ABI_VERSION"))
        .and_then(|rest| rest.trim().parse().ok())
        .ok_or_else(|| "exact.h declares no EXACT_ABI_VERSION".to_string())
}

/// The executors the app's composition links (LLP 1029 D2): the native
/// crate always; an `app.ts` uses the browser on web, Hermes on native;
/// wasmtime iff the native host crate names `Swappable`.
fn executors(app_dir: &Path, platform: &str) -> Vec<String> {
    let mut out = vec!["native".to_string()];
    if app_dir.join("app.ts").exists() {
        out.push(
            if platform == "web" {
                "browser"
            } else {
                "hermes"
            }
            .into(),
        );
    }
    let host = app_dir.join("apple/src/lib.rs");
    if platform != "web" && std::fs::read_to_string(host).is_ok_and(|s| s.contains("Swappable")) {
        out.push("wasmtime".into());
    }
    out.sort();
    out
}

/// The data crate's source-input digest (LLP 1030 D3a): its tree, the
/// workspace lockfile, the toolchain — never its compiled output, which a
/// `wasm32` build can keep while the native build moved.
fn data_crate(app_dir: &Path) -> Result<serde_json::Value, String> {
    let dir = app_dir.join("data");
    if !dir.is_dir() {
        return Ok(serde_json::Value::Null);
    }
    let mut files = Vec::new();
    walk(&dir, &dir, &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for rel in &files {
        let bytes = std::fs::read(dir.join(rel)).map_err(|e| format!("{}: {e}", rel.display()))?;
        h.update(rel.to_string_lossy().as_bytes());
        h.update([0]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    let tree = hex(&h.finalize());
    let lockfile = lockfile(app_dir).map(|p| {
        std::fs::read(&p)
            .map(|b| hex(&Sha256::digest(&b)))
            .map_err(|e| format!("{}: {e}", p.display()))
    });
    let lockfile = match lockfile {
        Some(r) => serde_json::Value::String(r?),
        None => serde_json::Value::Null,
    };
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let toolchain = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .map_or(serde_json::Value::Null, serde_json::Value::String);
    Ok(
        serde_json::json!({ "tree": tree, "files": files.len(), "lockfile": lockfile, "toolchain": toolchain }),
    )
}

/// Every file under `root`, as paths relative to it, skipping a `target`
/// directory (a crate built in place) and dot files.
fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
}

/// The nearest `Cargo.lock` at or above `app_dir` (the workspace's).
fn lockfile(app_dir: &Path) -> Option<PathBuf> {
    let mut dir = app_dir.canonicalize().ok()?;
    loop {
        let candidate = dir.join("Cargo.lock");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The declared shader inventory, by stem and interface digest;
/// `null` when the app has no shaders.
fn gpu_surfaces(app_dir: &Path, manifest: &Manifest) -> Result<serde_json::Value, String> {
    // The bake, hosts and dev server share the declared-root inventory.
    // @ref llp/1046.006.000-render-hooks.rfc.md#d5-shaders-that-live-with-the-game
    let gate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/app.mjs");
    let cargo = std::env::var_os("OUT_DIR").is_some();
    if cargo {
        println!("cargo:rerun-if-changed={}", gate.display());
    }
    let code = r#"
        import {pathToFileURL} from 'node:url';
        const {shaderFiles,shaderRoots,shaderPreludeFiles} = await import(pathToFileURL(process.argv[1]));
        const app={dir:process.argv[2],manifest:JSON.parse(process.argv[3])};
        process.stdout.write(JSON.stringify({roots:[...shaderRoots(app),...shaderPreludeFiles(app)],files:[...shaderFiles(app)].map(([name,bytes])=>[name,bytes.toString('utf8')])}));
    "#;
    let result = crate::bun()
        .args(["--input-type=module", "-e", code])
        .arg(gate)
        .arg(app_dir)
        .arg(manifest.json.to_string())
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "shader inventory: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    let inventory: serde_json::Value =
        serde_json::from_slice(&result.stdout).map_err(|e| e.to_string())?;
    for root in inventory["roots"].as_array().unwrap() {
        let path = Path::new(root.as_str().unwrap());
        if cargo && path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let mut shaders = Vec::new();
    for pair in inventory["files"].as_array().unwrap() {
        let name = pair[0].as_str().unwrap();
        let digest = exact_gpu_reflect::interface_digest(pair[1].as_str().unwrap())
            .map_err(|e| format!("shader {name}: {e}"))?;
        shaders.push((
            name.trim_end_matches(".wgsl").to_string(),
            format!("{digest:016x}"),
        ));
    }
    if shaders.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    shaders.sort();
    Ok(serde_json::json!(shaders
        .into_iter()
        .map(|(name, interface)| serde_json::json!({ "name": name, "interface": interface }))
        .collect::<Vec<_>>()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// JSON with sorted keys and no whitespace: the same bytes for the same
/// inputs, whatever built the value.
pub(crate) fn canonical(v: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap_or_default());
                out.push(':');
                canonical(&map[*k], out);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{compatibility_with_trust, Manifest};
    use std::path::{Path, PathBuf};

    #[test]
    fn resolved_manifest_is_the_only_reader_dialect() {
        let dir = app("resolved-only");
        for game in [
            serde_json::json!({"crate":"beacons-logic","type":"Beacons"}),
            serde_json::Value::Null,
        ] {
            std::fs::write(
                dir.join("app.json"),
                serde_json::json!({"id":"com.exact.beacons","name":"Beacons","game":game})
                    .to_string(),
            )
            .unwrap();
            assert!(Manifest::read(&dir).is_err());
        }
        std::fs::write(
            dir.join("app.json"),
            r#"{"app":{"id":"com.exact.beacons","name":"Beacons"}}"#,
        )
        .unwrap();
        assert_eq!(Manifest::read(&dir).unwrap().id, "com.exact.beacons");
        std::fs::create_dir_all(dir.join(".shells")).unwrap();
        std::fs::write(
            dir.join(".shells/app.json"),
            r#"{"app":{"id":"com.exact.resolved","name":"Resolved"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("app.json"), "{}").unwrap();
        assert_eq!(Manifest::read(&dir).unwrap().id, "com.exact.resolved");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mixed_source_ownership_is_hashed_even_when_the_host_ceiling_is_unchanged() {
        let dir = app("mixed-source-grants");
        let manifest = Manifest::read(&dir).unwrap();
        let mut compat = compatibility_with_trust(
            &dir,
            "web",
            "wasm32-unknown-unknown",
            &manifest,
            Some(""),
            "development",
        )
        .unwrap();
        let javascript = "net.fetch https://example.test/";
        let rust = "fs.read app:/data/rust";
        super::source_scopes(&mut compat, javascript, rust);
        assert_eq!(
            compat.inputs["grantCeiling"],
            format!("{rust}\n{javascript}")
        );
        assert_eq!(compat.inputs["javascriptGrants"], javascript);
        assert_eq!(compat.inputs["rustGrants"], rust);
        let first = compat.id.clone();
        super::source_scopes(&mut compat, &format!("{rust}\n{javascript}"), rust);
        assert_eq!(
            compat.inputs["grantCeiling"],
            format!("{rust}\n{javascript}")
        );
        assert_ne!(
            compat.id, first,
            "a child gaining a sibling's grant changes compatibility"
        );
        let second = compat.id.clone();
        super::source_scopes(&mut compat, &format!("{rust}\n{javascript}"), javascript);
        assert_ne!(
            compat.id, second,
            "Rust grant ownership is hashed independently"
        );
        let spelling = format!("{javascript}\n{rust}\n");
        super::source_scopes(&mut compat, &spelling, rust);
        assert_eq!(
            compat.inputs["grantCeiling"], spelling,
            "subset sources retain the JS receipt spelling like Mixed"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rust_policy_inherits_and_refuses_invalid_configuration() {
        let mut m = Manifest {
            json: serde_json::json!({}),
            id: "com.example.app".into(),
            name: "App".into(),
            declared: true,
        };
        for (platform, dev, prod) in [
            ("web", "browser", "browser"),
            ("ios", "wasm", "wasm"),
            ("macos", "native", "native"),
            ("linux", "native", "native"),
            ("android", "native", "wasm"),
            ("windows", "native", "native"),
        ] {
            assert_eq!(m.rust_mode(platform, true).unwrap(), dev);
            assert_eq!(m.rust_mode(platform, false).unwrap(), prod);
        }
        m.json["rust"] = serde_json::json!({"mode":"off","dev":true,"prod":false,"module":{"package":"app-data"},"platforms":{"ios":{"mode":"wasm","prod":false},"linux":{"prod":true}}});
        assert_eq!(m.rust_mode("ios", true).unwrap(), "wasm");
        assert_eq!(m.rust_mode("ios", false).unwrap(), "off");
        assert_eq!(m.rust_mode("linux", false).unwrap(), "native");
        assert_eq!(m.rust_mode("macos", false).unwrap(), "off");
        m.json["rust"] = serde_json::json!({"prod":false,"platforms":{"ios":true}});
        assert_eq!(m.rust_mode("ios", false).unwrap(), "wasm");
        for policy in [
            serde_json::json!(null),
            serde_json::json!("jit"),
            serde_json::json!({"enabled":false}),
            serde_json::json!({"mode":false}),
            serde_json::json!({"platforms":{"iphone":false}}),
            serde_json::json!({"dev":{"prod":false}}),
            serde_json::json!({"module":{"package":"bad/path"}}),
            serde_json::json!({"module":{}}),
        ] {
            m.json["rust"] = policy;
            assert!(m.rust_mode("ios", true).is_err(), "{}", m.json);
        }
        m.json["rust"] = serde_json::json!("native");
        assert!(m.rust_mode("ios", true).is_err());
        assert!(m.rust_mode("web", true).is_err());
        m.json["rust"] = serde_json::json!({"platforms":{"macos":{"dev":"tiered","prod":"native"},"linux":{"mode":"tiered","prod":false}}});
        assert_eq!(m.rust_mode("macos", true).unwrap(), "tiered");
        assert_eq!(m.rust_mode("macos", false).unwrap(), "native");
        assert_eq!(m.rust_mode("linux", true).unwrap(), "tiered");
        assert_eq!(m.rust_mode("linux", false).unwrap(), "off");
        m.json["rust"] = serde_json::json!("tiered");
        assert!(m.rust_mode("web", true).is_err());
        assert!(m.rust_mode("ios", false).is_err());
        m.json["rust"] = serde_json::json!(false);
        assert_eq!(m.rust_mode("ios", true).unwrap(), "off");
    }

    /// A minimal app: a data crate with one file, a manifest with one icon
    /// and a deploy policy, no shaders.
    fn app(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exact-compat-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data/src")).unwrap();
        std::fs::write(dir.join("data/src/lib.rs"), "pub struct App;\n").unwrap();
        std::fs::write(
            dir.join("data/Cargo.toml"),
            "[package]\nname = \"app-data\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("app.json"),
            r#"{"name":"App","icons":[{"src":"assets/a.png"}],"app":{"id":"com.example.app","name":"App"},"host":{"ios":{"minimumOS":"17.0","backgroundModes":["fetch"]}},"deploy":{"origin":"continuous","bundles":"continuous"}}"#,
        )
        .unwrap();
        dir
    }

    fn id(dir: &Path, platform: &str) -> String {
        let m = Manifest::read(dir).unwrap();
        compatibility_with_trust(
            dir,
            platform,
            "aarch64-apple-ios",
            &m,
            Some(""),
            "development",
        )
        .unwrap()
        .id
    }

    #[test]
    fn the_id_is_stable_and_moves_only_with_identity() {
        let dir = app("stable");
        let first = id(&dir, "ios");
        assert_eq!(first.len(), 32, "{first}");
        assert_eq!(first, id(&dir, "ios"), "stable across computations");
        // Policy is not identity (LLP 1030 D2).
        let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
        std::fs::write(
            dir.join("app.json"),
            manifest.replace("\"origin\":\"continuous\"", "\"origin\":\"manual\""),
        )
        .unwrap();
        assert_eq!(first, id(&dir, "ios"), "deploy.origin does not move it");
        // A platform is a cohort of its own.
        assert_ne!(first, id(&dir, "macos"), "platform moves it");
        // An icon a plan may select is identity (D3a).
        let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
        std::fs::write(
            dir.join("app.json"),
            manifest.replace(
                "[{\"src\":\"assets/a.png\"}]",
                "[{\"src\":\"assets/a.png\"},{\"src\":\"assets/holiday.png\"}]",
            ),
        )
        .unwrap();
        let with_icon = id(&dir, "ios");
        assert_ne!(first, with_icon, "an added icon moves it");
        // Which GPU artifact owns a surface is identity (LLP 1009 D6); an
        // app with one artifact carries no key at all.
        let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
        let m = Manifest::read(&dir).unwrap();
        let inputs = |m: &Manifest| {
            compatibility_with_trust(&dir, "ios", "aarch64-apple-ios", m, Some(""), "development")
                .unwrap()
                .inputs
        };
        assert!(inputs(&m).get("gpuModules").is_none());
        std::fs::write(
            dir.join("app.json"),
            manifest.replacen('{', r#"{"gpu":{"modules":{"world":["world"]}},"#, 1),
        )
        .unwrap();
        let modules = id(&dir, "ios");
        assert_ne!(with_icon, modules, "declaring a module moves it");
        let m = Manifest::read(&dir).unwrap();
        assert_eq!(
            inputs(&m)["gpuModules"],
            serde_json::json!({"world":["world"]})
        );
        std::fs::write(dir.join("app.json"), &manifest).unwrap();
        // A byte in the data crate is identity.
        std::fs::write(dir.join("data/src/lib.rs"), "pub struct App; // moved\n").unwrap();
        assert_ne!(with_icon, id(&dir, "ios"), "a data crate edit moves it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_inputs_name_every_field_and_an_undeclared_app_gets_defaults() {
        let dir = app("fields");
        let m = Manifest::read(&dir).unwrap();
        let c = compatibility_with_trust(
            &dir,
            "ios",
            "aarch64-apple-ios",
            &m,
            Some("net.fetch https://x/"),
            "development",
        )
        .unwrap();
        let i = &c.inputs;
        for key in [
            "kernelSchema",
            "formatVersion",
            "formatDigest",
            "abi",
            "executors",
            "rustMode",
            "rustAbi",
            "rustTarget",
            "rustModule",
            "typescriptPlacement",
            "rustPlacement",
            "dataCrate",
            "gpuSurfaces",
            "nativeModules",
            "icons",
            "capabilities",
            "keys",
            "trust",
            "grantCeiling",
            "platform",
            "arch",
            "minimumOS",
            "store",
            "app",
        ] {
            assert!(i.get(key).is_some(), "missing {key}: {i}");
        }
        assert_eq!(i["abi"]["c"], super::abi_version().unwrap());
        assert_eq!(i["rustMode"], "wasm");
        assert_eq!(i["rustAbi"], super::RUST_ABI);
        assert_eq!(i["rustTarget"], "wasm32-unknown-unknown");
        assert!(i["rustModule"].is_null());
        assert_eq!(i["executors"], serde_json::json!(["native", "wasmi"]));
        assert_eq!(i["arch"], "aarch64");
        assert_eq!(i["minimumOS"], "17.0");
        assert_eq!(
            i["capabilities"]["backgroundModes"],
            serde_json::json!(["fetch"])
        );
        assert_eq!(i["capabilities"]["urlSchemes"], serde_json::Value::Null);
        assert_eq!(i["store"]["L"], "A");
        assert_eq!(
            i["store"]["acceptedKinds"],
            serde_json::json!(["plan", "assets"])
        );
        assert_eq!(i["grantCeiling"], "net.fetch https://x/");
        assert!(c.to_json().starts_with("{\"id\":\""));
        // Policy rides beside the id (LLP 1030.000 D4): the channel, its
        // origin, and when a staged bundle applies — `next-launch` unsaid.
        assert!(
            c.to_json().contains(
                "\"delivery\":{\"activate\":\"next-launch\",\"channel\":\"prod\",\"origin\":null}"
            ),
            "{}",
            c.to_json()
        );
        // No manifest: the derived defaults, as scripts/app.mjs derives them.
        std::fs::remove_file(dir.join("app.json")).unwrap();
        let m = Manifest::read(&dir).unwrap();
        assert!(!m.declared);
        assert!(
            m.id.starts_with("com.exact.exact-compat-fields-"),
            "{}",
            m.id
        );
        assert!(m.name.starts_with("Exact-compat-fields-"), "{}", m.name);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn rust_capability_receipts_bind_executor_abi_target_and_module() {
        let dir = app("rust-capabilities");
        let mut manifest = Manifest::read(&dir).unwrap();
        let mut ids = Vec::new();
        for (mode, target, executor) in [
            ("native", Some("aarch64-apple-darwin"), Some("native-dylib")),
            ("wasm", Some("wasm32-unknown-unknown"), Some("wasmi")),
            ("off", None, None),
        ] {
            manifest.json["rust"] =
                serde_json::json!({"mode":mode,"module":{"package":"app-logic"}});
            let receipt = compatibility_with_trust(
                &dir,
                "macos",
                "aarch64-apple-darwin",
                &manifest,
                Some(""),
                "development",
            )
            .unwrap();
            assert_eq!(receipt.inputs["rustModule"], "app-logic");
            assert_eq!(receipt.inputs["rustTarget"], serde_json::json!(target));
            assert_eq!(
                receipt.inputs["rustAbi"],
                if mode == "off" {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(super::RUST_ABI)
                }
            );
            let expected = executor.map_or_else(
                || serde_json::json!(["native"]),
                |e| serde_json::json!(["native", e]),
            );
            assert_eq!(receipt.inputs["executors"], expected);
            assert!(
                !ids.contains(&receipt.id),
                "executor capability changes cohort identity"
            );
            ids.push(receipt.id);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn web_receipts_name_the_linked_composition_and_refuse_an_unlinked_store() {
        let dir = app("web-zero");
        let mut manifest = Manifest::read(&dir).unwrap();
        manifest.json["deploy"] = serde_json::json!({});
        manifest.json["app"]["origin"] = serde_json::json!("https://updates.example");
        std::fs::write(dir.join("app.ts"), "").unwrap();
        for setting in [None, Some(serde_json::json!("0"))] {
            if let Some(setting) = setting {
                manifest.json["deploy"]["store"] = serde_json::json!({"web":setting});
            }
            let plain =
                compatibility_with_trust(&dir, "web", "wasm32", &manifest, None, "production")
                    .unwrap();
            assert_eq!(plain.inputs["store"]["L"], "0");
            assert_eq!(
                plain.inputs["executors"],
                serde_json::json!(["browser", "browser-wasm", "native"])
            );
            assert_eq!(
                plain.inputs["store"]["acceptedKinds"],
                serde_json::json!([])
            );
            assert!(plain.inputs["keys"].is_null());
            assert!(plain.inputs["trust"].is_null());
            assert!(plain.inputs["abi"]["storeCodec"].is_null());
            assert_eq!(plain.origin, None);
            assert_eq!(plain.channel, "");
        }
        for setting in [
            serde_json::json!("A"),
            serde_json::json!(null),
            serde_json::json!(0),
        ] {
            manifest.json["deploy"]["store"]["web"] = setting;
            let error =
                compatibility_with_trust(&dir, "web", "wasm32", &manifest, None, "production")
                    .unwrap_err();
            assert!(error.contains("web host links no updater"), "{error}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn production_trust_requires_keys_and_has_a_distinct_cohort() {
        let dir = app("trust");
        let mut manifest = Manifest::read(&dir).unwrap();
        for keys in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!({})),
            Some(serde_json::json!([])),
            Some(serde_json::json!({"k":"invalid"})),
        ] {
            manifest.json["deploy"]["signing"] = serde_json::json!({});
            if let Some(keys) = keys {
                manifest.json["deploy"]["signing"]["keys"] = keys;
            }
            assert!(compatibility_with_trust(
                &dir,
                "linux",
                "aarch64",
                &manifest,
                None,
                "production"
            )
            .is_err());
        }
        manifest.json["deploy"]["signing"] = serde_json::json!({});
        let dev =
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "development")
                .unwrap();
        assert_eq!(dev.inputs["trust"], "development");
        assert!(
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "dev").is_err()
        );
        manifest.json["deploy"]["store"] = serde_json::json!({"linux":"0"});
        assert!(
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production")
                .is_ok()
        );
        manifest.json["app"]["origin"] = serde_json::json!("https://updates.example");
        let zero =
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production")
                .unwrap();
        assert!(zero.inputs["keys"].is_null());
        assert!(zero.inputs["trust"].is_null());
        assert!(zero.inputs["abi"]["storeCodec"].is_null());
        assert_eq!(zero.inputs["store"]["acceptedKinds"], serde_json::json!([]));
        assert!(zero.origin.is_none());
        manifest.json["deploy"]["signing"]["keys"] =
            serde_json::json!({"ignored":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="});
        let changed =
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "development")
                .unwrap();
        assert_eq!(
            zero.id, changed.id,
            "a missing updater has no key or trust epoch"
        );
        manifest.json["deploy"]["store"] = serde_json::json!({"linux":"A"});
        manifest.json["deploy"]["signing"]["keys"] =
            serde_json::json!({"k":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="});
        let production =
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production")
                .unwrap();
        let development =
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "development")
                .unwrap();
        assert_eq!(production.inputs["trust"], "production");
        assert_ne!(
            production.id, development.id,
            "unsigned development permission is its own trust epoch"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
