//! A settle's requests: handed to the executor, and an app's aborts answered.
use super::{lift, run, Dispatch, Executor, Held};
use exact_runner::{DataSource, Runner, RunnerError};

pub(super) fn hand_out<D: DataSource>(
    runner: &mut Runner<D>,
    executor: &Executor,
    held: &mut Held,
) {
    executor.forget(|ticket| runner.holds(ticket));
    // Of the uploads it let go, which had begun to go out (LLP 1016.000 D10).
    let started = executor.let_go_started(|ticket| runner.holds(ticket));
    runner.uploads_started(&started);
    lift(executor, held);
    for r in runner.take_requests() {
        // Device capabilities: the environment has none; nor may a long poll run on other work (LLP 1016.000 D8).
        if r.request.surface.is_some()
            || r.request.storage.is_some()
            || r.request.long_poll_refusal().is_some()
        {
            held.refused.insert(r.ticket);
            continue;
        }
        let dispatch = match r.request.continuation {
            Some(token) => runner.dispatch_work(token),
            None if r.request.is_native() => runner.native_work(&r.request),
            None => Dispatch::Missing,
        };
        run(executor, held, r, dispatch);
    }
    for (token, dispatch) in runner.release_work() {
        if let Some(r) = held.parked.remove(&token) {
            run(executor, held, r, dispatch);
        }
    }
    // Again for what this batch put behind a refusal with nothing earlier
    // in flight: no completion would come to lift it.
    lift(executor, held);
}

/// A fetch the app aborted: its answer goes on now, and its request is
/// cancelled whatever its method, as on a host. Whether any answer went on.
pub(super) fn settle_aborts<D: DataSource>(
    runner: &mut Runner<D>,
    executor: &Executor,
) -> Result<bool, RunnerError> {
    runner.collect_aborts();
    let mut aborted = false;
    while let Some((ticket, outcome)) = runner.take_aborted_answer() {
        runner.fulfill_measured(ticket, outcome, None)?;
        aborted = true;
    }
    executor.cancel(&runner.take_explicit_aborts());
    Ok(aborted)
}
