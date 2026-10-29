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
    WALL.store(500f64.to_bits(), Ordering::SeqCst);
    let mut bridge = Bridge::<NoData>::new();
    // Told before the boot, as `primeTime` does.
    bridge.set_live_clock(Some(wall));
    booted(&mut bridge);
    WALL.store(3500f64.to_bits(), Ordering::SeqCst);
    bridge.advance(3500.0, 0);
    assert_eq!(ticks(&bridge), 1.0, "three missed beats, one fire");
    // The agent takes the clock: every beat fires again.
    bridge.set_live_clock(None);
    bridge.advance(6500.0, 0);
    assert_eq!(ticks(&bridge), 4.0);
    // Back on the presenter's clock, a reboot keeps it, and starts where it
    // reads: its first beat is a beat after the boot, not one missed since 0.
    bridge.set_live_clock(Some(wall));
    booted(&mut bridge);
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().timer_due_ms(),
        Some(4500.0)
    );
    bridge.advance(3500.0, 0);
    assert_eq!(ticks(&bridge), 0.0);
    WALL.store(7500f64.to_bits(), Ordering::SeqCst);
    bridge.advance(7500.0, 0);
    assert_eq!(ticks(&bridge), 1.0, "four missed beats, one fire");
}

#[test]
fn the_agents_clock_boots_at_zero() {
    let mut bridge = Bridge::<NoData>::new();
    bridge.set_live_clock(None);
    booted(&mut bridge);
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().timer_due_ms(),
        Some(1000.0)
    );
}

#[test]
fn an_idle_viewport_resize_samples_the_presenters_clock_before_layout() {
    static OBSERVED: AtomicU64 = AtomicU64::new(0);
    extern "C" fn observed() -> f64 {
        f64::from_bits(OBSERVED.load(Ordering::SeqCst))
    }
    let plan = contract::compile(
        r#"component App
  state wide = false
  state width = 0
  task grow mount
    after(100, grow)
  action grow
    wide = true
  action resized(w: number, h: number, r: DOMRectReadOnly)
    width = w
  view
    view width=(wide ? 300 : 100) height=20 resize=resized
"#,
    )
    .unwrap()
    .encode();
    OBSERVED.store(500f64.to_bits(), Ordering::SeqCst);
    let mut bridge = Bridge::<NoData>::new();
    bridge.set_live_clock(Some(observed));
    bridge.boot(&plan, NoData, Hooks::none(), 100., 100.);
    OBSERVED.store(750f64.to_bits(), Ordering::SeqCst);
    bridge.resize(200., 100.);
    let host = bridge.host.as_ref().unwrap();
    assert_eq!(host.runner().slot("width"), Some(&Value::Number(300.)));
    assert_eq!(host.runner().now_ms(), 750.);
}

#[test]
fn wait_admission_crosses_the_apple_batch_boundary() {
    static NOW: AtomicU64 = AtomicU64::new(0);
    extern "C" fn sample() -> f64 {
        f64::from_bits(NOW.load(Ordering::SeqCst))
    }
    for (kind, mask) in [("frame", 2), ("update", 4), ("scrollend", 8), ("idle", 16)] {
        NOW.store(0.0f64.to_bits(), Ordering::SeqCst);
        let plan = contract::compile(&TICKER.replace(
            "every(1000, tick)",
            &format!("after(10, tick, wait={kind})"),
        ))
        .unwrap()
        .encode();
        let mut bridge = Bridge::<NoData>::new();
        bridge.set_live_clock(Some(sample));
        bridge.boot(&plan, NoData, Hooks::none(), 400., 400.);
        bridge.timer_waits(30, 0.0, 0);
        NOW.store(10.0f64.to_bits(), Ordering::SeqCst);
        bridge.advance(10.0, 0);
        assert_eq!(ticks(&bridge), 0.0, "{kind}");
        bridge.timer_waits(mask, 10.0, 1);
        assert_eq!(ticks(&bridge), 1.0, "{kind}");
        bridge.advance(110.0, 0);
        assert_eq!(ticks(&bridge), 1.0);
    }
}

#[test]
fn native_timer_batches_keep_independent_windows_across_boots_and_delivery() {
    extern "C" fn native_wall() -> f64 {
        0.0
    }
    let mut bridge = Bridge::<NoData>::new();
    bridge.native_timers(true);
    bridge.set_live_clock(Some(native_wall));
    for _ in 0..2 {
        let source = TICKER.replace("every(1000, tick)", "every(1000, tick, tolerance=100)");
        let plan = contract::compile(&source).unwrap().encode();
        bridge.boot(&plan, NoData, Hooks::none(), 390.0, 844.0);
        let batch: serde_json::Value = serde_json::from_slice(&bridge.output).unwrap();
        assert_eq!(batch["timers"], true);
        assert!(batch.get("timer_due_ms").is_none());
        assert!(batch.get("timer_tolerance_ms").is_none());
        assert_eq!(batch["nativeTimers"][0]["due"], 1000);
        assert_eq!(batch["nativeTimers"][0]["tolerance"], 100);
        bridge.advance(1100.0, 0);
        assert_eq!(ticks(&bridge), 0.0);
        bridge.fire_native_timer(0, 0, 1100.0);
        assert_eq!(ticks(&bridge), 1.0);
        let batch: serde_json::Value = serde_json::from_slice(&bridge.output).unwrap();
        assert_eq!(batch["nativeTimers"][0]["due"], 2000);
        assert_eq!(batch["nativeTimers"][0]["generation"], "0");
    }
    bridge.set_live_clock(None);
    bridge.advance(2000.0, 0);
    let batch: serde_json::Value = serde_json::from_slice(&bridge.output).unwrap();
    assert_eq!(batch["nativeTimers"], serde_json::json!([]));
    assert_eq!(ticks(&bridge), 2.0);
}
