use super::*;
use crate::{Manifest, Widget};
use exact_plan::Plan;
use serde_json::json;

fn widget() -> Widget {
    Widget {
        kind: "next-trains".into(),
        families: vec!["systemSmall".into()],
        component: "Card".into(),
        props: "Props".into(),
        refresh: "refreshCard".into(),
        preview: json!({"title":"Next train"}),
    }
}

fn source(view: &str, declarations: &str, members: &str) -> String {
    format!("shape Props\n  title: string\n{declarations}\ncomponent App\n  state nowMs = 0\n  action tick\n    nowMs = performanceNow()\n  task ticker mount\n    every(1000, tick)\n  view\n    scroll height=100\n      text nowMs.toString()\ncomponent Card\n  props\n    title: string\n{members}  view\n{view}\n")
}

#[test]
fn entry_is_small_pure_and_gated_at_decode() {
    let src = source("    column\n      text title", "", "");
    let app = crate::compile(&src).unwrap();
    let entry = compile(&src, &widget()).unwrap();
    assert_eq!(entry.nodes.len(), 2);
    assert_eq!(entry.slots.len(), 1);
    assert!(entry.resources.is_empty() && entry.actions.is_empty() && entry.timers.is_empty());
    assert!(entry.encode().len() < app.encode().len());
    assert_eq!(
        Plan::decode(&entry.encode()),
        Err(exact_plan::PlanError::WrongEntry)
    );
    assert_eq!(
        Plan::decode_static(Box::leak(entry.encode().into_boxed_slice())),
        Err(exact_plan::PlanError::WrongEntry)
    );
    assert_eq!(
        Plan::decode_snapshot(&app.encode(), "next-trains"),
        Err(exact_plan::PlanError::WrongEntry)
    );
    assert_eq!(
        Plan::decode_snapshot(&entry.encode(), "other"),
        Err(exact_plan::PlanError::WrongEntry)
    );
    assert!(Plan::decode_snapshot(&entry.encode(), "").is_err());
    assert_eq!(
        Plan::decode_snapshot(&entry.encode(), "next-trains").unwrap(),
        entry
    );
}

#[test]
fn entry_shares_route_urls_and_only_reachable_types_and_helpers() {
    let src = source(
        "    link href=path(\"home\")\n      text label(Shared(value=title))",
        "routes nav\n  tab home \"/\" render=build pages=appPages()\nshape Shared\n  value: string\nshape Unused\n  appOnly: number\nfn label(item: Shared): string = item.value\nfn unused(): string = \"app-only-helper\"",
        "  action unusedAction\n    log(unused())\n",
    );
    let plan = compile(&src, &widget()).unwrap();
    assert_eq!(plan.nodes.len(), 2);
    assert!(plan.resources.is_empty() && plan.actions.is_empty());
    assert!(plan.strings.iter().any(|s| s == "Shared"));
    for name in ["Unused", "appOnly", "app-only-helper", "appPages"] {
        assert!(!plan.strings.iter().any(|s| s == name), "{name}");
    }
}

#[test]
fn refusals_name_an_edit_including_shared_styles_and_functions() {
    for (view, decl, members, id, edit) in [
        (
            "    text title filter=\"blur(2px)\"",
            "",
            "",
            "snapshot-row",
            "remove",
        ),
        (
            "    text title class=Moving",
            "style Moving\n  animation=\"fade 1s\"",
            "",
            "snapshot-row",
            "remove",
        ),
        (
            "    text performanceNow().toString()",
            "",
            "",
            "snapshot-clock",
            "time",
        ),
        (
            "    text tickText()",
            "fn tickText(): string = performanceNow().toString()",
            "",
            "snapshot-clock",
            "progress",
        ),
        (
            "    text nowMs.toString()",
            "",
            "  state nowMs = 42\n",
            "snapshot-clock",
            "time",
        ),
        (
            "    button press=open\n      text title",
            "",
            "  action open\n    navigate(\"/\")\n",
            "snapshot-handler",
            "link",
        ),
        ("    native-card", "", "", "snapshot-native", "replace"),
        (
            "    scroll\n      text title",
            "",
            "",
            "snapshot-tag",
            "replace",
        ),
        (
            "    text title position=\"sticky\"",
            "",
            "",
            "snapshot-value",
            "write",
        ),
        (
            "    text title overflow=\"auto\"",
            "",
            "",
            "snapshot-value",
            "write",
        ),
        (
            "    text title",
            "",
            "  task ticker mount\n    every(1000, tick)\n  action tick\n    title = title\n",
            "snapshot-timer",
            "refresh",
        ),
        (
            "    text title",
            "",
            "  resource reply = fetchTitle() as shape string\n",
            "snapshot-request",
            "refresh",
        ),
    ] {
        let errors = compile(&source(view, decl, members), &widget()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.id == id && e.message.contains(edit)),
            "{id}: {errors:?}"
        );
    }
}

#[test]
fn manifest_and_preview_are_checked_and_no_widgets_is_unchanged() {
    let empty = Manifest {
        json: json!({}),
        id: "test.app".into(),
        name: "Test".into(),
        declared: true,
    };
    assert!(empty.widgets().unwrap().is_empty());
    for value in [
        json!({"live_activities":[]}),
        json!({"widgets":null}),
        json!({"widgets":[{"kind":"../escape"}]}),
    ] {
        assert!(Manifest {
            json: value,
            ..empty.clone()
        }
        .widgets()
        .is_err());
    }
    let src = source("    text title", "", "");
    let mut w = widget();
    w.preview = json!({"title":7});
    assert!(compile(&src, &w).unwrap_err()[0]
        .message
        .contains("preview"));
    w = widget();
    w.props = "Missing".into();
    assert!(compile(&src, &w).unwrap_err()[0]
        .message
        .contains("declare props shape"));
}

#[test]
fn manifest_entries_do_not_change_the_app_plan() {
    let dir = std::env::temp_dir().join(format!("exact-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, source("    text title", "", "")).unwrap();
    let mut json = json!({"app":{"id":"test.snapshot","name":"Test"}});
    std::fs::write(dir.join("app.json"), json.to_string()).unwrap();
    let before = crate::compile_path(&path).unwrap().encode();
    json["widgets"] = json!([{"kind":"next-trains","families":["systemSmall"],"component":"Card","props":"Props","refresh":"refreshCard","preview":{"title":"Next train"}}]);
    std::fs::write(dir.join("app.json"), json.to_string()).unwrap();
    let manifest = Manifest::read(&dir).unwrap();
    let entries = compile_widgets(&dir, &manifest).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries["next-trains"].plan.app_id, "test.snapshot");
    assert_eq!(before, crate::compile_path(&path).unwrap().encode());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn caltrain_entry_reaches_only_its_departure_card() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/caltrain");
    let manifest = Manifest::read(&app).unwrap();
    let entries = compile_widgets(&app, &manifest).unwrap();
    let entry = &entries["next-trains"].plan;
    assert_eq!(entry.nodes.len(), 5);
    assert_eq!(entry.resources.len(), 0);
    assert_eq!(entry.actions.len(), 0);
    assert_eq!(entry.slots.len(), 1);
    println!(
        "Caltrain widget: {} bytes, {} nodes, {} slots",
        entry.encode().len(),
        entry.nodes.len(),
        entry.slots.len()
    );
}
