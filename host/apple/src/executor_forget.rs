//! Letting requests go: the runner's forgotten ones (safe reads cancelled,
//! the rest let run) and an app's aborted ones (cancelled whatever their method).
use super::*;

impl Core {
    /// Let go of the work for every ticket the runner no longer `held`
    /// (LLP 1016 D5): its reply would only be dropped there. A completed
    /// outcome is dropped undrained and a queued safe HTTP read unrun; a
    /// running one is aborted. Other work (a write, a module turn) still runs,
    /// since a write that was sent, or a turn that has begun, is not undone.
    /// None of it holds a later ordered completion back, and each releases
    /// its count and bytes when it ends.
    pub(crate) fn forget(&self, held: impl Fn(u64) -> bool) {
        self.forget_with(held, &[]);
    }

    /// Cancel the requests an app aborted (`Runner::take_explicit_aborts`),
    /// whatever their method, as a browser does: a queued one is dropped
    /// unsent and a running exchange's transport aborted, which releases
    /// its lane's count, bytes and places when it ends. Work and streams are
    /// let go as `forget` lets them go.
    pub(crate) fn cancel(&self, tickets: &[u64]) {
        if !tickets.is_empty() {
            self.forget_with(|ticket| !tickets.contains(&ticket), tickets);
        }
    }

    fn forget_with(&self, held: impl Fn(u64) -> bool, explicit: &[u64]) {
        let mut aborts = Vec::new();
        let mut cancels = Vec::new();
        {
            let mut guard = self.shared.state.lock().unwrap();
            let state = &mut *guard;
            let busy = state.counts[0] > 0;
            for lane in 0..2 {
                let (counts, bytes) = (&mut state.counts[lane], &mut state.bytes[lane]);
                let (streams, light) = (&mut state.streams, &mut state.light);
                let agains = &mut state.agains;
                state.jobs[lane].retain_mut(|job| {
                    if held(job.ticket) {
                        return true;
                    }
                    if job.work.is_none() && (safe(&job.request) || explicit.contains(&job.ticket))
                    {
                        *counts -= 1;
                        *streams -= usize::from(job.request.stream);
                        *bytes -= job.charge;
                        return false;
                    }
                    job.forgotten = true;
                    true
                });
                let running = &mut state.running;
                state.completed[lane].retain(|done| {
                    if held(done.ticket) {
                        return true;
                    }
                    // Its bytes held by a job still physically running go
                    // when that ends (LLP 1120 D5).
                    if let Some(run) = running.iter_mut().find(|r| r.ticket == done.ticket) {
                        run.drained = true;
                    }
                    // A forgotten stream's message: the stream itself
                    // releases the reservation when its reader ends.
                    if !done.message {
                        *counts -= 1;
                        *light -= usize::from(done.light);
                        *agains -= usize::from(done.again);
                        *streams -= usize::from(done.stream);
                        *bytes -= done.bytes;
                    }
                    false
                });
            }
            for run in &mut state.running {
                // An app's abort reaches a write an earlier forget let run on.
                let by_app = explicit.contains(&run.ticket);
                if by_app || (!run.forgotten && !held(run.ticket)) {
                    run.forgotten = true;
                    let stream = run.stream.then_some(run.ticket);
                    let abort = if by_app {
                        run.exchange.clone()
                    } else {
                        run.abort.clone()
                    };
                    aborts.extend(abort.map(|abort| (stream, abort)));
                    // On the machine, cancelling a safe read (or an app's abort) claims it now.
                    if self.machine.is_some() && asynch::cancel(run, by_app) {
                        cancels.push((run.lane, (run.ticket, run.generation)));
                    }
                }
            }
            state.ordered.retain(|ticket| held(*ticket));
            // A parked upload let go unsent takes its size with it.
            let queued: std::collections::HashSet<u64> =
                state.jobs.iter().flatten().map(|job| job.ticket).collect();
            state.book.sized.retain(|ticket, _| queued.contains(ticket));
            // A re-ask let go before it found room: dropped, never settled.
            state.waiting.retain(|ticket| held(*ticket));
            state.waiting_set.retain(|ticket| held(*ticket));
            // A held request has not begun: one let go is dropped unsent,
            // its work destroyed by a worker.
            let (kept, gone): (VecDeque<_>, VecDeque<_>) = std::mem::take(&mut state.fenced)
                .into_iter()
                .partition(|f| held(f.ticket()));
            state.fenced = kept;
            state.fenced_bytes = fenced_bytes(&state.fenced);
            for f in gone {
                if let Fenced::Held(_, work, _) = f {
                    self.discard(state, work);
                }
            }
            rouse(&self.shared, state);
            place_waiting(state);
            // As in `complete`: emptying the lane can let a refusal settle.
            if has_ready(state) || (busy && state.counts[0] == 0) {
                wake(state);
            }
        }
        // Transport callbacks run outside the lock, as retirement's do.
        if let Some(machine) = &self.machine {
            for (lane, run) in cancels {
                machine.cancelled(lane, run);
            }
        }
        stream::release(&self.shared, aborts);
    }
}
