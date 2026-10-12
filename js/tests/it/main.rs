//! The executor's integration tests: one binary, so one link and one launch.
//! `tests/fixtures/` stays put: build.rs compiles those to bytecode.

mod admission;
mod background;
mod body_from;
mod caltrain;
mod castle;
mod compress;
mod ecdsa;
mod entropy;
mod forget;
mod inputs;
mod interrupt;
mod overlay;
mod pinned_engines;
mod pure;
mod render;
mod storage;
mod superseded;
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
