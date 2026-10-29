//! The executor's replies into the runner, each a commit (LLP 1016 D2).
use super::*;

#[cfg(all(test, unix))]
#[path = "abort_tests.rs"]
mod abort_tests;

impl<D: DataSource> Presenter<D> {
    /// The executor's replies into the runner (LLP 1016 D2), each a
    /// commit: the display loop calls this when the executor's fd is
    /// readable, the agent when it waits. `None` when nothing was queued.
    pub fn pump(&mut self, now_ms: f64) -> Option<String> {
        let region_error = self.poll_content_region();
        self.executor.begin_pump();
        let region_error = region_error.or(self.dispatch_authored_scroll());
        // An abort a source heard off this thread (a worker's let-go).
        self.host.runner_mut().collect_aborts();
        self.refusal_turn = !self.refusal_turn;
        let mut outcomes = if self.refusal_turn {
            self.host
                .take_request_refusal(self.executor.ordered_idle())
                .into_iter()
                .map(|(ticket, outcome)| (ticket, outcome, None))
                .collect()
        } else {
            self.executor.drain()
        };
        if outcomes.is_empty() {
            outcomes = if self.refusal_turn {
                self.executor.drain()
            } else {
                self.host
                    .take_request_refusal(self.executor.ordered_idle())
                    .into_iter()
                    .map(|(ticket, outcome)| (ticket, outcome, None))
                    .collect()
            };
        }
        if self.host.has_request_refusals(self.executor.ordered_idle()) {
            self.executor.notify();
        }
        if outcomes.is_empty() && !self.host.has_announced() {
            // A cancel with no answer to give (its request was retired) still goes.
            let aborted = self.host.runner_mut().take_explicit_aborts();
            self.executor.cancel(&aborted);
            let refined = self.refine_collections();
            return region_error
                .or(refined)
                .or(self.refresh_transform_geometry())
                .or(self.deliver_scroll_ends());
        }
        let e = self.host.fulfill_all(outcomes, now_ms);
        let after = self.after_commit();
        let ends = self.deliver_scroll_ends();
        region_error.or(e).or(after).or(ends)
    }
}
