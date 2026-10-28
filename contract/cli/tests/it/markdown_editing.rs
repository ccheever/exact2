//! A real editor compiles its toolbar commands and receives a typed selection record.
use exact_kernel::{Kernel, PropId};
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
    boldActive = ` ${s.formats} `.includes(" bold ")
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
        Event::selection_payload("bold heading2\n1\ncode\n\nhttps://example.com\npath").unwrap();
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
        Event::selection_payload("boldly italic\n0\n\n\n").unwrap(),
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

/// The editor's `input`, `selectionchange` and `beforeinput` are a text
/// field's, over its source (LLP 1045 D6): each carries the `InputEvent`
/// with the source selection, its `select` still its formats; and
/// `setSelectionRange` and `setRangeText` name it as they name a field.
#[test]
fn the_editors_text_events_carry_the_source_selection() {
    let source = r#"component App
  state seen = ""
  action moved(e: InputEvent)
    seen = `moved ${e.value}:${e.selectionStart}-${e.selectionEnd}:${e.selectionDirection}`
  action before(e: InputEvent)
    seen = `before ${e.inputType}:${e.selectionStart}-${e.selectionEnd}:${e.cancelable}:${e.isComposing}`
    preventDefault()
  action edited(v: string, e: InputEvent)
    seen = `input ${v}:${e.inputType}:${e.selectionStart}-${e.selectionEnd}:${e.isComposing}`
  action selected(s: MarkdownSelection)
    seen = `select ${s.formats}`
  action complete
    setRangeText("editor", "@a.ts ", 0, 1, "end")
    setSelectionRange("editor", 2, 4, "backward")
  view
    column
      textarea id="editor" testId="editor" value="**b**" markup="markdown" input=edited selectionchange=moved beforeinput=before select=selected
      button testId="complete" press=complete
        text "Complete"
"#;
    let plan = Plan::decode(&contract::compile(source).unwrap().encode()).unwrap();
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
    for (event, expected) in [
        (
            Event::of_host_kind(52, "1,4,backward,**b**").unwrap(),
            "moved **b**:1-4:backward",
        ),
        (
            Event::of_host_kind(53, "insertLineBreak,1,0,-1,2,2,none,**b**").unwrap(),
            "before insertLineBreak:2-2:true:false",
        ),
        (
            Event::of_host_kind(54, "insertCompositionText,0,1,3,あ3,3,none,**あb**").unwrap(),
            "input **あb**:insertCompositionText:3-3:true",
        ),
        (
            Event::selection_payload("bold\n0\n\n\n").unwrap(),
            "select bold",
        ),
    ] {
        r.dispatch(editor, event).unwrap();
        assert_eq!(r.slot("seen"), Some(&Value::str(expected)));
    }
    r.take_commands();
    r.dispatch(id(&r, "complete"), Event::Press).unwrap();
    let names: Vec<_> = r.take_commands().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["setRangeText", "setSelectionRange"]);
    // A `selectionchange` action declaring the `Selection` of a `text` is
    // the arity refusal a text field's gets.
    let error = contract::compile(&source.replace(
        "action moved(e: InputEvent)\n    seen = `moved ${e.value}:${e.selectionStart}-${e.selectionEnd}:${e.selectionDirection}`",
        "action moved(text: string, a: number, b: number, e: Selection)\n    seen = text",
    ))
    .unwrap_err();
    assert!(error.id.contains("handler"), "{error}");
}

/// `paragraphDirs` is the editor's (LLP 1045 D11): HTML's `dir` words, one
/// for each line of its source, reach its node as written and follow a bound
/// value. Another word, a list, and any other node are refused.
#[test]
fn paragraph_dirs_reach_the_editor_and_are_refused_elsewhere() {
    let source = r#"component App
  state dirs = "ltr rtl"
  action flip
    dirs = "rtl"
  view
    column
      textarea testId="editor" value="" markup="markdown" paragraphDirs=dirs
      textarea testId="fixed" value="" markup="markdown" paragraphDirs="auto"
      button testId="flip" press=flip
        text "Flip"
"#;
    let plan = Plan::decode(&contract::compile(source).unwrap().encode()).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let dirs = |r: &Runner<NoData>, name| {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id(name)[0]).unwrap();
        (
            node.id,
            node.props.str(PropId::ParagraphDirs).map(str::to_owned),
        )
    };
    assert_eq!(dirs(&r, "editor").1.as_deref(), Some("ltr rtl"));
    assert_eq!(dirs(&r, "fixed").1.as_deref(), Some("auto"));
    r.dispatch(dirs(&r, "flip").0, Event::Press).unwrap();
    assert_eq!(dirs(&r, "editor").1.as_deref(), Some("rtl"));
    let field = |tag: &str| {
        format!(
            "component App\n  state dirs = [\"rtl\"]\n  view\n    {tag} paragraphDirs=\"rtl\"\n"
        )
    };
    for (source, code) in [
        (field("textarea value=\"\""), "lower-paragraph-dirs-field"),
        (
            field("textarea value=\"\" markup=\"none\""),
            "lower-paragraph-dirs-field",
        ),
        (
            field("text \"a\" markup=\"markdown\""),
            "lower-paragraph-dirs-field",
        ),
        (
            field("textarea value=\"\" markup=\"markdown\"")
                .replace("\"rtl\"\n", "\"rtl right\"\n"),
            "lower-attr-value",
        ),
        (
            field("textarea value=\"\" markup=\"markdown\"").replace("=\"rtl\"\n", "=dirs\n"),
            "lower-attr-type",
        ),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, code, "{source}\n{error}");
    }
}
