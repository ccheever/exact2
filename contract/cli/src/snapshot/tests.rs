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
    for app_only in ["nowMs", "ticker", "tick"] {
        assert!(!entry.strings.iter().any(|name| name == app_only));
    }
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
    assert_eq!(plan.slots.len(), 2);
    let props = plan
        .slots
        .iter()
        .position(|s| plan.str(s.name) == "snapshot_props")
        .unwrap();
    assert_ne!(plan.router.unwrap().0 as usize, props);
    assert_eq!(
        plan.str(plan.slots[plan.router.unwrap().0 as usize].name),
        "nav"
    );
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
    let temp = TempDir::new();
    let dir = &temp.0;
    let path = dir.join("app.contract");
    std::fs::write(&path, source("    text title", "", "")).unwrap();
    let mut json = json!({"app":{"id":"test.snapshot","name":"Test"}});
    std::fs::write(dir.join("app.json"), json.to_string()).unwrap();
    let before = crate::compile_path(&path).unwrap().encode();
    json["widgets"] = json!([{"kind":"next-trains","families":["systemSmall"],"component":"Card","props":"Props","refresh":"refreshCard","preview":{"title":"Next train"}}]);
    std::fs::write(dir.join("app.json"), json.to_string()).unwrap();
    let manifest = Manifest::read(dir).unwrap();
    let entries = compile_widgets(dir, &manifest).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries["next-trains"].plan.app_id, "test.snapshot");
    assert_eq!(before, crate::compile_path(&path).unwrap().encode());
}

#[test]
fn caltrain_entry_reaches_only_its_departure_card() {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/caltrain");
    let manifest = Manifest::read(&app).unwrap();
    let entries = compile_widgets(&app, &manifest).unwrap();
    let entry = &entries["next-trains"].plan;
    assert!(entry.strings.iter().any(|s| s == "Mountain View"));
    for app_only in ["board-north", "seededEpoch", "change-station", "Caltrain"] {
        assert!(!entry.strings.iter().any(|s| s == app_only), "{app_only}");
    }
    assert_eq!(entry.resources.len(), 0);
    assert_eq!(entry.actions.len(), 0);
    assert_eq!(entry.slots.len(), 1);
}

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-snapshot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn schema_admission_names_real_props_and_styles() {
    for name in column("styles", "field") {
        assert!(StyleId::from_name(&name).is_some(), "{name}");
    }
    for name in column("props", "name") {
        assert!(PropId::from_name(&name).is_some(), "{name}");
    }
    assert!(admits_prop(PropId::AccessibilityElementsHidden));
    assert!(!admits_prop(PropId::NativeViewModuleName));
}

#[test]
fn constants_accessibility_and_conditional_classes_are_admitted() {
    let src = source(
        "    column
      text nowMs.toString() class=(nowMs > 0 ? Emph : Plain) aria-description=\"Clock label\" aria-level=2 lang=\"en\" title=\"Label\" aria-live=\"polite\"
      image \"symbol:sf/train\" aria-hidden=true",
        "style Emph
  color=\"red\"
style Plain
  color=\"blue\"
style Unused
  filter=\"blur(2px)\"",
        "  state nowMs = 42\n",
    );
    let plan = compile(&src, &widget()).unwrap();
    assert!(plan.strings.iter().any(|s| s == "Clock label"));
    let bad = src.replace("color=\"blue\"", "filter=\"blur(2px)\"");
    assert!(compile(&bad, &widget())
        .unwrap_err()
        .iter()
        .any(|e| e.id == "snapshot-row"));
    let bad = src.replace("class=(nowMs > 0 ? Emph : Plain)", "class=\"Emph\"");
    assert!(compile(&bad, &widget())
        .unwrap_err()
        .iter()
        .any(|e| e.id == "snapshot-class"));
}

#[test]
fn images_are_local_and_clock_reads_are_refused_even_on_live_leaves() {
    for image in [
        "\"assets/train.png\"",
        "\"app:/widgets/train.png\"",
        "\"symbol:sf/train\"",
        "`app:/widgets/${title}.png`",
    ] {
        compile(&source(&format!("    image {image}"), "", ""), &widget()).unwrap();
    }
    for image in [
        "\"https://example.com/a.png\"",
        "\"http://example.com/a.png\"",
        "\"//example.com/a.png\"",
        "title",
    ] {
        let errors =
            compile(&source(&format!("    image {image}"), "", ""), &widget()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.id == "snapshot-request" && e.message.contains("refresh must download")),
            "{errors:?}"
        );
    }
    for tag in ["text", "time", "progress"] {
        let errors = compile(
            &source(&format!("    {tag} value=performanceNow()"), "", ""),
            &widget(),
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|e| e.id == "snapshot-clock"),
            "{errors:?}"
        );
    }
}

#[test]
fn fonts_keep_only_reachable_literal_families() {
    let temp = TempDir::new();
    std::fs::create_dir(temp.0.join("assets")).unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/fixtures/fonts/assets/DejaVuSans.ttf"),
        temp.0.join("assets/font.ttf"),
    )
    .unwrap();
    let manifest = Manifest {
        id: "test.fonts".into(),
        name: "Test".into(),
        declared: true,
        json: json!({"widgets":[{"kind":"next-trains","component":"Card","props":"Props","refresh":"refreshCard","families":["systemSmall"],"preview":{"title":"system-ui"}}]}),
    };
    let fonts = "font \"A\" = \"assets/font.ttf\"\nfont \"B\" = \"assets/font.ttf\"\nfont \"Unused\" = \"assets/missing.ttf\"";
    for (family, count) in [("(title == \"a\" ? \"A\" : \"B\")", 2), ("title", 0)] {
        std::fs::write(
            temp.0.join("app.contract"),
            source(&format!("    text title font-family={family}"), fonts, ""),
        )
        .unwrap();
        let entries = compile_widgets(&temp.0, &manifest).unwrap();
        assert_eq!(entries["next-trains"].plan.faces.len(), count);
    }
}

#[test]
fn refresh_sources_are_typed_and_data_changes_fail_the_bake() {
    use exact_plan::{TypeKind, Value};
    use exact_runner::{DataError, DataSource};
    struct Data {
        broken_shape: bool,
    }
    impl DataSource for Data {
        fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
            if name != "refreshCard" {
                return Err(DataError::UnknownSource(name.into()));
            }
            assert_eq!(args, &[Value::Number(0.0)]);
            let mut fields = vec![
                Value::Number(0.0),
                Value::record(vec![Value::str("Next train")]),
            ];
            if self.broken_shape {
                fields.swap(0, 1);
            }
            Ok(Value::list(vec![Value::record(fields)]))
        }
    }
    let temp = TempDir::new();
    let path = temp.0.join("app.contract");
    std::fs::write(&path, source("    text title", "", "")).unwrap();
    let mut manifest = json!({"app":{"id":"test.refresh","name":"Test"},"widgets":[{"kind":"next-trains","families":["systemSmall"],"component":"Card","props":"Props","refresh":"refreshCard","preview":{"title":"Next train"}}]});
    std::fs::write(temp.0.join("app.json"), manifest.to_string()).unwrap();
    let app = crate::compile_path(&path).unwrap();
    let plan =
        &compile_widgets(&temp.0, &Manifest::read(&temp.0).unwrap()).unwrap()["next-trains"].plan;
    let source = plan.source(plan.snapshot_refresh.unwrap());
    assert_eq!(plan.str(source.name), "refreshCard");
    assert_eq!(source.params.len, 1);
    assert_eq!(
        plan.type_(plan.source_param(source.params.iter().next().unwrap()).ty)
            .kind,
        TypeKind::Number
    );
    let result = plan.type_(source.ty);
    assert_eq!(result.kind, TypeKind::List);
    let fields: Vec<_> = plan
        .type_(result.elem.unwrap())
        .fields
        .iter()
        .map(|id| plan.field(id))
        .collect();
    assert_eq!(
        fields.iter().map(|f| plan.str(f.name)).collect::<Vec<_>>(),
        ["date", "props"]
    );
    assert_eq!(
        fields[1].ty,
        plan.slots
            .iter()
            .find(|s| plan.str(s.name) == PROPS_SLOT)
            .unwrap()
            .ty
    );
    assert!(plan.resources.is_empty());
    let entries = super::entries(&temp.0).unwrap();
    let declarations = crate::typescript(&source_plan(&entries, &app).unwrap()).unwrap();
    assert!(declarations.contains("refreshCard"));
    let baked = bake_app(
        &entries,
        app.clone(),
        Data {
            broken_shape: false,
        },
        Data {
            broken_shape: false,
        },
    )
    .unwrap();
    assert_eq!(
        baked.encode(),
        crate::bake(
            app.clone(),
            Data {
                broken_shape: false
            }
        )
        .unwrap()
        .encode()
    );
    let err = bake_app(
        &entries,
        app.clone(),
        Data {
            broken_shape: false,
        },
        Data { broken_shape: true },
    )
    .unwrap_err();
    assert!(err.to_string().contains("field order"), "{err}");
    manifest["widgets"][0]["refresh"] = json!("renamedRefresh");
    std::fs::write(temp.0.join("app.json"), manifest.to_string()).unwrap();
    let err = bake_app(
        &super::entries(&temp.0).unwrap(),
        app,
        Data {
            broken_shape: false,
        },
        Data {
            broken_shape: false,
        },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("renamedRefresh") && err.to_string().contains("implement"),
        "{err}"
    );
}

#[test]
fn snapshot_refusals_accumulate_and_the_props_slot_is_reserved() {
    let src = source(
        "    text title class=(title + title) filter=\"blur(2px)\"",
        "",
        "  state snapshot_props = performanceNow()\n",
    );
    let errors = compile(&src, &widget()).unwrap_err();
    for id in [
        "snapshot-class",
        "snapshot-row",
        "snapshot-clock",
        "snapshot-props-name",
    ] {
        assert!(errors.iter().any(|e| e.id == id), "{id}: {errors:?}");
    }
    for declaration in ["state", "derive", "provide\n   "] {
        let src = source("    Child()", &format!("component Child\n  {declaration} snapshot_props = 42\n  view\n    text snapshot_props.toString()"), "");
        let errors = compile(&src, &widget()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.id == "snapshot-props-name" && e.message.contains("rename")),
            "{errors:?}"
        );
    }
}

#[test]
fn live_times_and_links_must_own_their_paragraph() {
    for format in ["relative", "timer", "countdown"] {
        let time = format!("time datetime=120000 format=\"{format}\"");
        let child = format!("component Tick\n  view\n    {time}");
        for body in [
            format!("    text \"Leaves \"\n      {time}"),
            "    text\n      text \"Leaves \"\n      Tick()".into(),
            "    text \"Leaves \"\n      if true\n        Tick()".into(),
            "    text\n      for n in [1, 2] key=n\n        Tick()".into(),
            "    text \"Leaves \"\n      match some(1)\n        case some(n)\n          Tick()\n        case none\n          text \"none\"".into(),
        ] {
            let errors = compile(&source(&body, &child, ""), &widget()).unwrap_err();
            assert!(errors.iter().any(|e| e.id == "snapshot-inline-live-time" && e.message == "make the live time its own text, so the widget can reserve its width"), "{body}: {errors:?}");
        }
        for body in [
            format!("    {time}"),
            "    text\n      Tick()".into(),
            "    text\n      if true\n        Tick()\n      else\n        text \"Unavailable\""
                .into(),
            "    column\n      for n in [1, 2] key=n\n        Tick()".into(),
        ] {
            let result = compile(&source(&body, &child, ""), &widget());
            assert!(result.is_ok(), "{body}: {result:?}");
        }
    }
    for body in [
        "    text \"Departure \"\n      time datetime=120000 format=\"date-full\"",
        "    text \"Departure \"\n      time datetime=120000",
        "    progress start=0 end=120000",
        "    link href=\"/departures\"\n      text title",
        "    text title href=\"/departures\"",
    ] {
        let result = compile(&source(body, "", ""), &widget());
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    let child = "component Linked\n  view\n    text \"departures\" href=\"/departures\"";
    for body in [
        "    text \"See \"\n      text title href=\"/departures\"",
        "    text\n      text \"See \"\n      Linked()",
        "    text \"See \"\n      if true\n        Linked()",
        "    text\n      for n in [1, 2] key=n\n        Linked()",
    ] {
        let errors = compile(&source(body, child, ""), &widget()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.id == "snapshot-inline-link"
                    && e.message == "make the link its own element"),
            "{body}: {errors:?}"
        );
    }
}

#[test]
fn refresh_checks_distinguish_pending_sources_and_result_mismatches() {
    use exact_plan::Value;
    use exact_runner::{Answer, DataError, DataSource, Request, Store};
    struct Data(Result<Answer, DataError>, bool);
    impl DataSource for Data {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("use the asynchronous seam")
        }
        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            self.0.clone()
        }
        fn discard(&mut self, token: u64) {
            assert_eq!(token, 7);
            self.1 = true;
        }
    }
    let plan = compile(&source("    text title", "", ""), &widget()).unwrap();
    let entries = std::collections::BTreeMap::from([(
        "next-trains".into(),
        Entry {
            widget: widget(),
            plan,
        },
    )]);
    let mut pending = Data(Ok(Answer::Later(Request::continuation(7))), false);
    check_refreshes(&entries, &mut pending).unwrap();
    assert!(pending.1);
    check_refreshes(
        &entries,
        &mut Data(
            Err(DataError::Unavailable(
                "seeded away; checked by declarations".into(),
            )),
            false,
        ),
    )
    .unwrap();
    for (value, message) in [
        (Value::str("oops"), "result expected List, got string"),
        (
            Value::list(vec![Value::record(vec![Value::Number(0.)])]),
            "result[0] is missing field `props`",
        ),
        (
            Value::list(vec![Value::record(vec![
                Value::Number(0.),
                Value::record(vec![Value::Number(42.)]),
            ])]),
            "result[0].props.title expected String, got number",
        ),
    ] {
        let error =
            check_refreshes(&entries, &mut Data(Ok(Answer::Now(value)), false)).unwrap_err();
        assert!(
            error.contains(message) && !error.contains("field order"),
            "{error}"
        );
    }
}
