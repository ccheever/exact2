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

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The C ABI's header, whose `EXACT_ABI_VERSION` is one of the numbers.
const ABI_HEADER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../host/apple/include/exact.h"
));
/// The GPU module's C ABI (LLP 1009 D2): unnumbered in the module today.
pub const GPU_MODULE_ABI: u32 = 1;
/// The update store's record codec (LLP 1030 D1): the first.
pub const STORE_CODEC: u32 = 2;
/// The domain separator over the canonical inputs.
const DOMAIN: &str = "exact2 compatibility id v1\n";

/// The app manifest (`app.json`, LLP 1030 D2; 1030.000 D7) as the bake
/// reads it — the JSON, and the identity every platform derives from it.
/// An app without one gets the defaults `scripts/app.mjs` gives it.
#[derive(Debug, Clone)]
pub struct Manifest {
    /// The manifest's JSON (an object; empty when the app declares none).
    pub json: serde_json::Value,
    /// The app identity, reverse-DNS (`app.id`).
    pub id: String,
    /// The name people see (`app.name`).
    pub name: String,
    /// Whether the app has an `app.json` at all.
    pub declared: bool,
}

impl Manifest {
    /// `app.json` in `app_dir`, or the derived defaults: `com.exact.<name>`
    /// and the directory's name capitalized.
    pub fn read(app_dir: &Path) -> Result<Manifest, String> {
        let name = app_dir
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .or_else(|| {
                app_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "app".into());
        let display = {
            let mut c = name.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        };
        let path = app_dir.join("app.json");
        if !path.exists() {
            return Ok(Manifest {
                json: serde_json::json!({}),
                id: format!("com.exact.{name}"),
                name: display,
                declared: false,
            });
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let app = json.get("app").and_then(|a| a.as_object());
        let id = app
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{}: `app.id` is required", path.display()))?
            .to_string();
        let name = app
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{}: `app.name` is required", path.display()))?
            .to_string();
        Ok(Manifest {
            json,
            id,
            name,
            declared: true,
        })
    }

    /// `host.<platform>` as an object (empty when absent).
    fn host(&self, platform: &str) -> serde_json::Map<String, serde_json::Value> {
        self.json
            .get("host")
            .and_then(|h| h.get(platform))
            .and_then(|p| p.as_object())
            .cloned()
            .unwrap_or_default()
    }

    /// `deploy.signing.keys`: canonical base64 of 32 raw Ed25519 bytes.
    /// Missing keys remain absent; malformed configured keys are refused.
    fn keys(&self) -> Result<serde_json::Value, String> {
        let value = self.json.pointer("/deploy/signing/keys");
        let keys = match value {
            Some(serde_json::Value::Object(keys)) => keys,
            None | Some(serde_json::Value::Null) => return Ok(serde_json::Value::Null),
            _ => return Err("deploy.signing.keys must be an object".into()),
        };
        const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        for (id, value) in keys {
            let valid = value.as_str().is_some_and(|key| {
                key.len() == 44
                    && key.ends_with('=')
                    && key
                        .bytes()
                        .take(43)
                        .all(|b| ALPHABET.as_bytes().contains(&b))
                    && ALPHABET
                        .find(key.as_bytes()[42] as char)
                        .is_some_and(|n| n % 4 == 0)
            });
            if id.is_empty() || !valid {
                return Err(format!(
                    "deploy.signing.keys[{id:?}] must name a base64 Ed25519 public key (32 bytes)"
                ));
            }
        }
        Ok(serde_json::Value::Object(keys.clone()))
    }

    /// The channel this build bakes in (`deploy.channel`; else the only key
    /// of `deploy.channels`; else `prod`) and its origin (`deploy.channels.
    /// <channel>`, else `app.origin`): where the binary's update store checks
    /// (LLP 1030.000 D4). A stream is `(channel, compatibility id)`; the
    /// channel is not part of the id.
    pub fn channel(&self) -> (String, Option<String>) {
        let deploy = self.json.get("deploy");
        let channels = deploy
            .and_then(|d| d.get("channels"))
            .and_then(|c| c.as_object());
        let channel = deploy
            .and_then(|d| d.get("channel"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| match channels {
                Some(c) if c.len() == 1 => c.keys().next().cloned(),
                _ => None,
            })
            .unwrap_or_else(|| "prod".to_string());
        let origin = channels
            .and_then(|c| c.get(&channel))
            .and_then(|v| v.as_str())
            .or_else(|| {
                self.json
                    .get("app")
                    .and_then(|a| a.get("origin"))
                    .and_then(|v| v.as_str())
            })
            .map(str::to_string);
        (channel, origin)
    }

    /// `deploy.activate` (LLP 1030.000 D4): when a staged bundle applies —
    /// `next-launch` (the default; `deliveryActivate` applies it sooner) or
    /// `app-decides` (only `deliveryActivate` does). Policy, not identity:
    /// it rides beside the id, never in it.
    pub fn activate(&self) -> String {
        self.json
            .get("deploy")
            .and_then(|d| d.get("activate"))
            .and_then(|v| v.as_str())
            .unwrap_or("next-launch")
            .to_string()
    }

    /// `deploy.store.<platform>`: `"A"` (an update store) or `"0"` (none);
    /// Native defaults to `"A"`; the web host links no updater (LLP 1030 D4).
    fn store(&self, platform: &str) -> String {
        self.json
            .get("deploy")
            .and_then(|d| d.get("store"))
            .and_then(|s| s.get(platform))
            .and_then(|v| v.as_str())
            .unwrap_or(if platform == "web" { "0" } else { "A" })
            .to_string()
    }
}

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
        s.push_str("}\n");
        s
    }
}

/// The compatibility id of the app at `app_dir` built for `platform`
/// (`ios`, `macos`, `linux`, `web`) and `target` (the triple), with the
/// data crate's grants when the caller has a `DataSource` to ask (the bake
/// does; `contract compat` on the command line does not).
pub fn compatibility_id(
    app_dir: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    grants: Option<&str>,
) -> Result<Compat, String> {
    // Cargo must rebake even when only the explicit trust selection changes.
    // External apps call this same entrypoint from their own build scripts.
    if std::env::var_os("OUT_DIR").is_some() {
        println!("cargo:rerun-if-env-changed=EXACT_UPDATE_TRUST");
    }
    let trust = match std::env::var("EXACT_UPDATE_TRUST") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => "production".into(),
        Err(_) => return Err("EXACT_UPDATE_TRUST is not UTF-8".into()),
    };
    let mut compat = compatibility_with_trust(app_dir, platform, target, manifest, grants, &trust)?;
    if let Some(out) = std::env::var_os("OUT_DIR") {
        crate::receipt::emit(
            &mut compat,
            app_dir,
            platform,
            target,
            manifest,
            Path::new(&out),
        )?;
    }
    Ok(compat)
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
    let executors = executors(app_dir, platform);
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
    let inputs = json!({
        "kernelSchema": format!("{:016x}", exact_kernel::SCHEMA_DIGEST),
        "formatVersion": exact_plan::FORMAT_VERSION,
        "formatDigest": format!("{:016x}", exact_plan::FORMAT_DIGEST),
        "abi": { "c": abi_version()?, "gpuModule": GPU_MODULE_ABI, "storeCodec": if binary_only { Value::Null } else { json!(STORE_CODEC) } },
        "executors": executors,
        "dataCrate": data_crate(app_dir)?,
        // Each shader's reflected interface digest (LLP 1030 D8) — entry
        // points, bindings, layouts, inputs, outputs, overrides, never the
        // text — so a colour edit does not move the id and a binding edit
        // does. Shaders are assets (1030.000 stage 1); the surface's Rust
        // binds this interface.
        "gpuSurfaces": gpu_surfaces(app_dir)?,
        "nativeModules": Value::Null,
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
        "platform": platform,
        "arch": target.split('-').next().unwrap_or(target),
        "minimumOS": host.get("minimumOS").and_then(|v| v.as_str()).map_or(Value::Null, |s| Value::String(s.to_string())),
        "store": { "L": store, "acceptedKinds": if binary_only { vec![] } else { kinds } },
        "app": manifest.id,
    });
    let mut canon = String::new();
    canonical(&inputs, &mut canon);
    let mut h = Sha256::new();
    h.update(DOMAIN.as_bytes());
    h.update(canon.as_bytes());
    let digest = h.finalize();
    let id = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    Ok(Compat {
        id,
        inputs,
        channel,
        origin,
        activate: manifest.activate(),
        target: target.into(),
        embedded: serde_json::Value::Null,
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

/// The surfaces' shaders under `gpu/shaders`, by stem, each with its
/// file digest; `null` when the app has no GPU crate.
fn gpu_surfaces(app_dir: &Path) -> Result<serde_json::Value, String> {
    let dir = app_dir.join("gpu/shaders");
    if !dir.is_dir() {
        return Ok(serde_json::Value::Null);
    }
    let mut shaders = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.extension().is_some_and(|x| x == "wgsl") {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let digest = exact_gpu_reflect::interface_digest(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            shaders.push((stem, format!("{digest:016x}")));
        }
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
fn canonical(v: &serde_json::Value, out: &mut String) {
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
        assert_eq!(i["executors"], serde_json::json!(["native"]));
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
                serde_json::json!(["browser", "native"])
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
