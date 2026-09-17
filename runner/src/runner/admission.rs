//! Admission failures occupy the already-bounded current target ticket, never
//! a separate failure queue. Supersession drops them with the old ticket.
use super::{DataSource, Outcome, Runner};
use crate::FailureKind;

impl<D: DataSource> Runner<D> {
    /// Record a host admission refusal on a still-current ticket. Repeating
    /// refusals cannot grow a queue: there is at most one ticket per target.
    pub fn refuse_request(&mut self, ticket: u64, reason: &'static str, ordered: bool) {
        if let Some(pending) = self.pending.iter_mut().find(|p| p.ticket == ticket) {
            pending.refusal = Some((reason, ordered));
        }
    }

    /// Take one refusal for ordinary transactional parsing/settlement. A parse
    /// refusal retains its ticket according to LLP 1016 but is not retried.
    pub fn take_request_refusal(&mut self, allow_ordered: bool) -> Option<(u64, Outcome)> {
        let pending = self.pending.iter_mut().find(|p| {
            p.refusal
                .is_some_and(|(_, ordered)| !ordered || allow_ordered)
        })?;
        Some((
            pending.ticket,
            Outcome::Failed {
                kind: FailureKind::Refused,
                message: pending.refusal.take().unwrap().0.into(),
            },
        ))
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
