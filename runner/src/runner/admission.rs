//! Admission failures occupy the already-bounded current target ticket, never
//! a separate failure queue. Supersession drops them with the old ticket.
use super::{CommitReceipt, DataSource, Outcome, Runner, RunnerError, Target};
use crate::FailureKind;

impl<D: DataSource> Runner<D> {
    /// Record a host admission refusal on a still-current ticket. Repeating
    /// refusals cannot grow a queue: there is at most one ticket per target.
    pub fn refuse_request(&mut self, ticket: u64, reason: &'static str, ordered: bool) {
        if let Some(pending) = self.pending.iter_mut().find(|p| p.ticket == ticket) {
            pending.refusal = Some((reason, ordered));
        }
    }

    /// Take one refusal for ordinary transactional parsing/settlement. A
    /// source that shapes the failure settles it like any reply. An ordered
    /// one its parse refuses too lets the ticket go (`release_refused`); an
    /// independent one retains its ticket (LLP 1027.002) and is not retried.
    pub fn take_request_refusal(&mut self, allow_ordered: bool) -> Option<(u64, Outcome)> {
        let pending = self.pending.iter_mut().find(|p| {
            p.refusal
                .is_some_and(|(_, ordered)| !ordered || allow_ordered)
        })?;
        let (reason, ordered) = pending.refusal.take().unwrap();
        pending.refused = ordered;
        Some((
            pending.ticket,
            Outcome::Failed {
                kind: FailureKind::Refused,
                message: reason.into(),
            },
        ))
    }

    /// A request refused ordered admission never ran, so unlike a reply the
    /// source can't parse, its ticket isn't kept pending with nothing to
    /// answer it. A mutation ends unsent: a write is never retried (LLP 1041
    /// D2). A resource is asked again once the last ordered refusal settles:
    /// the host settles one only after earlier ordered work has, and lifts
    /// its fence when none is left, so the ask meets an idle, open lane —
    /// asked sooner, it would be refused behind the others. That last one
    /// commits, showing what is no longer pending; until then, `None`.
    pub(super) fn release_refused(
        &mut self,
        ticket: u64,
        target: Target,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        self.pending.retain(|p| p.ticket != ticket);
        self.forgot = true;
        self.sync_pending_flags();
        let name = self.target_name(target);
        let next = match target {
            Target::Resource(i) => {
                self.refused_asks.push(i);
                "asked again"
            }
            Target::Mutation(_) => "it ends unsent",
        };
        self.log(format!(
            "request {ticket} ({name}) was refused admission: {next}"
        ));
        if self.has_ordered_request_refusals() {
            return Ok(None);
        }
        let which = std::mem::take(&mut self.refused_asks);
        self.commit_again(which, "admission refusals").map(Some)
    }

    /// A reply the source failed to take (it threw, ran over its budget, or
    /// answered outside its shape). The refused commit's rollback restored
    /// the request to the pending set, but the host has already delivered
    /// its one reply: left there it would be pending forever, and a button
    /// disabled on `pending(…)` with it. It is let go instead. A resource
    /// keeps the value it had and is not asked again here (a source that
    /// fails the same way every time would loop); `refresh` asks again. A
    /// mutation ends unsent.
    pub(super) fn release_failed(
        &mut self,
        ticket: u64,
        target: Target,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let failed_args = self
            .pending
            .iter()
            .find(|p| p.ticket == ticket)
            .map(|p| p.args.clone());
        self.pending.retain(|p| p.ticket != ticket);
        self.forgot = true;
        self.sync_pending_flags();
        let name = self.target_name(target);
        let next = match target {
            Target::Resource(i) => {
                // The last value now stands for the arguments that failed, so
                // settlement reuses it instead of asking again: a source that
                // fails the same way every time would otherwise loop.
                if let (Some(state), Some(args)) = (self.resources[i].as_mut(), failed_args) {
                    state.args = args;
                }
                "it keeps its last value"
            }
            Target::Mutation(_) => "it ends unsent",
        };
        let what = format!("request {ticket} ({name}) failed and is no longer pending: {next}");
        self.log(what.clone());
        self.commit_again(Vec::new(), "a failed request").map(Some)
    }

    /// The host must keep later ordered admissions behind these refusals.
    pub fn has_ordered_request_refusals(&self) -> bool {
        self.pending
            .iter()
            .any(|p| p.refusal.is_some_and(|(_, ordered)| ordered))
    }

    /// More refused admissions need a future host pump.
    pub fn has_request_refusals(&self, allow_ordered: bool) -> bool {
        self.pending.iter().any(|p| {
            p.refusal
                .is_some_and(|(_, ordered)| !ordered || allow_ordered)
        })
    }
}
