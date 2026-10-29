//! An orderly exit (LLP 1097 D10): the module's storage an answer started
//! and did not await is finished before the process ends, the host pumping
//! its background rounds as it does while running, within five seconds;
//! what is left then is dropped and said; then the store's writes are on
//! disk. A native dev restart's bound is the module's own (a second,
//! `exact_js::Module::unload`).
use crate::presenter::Presenter;
use exact_runner::DataSource;
use std::time::{Duration, Instant};

/// How long an orderly exit waits for the module's storage.
pub const EXIT_BOUND: Duration = Duration::from_secs(5);

/// Pump until the module has no storage queued or in flight, or `bound`.
pub fn finish<D: DataSource>(p: &mut Presenter<D>, bound: Duration) {
    // Every hatch scope ends before the module goes (LLP 1075.003.000.001 §2.1).
    p.end_hatches();
    p.end_packages();
    let deadline = Instant::now() + bound;
    while p.host().runner().background_operations() > 0 {
        if Instant::now() >= deadline {
            eprintln!(
                "exact: background: dropped at exit, {} storage operations waiting",
                p.host().runner().background_operations()
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
        if let Some(e) = p.pump(p.host().now()) {
            eprintln!("exact: {e}");
        }
    }
    // The store's writes, which its own thread makes, are on disk before the process ends (a moment past the
    // bound when the module took all of it).
    let left = deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(500));
    if !crate::host::drain_store_writes(left) {
        eprintln!("exact: the store's writes were not all on disk at exit");
    }
    for line in crate::host::take_store_warnings(p.host().runner().data_ref().app_id()) {
        eprintln!("exact: {line}");
    }
}
