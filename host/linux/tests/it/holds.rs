//! Native presentation ownership over actual host commits, clocks and lifetimes.
use exact_kernel::MonospaceMeasurer;
use exact_linux::Host;
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
  state target = 0
  state showing = true
  action retarget
    target = 120
  action remove
    showing = false
  view
    column
      button testId="retarget" press=retarget
        text "retarget"
      button testId="remove" press=remove
        text "remove"
      when showing
        box testId="row" opacity=(target == 0 ? 1 : 0.25) swiperight=retarget transition="opacity 180ms ease-out, translate spring(300, 30, 1)"
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
fn view(h: &Host<NoData>, name: &str) -> u32 {
    let k = h.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

#[test]
fn catch_uses_current_presentation_and_retains_newest_authored_target() {
    let mut h = fixture();
    let row = view(&h, "row");
    assert!(h
        .dispatch_at(view(&h, "retarget"), Event::Press, 0.)
        .is_none());
    h.tick(30.);
    let sample = h.presented(row).opacity;
    assert!(sample > 0.25 && sample < 1.);
    let start = h.hold_begin(row, Property::Opacity, 30.).unwrap().unwrap();
    assert!((start.value.x - sample as f64).abs() < 0.0001);
    assert!(!h.motion());
    h.hold_update(start.token, Value::scalar(0.9), 40.).unwrap();
    h.dispatch_at(view(&h, "retarget"), Event::Press, 40.);
    assert_eq!(h.presented(row).opacity, 0.9);
    assert!(h.has_hold(start.token));
    h.hold_end(start.token, HoldEnd::Cancel, 40.).unwrap();
    assert_eq!(h.presented(row).opacity, 0.9);
    h.tick(10_000.);
    assert_eq!(h.presented(row).opacity, 0.25);
}

#[test]
fn held_action_commits_before_release_and_duplicate_or_foreign_tokens_are_stale() {
    let mut h = fixture();
    let row = view(&h, "row");
    let old = h.hold_begin(row, Property::Translate, 0.).unwrap().unwrap();
    h.hold_update(old.token, Value::new(90., 0.), 0.).unwrap();
    let current = h.hold_begin(row, Property::Translate, 0.).unwrap().unwrap();
    assert_eq!(current.value.x, 90.);
    assert!(!h
        .hold_update(old.token, Value::new(f64::NAN, 0.), f64::NAN)
        .unwrap());
    assert!(!h.dispatch_held(old.token, f64::NAN).unwrap());
    assert!(h.dispatch_held(current.token, 0.).unwrap());
    assert!(h.has_hold(current.token));
    assert_eq!(h.presented(row).translate.0, 90.);
    assert!(h.hold_end(current.token, HoldEnd::Cancel, 0.).unwrap());
    assert!(!h
        .hold_end(current.token, HoldEnd::Cancel, f64::NAN)
        .unwrap());
    let mut other = fixture();
    assert!(!other
        .hold_update(current.token, Value::ZERO, f64::NAN)
        .unwrap());
    assert_eq!(other.now(), 0.);
    h.tick(10_000.);
    assert_eq!(h.presented(row).translate.0, 0.);
    assert_eq!(h.presented(row).opacity, 0.25);
}

#[test]
fn destruction_rejects_stale_before_invalid_clock_and_unknown_begin_creates_nothing() {
    let mut h = fixture();
    let row = view(&h, "row");
    let start = h.hold_begin(row, Property::Translate, 0.).unwrap().unwrap();
    h.dispatch_at(view(&h, "remove"), Event::Press, 1.);
    assert!(!h.has_hold(start.token));
    assert!(!h.hold_end(start.token, HoldEnd::Cancel, f64::NAN).unwrap());
    assert!(h
        .hold_begin(row, Property::Translate, f64::NAN)
        .unwrap()
        .is_none());
    assert_eq!(h.now(), 1.);
    assert!(!h.motion());
}

#[test]
fn invalid_live_samples_leave_host_and_engine_clocks_and_presentation_unchanged() {
    let mut h = fixture();
    let row = view(&h, "row");
    let start = h.hold_begin(row, Property::Scale, 10.).unwrap().unwrap();
    for (value, time) in [
        (Value::new(0.5, 1.), 100.),
        (Value::scalar(0.5), 9.),
        (Value::scalar(f64::NAN), 100.),
    ] {
        assert!(h.hold_update(start.token, value, time).is_err());
        assert_eq!(h.now(), 10.);
        assert_eq!(h.engine().now(), 0.01);
        assert_eq!(h.presented(row).scale, 1.);
        assert!(h.has_hold(start.token));
    }
}

#[test]
fn zero_distance_spring_keeps_release_velocity_and_no_transition_snaps() {
    let mut h = fixture();
    let row = view(&h, "row");
    let start = h.hold_begin(row, Property::Translate, 0.).unwrap().unwrap();
    h.hold_end(
        start.token,
        HoldEnd::Release {
            velocity: Value::new(200., 0.),
        },
        0.,
    )
    .unwrap();
    assert!(h.motion());
    h.tick(10.);
    assert!(h.presented(row).translate.0 > 0.);
    let opacity = h.hold_begin(row, Property::Scale, 10.).unwrap().unwrap();
    h.hold_update(opacity.token, Value::scalar(0.5), 10.)
        .unwrap();
    h.hold_end(opacity.token, HoldEnd::Cancel, 10.).unwrap();
    assert_eq!(h.presented(row).scale, 1.);
}

#[test]
fn overdue_timers_keep_runner_order_without_rewinding_held_or_unrelated_motion() {
    let plan = contract::compile(
        r#"component App
  state value = 1
  action step
    value = value + 1
  task ticker mount
    every(100, step)
  view
    column
      box testId="held" opacity=(1 / value) transition="opacity 500ms linear"
      box testId="other" scale=value transition="scale 500ms linear"
"#,
    )
    .unwrap()
    .encode();
    let mut h = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let held = view(&h, "held");
    let other = view(&h, "other");
    let token = h
        .hold_begin(held, Property::Opacity, 150.)
        .unwrap()
        .unwrap()
        .token;
    h.hold_update(token, Value::scalar(0.8), 150.).unwrap();
    assert!(h.hold_update(token, Value::scalar(f64::NAN), 250.).is_err());
    assert!(h.agent("{\"op\":\"state\"}").contains("\"value\":1"));
    assert!(h.advance(250.).is_none());
    assert!(h.agent("{\"op\":\"state\"}").contains("\"value\":3"));
    assert_eq!(h.presented(held).opacity, 0.8);
    assert!(h.presented(other).scale > 1. && h.presented(other).scale < 3.);
    assert_eq!(h.engine().now(), 0.25);
    h.hold_end(token, HoldEnd::Cancel, 250.).unwrap();
    h.tick(750.);
    assert!((h.presented(held).opacity - 1. / 3.).abs() < 0.0001);
}
