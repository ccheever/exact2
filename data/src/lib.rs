//! Language-independent application data capabilities and composition.
//! @ref LLP 1027.001 — the same operation behind either language's data seam.

pub mod envelope;
mod mixed;
pub mod placed;
pub mod storage;
pub use mixed::Mixed;
pub use placed::Placed;

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store, Target};

/// `answer`, for `target` when the caller named one: a composer records a
/// call with its target and hands the target on (LLP 1027 D1a).
fn answer<D: DataSource + ?Sized>(
    source: &mut D,
    target: Option<Target>,
    store: &mut Store,
    name: &str,
    args: &[Value],
) -> Result<Answer, DataError> {
    match target {
        Some(target) => source.answer_for(target, store, name, args),
        None => source.answer(store, name, args),
    }
}

/// `parse`, for `target` when the caller named one; see [`answer`].
fn parse<D: DataSource + ?Sized>(
    source: &mut D,
    target: Option<Target>,
    store: &mut Store,
    name: &str,
    args: &[Value],
    outcome: Outcome,
) -> Result<Answer, DataError> {
    match target {
        Some(target) => source.parse_for(target, store, name, args, outcome),
        None => source.parse(store, name, args, outcome),
    }
}

#[cfg(test)]
mod mixed_tests;
