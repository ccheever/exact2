//! A resource a turn over its budget refused (`DataError::Overrun`, exact-js's
//! own time or its wall backstop) is asked again, twice at most, before it
//! fails as any refused resource does (LLP 1027 as amended 2026-10-27; theme2's
//! 18:06 ruling and the 18:55 redesign). The refusal is the machine's load as
//! much as the source's: under load one stood a resource on a stale value until
//! the app's own 10 s redial (Excord's gateway, x2-perf4's evidence).
//!
//! The wait is a request the runner holds and never sends (`reask_at`): the
//! resource's ask in flight, so it reads exactly as one (`pending`, its last
//! value, no failure), settlement reuses it as one (a commit that doesn't ask
//! the resource again burns no try), and it keeps its ask's origin and
//! invocation. It comes due in the runner's own deadline order and is asked
//! again as a commit of that ask; the streak counts each re-ask as it is made.
//! A mutation is never asked again (LLP 1041 D2), nor a stream's message; an
//! argument change, a refresh or an answer ends the streak, as each replaces
//! or answers the ask.
use super::{lines, CommitReceipt, DataError, DataSource, PendingReq, Runner, RunnerError, Target};
use crate::failure::Failure;
use crate::Outcome;
use exact_plan::Value;

/// The re-ask being committed: its resource, the re-asks its streak has made
/// with it, and its arguments; and whether the reply being landed was refused
/// by its own turn over its budget.
#[derive(Default)]
pub(super) struct Overrun {
    pub(super) reasking: Option<(usize, usize, Vec<Value>)>,
    pub(super) replied: bool,
}

/// How long after each refusal the resource is asked again: spaced, so a spike
/// of load that refused it has passed (an idle tick under load can come at
/// once and refuse it again).
const REASK_MS: [f64; 2] = [250.0, 1000.0];

/// The delay before the re-ask after `reasks` in a streak; none once spent.
pub(super) fn reask_after(reasks: usize) -> Option<f64> {
    REASK_MS.get(reasks).copied()
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
    /// The streak an ask of `target` for `args` made now continues (a request
    /// sent, or a turn over its budget that refused it): a re-ask's, in its
    /// commit; none for a forced ask (a refresh starts anew); else that of the
    /// ask in flight for these arguments it replaces (a store reader asked
    /// again keeps its count).
    pub(super) fn overrun_reasks(&self, target: Target, args: &[Value], forced: bool) -> usize {
        match (target, &self.overrun.reasking) {
            // (the re-ask's own arguments: new ones start anew, forced)
            (Target::Resource(i), Some((r, reasks, asked))) if i == *r && asked == args => *reasks,
            _ if forced => 0,
            _ => (self.pending.iter())
                .find(|p| p.target == target && p.args == args)
                .map_or(0, |p| p.reasks),
        }
    }

    /// Hold resource `i`'s ask for `args`, refused by a turn over its budget,
    /// to be asked again (settlement's publish). Asked again unforced (a store
    /// reader), the same wait as the one standing keeps it: its time, its
    /// ticket and so its stamps. Otherwise it replaces the ask in flight, under
    /// a ticket of this commit's origin and invocation (a re-ask's commit is
    /// its ask's; a refresh's is its own).
    pub(super) fn hold_reask(
        &mut self,
        i: usize,
        source: String,
        args: Vec<Value>,
        reasks: usize,
        forced: bool,
        why: &str,
    ) {
        let target = Target::Resource(i);
        if !forced
            && (self.pending.iter()).any(|p| {
                p.target == target && p.reask_at.is_some() && p.args == args && p.reasks == reasks
            })
        {
            return;
        }
        let after = REASK_MS[reasks];
        let at = self.refused_at() + after;
        self.forget_ticket(target);
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.note_asked(target, ticket);
        self.pending.push(PendingReq {
            inspection_generation: self.inspected_request(i),
            refusal: None,
            refused: false,
            ticket,
            target,
            source,
            args,
            continuation: None,
            keepable: None,
            stream: None,
            upload: false,
            sent: false,
            grant: None,
            ask_again: false,
            origin: self.writes.tick(),
            write: None,
            held: false,
            long_poll: false,
            replied: false,
            reask_at: Some(at),
            reasks,
            module_again: None,
            effects: false,
        });
        self.said_reask(i, why, after, reasks);
    }

    /// When a turn over its budget refused the ask, for its re-ask's delay: a
    /// live clock's fresh reading, so a slow turn doesn't spend its own delay;
    /// the commit's time on a seek clock (the agent's, a test's).
    fn refused_at(&mut self) -> f64 {
        let read = (self.clock.sample.as_mut()).map(|sample| sample());
        read.filter(|t| t.is_finite() && *t <= super::MAX_CLOCK_MS)
            .map_or(self.now_ms, |t| t.max(self.now_ms))
    }

    /// Whether `ticket` is a re-ask's wait, held by the runner and never a
    /// host's to run: a render completes past it, and its checkpoint lists the
    /// resource as pending for the client to ask.
    pub fn is_reask_wait(&self, ticket: u64) -> bool {
        (self.pending.iter()).any(|p| p.ticket == ticket && p.reask_at.is_some())
    }

    fn said_reask(&mut self, i: usize, why: &str, after: f64, reasks: usize) {
        let name = self.plan.str(self.plan.resources[i].name).to_string();
        let line = lines::overrun_reask(&name, why, after, reasks + 1, REASK_MS.len());
        self.log(line);
    }

    /// Request `ticket`'s reply was refused by its own turn over its budget
    /// (`error` names its resource, not one settled after it): held to be
    /// asked again under a new ticket carrying the ask's origin and invocation
    /// (the host's is let go), `true`. A mutation's, a stream's, another
    /// resource's turn, or a spent streak: `false`, and the caller releases it
    /// as failed.
    pub(super) fn hold_overrun_reply(
        &mut self,
        ticket: u64,
        target: Target,
        error: &RunnerError,
    ) -> bool {
        let own = std::mem::take(&mut self.overrun.replied);
        let Target::Resource(i) = target else {
            return false;
        };
        let name = self.target_name(target);
        let named = matches!(
            error,
            RunnerError::Data { resource, error: DataError::Overrun(_) } if *resource == name
        );
        if !own || !named || self.streaming(i) {
            return false;
        }
        let Some(pos) =
            (self.pending.iter()).position(|p| p.ticket == ticket && p.stream.is_none())
        else {
            return false;
        };
        let reasks = self.pending[pos].reasks;
        let Some(after) = reask_after(reasks) else {
            return false;
        };
        let (wait, at) = (self.next_ticket, self.refused_at() + after);
        self.next_ticket += 1;
        let p = &mut self.pending[pos];
        p.ticket = wait;
        p.reask_at = Some(at);
        p.continuation = None;
        p.keepable = None;
        p.grant = None;
        p.held = false;
        p.long_poll = false;
        p.replied = false;
        p.refusal = None;
        p.refused = false;
        self.restamp(ticket, wait);
        // The source's turn is over: it hears so now, with no commit to come,
        // and what it set aside is kept or let go, as at a commit's end.
        self.forgot = true;
        self.hear_forgotten();
        self.keep_sent(false);
        self.log(lines::forgot(ticket, &name));
        self.said_reask(i, &Failure::of(error).message, after, reasks);
        true
    }

    /// The earliest re-ask due by `by`: its wait's ticket and time.
    pub(super) fn reask_due(&self, by: f64) -> Option<(u64, f64)> {
        (self.pending.iter())
            .filter_map(|p| Some((p.ticket, p.reask_at?)))
            .filter(|(_, at)| *at <= by)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }

    /// When the next re-ask after a turn over its budget is due: the host's
    /// wake (`timer_due_ms`), and where the agent's `settle` goes, the wait
    /// being no I/O to wait on ([`Runner::has_pending`] leaves it out).
    pub fn reask_due_ms(&self) -> Option<f64> {
        (self.pending.iter())
            .filter_map(|p| p.reask_at)
            .filter(|at| at.is_finite())
            .reduce(f64::min)
    }

    /// Ask again, forced, the resource wait `ticket` holds, as a commit of its
    /// ask's origin and invocation; the re-ask counts in its streak as it is
    /// made. A refused commit's rollback restores the wait intact, and it is
    /// due again after the next delay, that re-ask counted; its streak spent,
    /// the resource fails as on a third turn over its budget, and that commit
    /// is the receipt.
    pub(super) fn reask(&mut self, ticket: u64) -> Result<CommitReceipt, RunnerError> {
        let p = (self.pending.iter())
            .find(|p| p.ticket == ticket)
            .expect("a wait due");
        let (Target::Resource(i), reasks) = (p.target, p.reasks) else {
            unreachable!("only a resource waits to be asked again")
        };
        let args = p.args.clone();
        let (origin, id) = (self.ticket_origin(ticket), self.ticket_id(ticket));
        let result = self.as_invocation(origin, id, |r| {
            let before = r.overrun.reasking.replace((i, reasks + 1, args));
            let result = r.commit_again(vec![i], "a turn over its budget, asked again");
            r.overrun.reasking = before;
            result
        });
        let refused = match &result {
            Err(e) => Some(Failure::of(e).message),
            // Stood without asking it (a source not ready): its forced ask
            // replaces the wait at the first settlement that can.
            Ok(_) if self.refresh_next.contains(&i) => {
                if let Some(p) = self.pending.iter_mut().find(|p| p.ticket == ticket) {
                    p.reask_at = Some(f64::INFINITY);
                }
                None
            }
            // Stood after a refusal that failed what it owed.
            Ok(_) => Some("its commit was refused".to_string()),
        };
        let Some(why) = refused.filter(|_| self.pending.iter().any(|p| p.ticket == ticket)) else {
            return result;
        };
        match reask_after(reasks + 1) {
            Some(after) => {
                let at = self.refused_at() + after;
                if let Some(p) = self.pending.iter_mut().find(|p| p.ticket == ticket) {
                    p.reasks = reasks + 1;
                    p.reask_at = Some(at);
                }
                let why = format!("asked again, its commit was refused: {why}");
                self.said_reask(i, &why, after, reasks + 1);
            }
            None => {
                let failure = Failure::error(format!(
                    "asked again after a turn over its budget, its commit was refused: {why}"
                ));
                // The commit that fails it is what the host must see: the
                // refusal is in the journal and in the failure's reason.
                // (under its ask's origin and invocation, as each re-ask was)
                let released = self.as_invocation(origin, id, |r| {
                    r.release_failed(ticket, Target::Resource(i), failure)
                });
                return released.map(|r| r.expect("release_failed commits"));
            }
        }
        result
    }

    /// A reply to a re-ask's wait, which was never sent: its invocation ended
    /// (a wake's deadline fails every request it owns), and the resource fails
    /// as its ask would have; anything else is dropped.
    pub(super) fn reask_answered(
        &mut self,
        ticket: u64,
        outcome: Outcome,
        summary: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let target = (self.pending.iter())
            .find(|p| p.ticket == ticket)
            .expect("a wait")
            .target;
        let (origin, id) = (self.ticket_origin(ticket), self.ticket_id(ticket));
        match outcome {
            Outcome::Failed { message, .. } => self.as_invocation(origin, id, |r| {
                r.release_failed(ticket, target, Failure::error(message))
            }),
            _ => {
                self.log(lines::dropped(ticket, summary));
                Ok(None)
            }
        }
    }
}
