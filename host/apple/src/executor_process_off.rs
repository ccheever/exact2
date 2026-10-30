//! Desktop processes where no host starts programs (iOS, tvOS, Android,
//! Windows until LLP 1133.007's stage 3): every process request answers
//! `Unsupported` (§8), and none of the supervision is linked, so an app
//! there carries no process code (§14).
use super::{complete, failed, Core, Shared};
use exact_runner::{FailureKind, Request, SocketOp};
use ibex2::stdlib::abort::AbortController;
use std::sync::{Arc, Condvar};

pub(crate) struct Processes {
    pub(crate) drained: Condvar,
}

impl Processes {
    pub(crate) fn new() -> Processes {
        Processes {
            drained: Condvar::new(),
        }
    }
}

pub(super) fn run(
    shared: &Arc<Shared>,
    ticket: u64,
    _request: Request,
    _grants: &str,
    _host: &dyn Fn() -> ibex2::host::Host,
    _abort: AbortController,
) {
    let why = "this host starts no programs (LLP 1133.007 §8)";
    complete(shared, ticket, failed(FailureKind::Unsupported, why));
}

/// No child is running, so a batch naming one names an ended incarnation.
pub(crate) fn check(_shared: &Shared, ops: &[(u64, &SocketOp)]) -> Result<(), String> {
    match ops.is_empty() {
        true => Ok(()),
        false => Err("InvalidStateError: the child's incarnation has ended".into()),
    }
}

pub(crate) fn send(_shared: &Shared, _ops: &[(u64, &SocketOp)]) -> Vec<(u64, u64, u64)> {
    Vec::new()
}

pub(crate) fn queued(_shared: &Shared, _ticket: u64) -> Option<u64> {
    None
}

pub(crate) fn quit(_shared: &Shared) {}

pub(crate) fn launched(_shared: &Arc<Shared>) {}

/// A one-shot answers at once here, so it needs no room.
pub(crate) fn admit_one_shot(
    _shared: &Shared,
    _ticket: u64,
    _ceiling: usize,
) -> Result<(), &'static str> {
    Ok(())
}

pub(crate) fn release_one_shot(_shared: &Shared, _ticket: u64) {}

impl Core {
    pub(crate) fn allow_processes(&self) {}

    pub(crate) fn work_clock(&self) -> Option<Arc<dyn exact_runner::WorkClock>> {
        None
    }
}
