//! Entry plans beside the app plan, in one compatibility cohort (LLP 1126 D2).

use serde_json::{json, Map, Value};
use std::path::Path;

pub(crate) fn emit(
    out: &Path,
    typescript: bool,
    entries: &std::collections::BTreeMap<String, contract::snapshot::Entry>,
) -> Result<(Vec<Value>, Value), String> {
    contract::snapshot::write_entries(entries, out)?;
    let mut assets = Vec::new();
    let mut plans = Map::new();
    for (kind, entry) in entries {
        let name = format!("plans/{kind}.plan");
        let bytes = entry.plan.encode();
        let card =
            json!({"name":name,"sha256":exact_update::sha256_hex(&bytes),"bytes":bytes.len()});
        plans.insert(kind.clone(), card.clone());
        assets.push(card);
    }
    for name in ["plans/widgets.json", "plans/activities.json"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            entries
                .iter()
                .filter(|(_, e)| !e.plan.activity_slots.is_empty())
                .flat_map(|(kind, _)| {
                    [
                        format!("activities/{kind}.schema.json"),
                        format!("activities/{kind}.d.ts"),
                    ]
                }),
        )
    {
        if !out.join(&name).exists() {
            continue;
        }
        let bytes = std::fs::read(out.join(&name)).map_err(|e| e.to_string())?;
        assets.push(
            json!({"name":name, "sha256":exact_update::sha256_hex(&bytes), "bytes":bytes.len()}),
        );
    }
    if typescript && !entries.is_empty() {
        std::fs::create_dir_all(out.join("typescript")).map_err(|e| e.to_string())?;
        for file in ["app.hbc", "app.module.json"] {
            let bytes = std::fs::read(out.join(file)).map_err(|e| e.to_string())?;
            let name = format!("typescript/{file}");
            std::fs::write(out.join(&name), &bytes).map_err(|e| e.to_string())?;
            assets.push(
                json!({"name":name,"sha256":exact_update::sha256_hex(&bytes),"bytes":bytes.len()}),
            );
        }
    }
    Ok((assets, Value::Object(plans)))
}

pub(crate) fn copy(out: &Path, destination: &Path, assets: &[Value]) -> Result<(), String> {
    for tree in ["plans", "typescript", "activities"] {
        let target = destination.join(tree);
        if target.exists() {
            std::fs::remove_dir_all(target).map_err(|e| e.to_string())?;
        }
    }
    for card in assets {
        let Some(name) = card["name"].as_str().filter(|n| {
            n.starts_with("plans/") || n.starts_with("typescript/") || n.starts_with("activities/")
        }) else {
            continue;
        };
        std::fs::create_dir_all(destination.join(name).parent().unwrap())
            .map_err(|e| e.to_string())?;
        std::fs::copy(out.join(name), destination.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::Manifest;
    use exact_runner::{DataError, DataSource, Value as DataValue};

    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    struct Refresh;
    impl DataSource for Refresh {
        fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
            assert_eq!(name, "refresh");
            Ok(DataValue::list(vec![DataValue::record(vec![
                DataValue::Number(0.0),
                DataValue::record(vec![DataValue::str("Hello")]),
            ])]))
        }
    }

    #[test]
    fn ios_entries_carry_the_exact_typescript_pair() {
        let dir =
            TempDir(std::env::temp_dir().join(format!("exact-widget-pair-{}", std::process::id())));
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.0.join("app.contract"), "shape Props\n  title: string\ncomponent Card\n  props\n    title: string\n  view\n    text title\n").unwrap();
        std::fs::write(dir.0.join("app.json"), r#"{"app":{"id":"test.widget","name":"Test"},"widgets":[{"kind":"card","component":"Card","props":"Props","families":["systemSmall"],"refresh":"refresh","preview":{"title":"Hello"}}]}"#).unwrap();
        let entries = contract::snapshot::entries(&dir.0).unwrap();
        let out = dir.0.join("out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("app.hbc"), b"pinned bytes").unwrap();
        std::fs::write(out.join("app.module.json"), b"pinned receipt").unwrap();
        let (assets, _) = emit(&out, true, &entries).unwrap();
        let copied = dir.0.join("copy");
        copy(&out, &copied, &assets).unwrap();
        // The entry's plan, the gallery's widgets.json and the module pair.
        assert_eq!(assets.len(), 4);
        for name in ["app.hbc", "app.module.json"] {
            assert_eq!(
                std::fs::read(out.join(name)).unwrap(),
                std::fs::read(copied.join("typescript").join(name)).unwrap()
            );
        }
        for missing in ["app.hbc", "app.module.json"] {
            let bytes = std::fs::read(out.join(missing)).unwrap();
            std::fs::remove_file(out.join(missing)).unwrap();
            assert!(
                emit(&out, true, &entries).is_err(),
                "an incomplete TypeScript pair must fail the bake: {missing}"
            );
            std::fs::write(out.join(missing), bytes).unwrap();
        }
        let (assets, _) = emit(&out, false, &entries).unwrap();
        copy(&out, &copied, &assets).unwrap();
        assert_eq!(assets.len(), 2);
        assert!(!copied.join("typescript").exists());
    }

    #[test]
    fn receipt_keeps_app_bytes_and_removes_obsolete_entries() {
        let dir =
            TempDir(std::env::temp_dir().join(format!("exact-widget-bake-{}", std::process::id())));
        let out = dir.0.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let source = "shape Props\n  title: string\ncomponent App\n  view\n    text \"app\"\ncomponent Card\n  props\n    title: string\n  view\n    text title\n";
        std::fs::write(dir.0.join("app.contract"), source).unwrap();
        let widgets = json!([{"kind":"card","component":"Card","props":"Props","families":["systemSmall"],"refresh":"refresh","preview":{"title":"Hello"}}]);
        let mut before = None;
        for enabled in [false, true, false] {
            let mut json = json!({"app":{"id":"test.widget","name":"Test"}});
            if enabled {
                json["widgets"] = widgets.clone();
            }
            std::fs::write(dir.0.join("app.json"), json.to_string()).unwrap();
            let manifest = Manifest::read(&dir.0).unwrap();
            let plan = contract::compile_path(&dir.0.join("app.contract")).unwrap();
            let entries = contract::snapshot::entries(&dir.0).unwrap();
            let baked = contract::snapshot::bake_app(&entries, plan, Refresh, Refresh).unwrap();
            std::fs::write(out.join("app.plan"), baked.encode()).unwrap();
            let mut compat = crate::Compat {
                id: "a".repeat(32),
                inputs: json!({"trust":"development"}),
                channel: "development".into(),
                origin: None,
                activate: "next-launch".into(),
                target: String::new(),
                embedded: Value::Null,
                reach: Value::Null,
            };
            crate::receipt::emit(
                &mut compat,
                "development",
                &dir.0,
                ("web", "wasm32-unknown-unknown"),
                &manifest,
                &out,
                &entries,
            )
            .unwrap();
            let bytes = std::fs::read(out.join("app.plan")).unwrap();
            if let Some(before) = &before {
                assert_eq!(&bytes, before);
            } else {
                before = Some(bytes);
            }
            if enabled {
                let bytes = std::fs::read(out.join("plans/card.plan")).unwrap();
                let entry = exact_plan::Plan::decode_snapshot(&bytes, "card").unwrap();
                assert_eq!(
                    entry.str(entry.source(entry.snapshot_refresh.unwrap()).name),
                    "refresh"
                );
                assert_eq!(
                    compat.embedded["plans"]["card"]["sha256"],
                    exact_update::sha256_hex(&bytes)
                );
                assert_eq!(compat.embedded["assets"].as_array().unwrap().len(), 2);
                let registry: Value =
                    serde_json::from_slice(&std::fs::read(out.join("plans/widgets.json")).unwrap())
                        .unwrap();
                assert_eq!(registry[0]["kind"], "card");
                assert_eq!(registry[0]["families"], json!(["systemSmall"]));
            } else {
                assert!(compat.embedded.get("plans").is_none());
                assert!(!out.join("plans").exists());
            }
        }
    }

    #[test]
    fn activity_only_bake_copies_schemas_and_removes_obsolete_state_artifacts() {
        let dir = TempDir(
            std::env::temp_dir().join(format!("exact-activity-bake-{}", std::process::id())),
        );
        let out = dir.0.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/caltrain");
        let mut manifest = Manifest::read(&app).unwrap();
        manifest.json.as_object_mut().unwrap().remove("widgets");
        let entries = contract::snapshot::compile_entries(&app, &manifest).unwrap();
        assert_eq!(entries.len(), 2);
        let (assets, plans) = emit(&out, false, &entries).unwrap();
        let destination = dir.0.join("copy");
        copy(&out, &destination, &assets).unwrap();
        for a in manifest.live_activities().unwrap() {
            let plan = &entries[&a.kind].plan;
            let bytes = std::fs::read(out.join(format!("plans/{}.plan", a.kind))).unwrap();
            assert_eq!(plans[&a.kind]["sha256"], exact_update::sha256_hex(&bytes));
            assert_eq!(
                exact_plan::Plan::decode_snapshot(&bytes, &a.kind).unwrap(),
                *plan
            );
            for extension in ["schema.json", "d.ts"] {
                let name = format!("activities/{}.{extension}", a.kind);
                let card = assets.iter().find(|c| c["name"] == name).unwrap();
                let bytes = std::fs::read(destination.join(&name)).unwrap();
                assert_eq!(card["sha256"], exact_update::sha256_hex(&bytes));
                if extension == "schema.json" {
                    assert_eq!(
                        serde_json::from_slice::<Value>(&bytes).unwrap(),
                        contract::snapshot::state_schema(plan)
                    );
                } else {
                    let ts = String::from_utf8(bytes).unwrap();
                    assert!(
                        ts.contains("export type State = {")
                            && ts.contains(&contract::snapshot::shape_digest(plan))
                    );
                }
            }
        }
        let registry: Value =
            serde_json::from_slice(&std::fs::read(out.join("plans/activities.json")).unwrap())
                .unwrap();
        assert_eq!(registry.as_array().unwrap().len(), 2);
        assert_eq!(registry[0]["slots"].as_array().unwrap().len(), 8);
        let (empty, _) = emit(&out, false, &Default::default()).unwrap();
        copy(&out, &destination, &empty).unwrap();
        for tree in ["activities", "plans"] {
            assert!(!out.join(tree).exists() && !destination.join(tree).exists());
        }
    }
}
