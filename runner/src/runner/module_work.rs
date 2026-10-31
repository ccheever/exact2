//! Work that outlives its answer (LLP 1146 D2). A request with effects
//! ([`crate::Request::has_effects`]) that the runner lets go runs to its
//! end, as a browser lets a fetch finish: cancelling it would not undo what
//! its server did. The source keeps it ([`DataSource::keep_sent`]), the host
//! goes on running it (`holds`), and its reply reaches the let-go answer's
//! continuation in a module turn: outside every answer and every commit's
//! checkpoint, inside `fulfill`, so what the turn writes to the store stands,
//! and the commit after it reaches the host as any reply's does, presented
//! and persisted.
use super::{CommitReceipt, DataSource, InFlight, PendingReq, Runner, RunnerError, Target};
use crate::request::Outcome;
use crate::store::StoreCheckpoint;
use std::collections::BTreeMap;

/// What the journal and `state.pending` call a kept request.
pub(super) const NAME: &str = "module work";

#[derive(Default)]
pub(super) struct ModuleWork {
    /// Kept requests in flight: the runner's ticket, the source's token.
    tickets: BTreeMap<u64, u64>,
    /// Their module fetch tickets, so the app's abort still names one
    /// (`Runner::match_aborts`) for as long as it runs, with no cap.
    pub(super) fetches: BTreeMap<u64, u64>,
    /// Requests with effects the commit being made let go, offered to the
    /// source when it ends.
    letting: Vec<PendingReq>,
    /// The store as the turn of a reply to a request with effects left it:
    /// a refused commit goes back to here, not to before the reply.
    pub(super) standing: Option<StoreCheckpoint>,
    /// The resource whose reply's turn read the store in the commit being
    /// made: a refusal keeps that read, as it keeps the reply's failure.
    pub(super) reply_reader: Option<usize>,
    /// Kept tickets the host refused at admission, settled as failures, and
    /// whether each was ordered.
    refused: Vec<(u64, &'static str, bool)>,
}

impl<D: DataSource> Runner<D> {
    /// Whether `ticket` is a kept request's.
    pub(super) fn is_module_work(&self, ticket: u64) -> bool {
        self.module_work.tickets.contains_key(&ticket)
    }

    /// Whether a kept request the host has not refused is in flight.
    pub(super) fn module_work_effects(&self) -> bool {
        let refused = |t: &u64| self.module_work.refused.iter().any(|(r, ..)| r == t);
        (self.module_work.tickets.keys()).any(|t| !refused(t))
    }

    /// Kept requests' tickets in flight, for `pending`, `in_flight` and `clock settle`.
    pub(super) fn module_work_tickets(&self) -> Vec<(String, u64)> {
        (self.module_work.tickets.keys())
            .map(|t| (NAME.to_owned(), *t))
            .collect()
    }

    /// The host refused a kept ticket at admission: it settles as a failure,
    /// in its turn among the other refusals.
    pub(super) fn refuse_module_work(&mut self, ticket: u64, reason: &'static str, ordered: bool) {
        self.module_work.refused.push((ticket, reason, ordered));
    }

    /// A refusal of a kept ticket that may settle now (an ordered one only
    /// when ordered work may), if one is waiting.
    pub(super) fn module_work_refusal(
        &mut self,
        allow_ordered: bool,
    ) -> Option<(u64, &'static str)> {
        let refused = &mut self.module_work.refused;
        let at = refused
            .iter()
            .position(|(_, _, ordered)| !ordered || allow_ordered)?;
        let (ticket, reason, _) = refused.remove(at);
        Some((ticket, reason))
    }

    /// Whether a refusal of a kept ticket that may settle now is waiting.
    pub(super) fn has_module_work_refusal(&self, allow_ordered: bool) -> bool {
        (self.module_work.refused.iter()).any(|(_, _, ordered)| !ordered || allow_ordered)
    }

    /// Whether an ordered kept ticket's refusal is waiting: it fences ordered work.
    pub(super) fn has_ordered_module_work_refusal(&self) -> bool {
        (self.module_work.refused.iter()).any(|(_, _, ordered)| *ordered)
    }

    /// Request `p` is let go by the commit being made: one with effects is
    /// offered to its source when the commit ends.
    pub(super) fn let_go_request(&mut self, p: PendingReq) {
        // An upload let go before its reply is cut off; one the host refused
        // never reached the network and is not marked as asked; of any other
        // the host says after the commit whether it had started (LLP 1016.000 D10).
        if p.upload && p.sent {
            let end = super::UploadEnd::Failed(super::uploads::interrupted());
            self.upload_over(p.ticket, true, end);
        } else if p.upload && (p.refusal.is_some() || p.refused) {
            self.uploads.unmark(p.ticket);
        } else if p.upload {
            self.upload_let_go(p.ticket);
        }
        if p.admitted_effects() && !self.poisoned {
            self.module_work.letting.push(p);
        }
    }

    /// How many let-go requests with effects the commit being made collected.
    pub(super) fn letting_len(&self) -> usize {
        self.module_work.letting.len()
    }

    /// A refused commit put the requests it let go back in flight; those
    /// collected before its checkpoint are still let go.
    pub(super) fn letting_restored(&mut self, len: usize) {
        self.module_work.letting.truncate(len);
    }

    /// Put the store back for a refused commit: to where the turn of a reply
    /// to a request with effects left it, when one ran, journaling what so
    /// stands.
    /// Whether it kept any write.
    pub(super) fn restore_store_refused(&mut self, c: StoreCheckpoint) -> bool {
        let since = c.writes;
        match self.module_work.standing.take() {
            Some(standing) => {
                let kept = standing.writes > since;
                self.store.restore(standing);
                self.log_store_writes(since);
                kept
            }
            None => {
                self.store.restore(c);
                false
            }
        }
    }

    /// A reply's turn wrote the store, as a browser's continuation writes it
    /// outside every commit: each store reader that failed, but `except`, is
    /// asked again (LLP 1146 §2.3). A write an action makes in a commit
    /// leaves a failure standing until its arguments change or `refresh`.
    pub(super) fn ask_failed_readers(&mut self, except: Option<Target>) {
        for i in 0..self.failed_args.len() {
            // (an upload's failure is never asked again at its arguments: D10)
            let failed =
                self.store_readers[i] && self.failed_args[i].is_some() && !self.upload_failed(i);
            if failed && except != Some(Target::Resource(i)) {
                self.force_refresh(i);
            }
        }
    }

    /// At the end of every commit, after `forgotten`: the source keeps the
    /// let-go requests with effects it owns, and the host cancels the rest,
    /// as it cancels any request let go. A poisoned runner keeps nothing.
    pub(super) fn keep_sent(&mut self, refused: bool) {
        let mut letting = std::mem::take(&mut self.module_work.letting);
        if self.poisoned {
            letting.clear();
        }
        let in_flight: Vec<InFlight<'_>> = letting
            .iter()
            .map(|p| InFlight {
                target: p.target,
                source: &p.source,
                args: &p.args,
                continuation: p.continuation,
            })
            .collect();
        let kept = self.data.keep_sent(&self.store, &in_flight, refused);
        for (p, token) in letting.iter().zip(kept) {
            if let Some(token) = token {
                self.module_work.tickets.insert(p.ticket, token);
                if let Some(fetch) = p.fetch {
                    self.module_work.fetches.insert(fetch, p.ticket);
                }
                let name = self.target_name(p.target);
                self.log(format!(
                    "keep request {} ({name}): it may have changed its server, so it runs to its end",
                    p.ticket
                ));
            }
        }
    }

    /// A kept request came back (a refusal included): the let-go answer's
    /// continuation takes it in a module turn, then a commit of its own.
    pub(super) fn module_work_landed(
        &mut self,
        ticket: u64,
        outcome: Outcome,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(token) = self.module_work.tickets.remove(&ticket) else {
            return Ok(None);
        };
        self.module_work.fetches.retain(|_, t| *t != ticket);
        let (revision, writes) = (self.store.revision(), self.store.writes().len());
        self.log(format!("{NAME} {ticket}: {}", outcome.summary()));
        if let Err(e) = self.data.module_landed(&mut self.store, token, outcome) {
            self.log(format!("{NAME} failed: {e}"));
        }
        self.take_data_logs();
        // Its writes are in the store before the commit's checkpoint, which journals only those after
        self.log_store_writes(writes);
        self.module_commit(self.store.revision() != revision)
            .map(Some)
    }

    /// Whether resource `i` is a store reader in flight that module work
    /// wrote under: it is asked again once its reply lands, and until then,
    /// while no other write follows, its answer in flight stands. A watched
    /// topic's reader waits for its reply too, but another write asks it
    /// again at once.
    pub(super) fn asks_again_after_reply(&self, i: usize) -> bool {
        let revision = Some(self.store.revision());
        self.pending
            .iter()
            .any(|p| p.target == Target::Resource(i) && p.module_again == revision)
    }

    /// The commit after a module turn. When the turn wrote to the store, its
    /// readers are asked again: those not in flight now, those in flight
    /// once their reply lands, so a reader that awaits the kept request's
    /// work is not replaced while that work finishes. A stream, a long poll
    /// or a held request may never land, so its reader is asked again now. A
    /// turn that wrote nothing asks nothing again. Its refusal leaves the
    /// turn's writes standing: they were in the store before its checkpoint.
    fn module_commit(&mut self, wrote: bool) -> Result<CommitReceipt, RunnerError> {
        let mut now = Vec::new();
        if wrote {
            let revision = self.store.revision();
            // An upload is asked once for its arguments (LLP 1016.000 D10).
            let readers: Vec<usize> = (self.store_readers().into_iter())
                .filter(|i| !self.upload_stands(*i))
                .collect();
            for i in readers {
                let mut in_flight = false;
                for p in self.pending.iter_mut() {
                    if p.target == Target::Resource(i)
                        && p.stream.is_none()
                        && !p.long_poll
                        && !p.held
                    {
                        p.ask_again = true;
                        p.module_again = Some(revision);
                        in_flight = true;
                    }
                }
                if !in_flight {
                    now.push(i);
                }
            }
        }
        self.commit_again(now, NAME)
    }
}
