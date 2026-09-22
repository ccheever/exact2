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
  action selected(s) writes formats, mixed, link, unavailable
    formats = s.formats
    mixed = s.mixed
    link = s.link
    unavailable = s.unavailable
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
