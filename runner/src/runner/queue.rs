//! Sends that wait their turn (LLP 1092 D2–D4): a `queue` mutation has one
//! request in flight; a later send waits, with the arguments it was made
//! with, and is asked in a `next` commit of its own once the mutation is
//! free — its reply landed or let go, and that reply's `then` run.
//!
//! The waiting sends and the stalls are in the commit's checkpoint, so a
//! refused action queues nothing and a refused `next` puts its head back.
//! `next_due` is not: like `then_due` it is cleared before its commit, and
//! armed only by the scan ([`Runner::arm_next`]) after a commit concludes.

use super::*;
use std::collections::VecDeque;
use std::rc::Rc;

/// At most this many sends wait per mutation (LLP 1092 D4); the send in
/// flight does not count. One more refuses its action: dropping a write
/// silently is the bug a queue removes (LLP 1041 D2).
pub const QUEUE_BOUND: usize = 64;

/// A send that waits: the source it names and its send-time arguments.
#[derive(Clone)]
pub(super) struct Waiting {
    source: String,
    args: Vec<Value>,
}

/// The state a refused `next` saw. A standing commit that changes a slot,
/// a resource or a derive value from it clears the stall (D3); one that
/// changes none of them leaves the send waiting, so a permanently failing
/// ask is not retried every frame.
pub(super) struct Basis {
    slots: Vec<Value>,
    derives: Vec<Option<Value>>,
    resources: Vec<Option<crate::held::Held>>,
}

/// Every mutation's waiting sends, when its head is due, and its stall.
#[derive(Default)]
pub(super) struct Queues {
    waiting: Vec<VecDeque<Waiting>>,
    /// When each mutation's head is asked, as a one-shot timer: infinite
    /// while nothing is due. Reported by `timer_due_ms` as `then_due` is.
    pub(super) next_due: Vec<f64>,
    stalled: Vec<Option<Rc<Basis>>>,
    /// The last advance stopped on a `next`'s refusal: a frame's armed
    /// frame tasks fire anyway (D3).
    pub(super) stopped_on_next: bool,
}

/// What a commit's checkpoint keeps of the queues.
pub(super) struct Saved {
    waiting: Vec<VecDeque<Waiting>>,
    stalled: Vec<Option<Rc<Basis>>>,
}

/// How a `next` commit was refused: by its own ask (the source erred, or
/// its answer missed the shape), which can never succeed, or by anything
/// else, which may once the state changes.
pub(super) enum NextRefusal {
    Ask,
    Other,
}

impl Queues {
    pub(super) fn new(mutations: usize) -> Queues {
        Queues {
            waiting: vec![VecDeque::new(); mutations],
            next_due: vec![f64::INFINITY; mutations],
            stalled: vec![None; mutations],
            stopped_on_next: false,
        }
    }

    pub(super) fn save(&self) -> Saved {
        Saved {
            waiting: self.waiting.clone(),
            stalled: self.stalled.clone(),
        }
    }

    pub(super) fn restore(&mut self, s: Saved) {
        self.waiting = s.waiting;
        self.stalled = s.stalled;
    }

    /// Whether a send of `m` waits.
    pub(super) fn waits(&self, m: usize) -> bool {
        self.waiting.get(m).is_some_and(|w| !w.is_empty())
    }
}

impl<D: DataSource> Runner<D> {
    /// Whether `m` is free (D3): no request of it in flight, neither its
    /// `then` nor its `next` armed, and not stalled. Checked from state,
    /// never from the kind of commit.
    fn idle(&self, m: usize) -> bool {
        !self.pending.iter().any(|p| p.target == Target::Mutation(m))
            && !self.then_due[m].is_finite()
            && !self.queues.next_due[m].is_finite()
            && self.queues.stalled[m].is_none()
    }

    /// Whether a send of queue mutation `m` made now is asked now (D2):
    /// `m` is free and no send waits before it.
    pub(super) fn asks_now(&self, m: usize) -> bool {
        self.idle(m) && !self.queues.waits(m)
    }

    /// A send of `m` that joins its queue: nothing is asked and nothing
    /// read again (D4) until its turn. The 65th waiter refuses its action.
    pub(super) fn wait_turn(
        &mut self,
        m: usize,
        source: &str,
        args: &[Value],
    ) -> Result<(), RunnerError> {
        if self.queues.waiting[m].len() >= QUEUE_BOUND {
            return Err(RunnerError::QueueFull {
                mutation: self.plan.str(self.plan.mutations[m].name).to_string(),
            });
        }
        self.queues.waiting[m].push_back(Waiting {
            source: source.to_string(),
            args: args.to_vec(),
        });
        self.pending_mut[m] = true;
        Ok(())
    }

    /// The scan (D3), after every commit concludes, whether it stood or
    /// was refused, and on a refused admission's early return: a stalled
    /// mutation whose basis a standing commit changed is let go, and a
    /// free mutation with a send waiting is due now.
    pub(super) fn arm_next(&mut self, stood: bool) {
        for m in 0..self.queues.waiting.len() {
            if stood
                && self.queues.stalled[m]
                    .as_ref()
                    .is_some_and(|b| self.changed_from(b))
            {
                self.queues.stalled[m] = None;
            }
            if self.queues.waits(m) && self.idle(m) {
                self.queues.next_due[m] = self.now_ms;
            }
        }
    }

    /// Whether a slot, resource or derive value differs from `basis`.
    fn changed_from(&self, basis: &Basis) -> bool {
        use crate::compare::equivalent;
        let held = |a: &Option<crate::held::Held>, b: &Option<crate::held::Held>| match (a, b) {
            (Some(a), Some(b)) => crate::held::Held::equivalent(a, b),
            (a, b) => a.is_none() && b.is_none(),
        };
        let derive = |a: &Option<Value>, b: &Option<Value>| match (a, b) {
            (Some(a), Some(b)) => equivalent(a, b),
            (a, b) => a.is_none() && b.is_none(),
        };
        !(basis.slots.len() == self.slots.len()
            && basis
                .slots
                .iter()
                .zip(&self.slots)
                .all(|(a, b)| equivalent(a, b))
            && basis
                .derives
                .iter()
                .zip(&self.derives)
                .all(|(a, b)| derive(a, b))
            && basis
                .resources
                .iter()
                .zip(&self.resource_values)
                .all(|(a, b)| held(a, b)))
    }

    /// The earliest due `next` by `now_ms`, by index on ties.
    pub(super) fn due_next(&self, now_ms: f64) -> Option<(usize, f64)> {
        self.queues
            .next_due
            .iter()
            .enumerate()
            .filter(|(_, at)| **at <= now_ms)
            .min_by(|(ia, a), (ib, b)| a.partial_cmp(b).unwrap().then(ia.cmp(ib)))
            .map(|(m, at)| (m, *at))
    }

    /// A `next` commit (D3): `m`'s head, popped and asked as an action's
    /// send is. Its own ask's refusal drops the head (it can never be
    /// asked) and the successor is due; any other refusal keeps the head
    /// and stalls `m` until a standing commit changes the state.
    pub(super) fn run_next(&mut self, m: usize) -> Result<CommitReceipt, RunnerError> {
        self.queues.next_due[m] = f64::INFINITY;
        let name = self.plan.str(self.plan.mutations[m].name).to_string();
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        let (result, refusal) = match self.next_inner(m, &name) {
            Ok(receipt) => (Ok(receipt), None),
            Err((e, why)) => (Err(e), Some(why)),
        };
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_then(result.is_ok());
        if let Err(e) = &result {
            if self.poisoned {
                self.log_outcome(&format!("{name} next"), &result, was_poisoned);
            } else if let Some(NextRefusal::Ask) = refusal {
                let was = self.pending_mut[m];
                self.queues.waiting[m].pop_front();
                self.sync_pending_flags();
                self.log(format!("{name} queued send refused: {e:?}"));
                // Nothing waits behind it and nothing is in flight: the
                // view hears `pending` end, as after a failed reply.
                if was && !self.pending_mut[m] {
                    let _ = self.commit_again(Vec::new(), "a refused queued send");
                }
            } else {
                self.queues.stalled[m] = Some(Rc::new(Basis {
                    slots: self.slots.clone(),
                    derives: self.derives.clone(),
                    resources: self.resource_values.clone(),
                }));
                self.log(format!("{name} next refused ({e:?}); waits for a change"));
            }
        }
        self.arm_next(result.is_ok());
        result
    }

    fn next_inner(
        &mut self,
        m: usize,
        name: &str,
    ) -> Result<CommitReceipt, (RunnerError, NextRefusal)> {
        let other = |e| (e, NextRefusal::Other);
        if self.poisoned {
            return Err(other(RunnerError::Poisoned));
        }
        let Some(head) = self.queues.waiting[m].pop_front() else {
            return self.update().map_err(other);
        };
        let waiting = self.queues.waiting[m].len();
        self.log(format!("{name} next ({waiting} waiting)"));
        let later = match self.ask_send(m, &head.source, &head.args) {
            Ok(Asked::Now(slot, value)) => {
                self.slots[slot] = value;
                self.landed.push(m);
                None
            }
            Ok(Asked::Later(request)) => Some(request),
            Err(e) => return Err((e, NextRefusal::Ask)),
        };
        self.sync_pending_flags();
        if later.is_some() {
            self.pending_mut[m] = true;
        }
        // An asked send reads again what its mutation declares it changes,
        // with the arguments of this commit (D4).
        self.reread_next = self.declared_refreshes(m);
        if let Err(e) = self.router_change().and_then(|_| self.settle(false)) {
            if let Some(request) = &later {
                self.discard_request(request);
            }
            return Err(other(e));
        }
        if let Some(request) = later {
            self.enqueue(Target::Mutation(m), head.source, head.args, request, false);
        }
        self.update().map_err(other)
    }

    /// Drop every waiting send — poison, or a reload that does not carry
    /// them (D4). They were never asked: no ticket, no source call, one
    /// journal line per mutation.
    pub(super) fn forget_waiting(&mut self) {
        for m in 0..self.queues.waiting.len() {
            let n = std::mem::take(&mut self.queues.waiting[m]).len();
            self.queues.next_due[m] = f64::INFINITY;
            self.queues.stalled[m] = None;
            if n > 0 {
                let name = self.plan.str(self.plan.mutations[m].name).to_string();
                self.log(super::lines::forgot_waiting(n, &name));
            }
        }
    }

    /// Each mutation with sends waiting, and how many (the agent's
    /// `state.queued`, LLP 1092 D10).
    pub fn queued(&self) -> Vec<(String, usize)> {
        self.queues
            .waiting
            .iter()
            .enumerate()
            .filter(|(_, w)| !w.is_empty())
            .map(|(m, w)| {
                (
                    self.plan.str(self.plan.mutations[m].name).to_string(),
                    w.len(),
                )
            })
            .collect()
    }
}

/// What asking a send gave: an answer now for the mutation's slot, or a
/// request for the host.
pub(super) enum Asked {
    Now(usize, Value),
    Later(Request),
}

impl<D: DataSource> Runner<D> {
    /// Ask the source for one send of `m` (LLP 1016 §4): an answer now must
    /// conform to the mutation's shape.
    pub(super) fn ask_send(
        &mut self,
        m: usize,
        source: &str,
        args: &[Value],
    ) -> Result<Asked, RunnerError> {
        let ty = self.plan.mutations[m].ty;
        let name = |plan: &Plan| plan.str(plan.mutations[m].name).to_string();
        let answer = self
            .data
            .answer_for(Target::Mutation(m), &mut self.store, source, args)
            .map_err(|error| RunnerError::Data {
                resource: name(&self.plan),
                error,
            })?;
        match answer {
            Answer::Now(v) => {
                if !self.conforms(&v, ty) {
                    return Err(RunnerError::Shape {
                        resource: name(&self.plan),
                    });
                }
                let slot = self.mutation_slot(m)?;
                Ok(Asked::Now(slot, Value::some(v)))
            }
            Answer::Later(request) => Ok(Asked::Later(request)),
        }
    }
}
