//! The presenter's clock owns time until the agent's takes it (the timers
//! RFC, D1): a running runner and every one booted later take the choice.
use super::super::{Bridge, Hooks};
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use std::sync::atomic::{AtomicU64, Ordering};

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

static WALL: AtomicU64 = AtomicU64::new(0);

extern "C" fn wall() -> f64 {
    f64::from_bits(WALL.load(Ordering::SeqCst))
}

fn booted(bridge: &mut Bridge<NoData>) {
    let plan = contract::compile(TICKER).unwrap().encode();
    bridge.boot(&plan, NoData, Hooks::none(), 390.0, 844.0);
}

fn ticks(bridge: &Bridge<NoData>) -> f64 {
    bridge
        .host
        .as_ref()
        .unwrap()
        .runner()
        .slot("ticks")
        .and_then(Value::as_number)
        .unwrap()
}

#[test]
fn the_presenters_clock_reaches_every_boot_until_the_agents_takes_time() {
    WALL.store(3500f64.to_bits(), Ordering::SeqCst);
    let mut bridge = Bridge::<NoData>::new();
    // Told before the boot, as `primeTime` does.
    bridge.set_live_clock(Some(wall));
    booted(&mut bridge);
    bridge.advance(3500.0, 0);
    assert_eq!(ticks(&bridge), 1.0, "three missed beats, one fire");
    // The agent takes the clock: every beat fires again.
    bridge.set_live_clock(None);
    bridge.advance(6500.0, 0);
    assert_eq!(ticks(&bridge), 4.0);
    // Back on the presenter's clock, a reboot keeps it.
    bridge.set_live_clock(Some(wall));
    booted(&mut bridge);
    bridge.advance(3500.0, 0);
    assert_eq!(ticks(&bridge), 1.0);
}
