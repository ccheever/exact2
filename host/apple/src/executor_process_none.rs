//! A host whose process transport is not built: every real child, and the
//! questions only a real program answers, refuse with `Unsupported` (LLP
//! 1133.007 §8). A script under the agent still answers (§7).
use super::supervise::Child;
use super::{failed, ChildCtl, Ran, Shared, Spec};
use exact_runner::{FailureKind, Outcome};
use ibex2::stdlib::abort::AbortController;
use std::collections::HashMap;
use std::sync::Arc;

fn unsupported() -> Outcome {
    failed(
        FailureKind::Unsupported,
        "this host starts no programs yet (LLP 1133.007: its process transport is stage 1's)",
    )
}

pub(super) fn spawn(
    _shared: &Arc<Shared>,
    _ticket: u64,
    _n: u32,
    _spec: &Spec,
    _ran: &Ran,
    _ctl: &Arc<ChildCtl>,
) -> Result<Box<dyn Child>, Outcome> {
    Err(unsupported())
}

pub(super) fn exec(
    _shared: &Arc<Shared>,
    _spec: &Spec,
    _ran: &Ran,
    _abort: &AbortController,
) -> Outcome {
    unsupported()
}

pub(super) fn resolve(
    _program: &str,
    _env: Option<&[(String, String)]>,
) -> Result<Option<String>, Outcome> {
    Err(unsupported())
}

/// The login shell is the transport's to run.
pub(super) fn login_variables(_names: &[&str]) -> Result<HashMap<String, String>, Outcome> {
    Err(unsupported())
}

pub(super) fn quit(_grace_ms: f64) {}
