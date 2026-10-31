//! An orderly exit (LLP 1097 D10): the module's storage an answer started
//! and did not await, and a request with effects in flight (LLP 1146 D2),
//! finish before the process ends, the loop's turns running as they do while
//! the app runs, without input, within five seconds (a second for an agent's
//! quit, whose driver kills it soon after); what is left then is dropped and
//! said; then the hatches and packages end, and the store's writes are on
//! disk. A native dev restart's bound is the module's own (a second,
//! `exact_js::Module::unload`).
use crate::presenter::Presenter;
use exact_runner::DataSource;
use std::time::{Duration, Instant};

/// How long an orderly exit waits for the work the app must finish.
pub const EXIT_BOUND: Duration = Duration::from_secs(5);
/// How long an agent's quit waits: the driver kills the process two seconds
/// after closing its input (`scripts/agent-stdio.mjs` `close`), and the wait
/// and then the store's drain, half a second at least, both fit in that.
pub const AGENT_EXIT_BOUND: Duration = Duration::from_secs(1);

/// Run the loop's turns until the app need finish nothing
/// (`Runner::must_finish`), or `bound`: replies land, and the module calls
/// their turns make are served (`package_turn`). It answers what it dropped
/// at the bound, as it says it, if anything.
pub fn finish<D: DataSource>(p: &mut Presenter<D>, bound: Duration) -> Option<String> {
    let deadline = Instant::now() + bound;
    while p.host().runner().must_finish() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
        if let Some(e) = p.pump(p.host().now()) {
            eprintln!("exact: {e}");
        }
        p.package_turn();
    }
    let runner = p.host().runner();
    let dropped = runner.must_finish().then(|| {
        let mut left = Vec::new();
        let storage = runner.background_operations();
        if storage > 0 {
            left.push(format!("{storage} storage operations waiting"));
        }
        if runner.effects_in_flight() {
            left.push("a request with effects in flight".to_owned());
        }
        left.join(" and ")
    });
    if let Some(left) = &dropped {
        eprintln!("exact: background: dropped at exit, {left}");
    }
    // Every hatch scope ends before the module goes (LLP 1075.003.000.001 §2.1).
    p.end_hatches();
    p.end_packages();
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
    dropped
}
