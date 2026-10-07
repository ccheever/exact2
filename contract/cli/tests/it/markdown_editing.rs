//! A real editor compiles its toolbar commands and receives a typed selection record.
use exact_kernel::Kernel;
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected query {name}")
    }
}

const SOURCE: &str = r#"component App
  state formats = ""
  state mixed = false
  state link = ""
  state unavailable = ""
  state boldActive = false
  action selected(s)
    formats = s.formats
    mixed = s.mixed
    link = s.link
    unavailable = s.unavailable
    boldActive = includes(` ${s.formats} `, " bold ")
  action bold()
    format("editor", "bold")
  action setLink()
    format("editor", "link", "https://example.com")
  view
    column
      textarea id="editor" testId="editor" value="hello" markup="markdown" select=selected
      button testId="bold" press=bold
        text "Bold"
      button testId="link" press=setLink
        text "Link"
"#;

#[test]
fn toolbar_commands_and_select_record_roundtrip_and_run() {
    let plan = contract::compile(SOURCE).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id = |r: &Runner<NoData>, name| {
        r.kernel()
            .node_by_key(r.kernel().find_by_test_id(name)[0])
            .unwrap()
            .id
    };
    let editor = id(&r, "editor");
    assert_eq!(r.handlers_of(editor), [EventKind::Select]);
    let event =
        Event::selection_payload("bold heading2\n1\ncode\nhttps://example.com\npath").unwrap();
    r.dispatch(editor, event).unwrap();
    assert_eq!(r.slot("formats"), Some(&Value::str("bold heading2")));
    assert_eq!(r.slot("mixed"), Some(&Value::Bool(true)));
    assert_eq!(
        r.slot("link"),
        Some(&Value::str("https://example.com\npath"))
    );
    assert_eq!(r.slot("unavailable"), Some(&Value::str("code")));
    assert_eq!(r.slot("boldActive"), Some(&Value::Bool(true)));
    r.dispatch(
        editor,
        Event::selection_payload("boldly italic\n0\n\n").unwrap(),
    )
    .unwrap();
    assert_eq!(r.slot("boldActive"), Some(&Value::Bool(false)));
    r.dispatch(id(&r, "bold"), Event::Press).unwrap();
    r.dispatch(id(&r, "link"), Event::Press).unwrap();
    let commands = r.take_commands();
    assert_eq!(commands[0].name, "format");
    assert_eq!(commands[0].args, [Value::str("editor"), Value::str("bold")]);
    assert_eq!(
        commands[1].args,
        [
            Value::str("editor"),
            Value::str("link"),
            Value::str("https://example.com")
        ]
    );
    for invalid in ["bold\ntrue\n\n", "bold\n1\n", "bold,italic\n0\n\nx"] {
        assert!(Event::selection_payload(invalid).is_none(), "{invalid}");
    }
}

#[test]
fn select_rejects_a_wrong_payload_type_or_arity() {
    for (source, message) in [
        (
            SOURCE.replace("selected(s)", "selected(s: string)"),
            "type-handler-payload",
        ),
        (
            SOURCE.replace("selected(s)", "selected(extra: string, s)"),
            "handler-arity",
        ),
    ] {
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
}

/// `markup` is `"markdown"` or `"none"`, on a `text` or a `textarea`;
/// another word or another tag did nothing, silently (notes #1).
#[test]
fn markup_is_markdown_or_none_on_a_text_or_a_textarea() {
    // A plain textarea's `select` is HTML's, carrying its `InputEvent`
    // (x2apps codeedit #2); the editor's carries its formats.
    contract::compile(
        "component App\n  state at = 0\n  action moved(e: InputEvent)\n    at = e.selectionStart\n  view\n    textarea value=\"hello\" markup=\"none\" select=moved\n",
    )
    .unwrap();
    let error = contract::compile(
        &SOURCE
            .replace(
                "action selected(s)",
                "action selected(s: MarkdownSelection)",
            )
            .replace("markup=\"markdown\"", "markup=\"none\""),
    )
    .unwrap_err();
    assert_eq!(error.id, "type-handler-payload", "{error}");
    let error =
        contract::compile(&SOURCE.replace("markup=\"markdown\"", "markup=\"html\"")).unwrap_err();
    assert_eq!(error.id, "lower-attr-value", "{error}");
    let error = contract::compile(
        &SOURCE.replace("testId=\"bold\"", "testId=\"bold\" markup=\"markdown\""),
    )
    .unwrap_err();
    assert_eq!(error.id, "lower-attr-tag", "{error}");
    assert!(error.message.contains("not `button`"), "{error}");
}
