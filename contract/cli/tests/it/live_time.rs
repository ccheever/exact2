//! Live HTML time and progress, on a seek clock with no app tasks.
use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("a presentation called app data: {source}")
    }
}

fn text(r: &Runner<NoData>, id: &str) -> String {
    r.kernel()
        .node_by_key(r.kernel().find_by_test_id(id)[0])
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap_or("")
        .into()
}

#[test]
fn live_time_advances_without_evaluating_the_app() {
    let source = r#"component App
  state unchanged = 0
  derive observed = performanceNow()
  view
    row
      time datetime=-120000 format="relative" testId="relative"
        text "Fallback"
      time datetime=0 format="timer" testId="timer"
      time datetime=120000 format="countdown" testId="countdown"
      progress start=0 end=120000 testId="progress"
      text observed.toString() testId="app-clock"
"#;
    let plan = contract::bake(contract::compile(source).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let initial_slots = r.slot("unchanged").cloned();
    assert_eq!(text(&r, "relative"), "2 min. ago");
    assert_eq!(text(&r, "timer"), "00:00");
    assert_eq!(text(&r, "countdown"), "02:00");
    assert_eq!(r.timer_due_ms(), Some(1000.));
    assert!(r.advance(999.).unwrap().is_empty());
    r.advance(59_999.).unwrap();
    assert_eq!(text(&r, "relative"), "2 min. ago");
    r.advance(60_000.).unwrap();
    assert_eq!(text(&r, "relative"), "3 min. ago");
    assert_eq!(text(&r, "timer"), "01:00");
    assert_eq!(text(&r, "countdown"), "01:00");
    let bar = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("progress")[0])
        .unwrap();
    assert_eq!(bar.props.str(PropId::Value), Some("0.5"));
    assert_eq!(r.slot("unchanged").cloned(), initial_slots);
    assert_eq!(text(&r, "app-clock"), "0");
    r.advance(120_000.).unwrap();
    assert_eq!(text(&r, "countdown"), "00:00");
}

#[test]
fn live_time_validates_its_author_forms() {
    for body in [
        "time format=\"relative\"",
        "time datetime=0 format=\"unknown\"",
        "text datetime=0",
        "progress start=0",
        "progress start=2 end=1",
        "progress start=2 end=2",
        "progress start=0 end=-1",
        "progress start=0 end=1 value=0.2",
        "progress start=0 end=1 max=1",
    ] {
        assert!(
            contract::compile(&format!("component App\n  view\n    {body}\n")).is_err(),
            "{body}"
        );
    }
    for body in [
        "time datetime=0",
        "time datetime=0 format=\"date-full\"",
        "progress value=0.4 max=1",
        "progress",
        "progress start=0 end=1",
    ] {
        contract::compile(&format!("component App\n  view\n    {body}\n")).unwrap();
    }
}

fn prop(r: &Runner<NoData>, id: &str, p: PropId) -> String {
    r.kernel()
        .node_by_key(r.kernel().find_by_test_id(id)[0])
        .unwrap()
        .props
        .str(p)
        .unwrap()
        .into()
}

#[test]
fn duration_reservations_keep_the_initial_fields_and_grow_at_hour_digits() {
    let source = "component App\n  view\n    row\n      time datetime=0 format=\"timer\" testId=\"timer\"\n      time datetime=7200000 format=\"countdown\" testId=\"countdown\"\n";
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let initial = prop(&r, "timer", PropId::TextReservation);
    assert_eq!(initial.lines().count(), 11);
    assert!(initial.lines().all(|s| s.len() == 7), "{initial}");
    let countdown = prop(&r, "countdown", PropId::TextReservation);
    r.advance(2000.).unwrap();
    assert_eq!(text(&r, "timer"), "00:02");
    assert_eq!(prop(&r, "timer", PropId::TextReservation), initial);
    r.advance(3599000.).unwrap();
    assert_eq!(prop(&r, "timer", PropId::TextReservation), initial);
    r.advance(3600000.).unwrap();
    let hour = prop(&r, "timer", PropId::TextReservation);
    assert!(hour.lines().all(|s| s.len() == 8), "{hour}");
    r.advance(36000000.).unwrap();
    let ten = prop(&r, "timer", PropId::TextReservation);
    assert!(ten.lines().all(|s| s.len() == 9), "{ten}");
    assert_eq!(prop(&r, "countdown", PropId::TextReservation), countdown);
    assert_eq!(text(&r, "countdown"), "00:00");
}

#[test]
fn relative_zero_is_now_at_the_instant_and_thirty_seconds_afterward() {
    let mut r = Runner::boot(
        contract::compile(
            "component App\n  view\n    time datetime=0 format=\"relative\" testId=\"time\"\n",
        )
        .unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "time"), "now");
    assert!(r.advance(30000.).unwrap().is_empty());
    assert_eq!(text(&r, "time"), "now");
    r.advance(60000.).unwrap();
    assert_eq!(text(&r, "time"), "1 min. ago");
    assert!(r.journal().any(|s| s.contains("advance → 1 timer fired")));
}

#[test]
fn an_unrelated_commit_does_not_call_the_time_formatter() {
    use exact_kernel::{MonospaceMeasurer, TextMeasureRequest, TextMeasurer, TextMetrics};
    use std::{cell::Cell, rc::Rc};
    struct Counting(Rc<Cell<usize>>);
    impl TextMeasurer for Counting {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            MonospaceMeasurer::default().measure(r)
        }
        fn format_time(&mut self, r: &exact_kernel::time::TimeRequest<'_>) -> Option<String> {
            self.0.set(self.0.get() + 1);
            exact_kernel::time::english(r)
        }
    }
    let source = "component App\n  state n = 0\n  action bump\n    n = n + 1\n  view\n    column\n      button \"Bump\" press=bump testId=\"bump\"\n      text n.toString()\n      time datetime=-259200000 format=\"relative\" testId=\"time\"\n";
    let calls = Rc::new(Cell::new(0));
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::new(Box::new(Counting(calls.clone()))),
        Default::default(),
        "/",
    )
    .unwrap();
    let before = calls.get();
    let button = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("bump")[0])
        .unwrap()
        .id;
    r.dispatch(button, exact_runner::Event::Press).unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(r.timer_due_ms(), Some(43200000.));
    r.advance(60000.).unwrap();
    assert_eq!(calls.get(), before);
}

#[test]
fn datetime_edits_name_the_instant_and_the_wrong_tag() {
    let error =
        contract::compile("component App\n  view\n    time datetime=\"2026-10-17T12:00:00Z\"\n")
            .unwrap_err();
    assert!(
        error.message.contains("epoch milliseconds") && error.message.contains("date("),
        "{error:?}"
    );
    let bound = contract::compile(
        "component App\n  state raw = \"2026-10-17T12:00:00Z\"\n  view\n    time datetime=raw\n",
    )
    .unwrap_err();
    assert!(
        bound.message.contains("epoch milliseconds") && bound.message.contains("date("),
        "{bound:?}"
    );
    let error = contract::compile("component App\n  view\n    view datetime=0\n").unwrap_err();
    assert!(
        error.message.contains("`view`") && error.message.contains("`time`"),
        "{error:?}"
    );
}

#[test]
fn relative_uses_the_kernel_unit_and_rounding_above_an_hour() {
    let mut r = Runner::boot(contract::compile("component App\n  view\n    time datetime=9000000 format=\"relative\" testId=\"time\"\n").unwrap(), NoData,
        Kernel::with_monospace(), Default::default(), "/").unwrap();
    assert_eq!(text(&r, "time"), "in 3 hr.");
    assert_eq!(r.timer_due_ms(), Some(60000.));
    r.advance(60000.).unwrap();
    assert_eq!(text(&r, "time"), "in 2 hr.");
}

#[test]
fn a_runtime_inverted_interval_has_no_range_and_can_recover() {
    let source = r#"component App
  state end = 120000
  action invert
    end = -1
  action restore
    end = 120000
  view
    column
      button "Invert" press=invert testId="invert"
      button "Restore" press=restore testId="restore"
      progress start=0 end=end testId="progress"
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let bar = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("progress")[0])
        .unwrap()
        .id;
    for (button, valid) in [("invert", false), ("restore", true)] {
        let id = r
            .kernel()
            .node_by_key(r.kernel().find_by_test_id(button)[0])
            .unwrap()
            .id;
        r.dispatch(id, exact_runner::Event::Press).unwrap();
        let props = &r.kernel().node(bar).unwrap().props;
        assert_eq!(props.get(PropId::Start).is_some(), valid);
        assert_eq!(props.get(PropId::End).is_some(), valid);
        assert_eq!(props.get(PropId::Value).is_some(), valid);
    }
}
