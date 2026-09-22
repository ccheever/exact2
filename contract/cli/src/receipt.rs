//! The binary bake's compatibility and embedded receipt (LLP 1030 D3/D3a).
//! The Cargo build script knows its actual target and data-source grants;
//! Consumers read these bytes instead of reconstructing those facts.

use crate::compat::{Compat, Manifest, RUST_ABI};
use exact_update::{Baked, Envelope};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Complete the compatibility document from the plan already baked by the
/// app's build script. Production sequence provenance is a signed publisher
/// receipt; an unreceipted stream requires an explicit genesis bake.
pub(crate) fn emit(
    compat: &mut Compat,
    app: &Path,
    platform: &str,
    target: &str,
    manifest: &Manifest,
    out: &Path,
) -> Result<(), String> {
    for name in [
        "EXACT_UPDATE_RECEIPT",
        "EXACT_UPDATE_GENESIS",
        "EXACT_BAKE_OUTPUT",
        "EXACT_BAKE_ANALYSIS",
        "EXACT_RUST_BUNDLE",
        "EXACT_ASSET_ROOTS",
        "EXACT_GPU_PRODUCT",
        "EXACT_GPU_DEVELOPMENT",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let mut plan_bytes = std::fs::read(out.join("app.plan")).map_err(|e| {
        format!("the app bake must write OUT_DIR/app.plan before compatibility_id: {e}")
    })?;
    let mut plan = exact_plan::Plan::decode(&plan_bytes)
        .map_err(|e| format!("the baked plan is invalid: {e:?}"))?;
    if plan.app_id.is_empty() {
        plan.app_id.clone_from(&manifest.id);
        plan_bytes = plan.encode();
        std::fs::write(out.join("app.plan"), &plan_bytes).map_err(|e| e.to_string())?;
    } else if plan.app_id != manifest.id {
        return Err(format!(
            "the baked plan names app {}, but app.json names {}",
            plan.app_id, manifest.id
        ));
    }
    let plan_card = json!({"sha256": hash(&plan_bytes), "bytes": plan_bytes.len()});
    let mut assets = asset_cards(app)?;
    let rust_assets = if let Some(directory) = std::env::var_os("EXACT_RUST_BUNDLE") {
        stage_rust_bundle(
            Path::new(&directory),
            out,
            &plan_bytes,
            &compat.inputs,
            target,
        )?
    } else {
        // OUT_DIR survives incremental builds; a prior supplied pair must not
        // become part of a later build that deliberately supplies none.
        let stale = out.join("rust");
        if stale.exists() {
            std::fs::remove_dir_all(stale).map_err(|e| e.to_string())?;
        }
        Vec::new()
    };
    assets.extend(rust_assets);
    assets.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let graph = artifact_graph(&plan, &compat.inputs, &assets, target)?;
    std::fs::write(out.join("artifacts.json"), graph.to_string()).map_err(|e| e.to_string())?;
    compat.target = target.into();
    compat.embedded =
        json!({"seq":0,"plan":plan_card,"assets":assets,"entryDigest":null,"genesis":true});
    // The GPU product is built (and on Apple, signed) before the host. Its
    // exact bytes belong to this app/cohort; a sibling filename is not identity.
    if platform != "web" {
        if let Some(name) = std::env::var("EXACT_GPU_DEVELOPMENT")
            .ok()
            .filter(|_| std::env::var("EXACT_UPDATE_TRUST").as_deref() == Ok("development"))
        {
            // gpu-dev authenticates the independently completed module receipt
            // at load. Production always embeds the exact GPU digest below.
            compat.embedded["gpu"] = json!({"app":manifest.id,"cohort":compat.id,
                "name":name,"trust":"development","receipt":true});
        } else if let Some(path) = std::env::var_os("EXACT_GPU_PRODUCT") {
            let path = PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            let bytes =
                std::fs::read(&path).map_err(|e| format!("GPU product {}: {e}", path.display()))?;
            compat.embedded["gpu"] = json!({
                "name":path.file_name().and_then(|n| n.to_str()).ok_or("GPU product filename is not UTF-8")?,
                "sha256":hash(&bytes), "app":manifest.id, "cohort":compat.id,
                "trust":std::env::var("EXACT_UPDATE_TRUST").unwrap_or("production".into())
            });
        }
    }
    // A binary without an updater has no stream or rollback floor.
    // Native L=A artifacts still require authenticated sequence provenance.
    let development = compat.inputs["trust"] == "development";
    if development || compat.inputs["store"]["L"] == "0" {
        // Explicit development artifacts and updater-free hosts have a
        // local genesis; neither inherits a production stream receipt.
    } else if std::env::var("EXACT_BAKE_ANALYSIS").as_deref() == Ok("1") {
        // Classification compiles the real target before publication has a
        // sequence. This artifact cannot launch: Baked requires a u64 seq.
        // Only a later authenticated receipt bake may produce a release.
        compat.embedded["analysis"] = json!(true);
        compat.embedded["seq"] = Value::Null;
        compat.embedded["genesis"] = json!(false);
    } else if let Some(path) = std::env::var_os("EXACT_UPDATE_RECEIPT") {
        let path = PathBuf::from(path);
        println!("cargo:rerun-if-changed={}", path.display());
        let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        apply_release(compat, &raw, &plan_bytes)?;
    } else if compat.inputs["store"]["L"] != "0"
        && std::env::var("EXACT_UPDATE_GENESIS").as_deref() != Ok("1")
    {
        return Err("production embedding requires EXACT_UPDATE_RECEIPT naming its authenticated publisher receipt; a new stream must explicitly set EXACT_UPDATE_GENESIS=1 (developer builds use EXACT_UPDATE_TRUST=development)".into());
    }
    if let Some(destination) = std::env::var_os("EXACT_BAKE_OUTPUT") {
        let destination = PathBuf::from(destination);
        std::fs::create_dir_all(&destination)
            .map_err(|e| format!("{}: {e}", destination.display()))?;
        let stem = format!("{platform}-{target}");
        std::fs::write(destination.join(format!("{stem}.json")), compat.to_json())
            .map_err(|e| e.to_string())?;
        std::fs::write(destination.join(format!("{stem}.plan")), &plan_bytes)
            .map_err(|e| e.to_string())?;
        std::fs::write(
            destination.join(format!("{stem}.artifacts.json")),
            graph.to_string(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// Both Rust bakes and the copying helper use the same owned-directory walk.
// Pure Contract compilation remains portable; asset bakes run on macOS/Linux.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn asset_cards(app: &std::path::Path) -> Result<Vec<Value>, String> {
    use exact_filesystem::Directory;
    use std::path::{Component, PathBuf};
    let refusal = |e: std::io::Error| format!("bake asset gate: {e}");
    // Match path.resolve without following any user-controlled link.
    let mut absolute = PathBuf::new();
    for part in std::env::current_dir()
        .map_err(refusal)?
        .join(app)
        .components()
    {
        match part {
            Component::ParentDir => {
                absolute.pop();
            }
            Component::CurDir => {}
            part => absolute.push(part.as_os_str()),
        }
    }
    let mut cards = Vec::new();
    for (source, prefix) in [
        ("assets", "assets/"),
        ("deck", "deck/"),
        ("gpu/shaders", "shaders/"),
    ] {
        let path = absolute.join(source);
        let root = match Directory::root(
            path.to_str().ok_or("bake asset gate: non-UTF8 path")?,
            false,
        ) {
            Ok(root) => root,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(refusal(e)),
        };
        println!("cargo:rerun-if-changed={}", path.display());
        root.visit_files(prefix, &mut |name, parent, leaf| {
            let bytes = parent.read(leaf)?;
            cards.push(serde_json::json!({"name":name,"sha256":hash(&bytes),"bytes":bytes.len()}));
            Ok(())
        })
        .map_err(refusal)?;
    }
    cards.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(cards)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn asset_cards(_: &Path) -> Result<Vec<Value>, String> {
    Err("bake asset gate requires a macOS or Linux build host".into())
}

// Supply bytes, never just signed names. A binary's entry-zero asset roster is
// used by the updater to decide Current, so claiming absent Rust files would
// prevent their download and leave the embedded implementation answering them.
fn stage_rust_bundle(
    directory: &Path,
    out: &Path,
    plan: &[u8],
    inputs: &Value,
    target: &str,
) -> Result<Vec<Value>, String> {
    let mode = inputs["rustMode"]
        .as_str()
        .ok_or("Rust bundle requires a baked executor policy")?;
    let (executor, module_target, filename) = match mode {
        "wasm" | "browser" => ("wasm", "wasm32-unknown-unknown", "app.module.wasm"),
        "tiered" if !target.contains("-apple-ios") && !target.starts_with("wasm") => {
            ("tiered", target, "app.module.bin")
        }
        "native" if !target.contains("-apple-ios") && !target.starts_with("wasm") => {
            let name = if target.contains("-apple-darwin") {
                "app.module.dylib"
            } else if target.contains("-windows-") {
                "app.module.dll"
            } else {
                "app.module.so"
            };
            ("native", target, name)
        }
        _ => return Err("supplied Rust bundle is disabled by this host's executor policy".into()),
    };
    if !std::fs::symlink_metadata(directory)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_dir()
    {
        return Err("Rust bundle must be an ordinary directory, not a symlink".into());
    }
    let receipt_path = directory.join("app.module.json");
    let module_path = directory.join(filename);
    for path in [&receipt_path, &module_path] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let receipt = read_rust_artifact(&receipt_path, 1 << 20)?;
    let module = read_rust_artifact(&module_path, 32 << 20)?;
    let meta: Value =
        serde_json::from_slice(&receipt).map_err(|e| format!("Rust bundle receipt: {e}"))?;
    if meta["version"].as_u64() != Some(1)
        || meta["kind"] != "rust"
        || meta["abi"].as_u64() != Some(RUST_ABI.into())
    {
        return Err("Rust bundle has an unsupported receipt or ABI".into());
    }
    let expected_app = inputs["app"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Rust bundle needs an admitted app identity")?;
    let expected_grants = inputs["rustGrants"]
        .as_str()
        .ok_or("Rust bundle needs its baked source grants")?;
    let ceiling = inputs["grantCeiling"]
        .as_str()
        .ok_or("Rust bundle needs the baked grant ceiling")?;
    let admitted: Vec<_> = ceiling
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if expected_grants
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .any(|line| !admitted.contains(&line))
    {
        return Err("Rust source grants exceed the app ceiling".into());
    }
    if meta["appId"].as_str() != Some(expected_app)
        || meta["grants"].as_str().map(str::trim) != Some(expected_grants.trim())
        || meta["executor"].as_str() != Some(executor)
        || meta["target"].as_str() != Some(module_target)
    {
        return Err("Rust bundle changes the baked app, grants, executor or target".into());
    }
    for (field, name, bytes) in [
        ("plan", "app.plan", plan),
        ("module", filename, module.as_slice()),
    ] {
        let card = &meta[field];
        if card["file"].as_str() != Some(name)
            || card["bytes"].as_u64() != Some(bytes.len() as u64)
            || card["sha256"].as_str() != Some(hash(bytes).as_str())
        {
            return Err(format!(
                "Rust bundle {field} does not match its exact baked bytes"
            ));
        }
    }
    if executor == "wasm" && !module.starts_with(b"\0asm\x01\0\0\0") {
        return Err("Rust bundle module is not wasm version 1".into());
    }
    let destination = out.join("rust");
    if destination.exists() {
        std::fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    let mut cards = Vec::new();
    for (name, bytes) in [
        ("app.module.json", receipt.as_slice()),
        (filename, module.as_slice()),
    ] {
        std::fs::write(destination.join(name), bytes).map_err(|e| e.to_string())?;
        cards.push(json!({"name":format!("rust/{name}"),"sha256":hash(bytes),"bytes":bytes.len()}));
    }
    Ok(cards)
}

fn read_rust_artifact(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let metadata =
        std::fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.len() > limit as u64 {
        return Err(format!(
            "{} must be an ordinary bounded Rust artifact",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Rust bundle artifact exceeds its byte limit".into());
    }
    Ok(bytes)
}

// Shapes use semantic names and field order, never plan-local row numbers.
// The same source signature survives unrelated declarations being inserted.
fn shape(plan: &exact_plan::Plan, ty: exact_plan::TypesId, depth: usize) -> Result<Value, String> {
    use exact_plan::TypeKind;
    if depth > 128 {
        return Err("artifact source shape exceeds 128 levels".into());
    }
    let row = plan.type_(ty);
    Ok(match row.kind {
        TypeKind::Option | TypeKind::List => {
            json!({"kind":format!("{:?}", row.kind),"item":shape(plan, row.elem.ok_or("container type has no item")?, depth + 1)?})
        }
        TypeKind::Record => {
            let fields = row.fields.iter().map(|id| {
                let field = plan.field(id);
                Ok(json!({"name":plan.str(field.name),"type":shape(plan, field.ty, depth + 1)?}))
            }).collect::<Result<Vec<Value>, String>>()?;
            json!({"kind":"Record","fields":fields})
        }
        _ => json!(format!("{:?}", row.kind)),
    })
}

fn artifact_graph(
    plan: &exact_plan::Plan,
    inputs: &Value,
    assets: &[Value],
    target: &str,
) -> Result<Value, String> {
    let mut sources = serde_json::Map::new();
    for row in &plan.sources {
        let params = row
            .params
            .iter()
            .map(|id| shape(plan, plan.source_param(id).ty, 0))
            .collect::<Result<Vec<_>, _>>()?;
        sources.insert(
            plan.str(row.name).into(),
            json!({"params":params,"result":shape(plan, row.ty, 0)?}),
        );
    }
    // Every surface row includes deferred templates and branches. This is
    // prior plan demand, not a claim to enumerate the native module's exports.
    let mut calls = serde_json::Map::new();
    for row in &plan.surfaces {
        let name = plan.str(row.name);
        let arities = calls.entry(name).or_insert_with(|| json!([]));
        let arity = json!(row.args.len);
        let values = arities
            .as_array_mut()
            .ok_or("surface arities are not an array")?;
        if !values.contains(&arity) {
            values.push(arity);
            values.sort_by_key(|v| v.as_u64());
        }
    }
    let mut requires = serde_json::Map::new();
    requires.insert("surfaceCalls".into(), Value::Object(calls.clone()));
    for key in ["app", "kernelSchema", "formatVersion", "formatDigest"] {
        requires.insert(key.into(), inputs[key].clone());
    }
    // The plan's source table covers resource calls and sends, including
    // branches not mounted during bake. Installed cohorts retain their own
    // implementations of matching names/shapes (1030 D3's stated caveat).
    requires.insert("sources".into(), Value::Object(sources.clone()));
    if sources
        .keys()
        .any(|name| !exact_plan::runner_owned_source(name))
    {
        requires.insert("executors".into(), inputs["executors"].clone());
        requires.insert("grantCeiling".into(), inputs["grantCeiling"].clone());
    }
    let rust = assets
        .iter()
        .any(|asset| asset["name"] == "rust/app.module.json");
    let rust_requirements = if rust {
        let mode = inputs["rustMode"]
            .as_str()
            .ok_or("Rust artifact lacks executor policy")?;
        let target = if mode == "wasm" || mode == "browser" {
            "wasm32-unknown-unknown".to_string()
        } else {
            target.to_string()
        };
        json!({"rustMode":mode,"rustAbi":RUST_ABI,"rustTarget":target,"grantCeiling":inputs["grantCeiling"],"rustGrants":inputs["rustGrants"]})
    } else {
        json!({})
    };
    if rust {
        requires.remove("sources");
        requires.extend(rust_requirements.as_object().unwrap().clone());
    }
    let plan_bytes = plan.encode();
    let mut artifacts = vec![
        json!({"name":"app.plan","kind":"bundle","sha256":hash(&plan_bytes),"bytes":plan_bytes.len(),"requires":requires}),
    ];
    for asset in assets {
        let mut requires = serde_json::Map::new();
        if asset["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("rust/"))
        {
            requires.extend(rust_requirements.as_object().unwrap().clone());
        }
        if let Some(stem) = asset["name"]
            .as_str()
            .and_then(|n| n.strip_prefix("shaders/"))
            .and_then(|n| n.strip_suffix(".wgsl"))
        {
            let interface = inputs["gpuSurfaces"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["name"] == stem))
                .ok_or_else(|| format!("shader {stem} has no baked surface interface"))?;
            requires.insert("gpuSurfaces".into(), json!([interface]));
        }
        artifacts.push(json!({"name":asset["name"],"kind":"bundle","sha256":asset["sha256"],"bytes":asset["bytes"],"requires":requires}));
    }
    Ok(json!({"version":1,"sources":sources,"surfaceCalls":calls,"artifacts":artifacts}))
}

/// Refresh the resident compiler's candidate graph beside its plan. The
/// actual binary's compatibility, provided sources and input receipt stay
/// frozen until the binary-producing build runs again.
pub fn write_development_artifacts(plan: &exact_plan::Plan) -> Result<(), String> {
    let Some(path) = std::env::var_os("EXACT_DEV_BAKE") else {
        return Ok(());
    };
    let path = PathBuf::from(path);
    let mut receipt: Value =
        serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if receipt["trust"] != "development"
        || receipt["compat"]["inputs"]["app"] != plan.app_id
        || receipt["compat"]["inputs"]["platform"] != "web"
    {
        return Err("resident compiler needs its app's actual development web bake receipt".into());
    }
    let graph = artifact_graph(
        plan,
        &receipt["compat"]["inputs"],
        &[],
        "wasm32-unknown-unknown",
    )?;
    let rows = receipt["graph"]["artifacts"]
        .as_array_mut()
        .ok_or("bake receipt has no artifacts")?;
    let node = rows
        .iter_mut()
        .find(|row| row["name"] == "app.plan")
        .ok_or("bake receipt has no plan node")?;
    *node = graph["artifacts"][0].clone();
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, receipt.to_string())
        .and_then(|_| std::fs::rename(&temporary, &path))
        .map_err(|e| e.to_string())
}

fn apply_release(compat: &mut Compat, bytes: &[u8], plan: &[u8]) -> Result<(), String> {
    let receipt: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("publisher receipt is not JSON: {e}"))?;
    let envelope =
        Envelope::parse(&serde_json::to_vec(&receipt["envelope"]).map_err(|e| e.to_string())?)?;
    let baked = Baked::from_compat(&compat.to_json(), plan)?;
    envelope.verify(&baked.embedded.verification_keys)?;
    if envelope.app_id != baked.embedded.app_id
        || envelope.stream.channel != baked.embedded.channel
        || envelope.stream.compatibility_id != compat.id
        || envelope.stream.seq == 0
    {
        return Err(
            "publisher receipt does not name this app, channel, cohort and a published sequence"
                .into(),
        );
    }
    if envelope.plan.sha256 != compat.embedded["plan"]["sha256"]
        || envelope.plan.bytes != plan.len() as u64
    {
        return Err("publisher receipt does not name the exact embedded plan bytes".into());
    }
    let mut named = envelope
        .assets
        .iter()
        .map(|a| json!({"name":a.name,"sha256":a.sha256,"bytes":a.bytes}))
        .collect::<Vec<_>>();
    named.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    if Value::Array(named) != compat.embedded["assets"] {
        return Err("publisher receipt does not name the complete embedded asset roster".into());
    }
    compat.embedded["seq"] = json!(envelope.stream.seq);
    compat.embedded["entryDigest"] = json!(envelope.digest);
    compat.embedded["genesis"] = Value::Bool(false);
    Ok(())
}

fn hash(bytes: &[u8]) -> String {
    exact_update::sha256_hex(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn rust_fixture(plan: &[u8]) -> (PathBuf, Value, Value) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "exact-rust-bake-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Parallel tests can observe the same clock tick. Each fixture owns
        // its directory exclusively, including cleanup after staging.
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("input")).unwrap();
        let inputs =
            json!({"app":"com.exact.caltrain","grantCeiling":"","rustGrants":"","rustMode":"wasm"});
        let module = b"\0asm\x01\0\0\0";
        let receipt = json!({"version":1,"kind":"rust","abi":RUST_ABI,"appId":inputs["app"],"grants":"",
            "target":"wasm32-unknown-unknown","executor":"wasm",
            "plan":{"file":"app.plan","sha256":hash(plan),"bytes":plan.len()},
            "module":{"file":"app.module.wasm","sha256":hash(module),"bytes":module.len()}});
        std::fs::write(root.join("input/app.module.wasm"), module).unwrap();
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        (root, inputs, receipt)
    }

    #[test]
    fn runner_sources_do_not_require_executors_or_grants() {
        for name in ["exactSurface", "exactViewport", "exactDelivery", "appData"] {
            let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
            let ty = b.record("Facts", &[]);
            b.source(name, &[], ty);
            let graph = artifact_graph(
                &b.finish().unwrap(),
                &json!({"executors":["native"], "grantCeiling":"network"}),
                &[],
                "test",
            )
            .unwrap();
            let requires = &graph["artifacts"][0]["requires"];
            assert_eq!(
                requires.get("executors").is_some(),
                name == "appData",
                "{name}"
            );
            assert_eq!(
                requires.get("grantCeiling").is_some(),
                name == "appData",
                "{name}"
            );
        }
    }

    #[test]
    fn supplemental_rust_is_real_paired_bytes_with_executor_requirements() {
        let mut plan = exact_plan::builder::PlanBuilder::new(0, 0)
            .finish()
            .unwrap();
        plan.app_id = "com.exact.caltrain".into();
        let bytes = plan.encode();
        let (root, inputs, _) = rust_fixture(&bytes);
        let cards = stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            &bytes,
            &inputs,
            "aarch64-apple-ios",
        )
        .unwrap();
        assert_eq!(cards.len(), 2);
        for card in &cards {
            let actual =
                std::fs::read(root.join("out").join(card["name"].as_str().unwrap())).unwrap();
            assert_eq!(card["sha256"], hash(&actual));
            assert_eq!(card["bytes"], actual.len());
        }
        let graph = artifact_graph(&plan, &inputs, &cards, "aarch64-apple-ios").unwrap();
        for row in graph["artifacts"].as_array().unwrap() {
            assert_eq!(row["requires"]["rustAbi"], RUST_ABI);
            assert_eq!(row["requires"]["rustMode"], "wasm");
            assert_eq!(row["requires"]["rustTarget"], "wasm32-unknown-unknown");
        }
        assert!(graph["artifacts"][0]["requires"].get("sources").is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tiered_assets_bind_the_complete_container_and_exact_native_cohort() {
        let mut plan = exact_plan::builder::PlanBuilder::new(0, 0)
            .finish()
            .unwrap();
        plan.app_id = "com.exact.caltrain".into();
        let bytes = plan.encode();
        let (root, mut inputs, mut receipt) = rust_fixture(&bytes);
        let target = "aarch64-apple-darwin";
        let module = b"EXLT\x01\0\0\0\x08\0\0\0\0asm\x01\0\0\0native";
        inputs["rustMode"] = json!("tiered");
        receipt["executor"] = json!("tiered");
        receipt["target"] = json!(target);
        receipt["module"] =
            json!({"file":"app.module.bin","sha256":hash(module),"bytes":module.len()});
        std::fs::write(root.join("input/app.module.bin"), module).unwrap();
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        let stage = |host| {
            stage_rust_bundle(
                &root.join("input"),
                &root.join("out"),
                &bytes,
                &inputs,
                host,
            )
        };
        let cards = stage(target).unwrap();
        let graph = artifact_graph(&plan, &inputs, &cards, target).unwrap();
        for row in graph["artifacts"].as_array().unwrap() {
            assert_eq!(row["requires"]["rustMode"], "tiered");
            assert_eq!(row["requires"]["rustTarget"], target);
        }
        for incompatible in [
            "x86_64-apple-darwin",
            "aarch64-apple-ios",
            "wasm32-unknown-unknown",
        ] {
            assert!(stage(incompatible).is_err(), "{incompatible}");
        }
        // A changed native companion is covered by the same authenticated digest.
        let mut changed = module.to_vec();
        *changed.last_mut().unwrap() ^= 1;
        std::fs::write(root.join("input/app.module.bin"), changed).unwrap();
        assert!(stage(target).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn supplemental_rust_refuses_policy_identity_grants_target_and_tampered_pair() {
        let plan = b"exact baked plan";
        let (root, mut inputs, receipt) = rust_fixture(plan);
        for (key, value) in [
            ("appId", json!("another.app")),
            ("grants", json!("secret.keep token")),
            ("abi", json!(99)),
            ("kind", json!("javascript")),
            ("target", json!("aarch64-apple-ios")),
            ("executor", json!("native")),
        ] {
            let mut changed = receipt.clone();
            changed[key] = value;
            std::fs::write(root.join("input/app.module.json"), changed.to_string()).unwrap();
            assert!(
                stage_rust_bundle(
                    &root.join("input"),
                    &root.join("out"),
                    plan,
                    &inputs,
                    "aarch64-apple-ios"
                )
                .is_err(),
                "{key}"
            );
            assert!(!root.join("out/rust").exists());
        }
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        inputs["rustMode"] = json!("off");
        assert!(stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            plan,
            &inputs,
            "aarch64-apple-ios"
        )
        .is_err());
        inputs["rustMode"] = json!("wasm");
        assert!(stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            b"changed plan",
            &inputs,
            "aarch64-apple-ios"
        )
        .is_err());
        std::fs::write(root.join("input/app.module.wasm"), b"changed module").unwrap();
        assert!(stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            plan,
            &inputs,
            "aarch64-apple-ios"
        )
        .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mixed_rust_receipt_preserves_its_own_scope_under_the_union_ceiling() {
        let plan = b"mixed baked plan";
        let (root, mut inputs, mut receipt) = rust_fixture(plan);
        inputs["grantCeiling"] = json!("fs.read app:/data/rust\nnet.fetch https://example.test/");
        inputs["rustGrants"] = json!("fs.read app:/data/rust");
        inputs["javascriptGrants"] = json!("net.fetch https://example.test/");
        receipt["grants"] = inputs["rustGrants"].clone();
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        assert!(stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            plan,
            &inputs,
            "aarch64-apple-ios"
        )
        .is_ok());
        receipt["grants"] = inputs["grantCeiling"].clone();
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        assert!(
            stage_rust_bundle(
                &root.join("input"),
                &root.join("out"),
                plan,
                &inputs,
                "aarch64-apple-ios"
            )
            .is_err(),
            "a module cannot borrow its sibling's grant"
        );
        receipt["grants"] = inputs["rustGrants"].clone();
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        inputs["grantCeiling"] = inputs["javascriptGrants"].clone();
        assert!(
            stage_rust_bundle(
                &root.join("input"),
                &root.join("out"),
                plan,
                &inputs,
                "aarch64-apple-ios"
            )
            .is_err(),
            "source grants must fit the host ceiling"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signed_release_requires_the_complete_real_supplemental_roster() {
        let (mut compat, mut receipt, plan) = signed_fixture();
        let (root, inputs, _) = rust_fixture(&plan);
        let cards = stage_rust_bundle(
            &root.join("input"),
            &root.join("out"),
            &plan,
            &inputs,
            "aarch64-apple-ios",
        )
        .unwrap();
        for card in &cards {
            receipt["envelope"]["assets"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                "name":card["name"],"sha256":card["sha256"],"bytes":card["bytes"],
                "url":format!("../../blobs/{}",card["sha256"].as_str().unwrap())}));
        }
        // A fresh process-local test key: no publisher's private key is stored.
        let canonical = exact_update::canonical_bytes(&receipt["envelope"].to_string()).unwrap();
        let output = std::process::Command::new("bun").args(["-e", r#"
            const {generateKeyPairSync,sign}=require('node:crypto');
            const {publicKey,privateKey}=generateKeyPairSync('ed25519');
            process.stdout.write(JSON.stringify({public:publicKey.export({type:'spki',format:'der'}).subarray(-32).toString('base64'),
                signature:sign(null,Buffer.from(process.argv[1]),privateKey).toString('base64')}));
        "#]).arg(String::from_utf8(canonical).unwrap()).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let signing: Value = serde_json::from_slice(&output.stdout).unwrap();
        compat.inputs["keys"] = json!({"fixture":signing["public"]});
        receipt["envelope"]["signature"] =
            json!({"keyId":"fixture","ed25519":signing["signature"]});
        let receipt = serde_json::to_vec(&receipt).unwrap();
        assert!(apply_release(&mut compat, &receipt, &plan)
            .unwrap_err()
            .contains("complete embedded asset roster"));
        let assets = compat.embedded["assets"].as_array_mut().unwrap();
        assets.extend(cards);
        assets.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        apply_release(&mut compat, &receipt, &plan).unwrap();
        assert_eq!(compat.embedded["seq"], 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn the_embedded_roster_uses_the_copy_gate_and_refuses_symlink_assets() {
        let root = std::env::temp_dir().join(format!(
            "exact-bake-assets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::create_dir_all(root.join("deck/nested")).unwrap();
        std::fs::create_dir_all(root.join("gpu/shaders")).unwrap();
        std::fs::write(root.join("assets/image.png"), b"image").unwrap();
        std::fs::write(root.join("deck/nested/index.html"), b"deck").unwrap();
        std::fs::write(root.join("gpu/shaders/effect.wgsl"), b"shader").unwrap();
        let cards = asset_cards(&root).unwrap();
        assert_eq!(
            cards,
            vec![
                json!({"name":"assets/image.png","sha256":hash(b"image"),"bytes":5}),
                json!({"name":"deck/nested/index.html","sha256":hash(b"deck"),"bytes":4}),
                json!({"name":"shaders/effect.wgsl","sha256":hash(b"shader"),"bytes":6}),
            ]
        );
        assert_eq!(asset_cards(&root.join("missing/..")).unwrap(), cards);
        std::fs::write(root.join("outside"), b"must not be embedded").unwrap();
        std::os::unix::fs::symlink("../outside", root.join("assets/escape")).unwrap();
        assert!(asset_cards(&root).unwrap_err().contains("bake asset gate"));
        std::fs::remove_file(root.join("assets/escape")).unwrap();
        std::fs::rename(root.join("assets"), root.join("original-assets")).unwrap();
        for target in ["original-assets", "missing-assets"] {
            std::os::unix::fs::symlink(target, root.join("assets")).unwrap();
            assert!(asset_cards(&root).unwrap_err().contains("symlinks"));
            std::fs::remove_file(root.join("assets")).unwrap();
        }
        std::fs::rename(root.join("original-assets"), root.join("assets")).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(asset_cards(&root).unwrap().is_empty());
    }

    fn signed_fixture() -> (Compat, Value, Vec<u8>) {
        let raw = include_bytes!("../../../update/tests/fixtures/publisher/exact.json");
        let head = Envelope::parse(raw).unwrap();
        let plan = include_bytes!("../../../update/tests/fixtures/publisher/app.plan").to_vec();
        let key = include_str!("../../../update/tests/fixtures/publisher/caltrain-2026.pub").trim();
        let mut assets = head
            .assets
            .iter()
            .map(|a| json!({"name":a.name,"sha256":a.sha256,"bytes":a.bytes}))
            .collect::<Vec<_>>();
        assets.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let compat = Compat {
            id: head.stream.compatibility_id.clone(),
            inputs: json!({"app":head.app_id,"trust":"production","keys":{"caltrain-2026":key},"store":{"L":"A"}}),
            channel: head.stream.channel.clone(),
            origin: None,
            activate: "next-launch".into(),
            target: "aarch64-apple-ios".into(),
            embedded: json!({"seq":0,"genesis":true,"entryDigest":null,"plan":{"sha256":head.plan.sha256,"bytes":head.plan.bytes},"assets":assets}),
        };
        (
            compat,
            json!({"envelope":serde_json::from_slice::<Value>(raw).unwrap()}),
            plan,
        )
    }

    #[test]
    fn a_production_sequence_is_bound_to_a_signed_matching_complete_bundle() {
        let (mut compat, receipt, plan) = signed_fixture();
        apply_release(&mut compat, &serde_json::to_vec(&receipt).unwrap(), &plan).unwrap();
        assert_eq!(compat.embedded["seq"], 1);
        assert_eq!(compat.embedded["genesis"], false);
        assert_eq!(compat.embedded["entryDigest"].as_str().unwrap().len(), 64);
        let baked = Baked::from_compat(&compat.to_json(), &plan).unwrap();
        assert_eq!(baked.embedded.compatibility_id, compat.id);
        assert_eq!(baked.embedded.seq, 1);

        let (mut wrong, _, _) = signed_fixture();
        wrong.embedded["assets"] = json!([]);
        assert!(
            apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan)
                .unwrap_err()
                .contains("complete embedded asset roster")
        );
        let (mut wrong, _, _) = signed_fixture();
        wrong.embedded["plan"]["sha256"] = json!("0".repeat(64));
        assert!(
            apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan)
                .unwrap_err()
                .contains("embedded.plan does not match")
        );
        let (mut wrong, mut forged, _) = signed_fixture();
        forged["envelope"]["stream"]["seq"] = json!(41);
        assert!(apply_release(&mut wrong, &serde_json::to_vec(&forged).unwrap(), &plan).is_err());
        let (mut wrong, _, _) = signed_fixture();
        wrong.channel = "another".into();
        assert!(apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan).is_err());
    }
}
