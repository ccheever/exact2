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
        Event::Clipboard(EventKind::Paste, "1\t2\n3\t4".into()),
    )
    .unwrap();
    assert_eq!(r.slot("pasted"), Some(&Value::str("B2:1\t2\n3\t4")));
    r.dispatch(grid, Event::Clipboard(EventKind::Copy, String::new()))
        .unwrap();
    assert_eq!(r.slot("copies"), Some(&Value::Number(1.0)));
    r.dispatch(grid, Event::Clipboard(EventKind::Cut, String::new()))
        .unwrap();
    assert_eq!(r.slot("cuts"), Some(&Value::str("B2[]")));
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
