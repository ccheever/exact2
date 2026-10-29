//! What the agent sees in flight: the requests out by target and ticket
//! (`state.pending`), what `settle` waits on, what the server holds
//! (`clock held`) and the open streams (`state.streams`).

use super::*;

impl<D: DataSource> Runner<D> {
    /// Every request in flight: the resource's or mutation's name and its ticket.
    pub fn pending(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self
            .pending
            .iter()
            .filter(|p| !self.device_holds.iter().any(|h| h.ticket == p.ticket))
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect();
        out.extend(
            self.background_ticket()
                .map(|t| (super::background::NAME.to_string(), t)),
        );
        out.extend(self.tool_tickets());
        out.extend(self.module_work_tickets());
        out
    }

    /// Whether any request is in flight (the agent's `settle` waits on it).
    /// An open stream counts only until its first message (LLP 1016.000
    /// D5): after that it is open, not in flight, or `settle` never ends.
    /// A request held for the agent (LLP 1069.007 D3) is not I/O either: no
    /// clock waits on it, nor on one the server holds (LLP 1016.000 D9,
    /// [`Runner::held_requests`]), nor an answer's wait for its own timer
    /// (`data_timers.rs`). The module's background round is (LLP 1097 D9).
    pub fn has_pending(&self) -> bool {
        self.background_ticket().is_some()
            || (self.tool_tickets().iter()).any(|(_, t)| !self.data_timer_held(*t))
            || !self.module_work_tickets().is_empty()
            || self
                .pending
                .iter()
                .any(|p| p.waited() && !self.held_for_agent(p) && !self.data_timer_held(p.ticket))
    }

    /// Replies landed for held requests, by target, whatever they said (LLP
    /// 1016.000 D9): what `clock held` waits on to rise. A request let go
    /// (superseded, forgotten) never counts: its late reply is dropped.
    pub fn held_replies(&self) -> &[(String, u64)] {
        &self.held_replies
    }

    /// The requests the server holds (LLP 1016.000 D9): in flight, but no
    /// clock waits on them; each reply that leaves them out names them, as
    /// `state.pending` does (`held: true`).
    pub fn held_requests(&self) -> Vec<(String, u64)> {
        self.pending
            .iter()
            .filter(|p| (p.in_flight() && p.held || p.replied) && !self.held_for_agent(p))
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect()
    }

    /// A request held for the agent: a device's, or a module's (LLP 1133.002 §6).
    fn held_for_agent(&self, p: &PendingReq) -> bool {
        self.device_holds.iter().any(|h| h.ticket == p.ticket)
            || self.extensions.holds_token(p.continuation)
            || self.presence_holds(p.ticket)
    }

    /// The requests in flight, as `has_pending` counts them: the agent's
    /// `state.pending` (a held request is listed there under `device`).
    /// The module's background round is listed last, as `background`.
    pub fn in_flight(&self) -> Vec<(String, u64)> {
        let mut out: Vec<(String, u64)> = self
            .pending
            .iter()
            .filter(|p| p.waited() && !self.held_for_agent(p) && !self.data_timer_held(p.ticket))
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect();
        out.extend(
            self.background_ticket()
                .map(|t| (super::background::NAME.to_string(), t)),
        );
        out.extend((self.tool_tickets().into_iter()).filter(|(_, t)| !self.data_timer_held(*t)));
        out.extend(self.module_work_tickets());
        out
    }

    /// Every open stream: its resource, ticket, and counts (the agent's
    /// `state.streams`, LLP 1016.000 D5).
    pub fn streams(&self) -> Vec<(String, u64, StreamCount)> {
        self.pending
            .iter()
            .filter_map(|p| Some((self.target_name(p.target), p.ticket, p.stream?)))
            .collect()
    }

    /// The grant line that admitted stream `ticket`, when one of the app's
    /// does (`state.streams`).
    pub fn stream_grant(&self, ticket: u64) -> Option<&str> {
        let p = self.pending.iter().find(|p| p.ticket == ticket)?;
        p.grant.as_deref()
    }
}
