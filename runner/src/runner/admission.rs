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
