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

/// `elementFromPoint(x, y)` (LLP 1094 D10): the `id` nearest the front-most
/// box at a viewport point, over `frame`'s boxes, as DOM's
/// `elementFromPoint(x, y)?.closest("[id]")?.id`.
#[test]
fn element_from_point_names_the_front_most_box_by_its_nearest_id() {
    let plan = contract::compile(
        r#"component App
  state hit = "unset"
  state px = 0
  state py = 0
  action probe(x: number, y: number)
    hit = match elementFromPoint(x, y) { case some(id) => id, case none => "none" }
  view
    column
      column id="plain" height=40
        column height=20 width=50
      scroll id="pane" height=60
        column id="inside" height=200
      column id="ghosted" height=40 pointer-events="none"
        column id="under" height=40
      column id="pile" height=60
        column id="low" height=60 position="relative" z-index=2
        column height=60 margin-top=-60 position="relative" z-index=1
          column id="high" height=60
      button press=probe(0, 0) testId="read"
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
    let at = |r: &mut Runner<NoData>, x: f64, y: f64| {
        r.act("probe", vec![Value::Number(x), Value::Number(y)])
            .unwrap();
        r.slot("hit").and_then(Value::as_str).unwrap().to_owned()
    };
    // A box with no id of its own is named by its nearest ancestor's.
    assert_eq!(at(&mut r, 10.0, 10.0), "plain");
    assert_eq!(at(&mut r, 200.0, 30.0), "plain");
    // Under the scroller's clip: its content shows only inside the port.
    assert_eq!(at(&mut r, 10.0, 50.0), "inside");
    let pane = r.kernel().find_by_id("pane")[0];
    let pane = r.kernel().arena().local_id(pane.index);
    r.scrolled(Some(pane), 0.0, 150.0);
    assert_eq!(
        at(&mut r, 10.0, 85.0),
        "inside",
        "scrolled 150: its last 50 show"
    );
    assert_eq!(at(&mut r, 10.0, 95.0), "pane", "past its end, the port");
    // `pointer-events: none` is inherited: the point passes through both.
    assert_eq!(at(&mut r, 10.0, 110.0), "none");
    // The higher `z-index` in a sibling's subtree is in front, though the
    // other sibling comes later in tree order.
    assert_eq!(at(&mut r, 10.0, 170.0), "low");
    // Off every box.
    assert_eq!(at(&mut r, 10.0, 5000.0), "none");
}
