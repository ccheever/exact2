//! The web's `selectionchange` on a `text` (the reader diary): its part of
//! the reader's selection reaches an action that takes a `Selection`, by the
//! one rule for an event's record, or an action that takes nothing more.

use exact_kernel::Kernel;
use exact_plan::{EventKind, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const READER: &str = r#"component App
  state picked = ""
  state changes = 0
  action pick(para: string, s: Selection)
    picked = `${para}:${s.start}-${s.end}:${s.text}`
  action counted
    changes = changes + 1
  view
    column
      text "It was the best of times" selectionchange=pick("p1") testId="p1"
      text "it was the worst of times" selectionchange=counted testId="p2"
"#;

#[test]
fn a_selection_reaches_the_action_as_its_record() {
    let plan = contract::compile(READER).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id = |r: &Runner<NoData>, t: &str| {
        r.kernel()
            .node_by_key(r.kernel().find_by_test_id(t)[0])
            .unwrap()
            .id
    };
    let (p1, p2) = (id(&r, "p1"), id(&r, "p2"));
    assert_eq!(r.handlers_of(p1), vec![EventKind::Selectionchange]);
    let select = |text: &str, start, end| Event::SelectionChange {
        text: text.into(),
        start,
        end,
    };
    r.dispatch(p1, select("best", 11.0, 15.0)).unwrap();
    assert_eq!(r.slot("picked"), Some(&Value::str("p1:11-15:best")));
    r.dispatch(p1, select("", 0.0, 0.0)).unwrap();
    assert_eq!(r.slot("picked"), Some(&Value::str("p1:0-0:")));
    r.dispatch(p2, select("worst", 11.0, 16.0)).unwrap();
    assert_eq!(r.slot("changes"), Some(&Value::Number(1.0)));
    // A host's payload: the offsets, then the text verbatim.
    let Ok(event) = Event::of_host_kind(35, "11,15,best") else {
        panic!("kind 35")
    };
    r.dispatch(p1, event).unwrap();
    assert_eq!(r.slot("picked"), Some(&Value::str("p1:11-15:best")));
}

#[test]
fn selectionchange_belongs_to_text_and_takes_the_record_or_nothing_more() {
    let e = contract::compile(&READER.replace("column\n", "column selectionchange=counted\n"))
        .unwrap_err();
    assert_eq!(e.id, "lower-attr-tag", "{e}");
    assert!(e.message.contains("belongs to `text`"), "{e}");
    let e = contract::compile(
        &READER.replace("action counted\n", "action counted(a: string, b: string)\n"),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("for its `Selection`"), "{e}");
}
