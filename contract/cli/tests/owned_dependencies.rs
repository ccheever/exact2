//! Owned fallback dependencies and host delivery facts use the live scope.
use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Delivery, Outcome, Request, Response, Runner, Store,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn text<D: DataSource>(runner: &Runner<D>, id: &str) -> String {
    let keys = runner.kernel().find_by_test_id(id);
    assert_eq!(keys.len(), 1, "expected one text node for {id}");
    runner
        .kernel()
        .node_by_key(keys[0])
        .unwrap()
        .props
        .iter()
        .find_map(|(prop, value)| match value {
            PropValue::Str(value) if prop.name() == "text" => Some(value.clone()),
            _ => None,
        })
        .unwrap()
}

#[derive(Clone, Default)]
struct Dependencies {
    calls: Rc<RefCell<Vec<String>>>,
    parses: Rc<Cell<usize>>,
}

impl DataSource for Dependencies {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        assert_eq!(args, &[Value::str("child")]);
        self.calls.borrow_mut().push(source.into());
        match source {
            "base" => Ok(Answer::Now(Value::str("seed-child"))),
            "later" => Ok(Answer::Later(Request::get("https://example.test/child"))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        assert_eq!(source, "later");
        assert_eq!(args, &[Value::str("child")]);
        self.parses.set(self.parses.get() + 1);
        match outcome {
            Outcome::Response(response) => Ok(Answer::Now(Value::str(
                &String::from_utf8(response.body).unwrap(),
            ))),
            _ => Err(DataError::Unavailable("expected a successful reply".into())),
        }
    }
}

fn assert_called_once_each(calls: &RefCell<Vec<String>>) {
    let mut calls = calls.borrow().clone();
    calls.sort();
    assert_eq!(
        calls,
        ["base", "later"],
        "dependency retries must not requery"
    );
}

fn fallback_dependency(forward: bool, late_mount: bool) {
    let declarations = if forward {
        "  resource value = later(id) as shape string else base\n  resource base = base(id) as shape string\n"
    } else {
        "  resource base = base(id) as shape string\n  resource value = later(id) as shape string else base\n"
    };
    let source = format!(
        r#"component App
  state shown = {}
  action show writes shown
    shown = true
  action noop
  view
    column
      when shown
        Child(id="child")
component Child
  props
    id: string
{}  view
    column
      text value testId="value"
      text `${{pending(value)}}` testId="pending"
"#,
        !late_mount, declarations
    );
    let plan = contract::compile(&source).unwrap();
    let data = Dependencies::default();
    let calls = data.calls.clone();
    let parses = data.parses.clone();
    let mut runner = Runner::boot(plan, data, Kernel::with_monospace()).unwrap();
    if late_mount {
        assert!(calls.borrow().is_empty(), "unmounted children cannot query");
        assert!(runner.take_requests().is_empty());
        runner.act("show", vec![]).unwrap();
    }
    assert_eq!(text(&runner, "value"), "seed-child");
    assert_eq!(text(&runner, "pending"), "true");
    assert_called_once_each(&calls);
    let requests = runner.take_requests();
    assert_eq!(
        requests.len(),
        1,
        "one asynchronous answer yields one request"
    );
    assert_eq!(requests[0].request.url, "https://example.test/child");

    runner.act("noop", vec![]).unwrap();
    assert_called_once_each(&calls);
    assert!(runner.take_requests().is_empty());
    assert_eq!(text(&runner, "value"), "seed-child");
    assert_eq!(text(&runner, "pending"), "true");

    runner
        .fulfill(
            requests[0].ticket,
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: b"answer-child".to_vec(),
            }),
        )
        .unwrap();
    assert_eq!(text(&runner, "value"), "answer-child");
    assert_eq!(text(&runner, "pending"), "false");
    assert_eq!(parses.get(), 1);
    assert_called_once_each(&calls);
    assert!(runner.take_requests().is_empty());
}

#[test]
fn forward_fallback_dependency_at_boot_queries_each_source_once() {
    fallback_dependency(true, false);
}

#[test]
fn reverse_fallback_dependency_at_boot_queries_each_source_once() {
    fallback_dependency(false, false);
}

#[test]
fn forward_fallback_dependency_on_late_mount_queries_each_source_once() {
    fallback_dependency(true, true);
}

#[test]
fn reverse_fallback_dependency_on_late_mount_queries_each_source_once() {
    fallback_dependency(false, true);
}

struct AppSource {
    ready: bool,
    calls: Rc<Cell<usize>>,
}

impl DataSource for AppSource {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        self.calls.set(self.calls.get() + 1);
        Err(DataError::UnknownSource(source.into()))
    }

    fn ready(&self) -> bool {
        self.ready
    }
}

const DELIVERY: &str = r#"shape Facts
  seq: number
  staged: bool
component App
  resource rootFacts = exactDelivery() as shape Facts
  view
    column
      text `${rootFacts.seq}:${rootFacts.staged}` testId="root"
      Child()
component Child
  resource facts = exactDelivery() as shape Facts
  view
    text `${facts.seq}:${facts.staged}` testId="child"
"#;

fn delivery_facts(baked: bool, ready: bool) {
    let calls = Rc::new(Cell::new(0));
    let mut plan = contract::compile(DELIVERY).unwrap();
    if baked {
        // Bake sees embedded seq=0; the host below starts at seq=7.
        plan = contract::bake(
            plan,
            AppSource {
                ready: true,
                calls: calls.clone(),
            },
        )
        .unwrap();
        assert_eq!(calls.get(), 0, "bake must answer the built-in itself");
    }
    let delivery = Delivery {
        seq: 7,
        ..Default::default()
    };
    let mut runner = Runner::boot_with_delivery(
        plan,
        AppSource {
            ready,
            calls: calls.clone(),
        },
        Kernel::with_monospace(),
        None,
        vec![],
        delivery,
    )
    .unwrap();
    assert_eq!(text(&runner, "root"), "7:false");
    assert_eq!(text(&runner, "child"), "7:false");
    assert_eq!(calls.get(), 0, "host facts must bypass the app data source");
    assert!(runner.take_requests().is_empty());

    let next = Delivery {
        seq: 9,
        staged: true,
        ..Default::default()
    };
    assert!(runner.set_delivery(next.clone()).unwrap().is_some());
    assert_eq!(runner.delivery(), &next);
    assert_eq!(text(&runner, "root"), "9:true");
    assert_eq!(text(&runner, "child"), "9:true");
    assert!(runner.set_delivery(next).unwrap().is_none());
    assert_eq!(calls.get(), 0);
    assert!(runner.take_requests().is_empty());
}

#[test]
fn child_delivery_uses_host_facts_at_boot_and_on_update() {
    delivery_facts(false, true);
}

#[test]
fn child_delivery_works_while_the_app_source_is_unready() {
    delivery_facts(false, false);
}

#[test]
fn child_delivery_uses_host_facts_after_bake_and_on_update() {
    delivery_facts(true, true);
}

#[test]
fn baked_child_delivery_works_while_the_app_source_is_unready() {
    delivery_facts(true, false);
}
