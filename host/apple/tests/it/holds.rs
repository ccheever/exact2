//! Platform holds cross the real host, with authored commits and incarnation changes.
use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::{DataError, DataSource, Event};
struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn fixture() -> Host<NoData> {
    let plan = contract::compile(r#"component App
  state target = 1
  state showing = true
  action retarget
    target = 0.25
  action remove
    showing = false
  view
    column
      button testId="retarget" press=retarget
        text "retarget"
      button testId="remove" press=remove
        text "remove"
      when showing
        box testId="row" opacity=target transition="opacity 180ms ease-out, translate 180ms ease-out"
          text "row"
"#).unwrap().encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0
}
fn view(host: &Host<NoData>, name: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn token(batch: &str) -> u64 {
    batch
        .split("\"token\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn hold_survives_authored_commits_and_releases_to_latest_target() {
    let mut h = fixture();
    let row = view(&h, "row");
    let t = token(&h.hold_begin(row, Property::Opacity, 0.));
    let moved = h.hold_update(t, Value::scalar(0.9), 0.);
    assert!(moved.contains("\"x\":0.9"), "{moved}");
    assert!(moved.contains("\"motion\":false"));
    assert!(!moved.contains("\"op\":\"frame\""));
    h.dispatch_at(view(&h, "retarget"), Event::Press, 0.);
    assert!(h.has_hold(t));
    h.hold_end(t, HoldEnd::Cancel, 0.);
    assert!(!h.has_hold(t));
    let end = h.tick(180.);
    assert!(end.contains("\"x\":0.25"), "{end}");
}
#[test]
fn replacement_and_other_host_reject_old_handles_before_bad_input() {
    let mut h = fixture();
    let row = view(&h, "row");
    let old = token(&h.hold_begin(row, Property::Translate, 0.));
    h.hold_update(old, Value::new(91., 0.), 0.);
    let begin = h.hold_begin(row, Property::Translate, 0.);
    assert!(begin.contains("\"x\":91"), "{begin}");
    let current = token(&begin);
    for mut host in [fixture(), h] {
        let stale = host.hold_update(old, Value::new(f64::NAN, 0.), f64::NAN);
        assert!(stale.contains("\"error\":null"), "{stale}");
        host.hold_end(old, HoldEnd::Cancel, f64::NAN);
        if host.has_hold(current) {
            assert!(!host.has_hold(old));
        }
    }
}
#[test]
fn deletion_during_action_makes_end_harmless() {
    let mut h = fixture();
    let t = token(&h.hold_begin(view(&h, "row"), Property::Translate, 0.));
    h.hold_update(t, Value::new(80., 0.), 0.);
    assert!(h.has_hold(t)); // semantic action is permitted only while live
    h.dispatch_at(view(&h, "remove"), Event::Press, 0.);
    assert!(!h.has_hold(t));
    assert!(h
        .hold_end(t, HoldEnd::Cancel, f64::NAN)
        .contains("\"error\":null"));
}
#[test]
fn live_invalid_requests_preserve_hold_and_clock() {
    let mut h = fixture();
    let t = token(&h.hold_begin(view(&h, "row"), Property::Opacity, 10.));
    assert!(h
        .hold_update(t, Value::new(0.5, 1.), 100.)
        .contains("InvalidValueShape"));
    assert!(h
        .hold_update(t, Value::scalar(0.5), 9.)
        .contains("error\":\""));
    assert!(h.has_hold(t));
    assert!(h
        .hold_update(t, Value::scalar(0.5), 10.)
        .contains("\"error\":null"));
}

#[test]
fn catching_an_inflight_return_uses_engine_presentation_without_a_jump() {
    let mut h = fixture();
    let row = view(&h, "row");
    let t = token(&h.hold_begin(row, Property::Translate, 0.));
    h.hold_update(t, Value::new(120., 0.), 0.);
    h.hold_end(
        t,
        HoldEnd::Release {
            velocity: Value::new(-50., 0.),
        },
        0.,
    );
    let frame = h.tick(30.);
    let x: f64 = frame
        .split("\"x\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(x > 64. && x < 120., "{frame}");
    let begin = h.hold_begin(row, Property::Translate, 30.);
    let t = token(&begin);
    let sampled: f64 = begin
        .split("\"x\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((sampled - x).abs() < 0.001, "{begin}; {frame}");
    let held = h.hold_update(t, Value::new(sampled, 0.), 30.);
    assert!(held.contains("\"motion\":false"), "{held}");
    assert!(h.tick(300.).contains("\"ops\":[]"));
}

fn timer_fixture() -> Host<NoData> {
    let plan = contract::compile(r#"component App
  state target = 1
  action retarget
    target = 0.25
  task timer mount
    every(100, retarget)
  view
    box testId="row" opacity=target scale=target transition="opacity 180ms linear, scale 180ms linear"
"#).unwrap().encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0
}
#[test]
fn overdue_timer_retargets_a_held_property_without_rewinding_motion() {
    let mut h = timer_fixture();
    let row = view(&h, "row");
    let t = token(&h.hold_begin(row, Property::Opacity, 150.));
    h.hold_update(t, Value::scalar(0.9), 150.);
    let due = h.advance(250.);
    assert!(due.contains("\"error\":null"), "{due}");
    assert!(h.has_hold(t));
    assert!(
        due.contains("\"motion\":true"),
        "unrelated scale keeps running: {due}"
    );
    let unrelated = h.hold_begin(row, Property::Scale, 250.);
    let sampled: f64 = unrelated
        .split("\"x\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (sampled - (1. - 0.75 * 100. / 180.)).abs() < 1e-9,
        "{unrelated}"
    );
    h.hold_end(token(&unrelated), HoldEnd::Cancel, 250.);
    h.hold_end(t, HoldEnd::Cancel, 250.);
    let end = h.tick(430.);
    assert!(end.contains("\"x\":0.25"), "{end}");
}
#[test]
fn late_receipt_starts_at_last_presented_time_not_due_time_or_batch_end() {
    let mut h = timer_fixture();
    let row = view(&h, "row");
    h.tick(150.);
    h.advance(250.);
    let begin = h.hold_begin(row, Property::Opacity, 250.);
    let sampled: f64 = begin
        .split("\"x\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (sampled - (1. - 0.75 * 100. / 180.)).abs() < 1e-9,
        "{begin}"
    );
    let mut cold = timer_fixture();
    let row = view(&cold, "row");
    cold.advance(250.);
    let begin = cold.hold_begin(row, Property::Opacity, 250.);
    let sampled: f64 = begin
        .split("\"x\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (sampled - 0.375).abs() < 1e-9,
        "unobserved timer keeps due-time semantics: {begin}"
    );
}
