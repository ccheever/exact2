//! Geometry reads from actions (LLP 1051.000): `frame(id)` is where layout
//! put a node and `measure(id)` its box at `height: auto`, answered here by
//! the kernel, as on a native host. Outside an action there is no read at all
//! (the compiler refuses it: `contract/corpus/rejects.txt`).

use exact_kernel::{Kernel, Offer};
use exact_runner::{Capability, DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SRC: &str = r#"component App
  state room = 0
  state top = 0
  state natural = 0
  state sized = 0
  state answered = false
  state missing = false
  action read
    room = frame("bay").height
    top = frame("sheet").y
    natural = measure("sheet").height
    sized = frame("sheet").height
    answered = not frame("bay").unavailable and not measure("sheet").unavailable
    missing = frame("nowhere").unavailable
  view
    column id="bay" height=500
      column height=20
      column id="sheet" height=100 padding-top=10 padding-bottom=10 box-sizing="border-box"
        column height=240
      button press=read testId="read"
        text "read"
"#;

fn num(r: &Runner<NoData>, name: &str) -> f64 {
    r.slot(name).and_then(Value::as_number).unwrap()
}

fn flag(r: &Runner<NoData>, name: &str) -> bool {
    matches!(r.slot(name), Some(Value::Bool(true)))
}

fn boot() -> Runner<NoData> {
    let plan = contract::compile(SRC).unwrap();
    assert!(exact_runner::uses(&plan).has(Capability::Geometry));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn lay_out(r: &mut Runner<NoData>) {
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
}

#[test]
fn an_action_reads_where_layout_put_a_node_and_its_height_at_auto() {
    let mut r = boot();
    lay_out(&mut r);
    r.act("read", vec![]).unwrap();
    assert!(flag(&r, "answered"));
    assert_eq!(num(&r, "room"), 500.0);
    assert_eq!(num(&r, "top"), 20.0);
    assert_eq!(
        num(&r, "natural"),
        260.0,
        "240 of content and 20 of padding"
    );
    assert_eq!(
        num(&r, "sized"),
        100.0,
        "the authored height is what is laid out"
    );
    assert!(
        flag(&r, "missing"),
        "an id that names nothing is unavailable"
    );
    // The measure published nothing: the sheet is still laid out at 100.
    let sheet = r.kernel().find_by_id("sheet")[0];
    assert_eq!(r.kernel().node_by_key(sheet).unwrap().frame.height, 100.0);
}

#[test]
fn before_the_first_layout_every_read_is_unavailable() {
    let mut r = boot();
    r.act("read", vec![]).unwrap();
    assert!(!flag(&r, "answered"));
    assert_eq!((num(&r, "room"), num(&r, "natural")), (0.0, 0.0));
}

#[test]
fn a_read_sees_the_box_where_the_viewer_does_with_every_scroll_above_it_applied() {
    // The kanban diary's F4: a drop target under the pointer, read from a
    // scrolled column, without the app tracking `scrollTop` itself.
    let plan = contract::compile(
        r#"component App
  state y = 0
  state x = 0
  state top = 0
  state pane = 0
  action read
    y = frame("card").y
    x = frame("card").x
    top = measure("card").y
    pane = frame("pane").y
  view
    column
      column height=40
      scroll id="pane" height=200
        column height=300
        column id="card" height=50
      button press=read testId="read"
        text "read"
"#,
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    lay_out(&mut r);
    let pane = r.kernel().find_by_id("pane")[0];
    let pane = r.kernel().arena().local_id(pane.index);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        num(&r, "y"),
        340.0,
        "unscrolled: 40 above the pane, 300 above the card"
    );
    r.scrolled(Some(pane), 7.0, 120.0);
    r.scrolled(None, 0.0, 10.0);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        num(&r, "y"),
        210.0,
        "the pane's 120 and the page's 10 are applied"
    );
    assert_eq!(num(&r, "x"), -7.0);
    assert_eq!(num(&r, "top"), 210.0, "measure answers at the same origin");
    assert_eq!(
        num(&r, "pane"),
        30.0,
        "a scroller's own offset moves its content, not its box"
    );
}
