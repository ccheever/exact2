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
//!
//! An answer that awaits its own request (a fetch, a module's call) with a
//! timer of its own armed hands that request out carrying the timer's time
//! (`wake_ms`, and no continuation). It is I/O, so the clocks wait on it as
//! on any request; but if an advance reaches that time before its reply,
//! the request runs on as module work (LLP 1146 D2), its reply delivered in
//! a module turn, and the answer's continuation goes out in its place,
//! landing as a released wait does ([`DataSource::timer_before_reply`]). So
//! `Promise.race([fetch, timeout])` gives up on time, as a browser's does.
use super::super::{DataSource, Dispatch, InFlight, RequestOut, Runner};

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
    /// Requests whose answer's own timer is due at a time: each ticket and
    /// that time.
    races: Vec<(u64, f64)>,
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
            match (r.request.wake_ms, r.request.continuation) {
                (Some(wake), Some(token)) => {
                    self.clock.data.wakes.push((r.ticket, token, wake as f64))
                }
                (Some(wake), None) => self.clock.data.races.push((r.ticket, wake as f64)),
                _ => {}
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
        let mut races = std::mem::take(&mut self.clock.data.races);
        races.retain(|(ticket, _)| self.racing(*ticket));
        self.clock.data.races = races;
        out
    }

    /// Whether request `ticket` is still its target's, awaited by its answer.
    fn racing(&self, ticket: u64) -> bool {
        self.pending.iter().any(|p| p.ticket == ticket)
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
        let races = (data.races.iter())
            .filter(|(ticket, _)| self.racing(*ticket))
            .map(|(_, at)| *at);
        held.chain(wakes).chain(races).reduce(f64::min)
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
        let races = std::mem::take(&mut self.clock.data.races);
        for (ticket, when) in races {
            if when > at {
                self.clock.data.races.push((ticket, when));
            } else {
                out |= self.timer_first(ticket);
            }
        }
        out
    }

    /// Request `ticket`'s answer's timer came due before its reply: when its
    /// source can resume the answer, the request runs on as module work and
    /// the answer's continuation takes its place. Whether that went out.
    fn timer_first(&mut self, ticket: u64) -> bool {
        // A plain request out with the host: not a refused one, a held one,
        // an upload or a re-ask's wait, which end as their own.
        let plain = |p: &&super::super::PendingReq| {
            p.continuation.is_none()
                && p.stream.is_none()
                && !p.upload
                && !p.held
                && !p.long_poll
                && p.refusal.is_none()
                && !p.refused
                && p.reask_at.is_none()
        };
        let Some(pos) = (self.pending.iter()).position(|p| p.ticket == ticket && plain(&p)) else {
            return false;
        };
        let p = &self.pending[pos];
        let in_flight = InFlight {
            target: p.target,
            source: &p.source,
            args: &p.args,
            continuation: None,
        };
        let Some((token, mut request)) = self.data.timer_before_reply(&self.store, in_flight)
        else {
            return false;
        };
        self.keep_running(ticket, token, self.pending[pos].fetch);
        let next = self.next_ticket;
        self.next_ticket += 1;
        request.reach = self.reach_of(self.ticket_origin(ticket));
        let p = &mut self.pending[pos];
        p.ticket = next;
        p.continuation = request.continuation;
        p.fetch = None;
        p.keepable = None;
        p.effects = false;
        p.grant = None;
        let target = p.target;
        self.pass_on(ticket, next);
        let name = self.target_name(target);
        self.log(format!(
            "request {ticket} ({name}): its answer's timer came due first; it runs on as {}, and request {next} resumes the answer",
            super::super::module_work::NAME
        ));
        self.clock.data.landing.push(next);
        self.requests.push(RequestOut {
            ticket: next,
            target: name,
            request,
            forced: false,
        });
        self.sync_pending_flags();
        true
    }

    /// The wakes due since the last take, each asked again.
    pub(in crate::runner) fn take_woken(&mut self) -> Vec<(u64, Dispatch)> {
        (self.clock.data.woken.drain(..))
            .map(|token| (token, Dispatch::Again))
            .collect()
    }
}
