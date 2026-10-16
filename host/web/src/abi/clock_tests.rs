//! The page's clock owns time until the agent's takes it (the timers RFC,
//! D1): a running runner and every one booted later take the choice.
use super::super::Bridge;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use std::cell::Cell;

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

thread_local! {
    // Each test's own page clock: the tests run in parallel
    static PAGE: Cell<f64> = const { Cell::new(0.) };
}

fn page() -> f64 {
    PAGE.get()
}

fn ticks(bridge: &Bridge<NoData>) -> f64 {
    let runner = bridge.host.as_ref().unwrap().runner();
    runner.slot("ticks").and_then(Value::as_number).unwrap()
}

#[test]
fn the_pages_clock_reaches_every_boot_until_the_agents_takes_time() {
    let plan = contract::compile(TICKER).unwrap().encode();
    let mut bridge = Bridge::<NoData>::new();
    PAGE.set(3500.0);
    bridge.set_live_clock(Some(page));
    bridge.boot(&plan, NoData, 400., 400., "/");
    bridge.advance(3500., 0);
    assert_eq!(ticks(&bridge), 1.0, "three missed beats, one fire");
    // A dev restart, carrying the state and the timer, keeps the page's clock.
    let len = bridge.input_write(&plan);
    bridge.boot_plan(len, NoData, 400., 400., "/");
    PAGE.set(7500.0);
    bridge.advance(7500., 0);
    assert_eq!(ticks(&bridge), 2.0, "four missed beats, one fire");
    // The agent's clock: every beat fires again.
    bridge.set_live_clock(None);
    bridge.advance(10_500., 0);
    assert_eq!(ticks(&bridge), 5.0);
}

#[test]
fn an_input_the_pages_clock_overtook_is_presented_at_the_inputs_time() {
    crate::link::link_for_tests();
    let source = r#"component App
  state on = false
  action go
    on = true
  view
    button "Go" press=go testId="go" opacity=(on ? 0 : 1) transition="opacity 1s linear"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let mut bridge = Bridge::<NoData>::new();
    PAGE.set(2600.0);
    bridge.set_live_clock(Some(page));
    bridge.boot(&plan, NoData, 400., 400., "/");
    let host = bridge.host.as_mut().unwrap();
    let k = host.runner().kernel();
    let go = k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id;
    host.dispatch_at(go, exact_runner::Event::Press, 2000.);
    assert_eq!(
        host.runner().now_ms(),
        2600.0,
        "the action observed the clock"
    );
    // The fade starts at the input's time: a release at 2000 can still end
    // a hold there.
    assert_eq!(host.motion_now(), 2.0);
}
