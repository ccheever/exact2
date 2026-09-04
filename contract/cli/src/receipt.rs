//! The binary bake's compatibility and embedded receipt (LLP 1030 D3/D3a).
//! The Cargo build script knows its actual target and data-source grants;
//! Consumers read these bytes instead of reconstructing those facts.

use crate::compat::{Compat, Manifest};
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
    let assets = asset_cards(app)?;
    compat.target = target.into();
    compat.embedded =
        json!({"seq":0,"plan":plan_card,"assets":assets,"entryDigest":null,"genesis":true});
    // A binary without an updater has no stream or rollback floor.
    // Native L=A artifacts still require authenticated sequence provenance.
    let development = compat.inputs["trust"] == "development";
    if development {
        // Explicit development artifacts have a local genesis, never a
        // production sequence inherited from an absent receipt.
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
        println!(
            "cargo:rerun-if-changed={}",
            destination.join(format!("{stem}.json")).display()
        );
        std::fs::write(destination.join(format!("{stem}.json")), compat.to_json())
            .map_err(|e| e.to_string())?;
        std::fs::write(destination.join(format!("{stem}.plan")), &plan_bytes)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// The same directory-owned gate used by copying, including direct Cargo
// bakes. This tooling process does not link Unix filesystem code into a wasm
// consumer of `contract`; its Cargo cache is distinct from the calling build.
fn asset_cards(app: &Path) -> Result<Vec<Value>, String> {
    let gate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/filesystem.mjs");
    println!("cargo:rerun-if-changed={}", gate.display());
    for directory in ["assets", "deck", "gpu/shaders"] {
        println!("cargo:rerun-if-changed={}", app.join(directory).display());
    }
    let code = r#"
        import {pathToFileURL} from 'node:url';
        import {resolve} from 'node:path';
        import {createHash} from 'node:crypto';
        const {filesystem} = await import(pathToFileURL(process.argv[1]));
        const cards=[];
        for (const [source,prefix] of [['assets','assets'],['deck','deck'],['gpu/shaders','shaders']]) {
            const tree=filesystem({op:'tree',root:resolve(process.argv[2],source),optionalRoot:true});
            for (const [name,base64] of Object.entries(tree??{})) {
                const bytes=Buffer.from(base64,'base64');
                cards.push({name:`${prefix}/${name}`,sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length});
            }
        }
        process.stdout.write(JSON.stringify(cards.sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0)));
    "#;
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "-e", code])
        .arg(gate)
        .arg(app)
        .output()
        .map_err(|e| format!("bake asset gate: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "bake asset gate: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("bake asset inventory: {e}"))
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
        let cards = asset_cards(&root).unwrap();
        assert_eq!(
            cards,
            vec![
                json!({"name":"assets/image.png","sha256":hash(b"image"),"bytes":5}),
                json!({"name":"deck/nested/index.html","sha256":hash(b"deck"),"bytes":4}),
            ]
        );
        std::fs::write(root.join("outside"), b"must not be embedded").unwrap();
        std::os::unix::fs::symlink("../outside", root.join("assets/escape")).unwrap();
        assert!(asset_cards(&root).unwrap_err().contains("bake asset gate"));
        std::fs::remove_dir_all(root).unwrap();
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
                .contains("embedded plan bytes")
        );
        let (mut wrong, mut forged, _) = signed_fixture();
        forged["envelope"]["stream"]["seq"] = json!(41);
        assert!(apply_release(&mut wrong, &serde_json::to_vec(&forged).unwrap(), &plan).is_err());
        let (mut wrong, _, _) = signed_fixture();
        wrong.channel = "another".into();
        assert!(apply_release(&mut wrong, &serde_json::to_vec(&receipt).unwrap(), &plan).is_err());
    }
}
