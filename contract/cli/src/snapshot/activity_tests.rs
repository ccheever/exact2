use super::*;
use crate::{manifest::Activity, Manifest};
use serde_json::json;

fn activity() -> Activity {
    Activity {
        kind: "train".into(),
        component: "Activity".into(),
        state: "State".into(),
        preview: json!({"title":"Next train"}),
        title: None,
        description: None,
        push: true,
        frequent: false,
    }
}

fn source(leaf: &str) -> String {
    format!("shape State\n  title: string\nshape Size\n  width: number\n  height: number\ncomponent App\n  view\n    text \"app\"\ncomponent Activity\n  props\n    title: string\n  inject\n    snapshot_viewport: Size\n  view\n    view slot=\"lockScreen\"\n      {leaf}\n    view slot=\"compactLeading\"\n      text title\n    view slot=\"compactTrailing\"\n      text title\n    view slot=\"minimal\"\n      text title\n    view slot=\"expanded\"\n      view slot=\"leading\"\n        text title\n      view slot=\"trailing\"\n        text title\n      view slot=\"center\"\n        text title\n      view slot=\"bottom\"\n        text snapshot_viewport.width.toString()\n")
}

#[test]
fn one_activity_plan_has_all_placements_and_no_refresh() {
    let src = source("text title");
    let p = compile_activity(&src, &activity()).unwrap();
    assert!(p.snapshot_refresh.is_none() && p.actions.is_empty() && p.timers.is_empty());
    assert_eq!(
        p.slots
            .iter()
            .filter(|s| p.str(s.name) == STATE_SLOT)
            .count(),
        1
    );
    assert_eq!(
        p.activity_slots
            .iter()
            .map(|s| p.str(s.name))
            .collect::<Vec<_>>(),
        PLACEMENTS
    );
    assert_eq!(p.resources.len(), 1);
    assert_eq!(p.str(p.resources[0].source), "exactViewport");
    assert_eq!(
        exact_plan::Plan::decode_snapshot(&p.encode(), "train").unwrap(),
        p
    );
    assert_eq!(
        exact_plan::Plan::decode(&p.encode()),
        Err(exact_plan::PlanError::WrongEntry)
    );
    let mut bad = p.clone();
    bad.activity_slots[1].root = bad.activity_slots[0].root;
    assert!(exact_plan::Plan::decode_snapshot(&bad.encode(), "train").is_err());
    let mut bad = p.clone();
    bad.activity_slots.clear();
    assert!(exact_plan::Plan::decode_snapshot(&bad.encode(), "train").is_err());
    let mut entries = std::collections::BTreeMap::new();
    entries.insert(
        "train".into(),
        Entry {
            declaration: Declaration::Activity(activity()),
            plan: p,
        },
    );
    check_refreshes(&entries, &mut ()).unwrap();
    let app = crate::compile(&src).unwrap();
    assert_eq!(source_plan(&entries, &app).unwrap().encode(), app.encode());
}

#[test]
fn a_single_session_relayouts_slots_at_their_own_proposed_width() {
    use exact_kernel::{Kernel, Offer, PropId};
    use exact_runner::{Runner, Viewport};
    let src = source("text title").replace(
        "text snapshot_viewport.width.toString()",
        "text snapshot_viewport.width.toString() testId=\"region-width\"",
    );
    let plan = compile_activity(&src, &activity()).unwrap();
    let mut session = Runner::boot(
        plan,
        (),
        Kernel::with_monospace(),
        Viewport::sized(338., 200.),
        "/",
    )
    .unwrap();
    let root = session.roots()[0];
    for width in [22., 109., 165., 338.] {
        session.set_viewport(width, 200.).unwrap();
        session
            .kernel_mut()
            .compute_layout(root, Offer::definite(width as f32, 200.))
            .unwrap();
        let key = session.kernel().find_by_test_id("region-width")[0];
        assert_eq!(
            session
                .kernel()
                .node_by_key(key)
                .unwrap()
                .props
                .str(PropId::Text),
            Some(width.to_string().as_str())
        );
        let children = session.kernel().node(root).unwrap().children();
        assert_eq!(children.len(), 8);
        for child in children {
            let node = session.kernel().node(child).unwrap();
            let key = node.key;
            assert_eq!(
                session.kernel().laid_out_frame(key).unwrap().0.width,
                width as f32
            );
            assert!(session
                .kernel_mut()
                .fit_content_height(key)
                .unwrap()
                .is_finite());
        }
    }
}

#[test]
fn activity_slots_keep_snapshot_edits_and_require_known_unique_slots() {
    for (leaf, id, edit) in [
        (
            "text performanceNow().toString()",
            "snapshot-clock",
            "time datetime",
        ),
        ("button title press=go", "snapshot-handler", "link href"),
        ("input value=title", "snapshot-tag", "replace it"),
    ] {
        let src = source(leaf).replace(
            "  view\n    view slot=",
            "  action go\n    navigate(\"/\")\n  view\n    view slot=",
        );
        let errors = compile_activity(&src, &activity()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.id == id && e.message.contains(edit)),
            "{errors:?}"
        );
    }
    for (from, to, edit) in [
        ("slot=\"minimal\"", "slot=\"unknown\"", "unknown placement"),
        ("slot=\"minimal\"", "slot=\"lockScreen\"", "repeats"),
        (
            "    view slot=\"minimal\"\n      text title\n",
            "",
            "missing placement",
        ),
    ] {
        let errors =
            compile_activity(&source("text title").replace(from, to), &activity()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.id == "snapshot-placement" && e.message.contains(edit)),
            "{errors:?}"
        );
    }
}

#[test]
fn activity_manifest_refuses_unknown_keys_labels_flags_and_colliding_kinds() {
    let mut m = Manifest {
        json: json!({"live_activities":[{"kind":"train","component":"Activity","state":"State","preview":{}}]}),
        id: "test.activities".into(),
        name: "Activities".into(),
        declared: true,
    };
    assert!(m.live_activities().unwrap()[0].title.is_none());
    for (key, value, edit) in [
        ("refresh", json!("refresh"), "unknown; remove it"),
        ("title", json!("bad\nlabel"), "without control"),
        ("description", json!("\u{2028}"), "without control"),
        ("push", json!(1), "Boolean"),
    ] {
        let mut bad = m.clone();
        bad.json["live_activities"][0][key] = value;
        assert!(bad.live_activities().unwrap_err().contains(edit));
    }
    m.json["widgets"] = json!([{"kind":"TRAIN","component":"Card","props":"Props","families":["systemSmall"],"refresh":"refresh","preview":{}}]);
    assert!(m.live_activities().unwrap_err().contains("unique kind"));
}

#[test]
fn preview_shape_and_encoded_envelope_have_actionable_limits() {
    let mut a = activity();
    a.preview["unknown"] = json!(true);
    assert!(compile_activity(&source("text title"), &a)
        .unwrap_err()
        .iter()
        .any(|e| e.message.contains("supply preview")));
    a.preview = json!({"title":"🚆\"\\".repeat(1000)});
    let errors = compile_activity(&source("text title"), &a).unwrap_err();
    let message = &errors
        .iter()
        .find(|e| e.id == "snapshot-activity-size")
        .unwrap()
        .message;
    assert!(
        message.contains("bytes") && message.contains("4096") && message.contains("shorten"),
        "{message}"
    );
    let digest = "a".repeat(64);
    let base = payload_bytes("train", &digest, &digest, &json!({"title":""})).unwrap();
    assert_eq!(
        payload_bytes(
            "train",
            &digest,
            &digest,
            &json!({"title":"x".repeat(4096-base)})
        ),
        Ok(4096)
    );
    assert!(payload_bytes(
        "train",
        &digest,
        &digest,
        &json!({"title":"x".repeat(4097-base)})
    )
    .unwrap_err()
    .contains("4097 bytes"));
}

#[test]
fn caltrain_activities_bake_with_state_schemas_and_unchanged_app_plan() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/caltrain");
    let manifest = Manifest::read(&app).unwrap();
    let before = crate::compile_path(&app.join("app.contract"))
        .unwrap()
        .encode();
    let all = compile_entries(&app, &manifest).unwrap();
    assert_eq!(all.len(), 3);
    for a in manifest.live_activities().unwrap() {
        let plan = &all[&a.kind].plan;
        assert_eq!(plan.activity_slots.len(), 8);
        assert_eq!(shape_digest(plan).len(), 64);
        let schema = state_schema(plan);
        assert_eq!(schema["additionalProperties"], false);
        for key in a.preview.as_object().unwrap().keys() {
            assert!(schema["properties"].get(key).is_some());
        }
    }
    assert!(
        before
            == crate::compile_path(&app.join("app.contract"))
                .unwrap()
                .encode(),
        "compiling the activity entries must not change the app plan"
    );
}

#[test]
fn shape_digest_tracks_nested_order_types_enums_and_not_presentation() {
    let a = activity();
    let base = compile_activity(&source("text title"), &a).unwrap();
    let paint = compile_activity(&source("text title color=\"red\""), &a).unwrap();
    assert_eq!(shape_digest(&base), shape_digest(&paint));
    let mut a = a;
    a.preview = json!({"title":"Next train", "extra":[{"status":"ready", "value":null}]});
    let src = source("text title").replace("  title: string\nshape Size", "  title: string\n  extra: Nested[]\nshape Nested\n  status: enum<\"ready\", \"done\">\n  value: option<number>\nshape Size").replace("    title: string\n  inject", "    title: string\n    extra: Nested[]\n  inject");
    let nested = compile_activity(&src, &a).unwrap();
    let schema = state_schema(&nested);
    assert_eq!(
        schema["properties"]["extra"]["items"]["properties"]["status"]["enum"],
        json!(["ready", "done"])
    );
    assert_eq!(
        schema["properties"]["extra"]["items"]["properties"]["value"]["anyOf"][1]["type"],
        "null"
    );
    let reordered = compile_activity(
        &src.replace(
            "  status: enum<\"ready\", \"done\">\n  value: option<number>",
            "  value: option<number>\n  status: enum<\"ready\", \"done\">",
        ),
        &a,
    )
    .unwrap();
    assert_ne!(shape_digest(&nested), shape_digest(&reordered));
}
