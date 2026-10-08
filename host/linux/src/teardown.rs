//! An orderly exit (LLP 1097 D10): the module's storage an answer started
//! and did not await is finished before the process ends, the host pumping
//! its background rounds as it does while running, within five seconds;
//! what is left then is dropped and said. A native dev restart's bound is
//! the module's own (a second, `exact_js::Module::unload`).
use crate::presenter::Presenter;
use exact_runner::DataSource;
use std::time::{Duration, Instant};

/// How long an orderly exit waits for the module's storage.
pub const EXIT_BOUND: Duration = Duration::from_secs(5);

/// Pump until the module has no storage queued or in flight, or `bound`.
pub fn finish<D: DataSource>(p: &mut Presenter<D>, bound: Duration) {
    // Every hatch scope ends before the module goes (LLP 1075.003.000.001 §2.1).
    p.end_hatches();
    let deadline = Instant::now() + bound;
    while p.host().runner().background_operations() > 0 {
        if Instant::now() >= deadline {
            eprintln!(
                "exact: background: dropped at exit, {} storage operations waiting",
                p.host().runner().background_operations()
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
        if let Some(e) = p.pump(p.host().now()) {
            eprintln!("exact: {e}");
        }
    }
}
