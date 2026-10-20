//! DOM's clipboard events (x2apps spreadsheet F4, F14): `copy`, `cut` and
//! `paste` at the focused node, each offering its action an optional
//! `ClipboardEvent` whose `text` is what a paste carries (empty on copy and
//! cut, whose action writes the clipboard with `copyText`).

use exact_kernel::Kernel;
use exact_plan::{EventKind, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const GRID: &str = r#"component App
  state pasted = ""
  state copies = 0
  state cuts = ""
  action pasteAt(cell: string, e: ClipboardEvent)
    pasted = `${cell}:${e.text}`
  action copied
    copies = copies + 1
    copyText("a\tb")
  action cutAt(cell: string, e: ClipboardEvent)
    cuts = `${cell}[${e.text}]`
  view
    column key=noop paste=pasteAt("B2") copy=copied cut=cutAt("B2") testId="grid"
      text "grid"
  action noop(k: string)
    copies = copies
"#;

#[test]
fn clipboard_events_run_with_their_record() {
    let plan = contract::compile(GRID).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let grid = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("grid")[0])
        .unwrap()
        .id;
    assert_eq!(
        r.handlers_of(grid),
        vec![
            EventKind::Key,
            EventKind::Paste,
            EventKind::Copy,
            EventKind::Cut
        ]
    );
    r.dispatch(
        grid,
        Event::Clipboard(EventKind::Paste, "1\t2\n3\t4".into(), Vec::new()),
    )
    .unwrap();
    assert_eq!(r.slot("pasted"), Some(&Value::str("B2:1\t2\n3\t4")));
    r.dispatch(
        grid,
        Event::Clipboard(EventKind::Copy, String::new(), Vec::new()),
    )
    .unwrap();
    assert_eq!(r.slot("copies"), Some(&Value::Number(1.0)));
    r.dispatch(
        grid,
        Event::Clipboard(EventKind::Cut, String::new(), Vec::new()),
    )
    .unwrap();
    assert_eq!(r.slot("cuts"), Some(&Value::str("B2[]")));
}

/// A paste's files (`clipboardData.files`): the `doc:` handles the host
/// minted, beside the text (ABI kind 57); a paste of text alone has none.
#[test]
fn a_paste_hears_its_files() {
    const COMPOSER: &str = r#"component App
  state got = ""
  action pasted(e: ClipboardEvent)
    got = `${e.files.length}:${e.files.join("+")}:${e.text}`
    if e.files.length > 0
      preventDefault()
  view
    textarea testId="field" paste=pasted
"#;
    let plan = contract::compile(COMPOSER).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let field = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("field")[0])
        .unwrap()
        .id;
    r.dispatch(field, Event::of_host_kind(34, "plain").unwrap())
        .unwrap();
    assert_eq!(r.slot("got"), Some(&Value::str("0::plain")));
    assert!(!r.take_commands().iter().any(|c| c.name == "preventDefault"));
    r.dispatch(
        field,
        Event::of_host_kind(57, "2\ndoc:/1/image.png\ndoc:/2/notes.txt\nnotes.txt").unwrap(),
    )
    .unwrap();
    assert_eq!(
        r.slot("got"),
        Some(&Value::str("2:doc:/1/image.png+doc:/2/notes.txt:notes.txt"))
    );
    assert!(r.take_commands().iter().any(|c| c.name == "preventDefault"));
}

#[test]
fn a_clipboard_action_takes_the_record_or_nothing_more() {
    let e = contract::compile(
        &GRID.replace("action copied\n", "action copied(a: string, b: string)\n"),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("for its `ClipboardEvent`"), "{e}");
}
