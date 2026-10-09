//! Why a resource failed, as `failure(x)` answers it (LLP 1109 D3): a code
//! from a closed vocabulary an app can branch on, the same on every host,
//! and the message the agent's `state.failed` shows, which says more and
//! differs by host.
//!
//! A code names what reached the runner. A request's failure (`offline`,
//! `timeout`, `refused`) or a storage call's (`storage`) is the source's
//! only when the source lets it through: a TypeScript module that rethrows
//! the `fetch` rejection, or does not catch it. An error the source makes of
//! its own, an HTTP error status among them (a status is an answer, never a
//! failure), is `error`, and so is a Rust source's own `Err`, which crosses
//! its seam as `Unavailable`.

use crate::request::FailureKind;
use crate::runner::{DataError, RunnerError};

/// The closed vocabulary, documented in `docs/contract-grammar.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCode {
    /// A request the source made reached no server: no connection, DNS,
    /// TLS, a reset (`FailureKind::Network`, the driver's `fail fetch`).
    Offline,
    /// A request's deadline (`exactTimeout`) passed (`FailureKind::Timeout`).
    Timeout,
    /// The host refused a request outside the app's grants or limits: its
    /// admission, or a response over its size limit, the same on every host
    /// (`FailureKind::Refused`).
    Refused,
    /// The answer is outside the resource's declared shape.
    Shape,
    /// A storage call failed (a coded storage refusal: `denied`, `full`,
    /// `EBUSY`, a filesystem error, storage unavailable here).
    Storage,
    /// Anything else: the source's own error, an aborted request, a host
    /// that cannot make it, a module's budget, a seam that cannot carry it.
    Error,
}

impl FailureCode {
    /// Every code, in the grammar's order.
    pub const ALL: [FailureCode; 6] = [
        FailureCode::Offline,
        FailureCode::Timeout,
        FailureCode::Refused,
        FailureCode::Shape,
        FailureCode::Storage,
        FailureCode::Error,
    ];

    /// The code's spelling in Contract.
    pub fn name(self) -> &'static str {
        match self {
            FailureCode::Offline => "offline",
            FailureCode::Timeout => "timeout",
            FailureCode::Refused => "refused",
            FailureCode::Shape => "shape",
            FailureCode::Storage => "storage",
            FailureCode::Error => "error",
        }
    }

    /// The code spelled `name`.
    pub fn from_name(name: &str) -> Option<FailureCode> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// What a TypeScript module let through, by the class its seam names
    /// (`failure` in js/src/prelude.js `fail`): `Failed`, or `Unavailable`
    /// for its own error.
    pub fn seam_error(failure: Option<&str>, message: String) -> DataError {
        match failure.and_then(FailureCode::from_name) {
            Some(code) if code != FailureCode::Error => DataError::Failed(code, message),
            _ => DataError::Unavailable(message),
        }
    }

    /// A failed request's code, when the source lets its failure through.
    pub fn of_kind(kind: FailureKind) -> FailureCode {
        match kind {
            FailureKind::Network => FailureCode::Offline,
            FailureKind::Timeout => FailureCode::Timeout,
            FailureKind::Refused => FailureCode::Refused,
            FailureKind::Unsupported | FailureKind::Aborted => FailureCode::Error,
        }
    }
}

/// Why a resource's latest request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// What `failure(x).code` reads.
    pub code: FailureCode,
    /// What `failure(x).message` and `state.failed` read.
    pub message: String,
}

impl Failure {
    /// The failure a reply's error makes.
    pub fn of(error: &RunnerError) -> Failure {
        let (code, message) = match error {
            RunnerError::Data { error, .. } => match error {
                DataError::Failed(code, s) => (*code, s.clone()),
                DataError::UnknownSource(s)
                | DataError::BadArguments(s)
                | DataError::Unavailable(s)
                | DataError::DeferredAtBake(s)
                | DataError::Interface(s) => (FailureCode::Error, s.clone()),
            },
            RunnerError::Shape { resource, why } => (
                FailureCode::Shape,
                format!("`{resource}` answered outside its shape: {why}"),
            ),
            other => (FailureCode::Error, format!("{other:?}")),
        };
        Failure { code, message }
    }

    /// An `error` saying `message`.
    pub fn error(message: String) -> Failure {
        Failure {
            code: FailureCode::Error,
            message,
        }
    }
}

/// Resource `i`'s failure, or `None` while it has not failed: what
/// `failure(x)` reads, so a reader is current only while this is unchanged.
pub(crate) fn state(
    failed: &[Option<Vec<exact_plan::Value>>],
    why: &[Option<Failure>],
    i: usize,
) -> Option<Failure> {
    failed.get(i)?.as_ref()?;
    Some(
        why.get(i)
            .cloned()
            .flatten()
            .unwrap_or(Failure::error("it failed".into())),
    )
}

/// Every resource's [`state`].
pub(crate) fn states(
    failed: &[Option<Vec<exact_plan::Value>>],
    why: &[Option<Failure>],
) -> Vec<Option<Failure>> {
    (0..failed.len()).map(|i| state(failed, why, i)).collect()
}
