//! The display's clock owns time until a driver's takes it (the timers RFC,
//! D1): a running runner and every one a reload boots take the choice.
use super::super::{PainterChoice, Presenter};
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const TICKER: &str = r#"component App
  state ticks = 0
  action tick
    ticks = ticks + 1
  task ticker mount
    every(1000, tick)
  view
    text "x"
"#;

fn ticks(p: &Presenter<NoData>) -> f64 {
    p.host()
        .runner()
        .slot("ticks")
        .and_then(Value::as_number)
        .unwrap()
}

#[test]
fn the_display_clock_reaches_every_host_until_a_driver_takes_time() {
    let plan = crate::test_plan(TICKER);
    let (mut p, _) = Presenter::boot_with(
        &plan,
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    let wall = Rc::new(Cell::new(3500.0));
    let read = wall.clone();
    p.set_live_clock(Some(Rc::new(move || read.get())));
    p.clock(3500.0);
    assert_eq!(ticks(&p), 1.0, "three missed beats, one fire");
    // A reload's runner, carrying the state and the timer, takes the
    // display's clock too.
    p.reload(&plan, NoData).unwrap();
    wall.set(7500.0);
    p.clock(7500.0);
    assert_eq!(ticks(&p), 2.0, "four missed beats, one fire");
    // A driver takes the clock: every beat fires again.
    p.set_live_clock(None);
    p.clock(10_500.0);
    assert_eq!(ticks(&p), 5.0);
}

#[test]
fn an_input_the_live_clock_overtook_commits_without_rewinding_motion() {
    let source = r#"component App
  state on = false
  action go
    on = true
  view
    button "Go" press=go testId="go" opacity=(on ? 0 : 1) transition="opacity 1s linear"
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    p.set_live_clock(Some(Rc::new(|| 2600.0)));
    let k = p.host().kernel();
    let go = k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id;
    // The event says 2000; the runner observes 2600. The host presents the
    // commit at the input's time, and what follows it runs from there.
    let error = p.host.dispatch_at(go, exact_runner::Event::Press, 2000.0);
    assert!(error.is_none(), "{error:?}");
    assert_eq!(p.host().now(), 2000.0);
    assert_eq!(p.host().runner().now_ms(), 2600.0);
}

#[test]
fn retained_input_clock_runs_due_timer_before_action_and_arms_from_input() {
    let source = r#"component App
  state fired = 0
  state pressed = false
  action timer
    fired = 1
  action go
    pressed = true
  action later
    fired = 2
  task first when !pressed
    after(150, timer)
  task second when pressed
    after(50, later)
  view
    button "Go" press=go testId="go"
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host.advance(100.).is_none());
    let key = p.host.kernel().find_by_test_id("go")[0];
    let binding = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Press)
        .unwrap();
    assert_eq!(
        p.host
            .dispatch_retained(key, &binding, exact_plan::EventKind::Press, 200.),
        Ok(true)
    );
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Number(1.)));
    assert_eq!(p.host.runner().timer_due_ms(), Some(250.));
    assert!(p.host.advance(250.).is_none());
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Number(2.)));
}

#[test]
fn retained_input_clock_refusals_do_not_sample_or_fire_timers() {
    let source = r#"component App
  state value = "old"
  state disabled = false
  state fired = false
  action timer
    fired = true
  action change
    value = "new"
  action disable
    disabled = true
  action go(value: string)
    fired = true
  task first mount
    after(150, timer)
  view
    button "Go" press=go(value) testId="go" disabled=disabled
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host.advance(100.).is_none());
    let key = p.host.kernel().find_by_test_id("go")[0];
    let binding = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Press)
        .unwrap();
    let samples = Rc::new(Cell::new(0));
    let reads = samples.clone();
    p.set_live_clock(Some(Rc::new(move || {
        reads.set(reads.get() + 1);
        200.
    })));
    for at in [f64::NAN, f64::INFINITY, 99.] {
        assert_eq!(
            p.host
                .dispatch_retained(key, &binding, exact_plan::EventKind::Press, at),
            Ok(false)
        );
    }
    assert_eq!(samples.get(), 0);
    p.set_live_clock(None);
    p.host.runner_mut().act("change", vec![]).unwrap();
    let before = p.host.runner().kernel().epoch();
    assert_eq!(
        p.host
            .dispatch_retained(key, &binding, exact_plan::EventKind::Press, 200.),
        Ok(false)
    );
    assert_eq!(p.host.runner().kernel().epoch(), before);
    assert_eq!(p.host.runner().now_ms(), 100.);
    assert_eq!(p.host.now(), 100.);
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Bool(false)));
    let current = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Press)
        .unwrap();
    p.host.runner_mut().act("disable", vec![]).unwrap();
    assert_eq!(
        p.host
            .dispatch_retained(key, &current, exact_plan::EventKind::Press, 200.),
        Ok(false)
    );
    assert_eq!(p.host.runner().now_ms(), 100.);
    assert_eq!(p.host.runner().slot("fired"), Some(&Value::Bool(false)));
}

#[test]
fn retained_input_clock_keeps_live_release_time_and_coalesces_missed_beats() {
    let source = r#"component App
  state ticks = 0
  state pressed = false
  action timer
    ticks = ticks + 1
  action go
    pressed = true
  task ticker mount
    every(100, timer)
  view
    box swiperight=go testId="go" transition="translate -exact-spring(300, 30, 1)"
      text "Swipe"
"#;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    let key = p.host.kernel().find_by_test_id("go")[0];
    let view = p.host.kernel().node_by_key(key).unwrap().id;
    let binding = p
        .host
        .runner()
        .capture_action_binding(key, exact_plan::EventKind::Swiperight)
        .unwrap();
    let held = p
        .host
        .hold_begin(view, exact_motion::Property::Translate, 0.)
        .unwrap()
        .unwrap();
    p.set_live_clock(Some(Rc::new(|| 2600.)));
    assert_eq!(
        p.host
            .dispatch_retained_held(held.token, key, &binding, 2000.),
        Ok(true)
    );
    assert_eq!(ticks(&p), 1.);
    assert_eq!(p.host.runner().slot("pressed"), Some(&Value::Bool(true)));
    assert_eq!(p.host.now(), 2000.);
    assert_eq!(p.host.runner().now_ms(), 2600.);
    assert_eq!(
        p.host
            .hold_end(held.token, exact_motion::HoldEnd::Cancel, 2000.),
        Ok(true)
    );
    let before = p.host.runner().kernel().epoch();
    assert_eq!(
        p.host
            .dispatch_retained_held(held.token, key, &binding, 3000.),
        Ok(false)
    );
    assert_eq!(p.host.runner().kernel().epoch(), before);
    assert_eq!(p.host.now(), 2000.);
}
