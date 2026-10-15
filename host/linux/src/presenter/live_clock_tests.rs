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
