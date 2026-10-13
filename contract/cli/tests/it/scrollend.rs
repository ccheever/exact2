//! DOM's `scrollend` (LLP 1005, 2026-10-13): a handler on a `scroll` or a
//! `list`, no payload, heard once a scroll is over; a node with none takes
//! it as nothing and commits nothing.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct Rows;

impl DataSource for Rows {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "rows" => Ok(Value::list(
                (0..40).map(|i| Value::str(&format!("row {i}"))).collect(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

const APP: &str = r#"component App
  state ends = 0
  state moves = 0
  resource rows = rows() as shape list<string>
  action ended
    ends = ends + 1
  action moved(x: number, y: number)
    moves = moves + 1
  view
    column
      text `${ends}/${moves}` testId="count"
      scroll scroll=moved scrollend=ended testId="pane" height=100
        text "a"
      list virtualized=true scrollend=ended testId="feed" height=100 estimated-item-height=20
        for r in rows key=r
          text r
      column testId="plain"
"#;

fn boot() -> Runner<Rows> {
    let plan = contract::compile(APP).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        Rows,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn id(r: &Runner<Rows>, test: &str) -> exact_kernel::ViewId {
    let key = r.kernel().find_by_test_id(test)[0];
    r.kernel().node_by_key(key).unwrap().id
}

fn count(r: &Runner<Rows>) -> String {
    let n = id(r, "count");
    r.kernel()
        .node(n)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap_or("")
        .to_string()
}

#[test]
fn a_scroll_or_a_list_hears_its_end_and_a_node_without_one_commits_nothing() {
    let mut r = boot();
    r.dispatch(id(&r, "pane"), Event::Scrollend).unwrap();
    r.dispatch(id(&r, "feed"), Event::Scrollend).unwrap();
    assert_eq!(count(&r), "2/0", "no payload, and no `scroll` with it");
    let receipt = r.dispatch(id(&r, "plain"), Event::Scrollend).unwrap();
    assert_eq!(receipt, Default::default(), "no handler: nothing committed");
    assert_eq!(count(&r), "2/0");
}

#[test]
fn scrollend_takes_no_payload() {
    // ABI kind 51, as every host sends it: nothing after the kind.
    assert!(matches!(Event::of_host_kind(51, ""), Ok(Event::Scrollend)));
    let src = APP.replace("  action ended\n", "  action ended(x: number)\n");
    assert!(
        contract::compile(&src).is_err(),
        "an action that wants a payload is refused"
    );
}
