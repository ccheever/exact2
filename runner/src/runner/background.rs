//! The module's background work (LLP 1097 D5): storage an answer started
//! and did not await, finished after the answer under one ticket of its
//! own. The ticket is a request in flight, so `holds`, `has_pending` and
//! `clock settle` see it on every native host, but it is no target's: it
//! has no `then`, makes no commit, and never goes through `enqueue`, the
//! checkpoint or `forgotten`, so a refused commit cannot undo it and a
//! commit that lets requests go cannot drop it.
use super::{DataSource, Runner, BACKGROUND};
use crate::request::{Outcome, Request, RequestOut};

/// The round out for the module's background work, if any.
#[derive(Default)]
pub(super) struct Background {
    /// The runner ticket of the round the host holds.
    ticket: Option<u64>,
    /// Rounds delivered since the work began: `background: done (N operations)`.
    rounds: u64,
    /// The host refused the round at admission: its ticket, the reason and
    /// whether it was ordered. Settled through the round's own completion
    /// (`background_landed`), like any refusal, in its turn.
    pub(super) refusal: Option<(u64, &'static str, bool)>,
}

/// What the journal and `state.pending` call the background ticket.
pub(super) const NAME: &str = "background";

impl<D: DataSource> Runner<D> {
    /// Hand the host the module's next background round when it has one
    /// and none is out: polled whenever the host takes its requests, which
    /// it does after every answer, fulfill, release and round.
    pub(super) fn arm_background(&mut self) {
        if self.poisoned || self.background.ticket.is_some() {
            return;
        }
        let Some(request) = self.data.background(&self.store) else {
            return;
        };
        if self.background.rounds == 0 {
            let waiting = self.data.background_state().map_or(0, |s| s.queued);
            self.log(exact_num::text!(
                "background: storage ({} waiting)",
                waiting
            ));
        }
        self.send_background(request);
    }

    fn send_background(&mut self, request: Request) {
        debug_assert_eq!(request.continuation, Some(BACKGROUND));
        // A fresh ticket each round: a host never sees a ticket it has
        // completed come back (the one request in flight is still one).
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.background.ticket = Some(ticket);
        self.requests.push(RequestOut {
            ticket,
            target: NAME.into(),
            request,
            forced: false,
        });
    }

    /// The host's admission refusal of the round out, if any, that
    /// `allow_ordered` lets settle now.
    pub(super) fn background_refusal(&self, allow_ordered: bool) -> Option<(u64, &'static str)> {
        let (ticket, reason, ordered) = self.background.refusal?;
        (self.is_background(ticket) && (!ordered || allow_ordered)).then_some((ticket, reason))
    }

    /// Whether `ticket` is the background round's.
    pub(super) fn is_background(&self, ticket: u64) -> bool {
        self.background.ticket == Some(ticket)
    }

    /// The ticket of the background round out, for `in_flight` and `holds`.
    pub(super) fn background_ticket(&self) -> Option<u64> {
        self.background.ticket
    }

    /// A background round came back: the module delivers it, and its next
    /// round goes out at once. No commit, no `update()`: what the app's own
    /// promise reactions did is the module's; nothing Contract sees changed.
    pub(super) fn background_landed(&mut self, outcome: Outcome) {
        self.background.ticket = None;
        let landed = self.data.background_landed(&self.store, outcome);
        self.take_data_logs();
        match landed {
            Ok(Some(next)) => {
                self.background.rounds += 1;
                self.send_background(next);
            }
            Ok(None) => {
                // The background's run ends here: what it still has queued
                // goes out when its operation is next in flight.
                let rounds = std::mem::take(&mut self.background.rounds) + 1;
                self.log(exact_num::text!("background: done ({} operations)", rounds));
            }
            Err(
                super::DataError::UnknownSource(m)
                | super::DataError::BadArguments(m)
                | super::DataError::Unavailable(m)
                | super::DataError::Interface(m)
                | super::DataError::DeferredAtBake(m)
                | super::DataError::Failed(_, m),
            ) => {
                // The executor's own failure, not a failed operation (those
                // are the module's `storage failed:` lines, D8).
                self.background.rounds = 0;
                self.log(exact_num::text!("background failed: {}", m));
            }
        }
    }

    /// `poison()`: the round goes with the other requests. What the store
    /// already has completes on disk; what still waits in the module is
    /// named (D5).
    pub(super) fn drop_background(&mut self) {
        if self.background.ticket.take().is_some() {
            let waiting = self.data.background_state().map_or(0, |s| s.queued);
            self.background.rounds = 0;
            self.log(exact_num::text!(
                "background: dropped (poisoned), {} operations waiting",
                waiting
            ));
        }
    }

    /// The module's journal lines (LLP 1097 D8), into the journal.
    pub(super) fn take_data_logs(&mut self) {
        for line in self.data.take_logs() {
            self.log(line);
        }
    }

    /// The module's storage operations queued or in flight: the
    /// `background` count a `clock` reply carries beside `inflight` (LLP
    /// 1097 D9).
    pub fn background_operations(&self) -> u64 {
        self.data
            .background_state()
            .map_or(0, |s| s.queued + s.in_flight)
    }

    /// `state.background` (LLP 1097 D8), or `None` for a source with no
    /// background storage.
    pub fn background_state(&self) -> Option<crate::BackgroundState> {
        self.data.background_state()
    }
}

#[cfg(test)]
mod tests;
