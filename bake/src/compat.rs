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
/// The separately linked Rust data-source request/outcome wire ABI. 4: a
/// long poll's result tag 10 (LLP 1016.000 D8; theme2's Q-RUSTABI ruling: the
/// plan can't say which Rust sources poll, so every Rust app's id moves once,
/// and an older binary is never offered a module that may send one).
pub(crate) const RUST_ABI: u32 = 4;
/// The data-module runtime a TypeScript bundle may rely on in the binary's
/// own prelude (`js/src/prelude.js`, compiled into the host): moved when the
/// prelude learns something a bundle cannot do without and an older prelude
/// would silently ignore. 1: `fetch`'s `exactBodyFrom` (LLP 1108 D6 R2),
/// which an older prelude would send as an empty body. 2: `fetch`'s
/// `redirect`, which an older prelude would ignore and follow. 3: `fetch`'s
/// `exactLongPoll` (LLP 1016.000 D8), which an older prelude would ignore and
/// open as an event stream. 4: `fetch`'s `exactBodyTo` (LLP 1108 D6 R2),
/// which an older prelude would ignore, reading the body into memory and
/// writing no file. 5: an aborted `fetch`'s request cancelled on the wire and
/// its answer resumed at the abort (docs/reference.md), which an older
/// prelude would leave running, its reply (a 429 included) dropped; and,
/// sharing it (no binary had shipped 5), an aborted kept write's rejection in
/// a module turn (LLP 1146 D2), which an older prelude would run in the
/// aborting turn, where a refused commit drops its continuation's writes; and,
/// sharing it as well, `fetch`'s `exactUploadProgress` (LLP 1016.000 D10),
/// which an older prelude would ignore, giving the mapper no progress and the
/// reply as a byte stream's messages. 6: `storage.fs`'s `lstat`, `readlink`,
/// `access`, `chmod`, `symlink`, `digest` and `flock`, and `stat`'s `mode`
/// and `isSymbolicLink` (LLP 1133.007 D6a), which an older prelude lacks: a
/// call throws a plain `TypeError` rather than `NotSupportedError`, and the
/// fields read `undefined`. The operations work on `app:/` paths under an
/// existing `fs` grant, so the grant ceiling does not keep them from an
/// older binary. A module that reads only `stat().mode` names nothing below
/// and is not detected. 7: Fetch's `Response`: data code's own (its
/// constructor, `Response.json`, `Response.redirect`), a reply's `clone()`,
/// `statusText` and `bodyUsed`, and a body read once, which an older prelude
/// would make with no status and no body, refuse, or answer `undefined`.
pub(crate) const TYPESCRIPT_RUNTIME: u32 = 7;

/// What each runtime past the first added that a bundle names to use (see
/// [`TYPESCRIPT_RUNTIME`]): a `fetch` option an older prelude would ignore,
/// or, for 5's abort changes, what makes an abort signal. `mode` and
/// `access` are left out of 6's: they would match far more than D6a. 7's are
/// the `Response` itself and what it newly has, however a bundle reaches them
/// (`globalThis.Response`, a `clone` taken apart and called through `.call`);
/// `structuredClone` names none of them.
const RUNTIME_OPTIONS: [(u32, &str); 18] = [
    (7, "Response"),
    (7, "clone"),
    (7, "statusText"),
    (7, "bodyUsed"),
    (6, "isSymbolicLink"),
    (6, "symlink"),
    (6, "readlink"),
    (6, "lstat"),
    (6, "chmod"),
    (6, "digest"),
    (6, "flock"),
    (5, "exactUploadProgress"),
    (5, "AbortController"),
    (5, "AbortSignal"),
    (4, "exactBodyTo"),
    (3, "exactLongPoll"),
    (2, "redirect"),
    (1, "exactBodyFrom"),
];

/// The lowest runtime a TypeScript `bundle` needs: the newest whose option
/// it names anywhere (a property name survives bundling), else none. A push
/// records it, so a module that uses nothing newer still goes to a binary
/// whose prelude is older (LLP 1016.000 D10). A name in a string or a
/// comment counts too: such a module asks for more than it needs, never less.
pub fn typescript_runtime_of(bundle: &[u8]) -> u32 {
    let text = String::from_utf8_lossy(bundle);
    RUNTIME_OPTIONS
        .iter()
        .find(|(_, option)| text.contains(option))
        .map_or(0, |(runtime, _)| *runtime)
}
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
    /// Embedded bundle provenance and its complete static roster, and
    /// `trust`: the build's resolved trust, which every host's production
    /// policy reads. Beside the id, not in it: an updater-free (`L = 0`)
    /// cohort has no trust epoch, so `inputs.trust` is null there and a
    /// production and a development build share the id (LLP 1030 D4).
    pub embedded: serde_json::Value,
    /// What the app can reach and each host's spelling of it (LLP 1069.008
    /// D7): beside the id, not in it; `null` when the grants are unknown.
    pub reach: serde_json::Value,
    /// The binary's painter choice, outside the compatibility hash (LLP 1030 D3a).
    pub painter: Option<&'static str>,
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
        if let Some(painter) = self.painter {
            s.push_str(",\"painter\":");
            canonical(&serde_json::json!(painter), &mut s);
        }
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
/// child's exact grant declaration independently. Grants that do not parse
/// refuse the bake, whichever language declared them.
pub fn compatibility_id_sources(
    app_dir: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    grants: Option<&str>,
    rust_grants: Option<&str>,
) -> Result<Compat, String> {
    compatibility_id_sources_with_entries(
        app_dir,
        platform,
        target,
        manifest,
        grants,
        rust_grants,
        None,
    )
}

/// Reuse entry plans already compiled for source declarations and refresh checks.
/// With `None`, compatibility generation compiles the manifest's entries itself.
pub fn compatibility_id_sources_with_entries(
    app_dir: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    grants: Option<&str>,
    rust_grants: Option<&str>,
    entries: Option<&std::collections::BTreeMap<String, contract::snapshot::Entry>>,
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
    crate::reach::parsed(app_dir, grants, rust_grants, trust == "production")?;
    // @ref LLP 1100 D1 — a colour this platform can't show is refused here.
    // The app's plan, compiled once here: its colours checked, and (stage
    // 14 C2) its icons' fill rules read below.
    let plan = crate::colors::compiled(app_dir)?;
    if let Some(plan) = &plan {
        crate::colors::check_plan(app_dir, plan, platform)?;
    }
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
    crate::reach::derive(app_dir, platform, plan.as_ref(), &mut compat)?;
    // The plans that ship, each resolved once (the receipt writes the entry
    // plans): an even-odd icon in any moves the id (stage 14 C2,
    // icon_rules.rs), and one that binds `atomFaces` names its record's
    // fields (atom_faces.rs).
    let mut owned = None;
    let entries = crate::icon_rules::entries(app_dir, entries, &mut owned)?;
    let out = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from);
    let other = crate::icon_rules::shipped(app_dir, plan.as_ref(), out.as_deref())?;
    let ships = other.as_ref().or(plan.as_ref());
    crate::icon_rules::apply(&mut compat, ships, entries)?;
    crate::atom_faces::apply(&mut compat, ships, entries);
    if let Some(plan) = &plan {
        faces(&mut compat, plan, platform, target);
    }
    if let Some(out) = std::env::var_os("OUT_DIR") {
        // What an Apple archive links is part of its cohort (LLP 1047.001
        // D2), named before anything binds the id.
        if matches!(platform, "ios" | "macos") {
            crate::link::name_into(&mut compat, manifest, Path::new(&out))?;
        }
        crate::receipt::emit(
            &mut compat,
            &trust,
            app_dir,
            (platform, target),
            manifest,
            Path::new(&out),
            entries,
        )?;
    }
    Ok(compat)
}

/// The faced tools this platform's binary compiles a door for (LLP 1133.023
/// D8): each one's signature and face join the id, so a plan that changes
/// one is a binary change. tvOS (an `ios` bake for a tvOS target) and the
/// web compile none.
fn faces(compat: &mut Compat, plan: &exact_plan::Plan, platform: &str, target: &str) {
    let door = match platform {
        _ if target.contains("tvos") => return,
        "ios" | "macos" => "apple",
        "android" => "android",
        _ => return,
    };
    let faced: Vec<serde_json::Value> = (0..plan.tools.len())
        .filter_map(|i| {
            let face: serde_json::Value = serde_json::from_str(plan.str(plan.tools[i].face)).ok()?;
            let doors = face["doors"].as_array()?;
            doors.iter().any(|d| d == door).then(|| serde_json::json!({ "signature": contract::faces::tool_signature(plan, i), "face": face }))
        })
        .collect();
    if !faced.is_empty() {
        compat.inputs["faces"] = serde_json::json!(faced);
        compat.id = compatibility_digest(&compat.inputs);
    }
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

fn commands_digest() -> String {
    let mut h = Sha256::new();
    for name in contract_syntax::HOST_COMMANDS {
        let shape = match *name {
            "share" => "3",
            "stopSounds" => "1",
            "playSounds" => "1:4|5|6",
            "playSound" => "6",
            "sendMIDI" => "3",
            "scrollIntoView" => "4|7",
            "saveFile" => "3|4",
            _ => "positional",
        };
        h.update(format!("{name}:{shape}\n"));
    }
    h.finalize()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
        manifest.channel()?
    };
    if trust == "production"
        && (store != "0" || origin.is_some())
        && !binary_only
        && keys.as_object().is_none_or(|keys| keys.is_empty())
    {
        return Err("production updater requires deploy.signing.keys with at least one verification key; use EXACT_UPDATE_TRUST=development only for a development artifact".into());
    }
    let host = manifest.host(platform);
    let rust_module = manifest
        .json
        .pointer("/rust/module/package")
        .and_then(Value::as_str);
    // An executor runs a replacement of the module the manifest names; an
    // app that names none has nothing to replace, so it links none
    // (LLP 1047.001 D7, as `web_rust_mode` already does on the web). The
    // policy is still validated.
    let rust_mode = match manifest.rust_mode(platform, trust == "development")? {
        _ if rust_module.is_none() => "off",
        mode => mode,
    };
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
        // @ref LLP 1114.001 D1 — an OTA plan with a changed fixed command
        // shape must not run in a binary that reads its old arguments.
        "commands": commands_digest(),
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
        // A binary that replaces its Rust module binds the module's ABI,
        // target and grants above, never the source it was first built
        // from: an edit there is an update, not a new cohort (LLP 1029.000 §7).
        "dataCrate": if rust_mode == "off" { data_crate(app_dir)? } else { Value::Null },
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
            tags => json!({ "appleAbi": 3, "webAbi": 1, "tags": tags }),
        },
        // @ref LLP 1075.003.000.001 §4.3 — the hatch words this platform's
        // module is built to handle, from app.json: the cohort's capability.
        "hatches": match contract::native::hatches_on(manifest, platform)? {
            words if words.is_empty() => Value::Null,
            words => json!(words),
        },
        "icons": icons,
        "capabilities": {
            "backgroundModes": list("backgroundModes"),
            "urlSchemes": list("urlSchemes"),
            "associatedDomains": host.get("associatedDomains").and_then(|v| v.as_bool()).map_or(Value::Null, Value::Bool),
            // @ref LLP 1096 D8 — the Apple audio session, baked into the binary.
            "audioSession": manifest.json.get("audio_session").cloned().unwrap_or(Value::Null),
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
    if matches!(platform, "macos" | "ios") {
        if let Some(handlers) = manifest.json.get("protocol_handlers") {
            if !handlers.as_array().is_some_and(Vec::is_empty) {
                inputs["protocolHandlers"] = handlers.clone();
            }
        }
    }
    // The icon's menu each native binary carries (LLP 1133.023 D3): iOS's
    // Info.plist, Android's `shortcuts.xml`, the Mac's Dock menu, Windows'
    // jump list and Linux's desktop entry read it from the build.
    if platform != "web" {
        if let Some(shortcuts) =
            (manifest.json.get("shortcuts")).filter(|s| s.as_array().is_some_and(|s| !s.is_empty()))
        {
            inputs["shortcuts"] = shortcuts.clone();
        }
    }
    let widgets = manifest.widgets()?;
    if !widgets.is_empty() {
        inputs["widgets"] = json!(widgets
            .iter()
            .map(|w| json!({"kind":w.kind,"families":w.families,"watchFamilies":w.watch_families,"requiresSecrets":w.requires_secrets,"title":w.title,"description":w.description}))
            .collect::<Vec<_>>());
    }
    let activities = manifest.live_activities()?;
    if !activities.is_empty() {
        inputs["liveActivities"] = json!(activities.iter().map(|a| json!({
            "kind":a.kind, "title":a.title, "description":a.description, "push":a.push, "frequent":a.frequent
        })).collect::<Vec<_>>());
    }
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
    // A native TypeScript module runs on the binary's prelude: what it may
    // ask of it is identity. Only apps with one carry the key, so no other
    // app's id moves.
    if hermes {
        inputs["typescriptRuntime"] = json!(TYPESCRIPT_RUNTIME);
    }
    let id = compatibility_digest(&inputs);
    Ok(Compat {
        id,
        inputs,
        channel,
        origin,
        activate: manifest.activate(),
        target: target.into(),
        embedded: json!({ "trust": trust }),
        reach: serde_json::Value::Null,
        // Android shells also use the Linux bake; the target names the host.
        painter: manifest.painter(if target.contains("android") {
            "android"
        } else {
            platform
        })?,
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
    let platform_host = app_dir.join(platform).join("src/lib.rs");
    let host = if platform_host.is_file() {
        platform_host
    } else {
        app_dir.join("apple/src/lib.rs")
    };
    if platform != "web" && std::fs::read_to_string(host).is_ok_and(|s| s.contains("Swappable")) {
        out.push("wasmtime".into());
    }
    out.sort();
    out
}

/// The data crate's source-input digest (LLP 1030 D3a): its tree, the
/// workspace lockfile, the toolchain — never its compiled output, which a
/// `wasm32` build can keep while the native build moved. A `data/` without
/// a `Cargo.toml` is no crate: TypeScript modules there are the bundle's,
/// and an edit to them is an update, not a new cohort.
fn data_crate(app_dir: &Path) -> Result<serde_json::Value, String> {
    let dir = app_dir.join("data");
    if !dir.join("Cargo.toml").is_file() {
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
    let toolchain = toolchain().map_or(serde_json::Value::Null, serde_json::Value::String);
    Ok(
        serde_json::json!({ "tree": tree, "files": files.len(), "lockfile": lockfile, "toolchain": toolchain }),
    )
}

/// The toolchain's version line (`$RUSTC --version`), asked once per process and per
/// `RUSTC`: a process computes many ids, and a compiler does not change under it.
fn toolchain() -> Option<String> {
    static ASKED: std::sync::Mutex<Vec<(String, Option<String>)>> =
        std::sync::Mutex::new(Vec::new());
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let mut asked = ASKED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, version)) = asked.iter().find(|(name, _)| *name == rustc) {
        return version.clone();
    }
    let version = std::process::Command::new(&rustc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    asked.push((rustc, version.clone()));
    version
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
        .map_err(|e| format!("shader inventory: {}", crate::bun_error(e)))?;
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
#[path = "compat_tests.rs"]
mod tests;
