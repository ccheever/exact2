//! A textarea's paint-only decorations: declarations, the record, and what
//! the narrowing removed (LLP 1114.007).
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
const RECORD: &str =
    r##"TextDecorations(value=draft, spans=[TextDecorationSpan(name="mention", start=0, end=6)])"##;

#[test]
fn source_identity_drives_the_named_record_and_declarations_are_paint_rows() {
    let source = format!(
        r##"decoration mention color="#1083fe" text-decoration="underline"
decoration tag background-color="#eef" text-shadow="0 0 1px #000"
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
    assert_eq!(
        first,
        serde_json::json!({"value":"@alice","spans":[{"name":"mention","start":0,"end":6}]})
    );
    let styles = prop(&runner, "editor", PropId::DecorationStyles);
    assert_eq!(
        styles["mention"],
        serde_json::json!([
            [StyleId::TextColor as u16, "#1083fe"],
            [StyleId::TextDecorationLine as u16, "underline"]
        ])
    );
    assert_eq!(styles["tag"].as_array().unwrap().len(), 2);
    let key = runner.kernel().find_by_test_id("replace")[0];
    let id = runner.kernel().node_by_key(key).unwrap().id;
    runner.dispatch(id, Event::Press).unwrap();
    let second = prop(&runner, "editor", PropId::Decorations);
    assert_eq!(second["value"], "@carol");
    assert_eq!(first["spans"], second["spans"]);
    assert_ne!(
        first, second,
        "an equal span list for new text is a new record"
    );
}

#[test]
fn a_decoration_takes_only_what_highlight_admits() {
    for declaration in [
        "font-size=\"1.25rem\"",
        "font-weight=600",
        "font-family=\"serif\"",
        "line-height=1.3",
        "margin=\"1rem 0\"",
        "padding-left=3",
        "border-left=\"2px solid #888\"",
        "width=100",
        "display=\"flex\"",
        "press=\"bad\"",
    ] {
        let source =
            format!("decoration bad {declaration}\ncomponent App\n  view\n    text \"hi\"\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-decoration-style", "{error}");
        for allowed in [
            "`color`",
            "`background-color`",
            "`text-decoration`",
            "`text-shadow`",
        ] {
            assert!(error.message.contains(allowed), "{error}");
        }
    }
    let record = "TextDecorations(value=\"\", spans=[])";
    for tag in ["input", "text", "textarea markup=\"markdown\""] {
        let source = format!("component App\n  view\n    {tag} decorations={record}\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-decoration-field", "{error}");
    }
    let error = contract::compile("component App\n  view\n    textarea decorations=\"json\"\n")
        .unwrap_err();
    assert_eq!(error.id, "lower-attr-type", "{error}");
    for removed in [
        "TextDecorations(value=\"\", spans=[], blocks=[], hidden=[], atoms=[])",
        "TextDecorationAtom(name=\"a\", start=0, end=1, label=\"A\")",
    ] {
        let source = format!("component App\n  view\n    textarea decorations={removed}\n");
        assert!(contract::compile(&source).is_err(), "{removed}");
    }
}

#[test]
fn set_range_text_has_no_undo_mode_and_range_geometry_is_action_only() {
    for (tail, count) in [("", 2), (", 0, 1", 4), (", 0, 1, \"preserve\"", 5)] {
        let source = format!("component App\n  action replace\n    setRangeText(\"editor\", \"x\"{tail})\n  view\n    text \"Editor\"\n");
        let mut runner = boot(&source);
        runner.act("replace", vec![]).unwrap();
        let commands = runner.take_commands();
        assert_eq!(commands[0].name, "setRangeText");
        assert_eq!(commands[0].args.len(), count);
    }
    for tail in [", \"undo\"", ", 0, 1, \"select\", \"undo\"", ", 1"] {
        let source = format!("component App\n  action replace\n    setRangeText(\"editor\", \"x\"{tail})\n  view\n    text \"Editor\"\n");
        assert_eq!(
            contract::compile(&source).unwrap_err().id,
            "type-set-range-text",
            "{tail}"
        );
    }
    let mut runner = boot("component App\n  state absent = false\n  action read\n    absent = rangeFrame(\"editor\", 0, 1).unavailable\n  view\n    textarea id=\"editor\"\n");
    runner.act("read", vec![]).unwrap();
    assert_eq!(
        runner.slot("absent"),
        Some(&Value::Bool(true)),
        "a runner with no host reader answers unavailable"
    );
    let error = contract::compile("component App\n  derive width = rangeFrame(\"editor\", 0, 1).width\n  view\n    text width\n").unwrap_err();
    assert_eq!(error.id, "type-geometry-outside-action");
}

#[test]
fn imported_aliases_and_private_declarations_stay_in_the_fields_source_scope() {
    let dir = std::env::temp_dir().join(format!("exact-decoration-scope-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("child.contract"),
        r##"decoration shared color="#111111"
decoration private color="#333333"
component Child
  view
    textarea decorations=TextDecorations(value="", spans=[]) testId="child"
"##,
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        r##"use Child, shared as imported from "./child.contract"
decoration shared color="#222222"
component App
  view
    column
      textarea decorations=TextDecorations(value="", spans=[]) testId="root"
      Child()
"##,
    )
    .unwrap();
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
fn a_toggle_event_is_a_popovers_alone() {
    let mut runner = boot(
        r#"component App
  state heard = ""
  action toggled(e: ToggleEvent)
    heard = `${e.oldState}>${e.newState}`
  view
    column popover="auto" id="menu" toggle=toggled testId="menu"
      text "Menu"
"#,
    );
    let key = runner.kernel().find_by_test_id("menu")[0];
    let id = runner.kernel().node_by_key(key).unwrap().id;
    runner
        .dispatch(id, Event::of_host_kind(50, "closed,open").unwrap())
        .unwrap();
    assert_eq!(runner.slot("heard"), Some(&Value::str("closed>open")));
    for bad in ["unchecked,checked,2,5", "closed,open,2,3", "open"] {
        assert!(Event::of_host_kind(50, bad).is_err(), "{bad}");
    }
    let error = contract::compile(
        "component App\n  state at = 0\n  action toggled(e: ToggleEvent)\n    at = e.start\n  view\n    text \"hi\"\n",
    )
    .unwrap_err();
    assert!(error.message.contains("start"), "{error}");
}
