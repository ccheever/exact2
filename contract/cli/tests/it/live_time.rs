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
fn live_time_advances_without_evaluating_the_app_and_reserves_its_width() {
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
    assert_eq!(text(&r, "timer"), "0:00");
    assert_eq!(text(&r, "countdown"), "2:00");
    assert_eq!(r.timer_due_ms(), Some(1000.));
    assert!(r.advance(999.).unwrap().is_empty());
    r.advance(59_999.).unwrap();
    assert_eq!(text(&r, "relative"), "2 min. ago");
    r.advance(60_000.).unwrap();
    assert_eq!(text(&r, "relative"), "3 min. ago");
    assert_eq!(text(&r, "timer"), "1:00");
    assert_eq!(text(&r, "countdown"), "1:00");
    let bar = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("progress")[0])
        .unwrap();
    assert_eq!(bar.props.str(PropId::Value), Some("0.5"));
    assert_eq!(r.slot("unchanged").cloned(), initial_slots);
    assert_eq!(text(&r, "app-clock"), "0");
    r.advance(120_000.).unwrap();
    assert_eq!(text(&r, "countdown"), "0:00");
}

#[test]
fn live_time_validates_its_author_forms() {
    for body in [
        "time format=\"relative\"",
        "time datetime=0 format=\"unknown\"",
        "text datetime=0",
        "progress start=0",
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
