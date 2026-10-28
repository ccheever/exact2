//! The executor's integration tests: one binary, so one link and one launch.
//! `tests/fixtures/` stays put: build.rs compiles those to bytecode.

mod abort;
mod admission;
mod background;
mod body_from;
mod body_to;
mod caltrain;
mod castle;
mod compress;
mod context;
mod doc_copy;
mod ecdsa;
mod entropy;
mod forget;
mod heap;
mod inputs;
mod interrupt;
mod listen;
mod native_later;
mod no_store;
mod outlive;
mod overlay;
mod overrun;
mod pinned_engines;
mod presence;
mod pure;
mod render;
mod snapshot;
mod socket;
mod storage;
mod superseded;
mod timers;
mod turn_budget;
mod walk;
mod watch;
mod windows_lean;

/// What a host runs for `dispatch`: its work, or for a re-ask
/// (`Dispatch::Again`, LLP 1041 §8.4) the empty success a host settles it
/// with, as a harness standing in for a host's executor needs. These
/// harnesses test the module, not admission: a re-ask's place in a native
/// executor is tested through the real one (`admission.rs`, and the core's
/// `executor_again_tests.rs`).
pub fn runnable(dispatch: exact_runner::Dispatch) -> exact_runner::Dispatch {
    use exact_runner::{Dispatch, Work};
    match dispatch {
        Dispatch::Again => Dispatch::Run(Work::Now(Box::new(Dispatch::again_outcome))),
        other => other,
    }
}

/// A module's journal without the line a slow top-level evaluation adds
/// (`SLOW_LOAD_MS`): whether it appears depends on how fast the machine ran
/// the load, not on what the module did.
pub(crate) fn authored_logs(logs: Vec<String>) -> Vec<String> {
    logs.into_iter()
        .filter(|line| !line.starts_with("the module's top level took "))
        .collect()
}
