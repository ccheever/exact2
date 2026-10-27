//! A resource a turn over its budget refused (`DataError::Overrun`, exact-js's
//! own time or its wall backstop) is asked again, twice at most, before it
//! fails as any refused resource does (LLP 1027 as amended 2026-10-27; theme2's
//! 18:06 ruling). The refusal is the machine's load as much as the source's:
//! under load one stood a resource on a stale value until the app's own 10 s
//! redial (Excord's gateway, x2-perf4's evidence). While it waits the resource
//! reads exactly as during an ordinary ask in flight: `pending`, its last value,
//! no failure. A mutation is never asked again (LLP 1041 D2), nor a stream's
//! message; an argument change, a refresh or a success ends the streak.
use super::{lines, CommitReceipt, DataError, DataSource, Runner, RunnerError, Target};
use exact_plan::Value;

/// How long after each refusal the resource is asked again: spaced, so a spike
/// of load that refused it has passed (an idle tick under load can come at
/// once and refuse it again).
pub(super) const REASK_MS: [f64; 2] = [250.0, 1000.0];

/// A resource's re-ask after a turn over its budget.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Wait {
    /// The arguments it was refused for: asked again only for these.
    args: Vec<Value>,
    /// Re-asks scheduled in this streak.
    tries: usize,
    /// When it is asked again; infinite once asked, waiting for its answer.
    due_ms: f64,
}

/// Whether `error` is a turn over its budget.
pub(super) fn is_overrun(error: &RunnerError) -> bool {
    matches!(
        error,
        RunnerError::Data {
            error: DataError::Overrun(_),
            ..
        }
    )
}

impl<D: DataSource> Runner<D> {
    /// Resource `i`, asked for `args`, was refused by a turn over its budget:
    /// its re-ask scheduled, `true`; or, its streak spent, none, `false`, and
    /// the caller fails it as any refusal.
    pub(super) fn overran(&mut self, i: usize, args: &[Value], why: &str) -> bool {
        let tries = match &self.overruns[i] {
            Some(w) if w.args == args => w.tries,
            _ => 0,
        };
        let Some(&after) = REASK_MS.get(tries) else {
            self.overruns[i] = None;
            return false;
        };
        self.overruns[i] = Some(Wait {
            args: args.to_vec(),
            tries: tries + 1,
            due_ms: self.now_ms + after,
        });
        let name = self.plan.str(self.plan.resources[i].name).to_string();
        self.log(lines::overrun_reask(
            &name,
            why,
            after,
            tries + 1,
            REASK_MS.len(),
        ));
        true
    }

    /// Whether resource `i` waits for a re-ask (it reads as pending).
    pub(super) fn overrun_waits(&self, i: usize) -> bool {
        self.overruns.get(i).is_some_and(Option::is_some)
    }

    /// Resource `i` answered, or is asked anew: its streak ends.
    pub(super) fn overrun_ends(&mut self, i: usize) {
        if let Some(w) = self.overruns.get_mut(i) {
            *w = None;
        }
    }

    /// Resource `i` is asked for `args`: a wait for other arguments is stale.
    pub(super) fn overrun_args(&mut self, i: usize, args: &[Value]) {
        if self.overruns[i].as_ref().is_some_and(|w| w.args != args) {
            self.overruns[i] = None;
        }
    }

    /// The next re-ask, for the host's wake (`timer_due_ms`).
    pub(super) fn overrun_due_ms(&self) -> Option<f64> {
        self.overruns
            .iter()
            .flatten()
            .map(|w| w.due_ms)
            .filter(|ms| ms.is_finite())
            .reduce(f64::min)
    }

    /// The re-asks due by now, in one commit; a resource whose arguments the
    /// last settlement no longer asks (changed, or not settled at all) is
    /// dropped with no ask.
    pub(super) fn reask_overruns(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        let (mut which, mut dropped) = (Vec::new(), false);
        for i in 0..self.overruns.len() {
            let Some(w) = &self.overruns[i] else { continue };
            if w.due_ms > self.now_ms {
                continue;
            }
            let current = self.settled.as_ref().and_then(|s| s.resource_args.get(i));
            if current != Some(&w.args) {
                self.overruns[i] = None;
                dropped = true;
                continue;
            }
            if let Some(w) = self.overruns[i].as_mut() {
                w.due_ms = f64::INFINITY;
            }
            self.force_refresh(i);
            which.push(i);
        }
        if which.is_empty() && !dropped {
            return Ok(None);
        }
        // (a dropped wait leaves `pending`, and the view hears it in this commit too)
        self.sync_pending_flags();
        self.commit_again(which, "a turn over its budget, asked again")
            .map(Some)
    }

    /// The reply to `ticket`, resource `i`'s, was refused by a turn over its
    /// budget: its re-ask scheduled and the request let go, `Some`; or its
    /// streak spent, `None`, and the caller releases it as failed.
    pub(super) fn release_overrun(
        &mut self,
        ticket: u64,
        i: usize,
        why: &str,
    ) -> Option<Result<Option<CommitReceipt>, RunnerError>> {
        if self.streaming(i) {
            return None;
        }
        let args = self
            .pending
            .iter()
            .find(|p| p.ticket == ticket)?
            .args
            .clone();
        if !self.overran(i, &args, why) {
            return None;
        }
        self.pending.retain(|p| p.ticket != ticket);
        self.forgot = true;
        self.sync_pending_flags();
        Some(
            self.commit_again(Vec::new(), "a turn over its budget")
                .map(Some),
        )
    }

    /// Whether `target`'s reply error is a resource's turn over its budget.
    pub(super) fn overran_reply(target: Target, error: &RunnerError) -> Option<usize> {
        match target {
            Target::Resource(i) if is_overrun(error) => Some(i),
            _ => None,
        }
    }
}
