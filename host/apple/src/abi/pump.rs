use super::*;
impl<D: DataSource> Bridge<D> {
    /// The executor's queued outcomes into the runner (LLP 1016 D2): the
    /// presenter calls this on its thread after the wake; the output is the
    /// batch of every reply's commit.
    pub fn pump(&mut self, now_ms: f64) -> u32 {
        self.refusal_turn = !self.refusal_turn;
        let outcomes = match (self.host.as_mut(), self.executor.as_deref()) {
            (Some(host), Some(executor)) => {
                executor.begin_pump();
                // An abort a source heard off this thread (a worker's let-go).
                host.runner_mut().collect_aborts();
                let mut outcomes = if self.refusal_turn {
                    host.take_request_refusal(executor.ordered_idle())
                        .into_iter()
                        .map(|(ticket, outcome)| (ticket, outcome, None))
                        .collect()
                } else {
                    executor.drain()
                };
                if outcomes.is_empty() {
                    outcomes = if self.refusal_turn {
                        executor.drain()
                    } else {
                        host.take_request_refusal(executor.ordered_idle())
                            .into_iter()
                            .map(|(ticket, outcome)| (ticket, outcome, None))
                            .collect()
                    };
                }
                if host.has_request_refusals(executor.ordered_idle()) {
                    executor.notify();
                }
                outcomes
            }
            _ => vec![],
        };
        let out = match self.host.as_mut() {
            Some(h) => h.fulfill_all(outcomes, now_ms),
            None => not_booted(),
        };
        self.emit(out)
    }
}
