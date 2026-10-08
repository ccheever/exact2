//! Admission failures occupy the already-bounded current target ticket, never
//! a separate failure queue. Supersession drops them with the old ticket.
use super::{CommitReceipt, DataSource, Outcome, Runner, RunnerError, Target};
use crate::failure::Failure;
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
    /// source that shapes the failure settles it like any reply. A failure
    /// it cannot take ends the request without automatically asking again:
    /// earlier source turns may already have executed effects (LLP 1041 D2).
    pub fn take_request_refusal(&mut self, allow_ordered: bool) -> Option<(u64, Outcome)> {
        // An auth session's answer (LLP 1069.006), settled by the runner or
        // reported by the host, rides the same path.
        if let Some(settled) = crate::auth::take_any_settled(self) {
            return Some(settled);
        }
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

    /// A reply the source failed to take (it threw, ran over its budget, or
    /// answered outside its shape). The refused commit's rollback restored
    /// the request to the pending set, but the host has already delivered
    /// its one reply: left there it would be pending forever, and a button
    /// disabled on `pending(…)` with it. It is let go instead. A resource
    /// keeps the value it had and is not asked again here (a source that
    /// fails the same way every time would loop); `refresh` asks again. A
    /// mutation is not retried; effects from earlier turns are not undone.
    pub(super) fn release_failed(
        &mut self,
        ticket: u64,
        target: Target,
        why: Failure,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let (failed_args, ask_again) = self
            .pending
            .iter()
            .find(|p| p.ticket == ticket)
            .map(|p| (Some(p.args.clone()), p.ask_again))
            .unwrap_or_default();
        self.pending.retain(|p| p.ticket != ticket);
        self.forgot = true;
        self.sync_pending_flags();
        let name = self.target_name(target);
        let mut again = Vec::new();
        let next = match target {
            Target::Resource(i) => {
                // Keep the standing answer's arguments. Failure suppresses
                // another ask independently of that answer's store revision.
                self.failed_args[i] = failed_args;
                self.failed_why[i] = Some(why);
                // Unless a watched topic changed while it was in flight: the
                // answer may differ now, so it is asked once more, forced
                // (LLP 1016.002 D4).
                if ask_again {
                    again.push(i);
                }
                "it keeps its last value"
            }
            Target::Mutation(_) => "it is not retried",
        };
        let what = format!("request {ticket} ({name}) failed and is no longer pending: {next}");
        self.log(what.clone());
        if ask_again {
            self.log(super::lines::asked_again(ticket, &name));
        }
        self.commit_again(again, "a failed request").map(Some)
    }

    /// The host must keep later ordered admissions behind these refusals.
    pub fn has_ordered_request_refusals(&self) -> bool {
        self.pending
            .iter()
            .any(|p| p.refusal.is_some_and(|(_, ordered)| ordered))
    }

    /// More refused admissions need a future host pump.
    pub fn has_request_refusals(&self, allow_ordered: bool) -> bool {
        self.auth.has_settled_for(|t| self.holds(t))
            || self.pending.iter().any(|p| {
                p.refusal
                    .is_some_and(|(_, ordered)| !ordered || allow_ordered)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure::FailureCode;
    use crate::{Answer, DataError, Request, RequestOut, Response, Store};
    use exact_kernel::{Kernel, NodeType};
    use exact_plan::{asm::Asm, builder::PlanBuilder, Opcode, TypeKind, Value};

    #[derive(Default)]
    struct Source {
        deferred: bool,
        write_first: bool,
        shape_refusal: bool,
        asks: usize,
        synchronous_effects: usize,
        delivered_writes: usize,
    }

    impl DataSource for Source {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!("the fixture implements answer")
        }

        fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            self.asks += 1;
            store.observe_topic("value");
            if !self.deferred {
                return Ok(Answer::Now(Value::some(Value::str("standing"))));
            }
            // Like a synchronous native.call write before the first await:
            // this effect is outside Store's rollback and cannot be replayed.
            self.synchronous_effects += 1;
            Ok(Answer::Later(if self.write_first {
                Request::post_json("https://example.test/write", "{}")
            } else {
                Request::get("https://example.test/read")
            }))
        }

        fn parse(
            &mut self,
            _: &mut Store,
            _: &str,
            _: &[Value],
            outcome: Outcome,
        ) -> Result<Answer, DataError> {
            match outcome {
                Outcome::Response(Response { status: 201, .. }) => {
                    self.delivered_writes += 1;
                    Ok(Answer::Later(Request::get("https://example.test/read")))
                }
                Outcome::Response(_) => Ok(Answer::Now(Value::some(Value::str("fresh")))),
                Outcome::Failed { .. } if self.shape_refusal => Ok(Answer::Now(Value::NONE)),
                Outcome::Failed { kind, message } => {
                    Err(DataError::Failed(FailureCode::of_kind(kind), message))
                }
                other => panic!("unexpected outcome: {other:?}"),
            }
        }
    }

    fn boot(unwrap: bool) -> Runner<Source> {
        let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        let string = b.primitive(TypeKind::String);
        let optional = b.option(string);
        let value = b.resource("value", "read", &[], optional, None);
        if unwrap {
            let mut body = Asm::new();
            body.load_resource(value).simple(Opcode::Unwrap);
            let body = b.code(body);
            b.derive("shown", string, body);
        }
        let mut body = Asm::new();
        body.refresh(value);
        let body = b.code(body);
        b.action("refresh", &[], &[], body);
        b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        Runner::boot(
            b.finish().unwrap(),
            Source::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap()
    }

    fn one_request(r: &mut Runner<Source>) -> RequestOut {
        let mut requests = r.take_requests();
        assert_eq!(requests.len(), 1, "{requests:?}");
        requests.pop().unwrap()
    }

    fn ask(r: &mut Runner<Source>) -> RequestOut {
        r.data().deferred = true;
        r.act("refresh", vec![]).unwrap();
        one_request(r)
    }

    fn refuse(r: &mut Runner<Source>, ticket: u64) {
        r.refuse_request(ticket, "native executor admission limit reached", true);
        assert!(r.take_request_refusal(false).is_none());
        let (refused, outcome) = r.take_request_refusal(true).unwrap();
        assert_eq!(refused, ticket);
        assert!(r.fulfill(ticket, outcome).unwrap().is_some());
    }

    fn response(status: u16) -> Outcome {
        Outcome::Response(Response {
            status,
            headers: vec![],
            body: vec![],
        })
    }

    fn assert_failed_without_replay(r: &mut Runner<Source>, ticket: u64) {
        assert!(!r.has_pending());
        assert!(!r.holds(ticket));
        assert!(!r.has_ordered_request_refusals());
        assert!(r.take_requests().is_empty());
        assert_eq!(r.data().asks, 2, "boot and the explicit refresh only");
        assert_eq!(r.data().synchronous_effects, 1);
        assert_eq!(
            r.resource("value"),
            Some(&Value::some(Value::str("standing")))
        );
        assert_eq!(r.failed_args[0], Some(vec![]));
        // An unrelated pump and a stale completion cannot restart it either.
        r.advance(1.).unwrap();
        assert!(r.fulfill(ticket, response(200)).unwrap().is_none());
        assert!(r.take_requests().is_empty());
        assert_eq!(r.data().synchronous_effects, 1);
    }

    #[test]
    fn refusing_the_first_request_does_not_repeat_prior_synchronous_effects() {
        let mut r = boot(false);
        let request = ask(&mut r);
        assert_eq!(request.request.method, "GET");
        refuse(&mut r, request.ticket);
        assert_failed_without_replay(&mut r, request.ticket);
        let failure = r.failed_why[0].as_ref().unwrap();
        assert_eq!(failure.code, FailureCode::Refused);
        assert_eq!(failure.message, "native executor admission limit reached");
        // A user refresh remains an intentional new source invocation.
        let next = ask(&mut r);
        assert_ne!(next.ticket, request.ticket);
        r.fulfill(next.ticket, response(200)).unwrap();
        assert_eq!(r.data().synchronous_effects, 2);
        assert!(r.failed_resources().is_empty());
    }

    #[test]
    fn refusing_a_later_read_does_not_replay_a_write_that_already_completed() {
        let mut r = boot(false);
        r.data().write_first = true;
        let write = ask(&mut r);
        assert_eq!(write.request.method, "POST");
        // The host has executed the write and reports its successful response.
        r.fulfill(write.ticket, response(201)).unwrap();
        let read = one_request(&mut r);
        assert_eq!(read.request.method, "GET");
        assert_eq!(r.data().delivered_writes, 1);
        refuse(&mut r, read.ticket);
        assert_failed_without_replay(&mut r, read.ticket);
        assert_eq!(r.data().delivered_writes, 1);
    }

    #[test]
    fn a_refusal_that_traps_in_a_derive_releases_its_ticket_without_replay() {
        let mut r = boot(true);
        r.data().shape_refusal = true;
        let request = ask(&mut r);
        // `none` conforms to the resource's type, but the derive's unwrap
        // traps before updating the tree. The standing value is restored.
        refuse(&mut r, request.ticket);
        assert!(!r.is_poisoned());
        assert_failed_without_replay(&mut r, request.ticket);
        let failure = r.failed_why[0].as_ref().unwrap();
        assert_eq!(failure.code, FailureCode::Error);
        assert!(failure.message.contains("UnwrapNone"), "{failure:?}");
    }

    #[test]
    fn a_topic_change_still_refreshes_once_after_a_refused_request() {
        let mut r = boot(false);
        let request = ask(&mut r);
        for _ in 0..3 {
            r.changed("value").unwrap();
        }
        assert!(r.holds(request.ticket));
        assert!(r.take_requests().is_empty());
        refuse(&mut r, request.ticket);
        let next = one_request(&mut r);
        assert_ne!(next.ticket, request.ticket);
        assert_eq!(r.data().asks, 3, "one re-ask for all topic announcements");
        r.fulfill(next.ticket, response(200)).unwrap();
        assert!(!r.has_pending());
        assert!(r.take_requests().is_empty());
        assert_eq!(r.resource("value"), Some(&Value::some(Value::str("fresh"))));
        assert!(r.failed_resources().is_empty());
    }
}
