//! Entry plans beside the app plan, in one compatibility cohort (LLP 1126 D2).

use contract::Manifest;
use serde_json::{json, Map, Value};
use std::path::Path;

pub(crate) fn emit(
    app: &Path,
    out: &Path,
    manifest: &Manifest,
) -> Result<(Vec<Value>, Value), String> {
    let entries = contract::snapshot::compile_widgets(app, manifest).map_err(|all| {
        all.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let directory = out.join("plans");
    // A removed entry must not survive an incremental bake.
    if directory.exists() {
        std::fs::remove_dir_all(&directory).map_err(|e| e.to_string())?;
    }
    if entries.is_empty() {
        return Ok((vec![], Value::Object(Map::new())));
    }
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut assets = Vec::new();
    let mut plans = Map::new();
    let mut widgets = Map::new();
    for (kind, entry) in entries {
        let name = format!("plans/{kind}.plan");
        let bytes = entry.plan.encode();
        std::fs::write(out.join(&name), &bytes).map_err(|e| e.to_string())?;
        let card =
            json!({"name":name,"sha256":exact_update::sha256_hex(&bytes),"bytes":bytes.len()});
        plans.insert(kind.clone(), card.clone());
        assets.push(card);
        widgets.insert(kind, json!({"families":entry.widget.families,"component":entry.widget.component,
            "props":entry.widget.props,"refresh":entry.widget.refresh,"preview":entry.widget.preview}));
    }
    let bytes = serde_json::to_vec(&widgets).map_err(|e| e.to_string())?;
    std::fs::write(directory.join("widgets.json"), &bytes).map_err(|e| e.to_string())?;
    assets.push(json!({"name":"plans/widgets.json","sha256":exact_update::sha256_hex(&bytes),"bytes":bytes.len()}));
    Ok((assets, Value::Object(plans)))
}

pub(crate) fn copy(out: &Path, destination: &Path, assets: &[Value]) -> Result<(), String> {
    let target = destination.join("plans");
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    }
    for card in assets {
        let Some(name) = card["name"].as_str().filter(|n| n.starts_with("plans/")) else {
            continue;
        };
        std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        std::fs::copy(out.join(name), destination.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bake_keeps_the_app_bytes_and_removes_obsolete_entries() {
        let dir = std::env::temp_dir().join(format!("exact-widget-bake-{}", std::process::id()));
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let source = "shape Props\n  title: string\ncomponent App\n  view\n    text \"app\"\ncomponent Card\n  props\n    title: string\n  view\n    text title\n";
        std::fs::write(dir.join("app.contract"), source).unwrap();
        let manifest = Manifest {
            id: "test.widget".into(),
            name: "Test".into(),
            declared: true,
            json: json!({"widgets":[{"kind":"card","component":"Card","props":"Props","families":["systemSmall"],"refresh":"refresh","preview":{"title":"Hello"}}]}),
        };
        let app = contract::compile(source).unwrap().encode();
        std::fs::write(out.join("app.plan"), &app).unwrap();
        let (assets, plans) = emit(&dir, &out, &manifest).unwrap();
        assert_eq!(assets.len(), 2);
        let bytes = std::fs::read(out.join("plans/card.plan")).unwrap();
        assert_eq!(plans["card"]["sha256"], exact_update::sha256_hex(&bytes));
        assert!(exact_plan::Plan::decode_snapshot(&bytes, "card").is_ok());
        assert_eq!(std::fs::read(out.join("app.plan")).unwrap(), app);
        let (assets, plans) = emit(
            &dir,
            &out,
            &Manifest {
                json: json!({}),
                ..manifest
            },
        )
        .unwrap();
        assert!(assets.is_empty() && plans.as_object().unwrap().is_empty());
        assert!(!out.join("plans").exists());
        assert_eq!(std::fs::read(out.join("app.plan")).unwrap(), app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
