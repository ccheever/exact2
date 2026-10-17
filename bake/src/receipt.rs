//! The binary bake's compatibility and embedded receipt (LLP 1030 D3/D3a).
//! The Cargo build script knows its actual target and data-source grants;
//! Consumers read these bytes instead of reconstructing those facts.

use crate::compat::Compat;
use contract::Manifest;
use exact_update::{Baked, Envelope};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

mod watch;

/// A development bake's map from plan node to Contract source (LLP 1035.002
/// D6; LLP 1012.001.000 D6), `OUT_DIR/app.plan.map.json`: beside the plan,
/// never in the product, and only when compiling the app's contract again
/// gives the baked plan's own nodes. The digest inside names the plan it
/// describes, so the driver never joins it to another. A map that cannot be
/// made is absent; it never fails a build.
fn source_map(trust: &str, app: &Path, out: &Path, plan: &exact_plan::Plan, plan_bytes: &[u8]) {
    let path = out.join("app.plan.map.json");
    let _ = std::fs::remove_file(&path);
    if trust != "development" {
        return;
    }
    let Ok(source) = app.join("app.contract").canonicalize() else {
        return;
    };
    let Ok((compiled, map)) = contract::compile_path_mapped(&source) else {
        return;
    };
    if compiled.nodes == plan.nodes {
        let _ = std::fs::write(&path, map.json(plan_bytes));
    }
}

/// Complete the compatibility document from the plan already baked by the
/// app's build script. Production sequence provenance is a signed publisher
/// receipt; an unreceipted stream requires an explicit genesis bake.
pub(crate) fn emit(
    compat: &mut Compat,
    trust: &str,
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
        "EXACT_GPU_PRODUCT",
        "EXACT_GPU_MODULES",
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
    source_map(trust, app, out, &plan, &plan_bytes);
    let plan_card = json!({"sha256": hash(&plan_bytes), "bytes": plan_bytes.len()});
    let mut assets = asset_cards(app, out, manifest)?;
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
        let product = std::env::var_os("EXACT_GPU_PRODUCT")
            .map(|path| gpu_product(Path::new(&path)))
            .transpose()?;
        let development = std::env::var("EXACT_GPU_DEVELOPMENT").ok();
        if let Some(card) = gpu_card(&manifest.id, &compat.id, trust, development, product) {
            compat.embedded["gpu"] = card;
        }
        // One card per module (LLP 1009 D6), the same shape as the primary's
        // product card: a module always binds its signed digest.
        if let Ok(modules) = std::env::var("EXACT_GPU_MODULES") {
            let products: serde_json::Map<String, Value> =
                serde_json::from_str(&modules).map_err(|e| format!("EXACT_GPU_MODULES: {e}"))?;
            let mut cards = serde_json::Map::new();
            for (name, path) in products {
                let path = path.as_str().ok_or("EXACT_GPU_MODULES: a product path")?;
                let product = Some(gpu_product(Path::new(path))?);
                if let Some(card) = gpu_card(&manifest.id, &compat.id, trust, None, product) {
                    cards.insert(name, card);
                }
            }
            compat.embedded["gpuModules"] = Value::Object(cards);
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
        // Never watched: the builder rewrites these from OUT_DIR after every
        // Cargo run, so a watch here would rerun this bake on every next build.
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

/// The GPU module's baked identity. `trust` is the build's resolved trust
/// (unset is development), never the cohort input: a no-delivery (store L=0)
/// cohort omits trust, and the hosts read the explicit build trust here.
/// gpu-dev authenticates the independently completed module receipt at load;
/// production always embeds the exact GPU digest.
fn gpu_card(
    app: &str,
    cohort: &str,
    trust: &str,
    development: Option<String>,
    product: Option<(String, String)>,
) -> Option<Value> {
    match (development.filter(|_| trust == "development"), product) {
        (Some(name), _) => Some(json!({"app":app,"cohort":cohort,
            "name":name,"trust":"development","receipt":true})),
        (None, Some((name, sha256))) => Some(json!({"name":name,"sha256":sha256,
            "app":app,"cohort":cohort,"trust":trust})),
        (None, None) => None,
    }
}

// The same directory-owned gate used by copying, including direct Cargo
// bakes. This tooling process does not link Unix filesystem code into a wasm
// consumer of `contract`; its Cargo cache is distinct from the calling build.
fn asset_cards(app: &Path, out: &Path, manifest: &Manifest) -> Result<Vec<Value>, String> {
    let gate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/filesystem.mjs");
    println!("cargo:rerun-if-changed={}", gate.display());
    let code = r#"
        import {pathToFileURL} from 'node:url';
        import {resolve} from 'node:path';
        import {createHash} from 'node:crypto';
        const {filesystem} = await import(pathToFileURL(process.argv[1]));
        const {shaderFiles,shaderRoots,shaderPreludeFiles} = await import(pathToFileURL(resolve(process.argv[1],'../app.mjs')));
        const app={dir:process.argv[2],manifest:JSON.parse(process.argv[3])};
        const cards=[];
        for (const [source,prefix] of [['assets','assets'],['deck','deck']]) {
            const tree=filesystem({op:'tree',root:resolve(process.argv[2],source),optionalRoot:true});
            for (const [name,base64] of Object.entries(tree??{})) {
                const bytes=Buffer.from(base64,'base64');
                cards.push({name:`${prefix}/${name}`,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length});
            }
        }
        for (const [name,bytes] of shaderFiles(app)) cards.push({name:`shaders/${name}`,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length});
        process.stdout.write(JSON.stringify({cards:cards.sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0),roots:[...shaderRoots(app),...shaderPreludeFiles(app)]}));
    "#;
    let output = crate::bun()
        .args(["--input-type=module", "-e", code])
        .arg(gate)
        .arg(app)
        .arg(manifest.json.to_string())
        .output()
        .map_err(|e| format!("bake asset gate: {}", crate::bun_error(e)))?;
    if !output.status.success() {
        return Err(format!(
            "bake asset gate: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let inventory: Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("bake asset inventory: {e}"))?;
    let cards = inventory["cards"]
        .as_array()
        .ok_or("missing bake cards")?
        .clone();
    // Every client must stage these as distinct files, APFS included; the
    // publisher and every client parse refuse the same names.
    exact_update::check_asset_names(cards.iter().filter_map(|c| c["name"].as_str()))
        .map_err(|e| format!("bake asset gate: {e}"))?;
    // The helper has its own target directory. Other known bake destinations
    // may also be beneath an external app: never broaden a watch over them.
    let mut outputs = vec![
        out.to_path_buf(),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"),
    ];
    for name in [
        "EXACT_BAKE_OUTPUT",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_BUILD_DIR",
    ] {
        if let Some(path) = std::env::var_os(name) {
            outputs.push(PathBuf::from(path));
        }
    }
    println!("cargo:rerun-if-env-changed=EXACT_ASSET_ROOTS");
    let declared = std::env::var("EXACT_ASSET_ROOTS").ok();
    let roots = ["assets", "deck"]
        .into_iter()
        .filter_map(|root| watch::asset_tree(app, root, &outputs, declared.as_deref()))
        .chain(
            inventory["roots"]
                .as_array()
                .ok_or("missing shader roots")?
                .iter()
                .filter_map(|v| {
                    watch::shader_tree(
                        Path::new(v.as_str().unwrap()),
                        &outputs,
                        declared.as_deref(),
                    )
                }),
        );
    let paths = roots.collect::<std::collections::BTreeSet<_>>();
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(cards)
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
        || meta["abi"].as_u64() != Some(2)
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

/// The plan's requirement graph against a cohort's compatibility inputs
/// (`exact-bake graph`): what a publisher that starts from a signed stream
/// head, not a native bake, checks a new plan against.
pub fn plan_graph(plan_bytes: &[u8], inputs: &Value, target: &str) -> Result<Value, String> {
    let plan = exact_plan::Plan::decode(plan_bytes).map_err(|e| format!("the plan: {e:?}"))?;
    let mut graph = artifact_graph(&plan, inputs, &[], target)?;
    let node = &mut graph["artifacts"][0];
    node["sha256"] = json!(hash(plan_bytes));
    node["bytes"] = json!(plan_bytes.len());
    Ok(graph)
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
    if sources.keys().any(|name| {
        name != exact_runner::delivery::SOURCE
            && name != exact_runner::viewport::SOURCE
            && name != exact_runner::page::SOURCE
    }) {
        requires.insert("executors".into(), inputs["executors"].clone());
        requires.insert("grantCeiling".into(), inputs["grantCeiling"].clone());
    }
    // @ref LLP 1075.003.000.001 §4.3 — the hatch words the plan marks that
    // this platform's module handles: the installed module must handle each.
    // A word this platform does not handle is no requirement here: its nodes
    // are shown and never called.
    let handled = inputs["hatches"].as_array().cloned().unwrap_or_default();
    let mut marked: Vec<&str> = plan
        .bindings
        .iter()
        .filter(|b| {
            b.kind == exact_plan::BindingKind::Prop
                && exact_kernel::PropId::from_wire(b.id) == Some(exact_kernel::PropId::Hatch)
        })
        .filter_map(|b| crate::colors::literal(plan, b))
        .filter(|word| handled.iter().any(|h| h == word))
        .collect();
    marked.sort_unstable();
    marked.dedup();
    if !marked.is_empty() {
        requires.insert("hatches".into(), json!(marked));
    }
    // A purpose's text is in the installed binary's Info.plist: a bundle
    // whose purposes differ is a binary release (LLP 1069.008 D5).
    // What a TypeScript module may ask of the binary's prelude (compat.rs
    // `TYPESCRIPT_RUNTIME`): an older cohort's prelude would ignore it
    // silently (`exactBodyFrom` sent as an empty body), so it takes no bundle.
    if inputs.get("typescriptRuntime").is_some() {
        requires.insert(
            "typescriptRuntime".into(),
            inputs["typescriptRuntime"].clone(),
        );
    }
    if inputs.get("grantPurposes").is_some() {
        requires.insert("grantPurposes".into(), inputs["grantPurposes"].clone());
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
        json!({"rustMode":mode,"rustAbi":2,"rustTarget":target,"grantCeiling":inputs["grantCeiling"],"rustGrants":inputs["rustGrants"]})
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
    // The host's loaded modules the plan can reach (LLP 1047 D1's loaded
    // tier), deferred templates and branches included, each by the rule its
    // host loads it by: the Canvas 2D GPU module for a `Canvas`, the SVG
    // island module where `svg_islands` holds, the video arm for a `Video`,
    // the web arm for a `WebView`, and the sound arm for a declared sound
    // (LLP 1096 D8: a `sound` declaration is not a node). A build whose plan
    // is fixed leaves out the rest.
    let makes = |wanted: exact_kernel::NodeType| {
        plan.nodes
            .iter()
            .any(|node| exact_kernel::NodeType::from_wire(node.node_type) == Some(wanted))
    };
    let loads: Vec<&str> = [
        ("canvas", makes(exact_kernel::NodeType::Canvas)),
        ("svg", exact_runner::svg_islands(plan)),
        ("video", makes(exact_kernel::NodeType::Video)),
        ("web", makes(exact_kernel::NodeType::WebView)),
        ("sound", !plan.sounds.is_empty()),
        // LLP 1125 D6: the MIDI arm for a plan that asks for ports or hears an input.
        ("midi", exact_runner::uses::uses_midi(plan)),
    ]
    .into_iter()
    .filter_map(|(module, reached)| reached.then_some(module))
    .collect();
    // LLP 1098 D8: a node that binds `mediaTitle` (`metadata=`) claims the
    // media session, which iOS shows only for a playback session.
    let media_session = plan.nodes.iter().any(|node| {
        node.bindings.iter().any(|b| {
            let b = plan.binding(b);
            b.kind == exact_plan::BindingKind::Prop
                && b.id == exact_kernel::PropId::MediaTitle as u16
        })
    });
    // The linked tier the plan uses (LLP 1047 D2), by name: a native build
    // whose plan is fixed links these and no other (LLP 1047.001 D2).
    let uses: Vec<&str> = exact_runner::uses(plan).iter().map(|c| c.name()).collect();
    Ok(
        json!({"version":1,"sources":sources,"surfaceCalls":calls,"loads":loads,"uses":uses,"mediaSession":media_session,"launchBackground":launch_background(plan),"artifacts":artifacts}),
    )
}

/// LLP 1116.011 D2.3: the ground the app's first route stands on, which the
/// iOS launch screen shows until its first frame: `"grouped"` where the first
/// route's `background-color` is the role the compiler gives a route that
/// scrolls a grouped list (`-exact-grouped-background`, LLP 1116.007 D5),
/// `"system"` for any other (the platform's page, `systemBackground`). A route
/// is a node with a `navigationKey` and no `navigationBack` (the navigation
/// root has both); the first in the plan's order, past the page the compiler
/// writes for an unknown address (its test id `not-found`), is the first one
/// the author declared. A route whose background is written or computed is
/// the author's: the manifest's `background_color` names its launch colour.
fn launch_background(plan: &exact_plan::Plan) -> &'static str {
    use exact_kernel::PropId;
    use exact_plan::BindingKind::{Prop, Style};
    let literal = |node: &exact_plan::NodesRow, kind, id: u16| {
        node.bindings.iter().find_map(|b| {
            let b = plan.binding(b);
            (b.kind == kind && b.id == id).then(|| crate::colors::literal(plan, b))
        })
    };
    let route = plan.nodes.iter().find(|node| {
        let has = |id: PropId| literal(node, Prop, id as u16).is_some();
        has(PropId::NavigationKey)
            && !has(PropId::NavigationBack)
            && literal(node, Prop, PropId::TestId as u16) != Some(Some("not-found"))
    });
    let ground = route.and_then(|node| {
        literal(node, Style, exact_kernel::StyleId::BackgroundColor as u16).flatten()
    });
    if ground == Some("-exact-grouped-background") {
        "grouped"
    } else {
        "system"
    }
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

/// A signed GPU product's file name and digest, watched by the bake: its code
/// belongs to this app and cohort, which the loader checks before opening it.
fn gpu_product(path: &Path) -> Result<(String, String), String> {
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = std::fs::read(path).map_err(|e| format!("GPU product {}: {e}", path.display()))?;
    let name = path.file_name().and_then(|n| n.to_str());
    Ok((
        name.ok_or("GPU product filename is not UTF-8")?.to_string(),
        code_digest(&bytes),
    ))
}

/// A GPU product's digest: its exact bytes, except that a signed 64-bit
/// Mach-O's code signature is left out, with the three sizes that follow it
/// (the signature command's `datasize`, `__LINKEDIT`'s `vmsize` and
/// `filesize`) read as zero. A re-signer (AppDrop signs every .ipa again
/// for each recipient) keeps the digest; any change to the code it signs
/// does not. Anything else is hashed whole. GpuModule.swift's
/// `codeDigest` and host/apple/assets.mjs's `codeDigest` are this one.
fn code_digest(bytes: &[u8]) -> String {
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let macho = || -> Option<Vec<u8>> {
        if word(0)? != 0xfeed_facf {
            return None;
        }
        let (mut at, mut signature, mut zero) = (32, None, Vec::new());
        for _ in 0..word(16)? {
            let (cmd, size) = (word(at)?, word(at + 4)?);
            if cmd == 0x1d {
                signature = Some(word(at + 8)?);
                zero.push((at + 12, 4));
            } else if cmd == 0x19 && bytes.get(at + 8..at + 24)? == b"__LINKEDIT\0\0\0\0\0\0" {
                zero.extend([(at + 32, 8), (at + 48, 8)]);
            }
            at += size.max(8);
        }
        let mut code = bytes.get(..signature?)?.to_vec();
        for (at, len) in zero {
            code.get_mut(at..at + len)?.fill(0);
        }
        Some(code)
    };
    hash(&macho().unwrap_or_else(|| bytes.to_vec()))
}

fn hash(bytes: &[u8]) -> String {
    exact_update::sha256_hex(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// LLP 1012.001.000 D6: a development bake leaves its plan's map beside
    /// it, keyed by that plan's digest; a production bake leaves none, and a
    /// plan whose nodes the contract no longer compiles to gets none.
    #[test]
    fn a_development_bake_leaves_its_plans_source_map() {
        let root = std::env::temp_dir().join(format!("exact-map-{}", std::process::id()));
        let (app, out) = (root.join("app"), root.join("out"));
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            app.join("app.contract"),
            "component App\n  view\n    text \"a\" testId=\"a\"\n",
        )
        .unwrap();
        let plan = contract::compile_path(&app.join("app.contract")).unwrap();
        let bytes = plan.encode();
        let map = out.join("app.plan.map.json");
        source_map("development", &app, &out, &plan, &bytes);
        let written: Value = serde_json::from_slice(&std::fs::read(&map).unwrap()).unwrap();
        assert_eq!(written["digest"], contract::plan_digest(&bytes));
        source_map("production", &app, &out, &plan, &bytes);
        assert!(!map.exists(), "a production bake carries no map");
        let other =
            contract::compile("component App\n  view\n    column\n      text \"b\"\n").unwrap();
        source_map("development", &app, &out, &other, &other.encode());
        assert!(!map.exists(), "a map for other nodes is never written");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// LLP 1096 D8: a declared sound is not a node, so the plan's `sounds`
    /// table is what says a fixed build carries the sound arm.
    #[test]
    fn a_plan_with_a_sound_loads_the_sound_arm() {
        let without = exact_plan::builder::PlanBuilder::new(0, 0)
            .finish()
            .unwrap();
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        b.sound("assets/kick.wav", 4_800, 48_000, 1, "00");
        let with = b.finish().unwrap();
        for (plan, loads) in [(without, json!([])), (with, json!(["sound"]))] {
            let graph = artifact_graph(&plan, &json!({}), &[], "aarch64-apple-darwin").unwrap();
            assert_eq!(graph["loads"], loads);
        }
    }

    /// LLP 1075.003.000.001 §4.3: the plan requires the hatch words it marks
    /// that this platform's module handles, and no others.
    #[test]
    fn a_plan_requires_the_hatch_words_its_platform_handles() {
        let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
        for word in ["dot", "avatar", "avatar", "badge"] {
            let expr = b.constant(&exact_plan::Value::str(word));
            let bindings = [exact_plan::BindingsRow {
                kind: exact_plan::BindingKind::Prop,
                id: exact_kernel::PropId::Hatch as u16,
                expr,
                written: true,
            }];
            b.node(
                exact_kernel::NodeType::Video as u8,
                None,
                None,
                0,
                &bindings,
                &[],
                None,
            );
        }
        let plan = b.finish().unwrap();
        let requires = |inputs: Value| {
            let graph = artifact_graph(&plan, &inputs, &[], "aarch64-apple-darwin").unwrap();
            graph["artifacts"][0]["requires"]["hatches"].clone()
        };
        // This platform handles `avatar` and `dot`; `badge` is another platform's.
        assert_eq!(
            requires(json!({"hatches": ["avatar", "dot", "unused"]})),
            json!(["avatar", "dot"])
        );
        // A platform that handles none of them is asked for nothing.
        assert_eq!(requires(json!({"hatches": null})), Value::Null);
        assert_eq!(requires(json!({})), Value::Null);
    }

    /// LLP 1108 D6 R2: a TypeScript app's plan requires the prelude runtime
    /// its compatibility inputs carry (`exactBodyFrom`), so an older cohort
    /// takes no bundle; an app without one requires none.
    #[test]
    fn a_plan_requires_the_typescript_runtime_its_inputs_carry() {
        let plan = exact_plan::builder::PlanBuilder::new(0, 0)
            .finish()
            .unwrap();
        let requires = |inputs: Value| {
            let graph = artifact_graph(&plan, &inputs, &[], "aarch64-apple-ios").unwrap();
            graph["artifacts"][0]["requires"]["typescriptRuntime"].clone()
        };
        assert_eq!(
            requires(json!({"typescriptRuntime": crate::compat::TYPESCRIPT_RUNTIME})),
            json!(crate::compat::TYPESCRIPT_RUNTIME)
        );
        assert_eq!(requires(json!({})), Value::Null);
    }

    /// LLP 1098 D8: the receipt says whether a node claims the media session.
    #[test]
    fn a_plan_with_metadata_claims_the_media_session() {
        for (prop, claims) in [
            (exact_kernel::PropId::Src, false),
            (exact_kernel::PropId::MediaTitle, true),
        ] {
            let mut b = exact_plan::builder::PlanBuilder::new(0, 0);
            let expr = b.constant(&exact_plan::Value::str("Episode 1"));
            let bindings = [exact_plan::BindingsRow {
                kind: exact_plan::BindingKind::Prop,
                id: prop as u16,
                expr,
                written: true,
            }];
            b.node(
                exact_kernel::NodeType::Video as u8,
                None,
                None,
                0,
                &bindings,
                &[],
                None,
            );
            let graph = artifact_graph(
                &b.finish().unwrap(),
                &json!({}),
                &[],
                "aarch64-apple-darwin",
            )
            .unwrap();
            assert_eq!(
                (graph["loads"].clone(), graph["mediaSession"].clone()),
                (json!(["video"]), json!(claims))
            );
        }
    }

    /// LLP 1116.011 D2.3: the receipt says what the first route stands on, for
    /// the iOS launch screen: the grouped grey where the route scrolls a
    /// grouped list (stopwatch, round 13), the page's own otherwise.
    #[test]
    fn a_plan_says_the_ground_its_first_route_stands_on() {
        const SCREEN: &str = "routes nav
  home \"/\"
component App
  view
    main navigationKey=`${top(nav).id}` navigationBack=\"back\"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll=\"l\" ROUTE position=\"absolute\" inset=0
          header
            text \"Laps\" role=\"heading\" aria-level=1
          LIST
            section
              row
                text \"One\"
";
        const GROUPED: &str =
            "list id=\"l\" appearance=\"auto\" listStyle=\"inset-grouped\" flex=1 min-height=0";
        let launch = |route: &str, list: &str| {
            let plan = contract::compile(&SCREEN.replace("ROUTE", route).replace("LIST", list))
                .unwrap_or_else(|e| panic!("{e}"));
            artifact_graph(&plan, &json!({}), &[], "aarch64-apple-ios-sim").unwrap()
                ["launchBackground"]
                .clone()
        };
        assert_eq!(launch("", GROUPED), json!("grouped"));
        assert_eq!(
            launch("", &GROUPED.replace("inset-grouped", "grouped")),
            json!("grouped")
        );
        // Any other scroll, or a page of its own, is the platform's.
        assert_eq!(
            launch("", "scroll id=\"l\" flex=1 min-height=0"),
            json!("system")
        );
        assert_eq!(
            launch("background-color=\"-exact-background\"", GROUPED),
            json!("system")
        );
        // No route at all, as a plan with no navigation root is.
        let bare =
            contract::compile("component App\n  view\n    text \"a\" testId=\"a\"\n").unwrap();
        assert_eq!(
            artifact_graph(&bare, &json!({}), &[], "aarch64-apple-ios-sim").unwrap()
                ["launchBackground"],
            json!("system")
        );
    }

    #[test]
    fn a_gpu_digest_survives_re_signing_but_not_a_code_change() {
        // A 64-bit Mach-O: __LINKEDIT, then the signature command, the code,
        // and the signature at offset 160.
        let signed = |signature: &[u8], code: u8| {
            let mut b = vec![0u8; 160];
            b[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
            b[16..20].copy_from_slice(&2u32.to_le_bytes());
            b[32..36].copy_from_slice(&0x19u32.to_le_bytes());
            b[36..40].copy_from_slice(&72u32.to_le_bytes());
            b[40..50].copy_from_slice(b"__LINKEDIT");
            b[64..72].copy_from_slice(&(signature.len() as u64).to_le_bytes());
            b[80..88].copy_from_slice(&(signature.len() as u64 + 8).to_le_bytes());
            b[104..108].copy_from_slice(&0x1du32.to_le_bytes());
            b[108..112].copy_from_slice(&16u32.to_le_bytes());
            b[112..116].copy_from_slice(&160u32.to_le_bytes());
            b[116..120].copy_from_slice(&(signature.len() as u32).to_le_bytes());
            b[150] = code;
            b.extend_from_slice(signature);
            b
        };
        let ours = code_digest(&signed(b"ad-hoc", 1));
        assert_eq!(
            ours,
            code_digest(&signed(b"a recipient's longer signature", 1))
        );
        assert_ne!(ours, code_digest(&signed(b"ad-hoc", 2)));
        assert_ne!(ours, hash(&signed(b"ad-hoc", 1)));
        assert_eq!(code_digest(b"not Mach-O"), hash(b"not Mach-O"));
    }

    #[test]
    fn a_no_delivery_development_bake_names_its_gpu_dev_module() {
        // Games are store L=0: the cohort has no trust input, yet the
        // host-less proof's split host bake must name the gpu-dev module.
        let card = gpu_card(
            "com.example.game",
            "c",
            "development",
            Some("libg.so".into()),
            None,
        );
        assert_eq!(card.as_ref().unwrap()["name"], "libg.so");
        assert_eq!(card.as_ref().unwrap()["receipt"], true);
        let product = Some(("libg.so".into(), "ab".into()));
        let release = gpu_card(
            "com.example.game",
            "c",
            "production",
            Some("libg.so".into()),
            product,
        );
        assert_eq!(release.as_ref().unwrap()["sha256"], "ab");
        assert_eq!(release.unwrap()["trust"], "production");
        assert!(gpu_card(
            "com.example.game",
            "c",
            "production",
            Some("libg.so".into()),
            None
        )
        .is_none());
    }

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
        let receipt = json!({"version":1,"kind":"rust","abi":2,"appId":inputs["app"],"grants":"",
            "target":"wasm32-unknown-unknown","executor":"wasm",
            "plan":{"file":"app.plan","sha256":hash(plan),"bytes":plan.len()},
            "module":{"file":"app.module.wasm","sha256":hash(module),"bytes":module.len()}});
        std::fs::write(root.join("input/app.module.wasm"), module).unwrap();
        std::fs::write(root.join("input/app.module.json"), receipt.to_string()).unwrap();
        (root, inputs, receipt)
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
            assert_eq!(row["requires"]["rustAbi"], 2);
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
        let output = crate::bun().args(["-e", r#"
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
        std::fs::write(root.join("assets/image.png"), b"image").unwrap();
        std::fs::write(root.join("deck/nested/index.html"), b"deck").unwrap();
        let manifest = Manifest::read(&root).unwrap();
        let cards = asset_cards(&root, &root.join("out"), &manifest).unwrap();
        assert_eq!(
            cards,
            vec![
                json!({"name":"assets/image.png","sha256":hash(b"image"),"bytes":5}),
                json!({"name":"deck/nested/index.html","sha256":hash(b"deck"),"bytes":4}),
            ]
        );
        std::fs::write(root.join("assets/two words.png"), b"space").unwrap();
        assert!(asset_cards(&root, &root.join("out"), &manifest)
            .unwrap_err()
            .contains("bake asset gate: the asset name assets/two words.png is not portable"));
        std::fs::remove_file(root.join("assets/two words.png")).unwrap();
        std::fs::write(root.join("outside"), b"must not be embedded").unwrap();
        std::os::unix::fs::symlink("../outside", root.join("assets/escape")).unwrap();
        assert!(asset_cards(&root, &root.join("out"), &manifest)
            .unwrap_err()
            .contains("bake asset gate"));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn signed_fixture() -> (Compat, Value, Vec<u8>) {
        let raw = include_bytes!("../../update/tests/it/fixtures/publisher/exact.json");
        let head = Envelope::parse(raw).unwrap();
        let plan = include_bytes!("../../update/tests/it/fixtures/publisher/app.plan").to_vec();
        let key = include_str!("../../update/tests/it/fixtures/publisher/caltrain-2026.pub").trim();
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
            reach: Value::Null,
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
