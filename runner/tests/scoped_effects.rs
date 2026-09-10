//! Runtime-only proofs of the store fixpoint, placeholder readiness and rollback.
use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, EventKind, Opcode, RegionKind, TypeKind, Value};
use exact_runner::{Answer, DataError, DataSource, Event, Request, Runner, Store};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct Data {
    ready: Rc<Cell<bool>>,
    asks: Rc<RefCell<Vec<String>>>,
    begun: Rc<RefCell<Vec<u64>>>,
    canceled: Rc<RefCell<Vec<u64>>>,
}
impl DataSource for Data {
    fn answer_scoped(
        &mut self,
        context: u64,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.begun.borrow_mut().push(context);
        self.answer(store, source, args)
    }
    fn cancel_scoped(&mut self, context: u64) {
        self.canceled.borrow_mut().push(context);
    }
    fn ready(&self) -> bool {
        self.ready.get()
    }
    fn grants(&self) -> &str {
        "secret.keep flag"
    }
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.asks.borrow_mut().push(source.into());
        match source {
            "read" => Ok(Answer::Now(Value::str(store.get("flag").unwrap_or("old")))),
            "closed" => Ok(Answer::Now(Value::Bool(store.get("flag").is_some()))),
            "write" => {
                store.set("flag", "new")?;
                Ok(Answer::Now(Value::str("written")))
            }
            "pending" if self.ready() => {
                Ok(Answer::Later(Request::get("https://example.test/owned")))
            }
            "guard" if args == [Value::Bool(true)] => {
                Err(DataError::Unavailable("guard refuses".into()))
            }
            "guard" => Ok(Answer::Now(Value::str("ok"))),
            _ => Err(DataError::Unavailable("source is not ready".into())),
        }
    }
}
fn root(b: &mut PlanBuilder) -> exact_plan::NodesId {
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None)
}
fn scope(
    b: &mut PlanBuilder,
    root: exact_plan::NodesId,
    order: u32,
) -> (exact_plan::RegionsId, exact_plan::ArmsId) {
    let unit = b.constant(&Value::Unit);
    let (scope, arms) = b.region(RegionKind::Scope, Some(root), None, order, unit, unit, 1);
    (scope, arms[0])
}

#[test]
fn a_scoped_writer_settles_root_and_scoped_readers_in_one_commit() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    b.resource("rootRead", "read", &[], string, None);
    let root = root(&mut b);
    let (scope, _) = scope(&mut b, root, 0);
    let read = b.resource("childRead", "read", &[], string, None);
    b.set_resource_owner(read, scope);
    let writer = b.resource("childWrite", "write", &[], string, None);
    b.set_resource_owner(writer, scope);
    let data = Data::default();
    data.ready.set(true);
    let r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    assert_eq!(r.resource("rootRead"), Some(&Value::str("new")));
    let snapshots = r.owned_resource_snapshots();
    assert_eq!(snapshots[0].value, Value::str("new"));
    assert!(snapshots[0].reader);
    assert!(!snapshots[1].reader);
    assert_eq!(r.store().get("flag"), Some("new"));
}

#[test]
fn store_fixpoint_can_create_and_destroy_a_scope_in_the_same_batch() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let boolean = b.primitive(TypeKind::Bool);
    let string = b.primitive(TypeKind::String);
    let closed = b.resource("closed", "closed", &[], boolean, None);
    let root = root(&mut b);
    let mut subject = Asm::new();
    subject.load_resource(closed).simple(Opcode::Not);
    let subject = b.code(subject);
    let unit = b.constant(&Value::Unit);
    let (_, arms) = b.region(RegionKind::When, Some(root), None, 0, subject, unit, 2);
    let unit = b.constant(&Value::Unit);
    let (scope, arms) = b.region(RegionKind::Scope, None, Some(arms[0]), 0, unit, unit, 1);
    let writer = b.resource("childWrite", "write", &[], string, None);
    b.set_resource_owner(writer, scope);
    b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None);
    let data = Data::default();
    data.ready.set(true);
    let r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    assert_eq!(r.resource("closed"), Some(&Value::Bool(true)));
    assert!(r.owned_resource_snapshots().is_empty());
    assert_eq!(r.kernel().live_count(), 1);
}

#[test]
fn data_ready_launches_every_mounted_pending_seed_without_root_resources() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let root = root(&mut b);
    for i in 0..2 {
        let (scope, _) = scope(&mut b, root, i);
        let res = b.resource(&format!("child{i}"), "pending", &[], string, None);
        b.set_resource_owner(res, scope);
        b.resource_boot(
            res,
            &Value::list(vec![]),
            &Value::list(vec![]),
            &Value::str(&format!("seed{i}")),
            false,
            true,
        );
    }
    let data = Data::default();
    let ready = data.ready.clone();
    let asks = data.asks.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    assert!(asks.borrow().is_empty());
    assert!(r.take_requests().is_empty());
    assert!(r.data_ready().unwrap().is_none());
    ready.set(true);
    assert!(r.data_ready().unwrap().is_some());
    assert_eq!(r.take_requests().len(), 2);
    assert_eq!(
        r.owned_resource_snapshots()
            .iter()
            .map(|s| s.value.clone())
            .collect::<Vec<_>>(),
        [Value::str("seed0"), Value::str("seed1")]
    );
    assert!(r.owned_resource_snapshots().iter().all(|s| s.pending));
    assert!(r.data_ready().unwrap().is_none());
}

#[test]
fn prewalk_failure_restores_owned_writes_pending_and_undrained_requests() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let boolean = b.primitive(TypeKind::Bool);
    let string = b.primitive(TypeKind::String);
    let number = b.primitive(TypeKind::Number);
    let option = b.option(string);
    let no = b.constant(&Value::Bool(false));
    let flag = b.slot("flag", boolean, no);
    let mut arg = Asm::new();
    arg.load_slot(flag);
    let arg = b.code(arg);
    b.resource("guard", "guard", &[arg], string, None);
    let root = root(&mut b);
    let (scope, arm) = scope(&mut b, root, 0);
    let zero = b.constant(&Value::Number(0.0));
    let count = b.slot("count", number, zero);
    b.set_slot_owner(count, scope);
    let none = b.constant(&Value::NONE);
    let result = b.slot("result", option, none);
    b.set_slot_owner(result, scope);
    let mutation = b.mutation("save", result, string);
    let source = b.str("pending");
    let mut send = Asm::new();
    send.send(mutation, source, 0);
    let send = b.code(send);
    let send = b.action("send", &[], &[result], send);
    let mut bad = Asm::new();
    bad.number(9.0)
        .store_slot(count)
        .bool(true)
        .store_slot(flag)
        .send(mutation, source, 0);
    let bad = b.code(bad);
    let bad = b.action("fail", &[], &[result, count, flag], bad);
    b.node(
        NodeType::View as u8,
        None,
        Some(arm),
        0,
        &[],
        &[(EventKind::Press, send, &[]), (EventKind::Focus, bad, &[])],
        None,
    );
    let data = Data::default();
    data.ready.set(true);
    let begun = data.begun.clone();
    let canceled = data.canceled.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    // The root is allocated first, then the scoped view.
    r.dispatch(2, Event::Press).unwrap();
    let pending = r.pending();
    let original_context = *begun.borrow().last().unwrap();
    let before = exact_runner::agent::state(&r);
    assert!(r.dispatch(2, Event::Focus).is_err());
    assert!(!r.is_poisoned());
    assert_eq!(r.pending(), pending);
    assert_eq!(
        r.take_requests()
            .iter()
            .map(|r| r.ticket)
            .collect::<Vec<_>>(),
        [pending[0].1]
    );
    assert_eq!(exact_runner::agent::state(&r), before);
    assert!(
        !canceled.borrow().contains(&original_context),
        "a refused replacement must preserve the old executor call"
    );
    assert!(begun
        .borrow()
        .iter()
        .filter(|id| **id != original_context)
        .all(|id| canceled.borrow().contains(id)));
}

#[test]
fn a_late_unsnapshotted_scope_defers_its_source_until_ready() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let boolean = b.primitive(TypeKind::Bool);
    let string = b.primitive(TypeKind::String);
    let no = b.constant(&Value::Bool(false));
    let shown = b.slot("shown", boolean, no);
    let mut show = Asm::new();
    show.bool(true).store_slot(shown);
    let show = b.code(show);
    b.action("show", &[], &[shown], show);
    let root = root(&mut b);
    let unit = b.constant(&Value::Unit);
    let mut subject = Asm::new();
    subject.load_slot(shown);
    let subject = b.code(subject);
    let (_, arms) = b.region(RegionKind::When, Some(root), None, 0, subject, unit, 2);
    let (scope, _) = b.region(RegionKind::Scope, None, Some(arms[0]), 0, unit, unit, 1);
    let res = b.resource("late", "pending", &[], string, None);
    b.set_resource_owner(res, scope);
    let fallback = b.constant(&Value::str("loading"));
    b.set_resource_fallback(res, fallback);
    let data = Data::default();
    let ready = data.ready.clone();
    let asks = data.asks.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    r.act("show", vec![]).unwrap();
    assert!(asks.borrow().is_empty());
    assert!(r.take_requests().is_empty());
    assert!(!r.is_poisoned());
    assert_eq!(r.owned_resource_snapshots()[0].value, Value::str("loading"));
    assert!(r.owned_resource_snapshots()[0].pending);
    ready.set(true);
    assert!(r.data_ready().unwrap().is_some());
    assert_eq!(&*asks.borrow(), &["pending"]);
    assert_eq!(r.take_requests().len(), 1);
}

#[test]
fn a_continuation_that_writes_store_keeps_its_next_request() {
    struct Continuation;
    impl DataSource for Continuation {
        fn grants(&self) -> &str {
            "secret.keep flag"
        }
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!()
        }
        fn answer(
            &mut self,
            store: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            match source {
                "read" => Ok(Answer::Now(Value::str(store.get("flag").unwrap_or("old")))),
                "pending" => Ok(Answer::Later(Request::get("https://example.test/first"))),
                _ => unreachable!(),
            }
        }
        fn parse(
            &mut self,
            store: &mut Store,
            _: &str,
            _: &[Value],
            _: exact_runner::Outcome,
        ) -> Result<Answer, DataError> {
            let _ = store.get("flag");
            store.set("flag", "new")?;
            Ok(Answer::Later(Request::get("https://example.test/next")))
        }
    }
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    b.resource("reader", "read", &[], string, None);
    let root = root(&mut b);
    let (scope, _) = scope(&mut b, root, 0);
    let res = b.resource("pending", "pending", &[], string, None);
    b.set_resource_owner(res, scope);
    let fallback = b.constant(&Value::str("loading"));
    b.set_resource_fallback(res, fallback);
    let mut r = Runner::boot(b.finish().unwrap(), Continuation, Kernel::with_monospace()).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(
        ticket,
        exact_runner::Outcome::Response(exact_runner::Response {
            status: 200,
            headers: vec![],
            body: vec![],
        }),
    )
    .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.url, "https://example.test/next");
    assert_eq!(r.resource("reader"), Some(&Value::str("new")));
    assert!(r.owned_resource_snapshots()[0].reader);
}

#[test]
fn a_root_scope_chain_still_has_exactly_one_visual_root() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let unit = b.constant(&Value::Unit);
    let (_, arms) = b.region(RegionKind::Scope, None, None, 0, unit, unit, 1);
    let (_, arms) = b.region(RegionKind::Scope, None, Some(arms[0]), 0, unit, unit, 1);
    b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None);
    let r = Runner::boot(
        b.finish().unwrap(),
        Data::default(),
        Kernel::with_monospace(),
    )
    .unwrap();
    assert_eq!(r.roots(), [1]);
    assert_eq!(r.kernel().live_count(), 1);
    assert!(r.journal().any(|line| line.contains("scope 1 mounted")));
    assert!(r.journal().any(|line| line.contains("scope 2 mounted")));
}

#[test]
fn equal_argument_calls_use_distinct_contexts_and_chain_only_the_originating_call() {
    #[derive(Default)]
    struct ContextData {
        live: Rc<RefCell<std::collections::BTreeSet<u64>>>,
        parsed: Rc<RefCell<Vec<u64>>>,
    }
    impl DataSource for ContextData {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("unqualified call")
        }
        fn answer_scoped(
            &mut self,
            context: u64,
            _: &mut Store,
            _: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            assert!(self.live.borrow_mut().insert(context));
            Ok(Answer::Later(Request::get("https://example.test/same")))
        }
        fn parse_scoped(
            &mut self,
            context: u64,
            _: &mut Store,
            _: &str,
            _: &[Value],
            outcome: exact_runner::Outcome,
        ) -> Result<Answer, DataError> {
            assert!(self.live.borrow().contains(&context));
            self.parsed.borrow_mut().push(context);
            if matches!(outcome, exact_runner::Outcome::Response(ref r) if r.body == b"again") {
                Ok(Answer::Later(Request::get("https://example.test/next")))
            } else {
                Ok(Answer::Now(Value::str(&format!("call{context}"))))
            }
        }
        fn cancel_scoped(&mut self, context: u64) {
            assert!(self.live.borrow_mut().remove(&context));
        }
    }
    fn reply(r: &mut Runner<ContextData>, ticket: u64, body: &[u8]) {
        r.fulfill(
            ticket,
            exact_runner::Outcome::Response(exact_runner::Response {
                status: 200,
                headers: vec![],
                body: body.into(),
            }),
        )
        .unwrap();
    }
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let boolean = b.primitive(TypeKind::Bool);
    let string = b.primitive(TypeKind::String);
    let yes = b.constant(&Value::Bool(true));
    let shown = b.slot("shown", boolean, yes);
    let mut hide = Asm::new();
    hide.bool(false).store_slot(shown);
    let hide = b.code(hide);
    b.action("hide", &[], &[shown], hide);
    let root = root(&mut b);
    let unit = b.constant(&Value::Unit);
    let mut subject = Asm::new();
    subject.load_slot(shown);
    let subject = b.code(subject);
    let (_, outer) = b.region(RegionKind::When, Some(root), None, 0, subject, unit, 2);
    for i in 0..2 {
        let (scope, arms) = b.region(RegionKind::Scope, None, Some(outer[0]), i, unit, unit, 1);
        let res = b.resource(&format!("r{i}"), "same", &[], string, None);
        b.set_resource_owner(res, scope);
        let fallback = b.constant(&Value::str("loading"));
        b.set_resource_fallback(res, fallback);
        let mut refresh = Asm::new();
        refresh.refresh(res);
        let refresh = b.code(refresh);
        let refresh = b.action(&format!("refresh{i}"), &[], &[], refresh);
        b.node(
            NodeType::View as u8,
            None,
            Some(arms[0]),
            0,
            &[],
            &[(EventKind::Press, refresh, &[])],
            None,
        );
    }
    let data = ContextData::default();
    let live = data.live.clone();
    let parsed = data.parsed.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    assert_eq!(live.borrow().iter().copied().collect::<Vec<_>>(), [1, 2]);
    let requests = r.take_requests();
    reply(&mut r, requests[1].ticket, b"done");
    assert_eq!(r.owned_resource_snapshots()[1].value, Value::str("call2"));
    reply(&mut r, requests[0].ticket, b"again");
    let next = r.take_requests();
    assert_eq!(next.len(), 1);
    assert_eq!(&*parsed.borrow(), &[2, 1]);
    assert_eq!(live.borrow().iter().copied().collect::<Vec<_>>(), [1]);
    r.dispatch(2, Event::Press).unwrap();
    let replacement = r.take_requests();
    assert_eq!(replacement.len(), 1);
    assert_eq!(live.borrow().iter().copied().collect::<Vec<_>>(), [3]);
    reply(&mut r, next[0].ticket, b"obsolete");
    assert_eq!(&*parsed.borrow(), &[2, 1]);
    r.act("hide", vec![]).unwrap();
    assert!(live.borrow().is_empty());
    reply(&mut r, replacement[0].ticket, b"unmounted");
    assert_eq!(&*parsed.borrow(), &[2, 1]);
}

#[test]
fn send_then_assign_dispatches_without_reply_interest_and_still_obeys_scope_teardown() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let boolean = b.primitive(TypeKind::Bool);
    let string = b.primitive(TypeKind::String);
    let option = b.option(string);
    let yes = b.constant(&Value::Bool(true));
    let shown = b.slot("shown", boolean, yes);
    let mut hide = Asm::new();
    hide.bool(false).store_slot(shown);
    let hide = b.code(hide);
    b.action("hide", &[], &[shown], hide);
    let root = root(&mut b);
    let unit = b.constant(&Value::Unit);
    let mut subject = Asm::new();
    subject.load_slot(shown);
    let subject = b.code(subject);
    let (_, arms) = b.region(RegionKind::When, Some(root), None, 0, subject, unit, 2);
    let (scope, arms) = b.region(RegionKind::Scope, None, Some(arms[0]), 0, unit, unit, 1);
    let none = b.constant(&Value::NONE);
    let result = b.slot("result", option, none);
    b.set_slot_owner(result, scope);
    let mutation = b.mutation("logout", result, string);
    let source = b.str("pending");
    let mut logout = Asm::new();
    logout
        .send(mutation, source, 0)
        .simple(Opcode::None)
        .store_slot(result);
    let logout = b.code(logout);
    let logout = b.action("logout", &[], &[result], logout);
    b.node(
        NodeType::View as u8,
        None,
        Some(arms[0]),
        0,
        &[],
        &[(EventKind::Press, logout, &[])],
        None,
    );
    let data = Data::default();
    data.ready.set(true);
    let canceled = data.canceled.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    r.dispatch(2, Event::Press).unwrap();
    assert!(r.pending().is_empty());
    assert!(!r.has_pending());
    assert!(
        canceled.borrow().is_empty(),
        "executor work must survive until dispatch"
    );
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    // Data has no parse implementation: calling it would refuse this reply.
    assert!(r
        .fulfill(
            requests[0].ticket,
            exact_runner::Outcome::Response(exact_runner::Response {
                status: 200,
                headers: vec![],
                body: vec![]
            })
        )
        .unwrap()
        .is_none());
    assert_eq!(&*canceled.borrow(), &[1]);
    r.dispatch(2, Event::Press).unwrap();
    r.act("hide", vec![]).unwrap();
    assert!(
        r.take_requests().is_empty(),
        "teardown also prunes sends without reply interest"
    );
    assert_eq!(&*canceled.borrow(), &[1, 2]);
}

#[test]
fn changed_unready_args_recompute_pending_fallback_but_preserve_a_settled_answer() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let a = b.constant(&Value::str("a"));
    let id = b.slot("id", string, a);
    let mut choose = Asm::new();
    choose.load_param(0).store_slot(id);
    let choose = b.code(choose);
    b.action("choose", &[("id", string)], &[id], choose);
    let root = root(&mut b);
    let (scope, _) = scope(&mut b, root, 0);
    let mut arg = Asm::new();
    arg.load_slot(id);
    let arg = b.code(arg);
    let res = b.resource("value", "guard", &[arg], string, None);
    b.set_resource_owner(res, scope);
    let prefix = b.str("loading-");
    let mut fallback = Asm::new();
    fallback.str(prefix).load_slot(id).simple(Opcode::Concat);
    let fallback = b.code(fallback);
    b.set_resource_fallback(res, fallback);
    b.resource_boot(
        res,
        &Value::list(vec![]),
        &Value::list(vec![Value::str("a")]),
        &Value::str("loading-a"),
        false,
        true,
    );
    let data = Data::default();
    let ready = data.ready.clone();
    let asks = data.asks.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    assert_eq!(
        r.owned_resource_snapshots()[0].value,
        Value::str("loading-a")
    );
    r.act("choose", vec![Value::str("b")]).unwrap();
    assert_eq!(
        r.owned_resource_snapshots()[0].value,
        Value::str("loading-b")
    );
    assert!(asks.borrow().is_empty());
    assert!(r.take_requests().is_empty());
    ready.set(true);
    r.data_ready().unwrap();
    assert_eq!(r.owned_resource_snapshots()[0].value, Value::str("ok"));
    assert_eq!(asks.borrow().len(), 1);
    ready.set(false);
    r.act("choose", vec![Value::str("c")]).unwrap();
    assert_eq!(r.owned_resource_snapshots()[0].value, Value::str("ok"));
    assert_eq!(
        asks.borrow().len(),
        1,
        "unready argument changes must not invoke the source"
    );
}

#[test]
fn a_root_forward_fallback_retries_without_starting_another_call() {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let pending = b.resource("value", "pending", &[], string, None);
    let base = b.resource("base", "guard", &[], string, None);
    let mut fallback = Asm::new();
    fallback.load_resource(base);
    let fallback = b.code(fallback);
    b.set_resource_fallback(pending, fallback);
    let unit = b.constant(&Value::Unit);
    b.action("noop", &[], &[], unit);
    root(&mut b);
    let data = Data::default();
    data.ready.set(true);
    let asks = data.asks.clone();
    let begun = data.begun.clone();
    let mut r = Runner::boot(b.finish().unwrap(), data, Kernel::with_monospace()).unwrap();
    assert_eq!(r.resource("value"), Some(&Value::str("ok")));
    assert_eq!(&*asks.borrow(), &["pending", "guard"]);
    assert_eq!(&*begun.borrow(), &[1, 2]);
    assert_eq!(r.take_requests().len(), 1);
    r.act("noop", vec![]).unwrap();
    assert_eq!(&*asks.borrow(), &["pending", "guard"]);
    assert!(r.take_requests().is_empty());
}
