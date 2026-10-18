//! Shared editor declarations, serialization and command boundaries (LLP 1114.007).
use exact_kernel::{Kernel, PropId, StyleId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};
use serde_json::Value as Json;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}
fn boot(source: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn prop(runner: &Runner<NoData>, test: &str, prop: PropId) -> Json {
    let key = runner.kernel().find_by_test_id(test)[0];
    serde_json::from_str(
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(prop)
            .unwrap(),
    )
    .unwrap()
}
const RECORD: &str = r##"TextDecorations(value=draft, spans=[TextDecorationSpan(name="mention", start=0, end=6)], blocks=[TextDecorationBlock(name="heading", start=0, end=6, marker="checkbox:off")], hidden=[TextDecorationHidden(start=0, end=1, reveal=some(TextDecorationRange(start=0, end=6)))], atoms=[TextDecorationAtom(name="mention", start=1, end=6, label="Alice")])"##;

#[test]
fn source_identity_drives_the_named_record_and_declarations_expand_css_rows() {
    let source = format!(
        r##"decoration mention color="#1083fe"
decoration heading font-size="1.25rem" font-weight=600 line-height=1.3 margin="1.25rem 0 0.5rem"
component App
  state draft = "@alice"
  derive marks = {RECORD}
  action replace
    draft = "@carol"
  view
    column
      textarea value=draft decorations=marks testId="editor"
      button press=replace testId="replace"
        text "Replace"
"##
    );
    let mut runner = boot(&source);
    let first = prop(&runner, "editor", PropId::Decorations);
    assert_eq!(first["value"], "@alice");
    assert_eq!(
        first["spans"][0],
        serde_json::json!({"name":"mention","start":0,"end":6})
    );
    assert_eq!(first["blocks"][0]["marker"], "checkbox:off");
    assert_eq!(first["hidden"][0]["reveal"]["end"], 6);
    assert_eq!(first["atoms"][0]["label"], "Alice");
    let styles = prop(&runner, "editor", PropId::DecorationStyles);
    assert_eq!(
        styles["mention"][0],
        serde_json::json!([StyleId::TextColor as u16, "#1083fe"])
    );
    let rows = styles["heading"].as_array().unwrap();
    assert!(rows.contains(&serde_json::json!([StyleId::FontWeight as u16, 600.0])));
    assert!(rows.contains(&serde_json::json!([StyleId::MarginBottom as u16, "0.5rem"])));
    let key = runner.kernel().find_by_test_id("replace")[0];
    let id = runner.kernel().node_by_key(key).unwrap().id;
    runner.dispatch(id, Event::Press).unwrap();
    let second = prop(&runner, "editor", PropId::Decorations);
    assert_eq!(second["value"], "@carol");
    assert_eq!(first["spans"], second["spans"]);
    assert_ne!(first, second);
}

#[test]
fn unsupported_rows_and_other_editor_owners_are_refused() {
    for declaration in [
        "width=100",
        "display=\"flex\"",
        "padding-right=3",
        "press=\"bad\"",
    ] {
        let source =
            format!("decoration bad {declaration}\ncomponent App\n  view\n    text \"hi\"\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-decoration-style", "{error}");
    }
    let record = "TextDecorations(value=\"\", spans=[], blocks=[], hidden=[], atoms=[])";
    for tag in ["input", "text", "textarea markup=\"markdown\""] {
        let source = format!("component App\n  view\n    {tag} decorations={record}\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-decoration-field", "{error}");
    }
    let error = contract::compile("component App\n  view\n    textarea decorations=\"json\"\n")
        .unwrap_err();
    assert_eq!(error.id, "lower-attr-type", "{error}");
}

#[test]
fn undo_extension_keeps_existing_commands_and_range_geometry_is_action_only() {
    for (tail, count) in [
        ("", 2),
        (", 0, 1", 4),
        (", 0, 1, \"preserve\"", 5),
        (", \"undo\"", 3),
        (", 0, 1, \"select\", \"undo\"", 6),
    ] {
        let source = format!("component App\n  action replace\n    setRangeText(\"editor\", \"x\"{tail})\n  view\n    text \"Editor\"\n");
        let mut runner = boot(&source);
        runner.act("replace", vec![]).unwrap();
        let commands = runner.take_commands();
        assert_eq!(commands[0].name, "setRangeText");
        assert_eq!(commands[0].args.len(), count);
        if count == 3 || count == 6 {
            assert_eq!(commands[0].args[count - 1], Value::str("undo"));
        }
    }
    for tail in [", \"silent\"", ", 0, 1, \"end\", \"silent\"", ", 1"] {
        let source = format!("component App\n  action replace\n    setRangeText(\"editor\", \"x\"{tail})\n  view\n    text \"Editor\"\n");
        assert_eq!(
            contract::compile(&source).unwrap_err().id,
            "type-set-range-text"
        );
    }
    let mut runner = boot("component App\n  state absent = false\n  action read\n    absent = rangeFrame(\"editor\", 0, 1).unavailable\n  view\n    textarea id=\"editor\"\n");
    runner.act("read", vec![]).unwrap();
    assert_eq!(
        runner.slot("absent"),
        Some(&Value::Bool(true)),
        "no host range provider yet"
    );
    let error = contract::compile("component App\n  derive width = rangeFrame(\"editor\", 0, 1).width\n  view\n    text width\n").unwrap_err();
    assert_eq!(error.id, "type-geometry-outside-action");
}

#[test]
fn imported_aliases_and_private_declarations_stay_in_the_fields_source_scope() {
    let dir = std::env::temp_dir().join(format!("exact-decoration-scope-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("child.contract"), r##"decoration shared color="#111111"
decoration private font-weight=700
component Child
  view
    textarea decorations=TextDecorations(value="", spans=[], blocks=[], hidden=[], atoms=[]) testId="child"
"##).unwrap();
    std::fs::write(dir.join("app.contract"), r##"use Child, shared as imported from "./child.contract"
decoration shared color="#222222"
component App
  view
    column
      textarea decorations=TextDecorations(value="", spans=[], blocks=[], hidden=[], atoms=[]) testId="root"
      Child()
"##).unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = prop(&runner, "root", PropId::DecorationStyles);
    let child = prop(&runner, "child", PropId::DecorationStyles);
    assert_eq!(root["shared"][0][1], "#222222");
    assert_eq!(root["imported"][0][1], "#111111");
    assert!(root.get("private").is_none());
    assert_eq!(child["shared"][0][1], "#111111");
    assert!(child.get("private").is_some());
    assert!(child.get("imported").is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_checkbox_toggle_identifies_its_source_without_mutating_the_value() {
    let mut runner = boot(
        r#"component App
  state heard = ""
  action toggled(e: ToggleEvent)
    heard = `${e.oldState}>${e.newState}:${e.start}-${e.end}`
  view
    textarea value="- [ ] task" toggle=toggled testId="field"
"#,
    );
    let key = runner.kernel().find_by_test_id("field")[0];
    let id = runner.kernel().node_by_key(key).unwrap().id;
    runner
        .dispatch(
            id,
            Event::of_host_kind(50, "unchecked,checked,2,5").unwrap(),
        )
        .unwrap();
    assert_eq!(
        runner.slot("heard"),
        Some(&Value::str("unchecked>checked:2-5"))
    );
    runner
        .dispatch(id, Event::of_host_kind(50, "closed,open").unwrap())
        .unwrap();
    assert_eq!(runner.slot("heard"), Some(&Value::str("closed>open:-1--1")));
    assert_eq!(
        runner.kernel().node(id).unwrap().props.str(PropId::Value),
        Some("- [ ] task")
    );
    for bad in [
        "unchecked,checked,5,2",
        "unchecked,checked,-1,2",
        "unchecked,checked,2,2",
        "closed,open,2,3",
        "unchecked,checked,a,2",
    ] {
        assert!(Event::of_host_kind(50, bad).is_err(), "{bad}");
    }
}
