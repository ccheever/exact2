//! Opt-in startup timestamps, using the host's existing Instant epoch.
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};
static START: OnceLock<Instant> = OnceLock::new();
static SEEN: Mutex<[bool; 2]> = Mutex::new([false; 2]);
pub(crate) fn start(at: Instant) {
    if std::env::var_os("EXACT_WORLD_TIMING").is_some() {
        let _ = START.set(at);
    }
}
pub(crate) fn enabled() -> bool {
    START.get().is_some()
}
pub(crate) fn tick(at: Instant, reply: &serde_json::Value) {
    if reply["world"]["tick"].as_u64().is_some_and(|t| t > 0) {
        record(0, "first_tick", at);
    }
}
pub(crate) fn published() {
    record(1, "first_publication", Instant::now());
}
fn record(index: usize, event: &str, at: Instant) {
    let Some(start) = START.get() else {
        return;
    };
    let mut seen = SEEN.lock().unwrap();
    if !seen[index] {
        seen[index] = true;
        eprintln!(
            "exact-world-startup: {{\"event\":\"{event}\",\"ms\":{}}}",
            at.duration_since(*start).as_secs_f64() * 1000.
        );
    }
}
