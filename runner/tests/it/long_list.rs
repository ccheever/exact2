//! A `for` that builds a long list whole is said once, while measuring.
//! @ref LLP 1144 D4.

use exact_kernel::{Kernel, ViewId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("queried {name}")
    }
}

const SOURCE: &str = "component App
  state n = 3
  action grow
    n = n + 300
  view
    column
      button press=grow testId=\"grow\"
        text \"Grow\"
      scroll height=200
        for i in range(n) key=i
          text `${i}`
";

fn boot(measure: bool) -> Runner<NoData> {
    boot_source(SOURCE, measure)
}

fn boot_source(source: &str, measure: bool) -> Runner<NoData> {
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.measure(measure, false);
    r
}

fn grow(r: &mut Runner<NoData>) {
    let key = r.kernel().find_first_by_test_id("grow").unwrap();
    let id: ViewId = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
}

fn said(r: &Runner<NoData>) -> usize {
    r.journal().filter(|l| l.contains("long-list:")).count()
}

#[test]
fn a_long_for_is_said_once_while_measuring_and_never_otherwise() {
    let mut r = boot(true);
    assert_eq!(said(&r), 0, "three rows are no long list");
    grow(&mut r);
    assert_eq!(said(&r), 1);
    let line = r
        .journal()
        .find(|l| l.contains("long-list:"))
        .unwrap()
        .to_string();
    assert!(
        line.contains("repeats 303 rows") && line.contains("list virtualized=true"),
        "{line}"
    );
    grow(&mut r);
    assert_eq!(said(&r), 1, "once a session");

    let mut quiet = boot(false);
    grow(&mut quiet);
    assert_eq!(
        said(&quiet),
        0,
        "a runner that measures nothing says nothing"
    );

    // A virtualized list's rows are the collection's: never said.
    let virtualized = SOURCE.replace("scroll height=200", "list virtualized=true height=200");
    let mut listed = boot_source(&virtualized, true);
    grow(&mut listed);
    assert_eq!(said(&listed), 0, "a virtualized list builds what shows");
}
