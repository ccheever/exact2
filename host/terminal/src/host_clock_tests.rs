//! The loop's clock owns the terminal's time (the timers RFC, D1): a repeat
//! that missed beats while the process was stopped fires once.
use super::*;
use exact_plan::Value;
use std::time::{Duration, Instant};

const TICKER: &str = r#"component App
  state ticks = 0
  action tick
    ticks = ticks + 1
  task ticker mount
    every(1000, tick)
  view
    text "x"
"#;

#[test]
fn a_loop_back_from_a_stop_fires_a_missed_repeat_once() {
    let plan = contract::compile(TICKER).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 80, 40).unwrap();
    // The loop started 3.5 s ago and was stopped since.
    let start = Instant::now() - Duration::from_millis(3500);
    host.live_from(start);
    host.tick(start.elapsed().as_secs_f64() * 1000.0);
    let ticks = host.runner.slot("ticks").and_then(Value::as_number);
    assert_eq!(ticks, Some(1.0), "three missed beats, one fire");
}
