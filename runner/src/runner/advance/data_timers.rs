//! An answer's own timers (LLP 1027.000 D1 as amended 2026-10-28). A
//! TypeScript answer that awaits only a timer it armed hands the runner a
//! continuation due at a time on the runner's clock ([`Request::due_ms`]).
//! The runner keeps it back from the host until an advance reaches that
//! time, after every task, `then` and re-ask due by then, and hands it out;
//! the host settles it as any continuation, and the answer's timer runs in
//! that turn. One that also waits for other answers' work
//! ([`Request::wake_ms`]) goes to the host at once, as such a wait always
//! has, and the advance that reaches its time releases it
//! ([`Runner::release_work`]): whichever comes first asks it again. Until
//! then neither is I/O: `has_pending` and `in_flight` leave it out, and
//! `clock settle` advances to it instead of waiting on it. Once released, it
//! is landing until its reply comes ([`Runner::data_timer_landing`]): a
//! jump waits for that reply before it goes on, as it does for a task's
//! request. One whose answer is let go meanwhile is dropped, and its source
//! told so ([`DataSource::discard`]) when it never went out.
use super::super::{DataSource, Dispatch, RequestOut, Runner};

/// What the runner keeps for answers' timers.
#[derive(Default)]
pub(in crate::runner) struct DataTimers {
    /// Timer waits kept back until due.
    held: Vec<RequestOut>,
    /// Waits for other answers' work out with the host, each released at
    /// its time: its ticket, its continuation token and that time.
    wakes: Vec<(u64, u64, f64)>,
    /// The tokens of wakes due, for the host's next `release_work`.
    woken: Vec<u64>,
    /// The tickets released and not yet replied to.
    landing: Vec<u64>,
}

/// A due time as the runner's clock reads it.
fn due(r: &RequestOut) -> Option<f64> {
    r.request.due_ms.map(|ms| ms as f64)
}

impl<D: DataSource> Runner<D> {
    /// The requests to hand the host now: a timer wait not yet due is kept
    /// back, and a wake is noted. What was let go meanwhile is dropped.
    pub(in crate::runner) fn hold_data_timers(
        &mut self,
        requests: Vec<RequestOut>,
    ) -> Vec<RequestOut> {
        let now = self.now_ms;
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            if due(&r).is_some_and(|at| at > now) {
                self.clock.data.held.push(r);
                continue;
            }
            if let (Some(wake), Some(token)) = (r.request.wake_ms, r.request.continuation) {
                self.clock.data.wakes.push((r.ticket, token, wake as f64));
            }
            out.push(r);
        }
        let held = std::mem::take(&mut self.clock.data.held);
        let (kept, gone): (Vec<_>, Vec<_>) = held.into_iter().partition(|r| self.holds(r.ticket));
        self.clock.data.held = kept;
        for r in gone {
            self.discard_request(&r.request);
        }
        let mut wakes = std::mem::take(&mut self.clock.data.wakes);
        wakes.retain(|(ticket, _, _)| self.holds(*ticket));
        self.clock.data.wakes = wakes;
        let mut landing = std::mem::take(&mut self.clock.data.landing);
        landing.retain(|ticket| self.holds(*ticket));
        self.clock.data.landing = landing;
        out
    }

    /// Whether request `ticket` waits for an answer's timer not yet due: no
    /// I/O, so no clock waits on it.
    pub(in crate::runner) fn data_timer_held(&self, ticket: u64) -> bool {
        self.data_timer_due(ticket).is_some()
    }

    /// When request `ticket`'s answer's timer is due, while that is ahead.
    fn data_timer_due(&self, ticket: u64) -> Option<f64> {
        let now = self.now_ms;
        let data = &self.clock.data;
        let held = (data.held.iter().chain(&self.requests))
            .filter(|r| r.ticket == ticket)
            .filter_map(|r| due(r).or(r.request.wake_ms.map(|ms| ms as f64)));
        let wakes = (data.wakes.iter())
            .filter(|(t, _, _)| *t == ticket)
            .map(|(_, _, at)| *at);
        held.chain(wakes).find(|at| *at > now)
    }

    /// When the earliest answer's timer the runner waits for is due, of
    /// those whose answer is still in flight: a host's next deadline, and a
    /// time `clock settle` advances to.
    pub fn data_timer_due_ms(&self) -> Option<f64> {
        let data = &self.clock.data;
        let held = (data.held.iter())
            .filter(|r| self.holds(r.ticket))
            .filter_map(due);
        let wakes = (data.wakes.iter())
            .filter(|(ticket, _, _)| self.holds(*ticket))
            .map(|(_, _, at)| *at);
        held.chain(wakes).reduce(f64::min)
    }

    /// Whether a timer's continuation an advance released is still out:
    /// a jump lands its reply before it goes on.
    pub fn data_timer_landing(&self) -> bool {
        (self.clock.data.landing.iter()).any(|ticket| self.holds(*ticket))
    }

    /// The requests waiting for an answer's timer, for the agent's
    /// `state.pending`: each target's name (a tool handler's, as
    /// `tool_tickets` names it), its ticket and when it is due.
    pub fn data_timer_waits(&self) -> Vec<(String, u64, f64)> {
        let targets = (self.pending.iter()).map(|p| (self.target_name(p.target), p.ticket));
        (targets.chain(self.tool_tickets()))
            .filter_map(|(name, ticket)| Some((name, ticket, self.data_timer_due(ticket)?)))
            .collect()
    }

    /// Hand out every timer wait due by `at`, and release every wake due by
    /// then; what was let go is dropped. Whether any went out.
    pub(in crate::runner) fn release_data_timers(&mut self, at: f64) -> bool {
        let held = std::mem::take(&mut self.clock.data.held);
        let (due_now, later): (Vec<_>, Vec<_>) = held
            .into_iter()
            .partition(|r| due(r).is_some_and(|d| d <= at));
        self.clock.data.held = later;
        let mut out = false;
        for r in due_now {
            if self.holds(r.ticket) {
                self.clock.data.landing.push(r.ticket);
                self.requests.push(r);
                out = true;
            } else {
                self.discard_request(&r.request);
            }
        }
        let wakes = std::mem::take(&mut self.clock.data.wakes);
        for (ticket, token, when) in wakes {
            if when > at {
                self.clock.data.wakes.push((ticket, token, when));
            } else if self.holds(ticket) {
                self.clock.data.woken.push(token);
                self.clock.data.landing.push(ticket);
                out = true;
            }
        }
        out
    }

    /// The wakes due since the last take, each asked again.
    pub(in crate::runner) fn take_woken(&mut self) -> Vec<(u64, Dispatch)> {
        (self.clock.data.woken.drain(..))
            .map(|token| (token, Dispatch::Again))
            .collect()
    }
}
