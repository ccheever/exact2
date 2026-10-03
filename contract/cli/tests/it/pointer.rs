//! `pointerdown` and `pointerup` (LLP 1005 §3; Charlie, 2026-10-03, the
//! Signal Clone's hold-to-record mic as the consumer): DOM's names for a
//! touch or the primary button going down on a node and coming up, before
//! and apart from `press`.

use exact_kernel::Kernel;
use exact_plan::{EventKind, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const MIC: &str = r#"component App
  state recording = false
  state sent = 0
  state log = ""
  action begin
    recording = true
    log = `${log}d`
  action end
    recording = false
    sent = sent + 1
    log = `${log}u`
  action tapped
    log = `${log}p`
  view
    button pointerdown=begin pointerup=end press=tapped testId="mic"
      text (recording ? "recording" : "idle")
"#;

#[test]
fn pointer_down_and_up_run_their_actions_beside_press() {
    let plan = contract::compile(MIC).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let mic = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("mic")[0])
        .unwrap()
        .id;
    assert_eq!(
        r.handlers_of(mic),
        vec![
            EventKind::Pointerdown,
            EventKind::Pointerup,
            EventKind::Press
        ]
    );
    r.dispatch(mic, Event::Pointerdown).unwrap();
    assert_eq!(r.slot("recording"), Some(&Value::Bool(true)));
    r.dispatch(mic, Event::Pointerup).unwrap();
    r.dispatch(mic, Event::Press).unwrap();
    assert_eq!(r.slot("sent"), Some(&Value::Number(1.0)));
    assert_eq!(
        r.slot("log"),
        Some(&Value::str("dup")),
        "DOM's order: down, up, then the click"
    );
}

#[test]
fn a_pointer_handler_takes_no_payload() {
    let src = MIC.replace("action begin\n", "action begin(x: number)\n");
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "analyze-handler-arity", "{e}");
}
