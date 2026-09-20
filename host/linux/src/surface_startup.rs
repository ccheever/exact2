//! Opt-in startup timestamps, using the host's existing Instant epoch.
use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};
static START: OnceLock<Instant> = OnceLock::new();
static SEEN: Mutex<[bool; 11]> = Mutex::new([false; 11]);
pub(crate) fn start(at: Instant) {
    if std::env::var_os("EXACT_WORLD_TIMING").is_some() {
        let _ = START.set(at);
    }
}
pub(crate) fn record(index: usize, event: &str) {
    let Some(start) = START.get() else {
        return;
    };
    let at = Instant::now();
    let mut seen = SEEN.lock().unwrap();
    if !seen[index] {
        seen[index] = true;
        eprintln!(
            "exact-world-startup: {{\"event\":\"{event}\",\"ms\":{}}}",
            at.duration_since(*start).as_secs_f64() * 1000.
        );
    }
}
pub(crate) fn published() {
    record(10, "first_publication");
}
