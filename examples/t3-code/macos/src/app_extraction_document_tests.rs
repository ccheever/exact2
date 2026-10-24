//! Ordered inline-document transport through mounted production Runner handlers.
//! Both explicit artifact paths also exercise the independently compiled original graph.
//! The fixture has no external executor. Resize and iframe events are synthesized here,
//! so this witnesses authored routing and queue settlement, not native bridge interception.

use super::composer_accessibility_tests::set;
use super::contract_keys_tests::empty;
use exact_kernel::Kernel;
use exact_plan::{EventKind, Plan, TypesId, Value};
use exact_runner::{
    Answer, DataError, DataSource, Event, Outcome, Request, ResizeRect, Response, Runner, Store,
    Viewport,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
enum Layout {
    Original,
    Extracted,
}

fn plans() -> impl Iterator<Item = (&'static [u8], Layout)> {
    crate::contract_keys_tests::extraction_plans().map(|(bytes, original)| {
        (
            bytes,
            if original {
                Layout::Original
            } else {
                Layout::Extracted
            },
        )
    })
}
const EPOCH: f64 = 1_791_446_400_000.0;
const DOCUMENT: &str = "document-owner";
const FRAME: &str = "inline-frame-document-owner";

#[derive(Debug, PartialEq)]
struct Call {
    method: &'static str,
    source: String,
    args: Vec<Value>,
}

struct Source {
    ready: bool,
    values: HashMap<String, Value>,
    change: Value,
    calls: Vec<Call>,
    commands: Vec<Vec<Value>>,
    parsed: Vec<Vec<Value>>,
}

impl Source {
    fn value(&self, source: &str) -> Result<Value, DataError> {
        self.values
            .get(source)
            .cloned()
            .ok_or_else(|| DataError::UnknownSource(source.to_owned()))
    }

    fn record(&mut self, method: &'static str, source: &str, args: &[Value]) {
        self.calls.push(Call {
            method,
            source: source.to_owned(),
            args: args.to_vec(),
        });
    }
}

impl DataSource for Source {
    fn ready(&self) -> bool {
        self.ready
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.record("query", source, args);
        self.value(source)
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        self.record("answer", source, args);
        if source == "command" {
            self.commands.push(args.to_vec());
            Ok(Answer::Later(Request::get("https://fixture.invalid/")))
        } else {
            self.value(source).map(Answer::Now)
        }
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.record("parse", source, args);
        assert_eq!(outcome, response());
        assert_eq!(source, "command");
        self.parsed.push(args.to_vec());
        Ok(Answer::Now(self.change.clone()))
    }
}

fn element(plan: &Plan, mut ty: TypesId, path: &[&str]) -> TypesId {
    for name in path {
        let index = plan.field_index(ty, name).unwrap();
        ty = plan
            .field(plan.type_(ty).fields.iter().nth(index).unwrap())
            .ty;
    }
    plan.type_(ty).elem.expect("an authored list field")
}

fn boot(bytes: &[u8]) -> Runner<Source> {
    let plan = Plan::decode(bytes).expect("a complete compiled app plan");
    let document_mutation = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "inlineDocumentChanged")
        .unwrap();
    assert!(document_mutation.queue);
    assert!(
        !plan
            .mutations
            .iter()
            .find(|row| plan.str(row.name) == "localChanged")
            .unwrap()
            .queue
    );
    let mut change = empty(&plan, document_mutation.ty);
    set(
        &plan,
        document_mutation.ty,
        &mut change,
        &["revision"],
        Value::Number(1.0),
    );
    assert!(change.conforms(&plan, document_mutation.ty));

    // Typed defaults answer the complete graph without loading its baked data module.
    let mut values = HashMap::new();
    for row in &plan.resources {
        let source = plan.str(row.source);
        if !exact_plan::runner_owned_source(source) {
            let value = empty(&plan, row.ty);
            if let Some(previous) = values.insert(source.to_owned(), value.clone()) {
                assert_eq!(
                    previous, value,
                    "compatible defaults for shared source {source}"
                );
            }
        }
    }
    let data = plan
        .resources
        .iter()
        .find(|row| plan.str(row.name) == "data")
        .unwrap();
    let message_ty = element(&plan, data.ty, &["messages"]);
    let document_ty = element(&plan, message_ty, &["inlineDocuments"]);
    let post_ty = element(&plan, document_ty, &["posts"]);
    let mut post = empty(&plan, post_ty);
    set(&plan, post_ty, &mut post, &["key"], Value::str("reply-1"));
    set(
        &plan,
        post_ty,
        &mut post,
        &["text"],
        Value::str("reply-1-body"),
    );
    let mut document = empty(&plan, document_ty);
    for (field, value) in [
        ("key", Value::str(DOCUMENT)),
        ("kind", Value::str("html")),
        ("title", Value::str("Fixture document")),
        ("src", Value::str("https://fixture.invalid/document")),
        ("ready", Value::Bool(true)),
        ("height", Value::Number(340.0)),
        ("posts", Value::list(vec![post])),
        (
            "permission",
            Value::str("Allow access?\nFixture permission"),
        ),
        ("permissionSerial", Value::Number(1.0)),
    ] {
        set(&plan, document_ty, &mut document, &[field], value);
    }
    let mut message = empty(&plan, message_ty);
    for (field, value) in [
        ("id", Value::str("fixture-message")),
        ("kind", Value::str("inline-document")),
        ("first", Value::Bool(true)),
        ("last", Value::Bool(true)),
        ("inlineDocuments", Value::list(vec![document])),
    ] {
        set(&plan, message_ty, &mut message, &[field], value);
    }
    let mut snapshot = empty(&plan, data.ty);
    for (path, value) in [
        (&["revision"][..], Value::Number(1.0)),
        (&["connected"][..], Value::Bool(true)),
        (&["threadId"][..], Value::str("fixture-thread")),
        (&["snapshotOwner"][..], Value::str("fixture-snapshot")),
        (&["look", "chatMax"][..], Value::Number(728.0)),
        (&["messages"][..], Value::list(vec![message])),
    ] {
        set(&plan, data.ty, &mut snapshot, path, value);
    }
    assert!(snapshot.conforms(&plan, data.ty));
    values.insert(plan.str(data.source).to_owned(), snapshot.clone());
    let source = Source {
        ready: false,
        values,
        change,
        calls: Vec::new(),
        commands: Vec::new(),
        parsed: Vec::new(),
    };
    let mut runner = Runner::boot_stored(
        plan,
        source,
        Kernel::with_monospace(),
        Vec::new(),
        Viewport {
            width: 1280.0,
            height: 840.0,
            ..Default::default()
        },
        "/",
    )
    .unwrap();
    runner.set_time(EPOCH, 0.0).unwrap();
    runner.data().ready = true;
    runner.data_ready().unwrap();
    assert_eq!(runner.resource("data"), Some(&snapshot));
    assert_eq!(runner.now_ms(), 0.0);
    assert!(
        runner.pending().is_empty(),
        "all setup resource requests are answered"
    );
    assert!(runner.queued().is_empty());
    assert!(
        runner.take_requests().is_empty(),
        "setup emitted no external requests"
    );
    node_id(&runner, FRAME);
    node_id(&runner, "inline-permission-document-owner-1-confirm");
    node_test(&runner, "inline-doc-document-owner");
    let data_args = runner.resource_args("data").unwrap();
    assert_eq!(data_args[0], Value::Number(EPOCH));
    // No Host resize rounds run: the post is still unsent until its explicit event.
    runner.take_commands();
    runner.data().calls.clear();
    assert!(runner.data_ref().commands.is_empty());
    runner
}

fn node_id(runner: &Runner<Source>, id: &str) -> u32 {
    let keys = runner.kernel().find_by_id(id);
    assert_eq!(keys.len(), 1, "one mounted {id}");
    runner.kernel().node_by_key(keys[0]).unwrap().id
}

fn node_test(runner: &Runner<Source>, id: &str) -> u32 {
    let keys = runner.kernel().find_by_test_id(id);
    assert_eq!(keys.len(), 1, "one mounted {id}");
    runner.kernel().node_by_key(keys[0]).unwrap().id
}

fn measure_node(runner: &Runner<Source>) -> u32 {
    let boundary = node_id(runner, "inline-box-document-owner");
    let mut parent = runner.kernel().node(node_id(runner, FRAME)).unwrap().parent;
    while let Some(view) = parent {
        assert_ne!(
            view, boundary,
            "the content scroll must precede the outer resize handler"
        );
        if runner.handlers_of(view).contains(&EventKind::Resize) {
            return view;
        }
        parent = runner.kernel().node(view).unwrap().parent;
    }
    panic!("the iframe's content scroll is mounted")
}

fn post_node(runner: &Runner<Source>) -> u32 {
    let outer = node_test(runner, "inline-doc-document-owner");
    let container = node_id(runner, "inline-box-document-owner");
    let nodes: Vec<_> = runner
        .kernel()
        .node(outer)
        .unwrap()
        .children()
        .into_iter()
        .filter(|view| *view != container)
        .filter(|view| runner.handlers_of(*view).contains(&EventKind::Resize))
        .collect();
    assert_eq!(nodes.len(), 1, "one authored PostDelivery resize box");
    nodes[0]
}

fn call(method: &'static str, source: &str, args: &[Value]) -> Call {
    Call {
        method,
        source: source.to_owned(),
        args: args.to_vec(),
    }
}

fn calls(runner: &mut Runner<Source>, expected: Vec<Call>) {
    assert_eq!(std::mem::take(&mut runner.data().calls), expected);
}

fn hosts(runner: &mut Runner<Source>, expected: Vec<(String, Vec<Value>)>) {
    let actual: Vec<_> = runner
        .take_commands()
        .into_iter()
        .map(|command| (command.name, command.args))
        .collect();
    assert_eq!(actual, expected);
}

fn request(runner: &mut Runner<Source>, target: &str) -> u64 {
    let requests = runner.take_requests();
    assert_eq!(requests.len(), 1, "the complete outgoing request vector");
    let request = &requests[0];
    assert_eq!(request.target, target);
    assert_eq!(request.request, Request::get("https://fixture.invalid/"));
    assert!(!request.forced);
    request.ticket
}

fn queued_event(
    runner: &mut Runner<Source>,
    view: u32,
    event: Event,
    count: usize,
    ticket: u64,
    expected_hosts: Vec<(String, Vec<Value>)>,
) {
    runner.dispatch(view, event).unwrap();
    calls(runner, vec![]);
    assert!(runner.take_requests().is_empty());
    hosts(runner, expected_hosts);
    assert_eq!(
        runner.queued(),
        vec![("inlineDocumentChanged".to_owned(), count)]
    );
    assert_eq!(
        runner.pending(),
        vec![("inlineDocumentChanged".to_owned(), ticket)]
    );
}

fn response() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: vec![],
    })
}

#[test]
fn extraction_preserves_document_order_refreshes_and_newest_local_control() {
    for (bytes, _) in plans() {
        let mut runner = boot(bytes);
        let snapshot_args = runner.resource_args("data").unwrap().to_vec();
        let snapshot_source = {
            let plan = runner.plan();
            let row = plan
                .resources
                .iter()
                .find(|row| plan.str(row.name) == "data")
                .unwrap();
            plan.str(row.source).to_owned()
        };
        let payloads: Vec<Vec<Value>> = [
            ("message", "initialize"),
            ("measure", "728|340"),
            ("message", "adapter-ready"),
            ("ack", "reply-1"),
            ("permission", "1:allow"),
        ]
        .map(|(op, value)| {
            vec![
                Value::str(&format!("chatlocal:inline-doc-{op}")),
                Value::str(DOCUMENT),
                Value::str(value),
                Value::Number(0.0),
            ]
        })
        .to_vec();
        let frame = node_id(&runner, FRAME);
        assert!(runner.handlers_of(frame).contains(&EventKind::Message));
        runner
            .dispatch(frame, Event::Message("initialize".to_owned()))
            .unwrap();
        calls(&mut runner, vec![call("answer", "command", &payloads[0])]);
        let mut ticket = request(&mut runner, "inlineDocumentChanged");
        hosts(&mut runner, vec![]);
        assert!(runner.queued().is_empty());
        assert_eq!(
            runner.pending(),
            vec![("inlineDocumentChanged".to_owned(), ticket)]
        );

        let scroll = measure_node(&runner);
        queued_event(
            &mut runner,
            scroll,
            Event::Resize(ResizeRect {
                x: 0.0,
                y: 0.0,
                width: 728.0,
                height: 340.0,
            }),
            1,
            ticket,
            vec![],
        );
        queued_event(
            &mut runner,
            frame,
            Event::Message("adapter-ready".to_owned()),
            2,
            ticket,
            vec![],
        );
        let post = post_node(&runner);
        // A zero-size observation does not acknowledge or consume this post.
        queued_event(
            &mut runner,
            post,
            Event::Resize(ResizeRect::default()),
            2,
            ticket,
            vec![],
        );
        queued_event(
            &mut runner,
            post,
            Event::Resize(ResizeRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
            3,
            ticket,
            vec![(
                "postMessage".to_owned(),
                vec![
                    Value::str("reply-1-body"),
                    Value::str(FRAME),
                    Value::str("iframe"),
                ],
            )],
        );
        // The same positive delivery observes the actual authored sent guard.
        queued_event(
            &mut runner,
            post,
            Event::Resize(ResizeRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
            3,
            ticket,
            vec![],
        );
        let confirm = node_id(&runner, "inline-permission-document-owner-1-confirm");
        queued_event(&mut runner, confirm, Event::Press, 4, ticket, vec![]);
        assert_eq!(runner.data_ref().commands, vec![payloads[0].clone()]);

        // Each real reply refreshes data, then one next commit asks the next send-time payload.
        for index in 0..payloads.len() {
            assert!(runner.fulfill(ticket, response()).unwrap().is_some());
            calls(
                &mut runner,
                vec![
                    call("parse", "command", &payloads[index]),
                    call("answer", &snapshot_source, &snapshot_args),
                ],
            );
            assert!(runner.take_requests().is_empty());
            hosts(&mut runner, vec![]);
            assert!(runner.pending().is_empty());
            let remaining = payloads.len() - index - 1;
            let expected_queue = if remaining == 0 {
                vec![]
            } else {
                vec![("inlineDocumentChanged".to_owned(), remaining)]
            };
            assert_eq!(runner.queued(), expected_queue);
            let advanced = runner.land_then();
            assert!(advanced.error.is_none(), "{advanced:?}");
            assert_eq!(advanced.now_ms, 0.0);
            hosts(&mut runner, vec![]);
            if index + 1 < payloads.len() {
                calls(
                    &mut runner,
                    vec![call("answer", "command", &payloads[index + 1])],
                );
                let next = request(&mut runner, "inlineDocumentChanged");
                assert_ne!(next, ticket);
                ticket = next;
                assert_eq!(
                    runner.pending(),
                    vec![("inlineDocumentChanged".to_owned(), ticket)]
                );
            } else {
                calls(&mut runner, vec![]);
                assert!(runner.take_requests().is_empty());
            }
        }
        assert_eq!(runner.data_ref().commands, payloads);
        assert_eq!(runner.data_ref().parsed, payloads);
        assert!(runner.pending().is_empty());
        assert!(runner.queued().is_empty());

        // This retained Root action is a separate authored-action control, not an iframe event.
        let local_args = |id: &str| {
            vec![
                Value::str("editorlocal:chip-close"),
                Value::str(id),
                Value::str(""),
                Value::Number(EPOCH),
            ]
        };
        let mut local_tickets = Vec::new();
        for id in ["first", "newest"] {
            runner
                .act(
                    "editorOp",
                    vec![Value::str("chip-close"), Value::str(id), Value::str("")],
                )
                .unwrap();
            calls(
                &mut runner,
                vec![call("answer", "command", &local_args(id))],
            );
            let ticket = request(&mut runner, "localChanged");
            local_tickets.push(ticket);
            hosts(&mut runner, vec![]);
            assert!(runner.queued().is_empty());
            assert_eq!(runner.pending(), vec![("localChanged".to_owned(), ticket)]);
        }
        assert_ne!(local_tickets[0], local_tickets[1]);
        // The replaced request cannot parse a stale reply or refresh data.
        assert!(runner
            .fulfill(local_tickets[0], response())
            .unwrap()
            .is_none());
        calls(&mut runner, vec![]);
        assert!(runner.take_requests().is_empty());
        hosts(&mut runner, vec![]);
        assert_eq!(
            runner.pending(),
            vec![("localChanged".to_owned(), local_tickets[1])]
        );
        assert!(runner
            .fulfill(local_tickets[1], response())
            .unwrap()
            .is_some());
        calls(
            &mut runner,
            vec![
                call("parse", "command", &local_args("newest")),
                call("answer", &snapshot_source, &snapshot_args),
            ],
        );
        assert!(runner.take_requests().is_empty());
        hosts(&mut runner, vec![]);
        let advanced = runner.land_then();
        assert!(advanced.error.is_none(), "{advanced:?}");
        assert_eq!(advanced.now_ms, 0.0);
        calls(&mut runner, vec![]);
        assert!(runner.take_requests().is_empty());
        hosts(&mut runner, vec![]);
        assert!(runner.pending().is_empty());
        assert!(runner.queued().is_empty());
        let mut all_commands = payloads.clone();
        all_commands.extend([local_args("first"), local_args("newest")]);
        assert_eq!(runner.data_ref().commands, all_commands);
        let mut parsed = payloads.clone();
        parsed.push(local_args("newest"));
        assert_eq!(runner.data_ref().parsed, parsed);
    }
}

// Guest commands use the authored injected Resize route, including beneath inert overlays.
// KeyboardDispatch rows are fixture inputs here; the TypeScript test owns key selection.
mod guest_shortcuts {
    use super::*;

    const KEY: &str = "shortcut-owner";

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Blocker {
        None,
        Ssh,
        Confirmation,
        ServerUpdate,
        UpdateEverything,
        ImagePreview,
        Permission,
        DeviceSetup,
        ComposerFile,
        Modal,
        Palette,
        Settings,
    }

    #[derive(Debug)]
    struct Setup {
        calls: Vec<Call>,
        hosts: Vec<(String, Vec<Value>)>,
    }

    fn overlay(
        plan: &Plan,
        values: &mut HashMap<String, Value>,
        name: &str,
        path: &[&str],
        value: Value,
    ) {
        let row = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap();
        let answer = values.get_mut(plan.str(row.source)).unwrap();
        set(plan, row.ty, answer, path, value);
        assert!(answer.conforms(plan, row.ty), "{name}");
    }

    fn resource_field(runner: &Runner<Source>, name: &str, path: &[&str]) -> Value {
        let plan = runner.plan();
        let row = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap();
        let mut ty = row.ty;
        let mut value = runner.resource(name).unwrap();
        for name in path {
            let index = plan.field_index(ty, name).unwrap();
            let Value::Record(fields) = value else {
                panic!("record field {name}")
            };
            value = &fields[index];
            ty = plan
                .field(plan.type_(ty).fields.iter().nth(index).unwrap())
                .ty;
        }
        value.clone()
    }

    fn projected(
        runner: &Runner<Source>,
        layout: Layout,
        flat: &str,
        record: &str,
        field: &str,
    ) -> Value {
        match layout {
            Layout::Original => runner.slot(flat).unwrap().clone(),
            Layout::Extracted => {
                let plan = runner.plan();
                let slot = plan
                    .slots
                    .iter()
                    .find(|row| plan.str(row.name) == record)
                    .unwrap();
                let Value::Record(fields) = runner.slot(record).unwrap() else {
                    panic!("the extracted {record}")
                };
                fields[plan.field_index(slot.ty, field).unwrap()].clone()
            }
        }
    }

    fn palette_open(runner: &Runner<Source>, layout: Layout) -> Value {
        projected(runner, layout, "paletteOpen", "paletteSession", "open")
    }

    fn delivery(runner: &Runner<Source>) -> u32 {
        let outer = node_test(runner, &format!("inline-doc-{KEY}"));
        let container = node_id(runner, &format!("inline-box-{KEY}"));
        let nodes: Vec<_> = runner
            .kernel()
            .node(outer)
            .unwrap()
            .children()
            .into_iter()
            .filter(|view| *view != container)
            .filter(|view| runner.handlers_of(*view).contains(&EventKind::Resize))
            .collect();
        assert_eq!(
            nodes.len(),
            1,
            "one shortcut delivery, with no post-delivery sibling"
        );
        nodes[0]
    }

    fn resize(runner: &mut Runner<Source>, view: u32, width: f64, height: f64) {
        runner
            .dispatch(
                view,
                Event::Resize(ResizeRect {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                }),
            )
            .unwrap();
    }

    fn quiet(runner: &mut Runner<Source>) {
        calls(runner, vec![]);
        assert!(
            runner.take_requests().is_empty(),
            "the complete outgoing request vector"
        );
        hosts(runner, vec![]);
        assert!(runner.pending().is_empty());
        assert!(runner.queued().is_empty());
    }

    // Setup has its own retained trace, outside the guest-event assertions.
    // No request is executed, fulfilled, or silently removed during preparation.
    fn setup_trace(runner: &mut Runner<Source>) -> Setup {
        assert!(
            runner.take_requests().is_empty(),
            "setup emitted an external request"
        );
        assert!(runner.pending().is_empty());
        assert!(runner.queued().is_empty());
        assert!(
            runner.data_ref().commands.is_empty(),
            "setup invoked a command mutation"
        );
        let trace = Setup {
            calls: std::mem::take(&mut runner.data().calls),
            hosts: runner
                .take_commands()
                .into_iter()
                .map(|command| (command.name, command.args))
                .collect(),
        };
        let plan = runner.plan();
        for observed in &trace.calls {
            assert_eq!(observed.method, "answer", "{trace:?}");
            assert!(
                observed.source == "transportPump"
                    || plan
                        .resources
                        .iter()
                        .any(|row| plan.str(row.source) == observed.source),
                "only the declared setup resources and transport pump: {trace:?}"
            );
        }
        for (name, args) in &trace.hosts {
            assert!(
                (name == "setRootFontSize" && args == &vec![Value::Number(16.0)])
                    || (name == "focus"
                        && ["composer", "app-confirm-cancel", "palette-input",]
                            .iter()
                            .any(|target| args == &vec![Value::str(target)])),
                "the complete setup host vector is retained here: {trace:?}"
            );
        }
        trace
    }

    fn first_palette_calls(runner: &Runner<Source>) -> Vec<Call> {
        let mut expected = Vec::new();
        for row in &runner.plan().resources {
            let name = runner.plan().str(row.name);
            let mut args = match name {
                "palette" | "shortcuts" | "settings" => {
                    runner.resource_args(name).unwrap().to_vec()
                }
                _ => continue,
            };
            match name {
                "palette" => {
                    assert_eq!(args.len(), 9);
                    assert_eq!(args[0], Value::Bool(false));
                    args[0] = Value::Bool(true);
                }
                "shortcuts" => {
                    assert_eq!(args.len(), 24);
                    assert_eq!(args[7], Value::Bool(false));
                    assert_eq!(args[11], Value::Bool(false));
                    args[7] = Value::Bool(true);
                    args[11] = Value::Bool(true);
                }
                "settings" => {
                    assert_eq!(args.len(), 8);
                    assert_eq!(args[4], Value::Bool(false));
                    args[4] = Value::Bool(true);
                }
                _ => unreachable!(),
            }
            expected.push(call("answer", runner.plan().str(row.source), &args));
        }
        assert_eq!(expected.len(), 3);
        expected
    }

    fn boot_shortcut(
        bytes: &[u8],
        layout: Layout,
        name: &str,
        sidebar_open: bool,
        blocker: Blocker,
    ) -> (Runner<Source>, Vec<Setup>) {
        let plan = Plan::decode(bytes).expect("a complete compiled app graph");
        let mutation = plan
            .mutations
            .iter()
            .find(|row| plan.str(row.name) == "localChanged")
            .unwrap();
        assert!(!mutation.queue);
        let mut change = empty(&plan, mutation.ty);
        set(
            &plan,
            mutation.ty,
            &mut change,
            &["revision"],
            Value::Number(1.0),
        );

        let mut values = HashMap::new();
        for row in &plan.resources {
            let source = plan.str(row.source);
            if !exact_plan::runner_owned_source(source) {
                let value = empty(&plan, row.ty);
                if let Some(previous) = values.insert(source.to_owned(), value.clone()) {
                    assert_eq!(previous, value, "shared-source defaults for {source}");
                }
            }
        }
        // The 1ms confirmation setup also runs the real mount transport tasks.
        let transport = plan
            .mutations
            .iter()
            .find(|row| plan.str(row.name) == "transportPumped")
            .unwrap();
        assert!(transport.queue);
        values.insert("transportPump".to_owned(), empty(&plan, transport.ty));

        let data = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == "data")
            .unwrap();
        let message_ty = element(&plan, data.ty, &["messages"]);
        let document_ty = element(&plan, message_ty, &["inlineDocuments"]);
        let mut document = empty(&plan, document_ty);
        for (field, value) in [
            ("key", Value::str(KEY)),
            ("kind", Value::str("html")),
            ("title", Value::str("Fixture guest shortcut")),
            ("src", Value::str("https://fixture.invalid/shortcut")),
            ("ready", Value::Bool(true)),
            ("height", Value::Number(340.0)),
            ("shortcut", Value::str(name)),
            ("shortcutSerial", Value::Number(1.0)),
        ] {
            set(&plan, document_ty, &mut document, &[field], value);
        }
        if blocker == Blocker::Permission {
            set(
                &plan,
                document_ty,
                &mut document,
                &["permission"],
                Value::str("Allow access?"),
            );
            set(
                &plan,
                document_ty,
                &mut document,
                &["permissionSerial"],
                Value::Number(1.0),
            );
        }
        let mut message = empty(&plan, message_ty);
        for (field, value) in [
            ("id", Value::str("shortcut-message")),
            ("kind", Value::str("inline-document")),
            ("first", Value::Bool(true)),
            ("last", Value::Bool(true)),
            ("inlineDocuments", Value::list(vec![document])),
        ] {
            set(&plan, message_ty, &mut message, &[field], value);
        }
        for (path, value) in [
            (&["revision"][..], Value::Number(1.0)),
            (&["connected"][..], Value::Bool(true)),
            (&["threadId"][..], Value::str("shortcut-thread")),
            (&["snapshotOwner"][..], Value::str("shortcut-snapshot")),
            (&["look", "chatMax"][..], Value::Number(728.0)),
            (&["sidebarOpen"][..], Value::Bool(sidebar_open)),
            (&["messages"][..], Value::list(vec![message])),
        ] {
            overlay(&plan, &mut values, "data", path, value);
        }
        match blocker {
            Blocker::Ssh => overlay(
                &plan,
                &mut values,
                "sshPrompt",
                &["open"],
                Value::Bool(true),
            ),
            Blocker::ServerUpdate => overlay(
                &plan,
                &mut values,
                "data",
                &["serverUpdate", "confirmTitle"],
                Value::str("Update?"),
            ),
            Blocker::UpdateEverything => overlay(
                &plan,
                &mut values,
                "data",
                &["updateEverything", "confirm"],
                Value::Bool(true),
            ),
            Blocker::ImagePreview => {
                let image_ty = element(&plan, data.ty, &["composerImagePreview"]);
                overlay(
                    &plan,
                    &mut values,
                    "data",
                    &["composerImagePreview"],
                    Value::list(vec![empty(&plan, image_ty)]),
                );
            }
            Blocker::DeviceSetup => overlay(
                &plan,
                &mut values,
                "shell",
                &["panel", "deviceSetup"],
                Value::Bool(true),
            ),
            Blocker::ComposerFile => overlay(
                &plan,
                &mut values,
                "shell",
                &["composerFile", "open"],
                Value::Bool(true),
            ),
            Blocker::Modal => overlay(&plan, &mut values, "welcome", &["show"], Value::Bool(true)),
            Blocker::Palette => overlay(
                &plan,
                &mut values,
                "data",
                &["legacy", "enabled"],
                Value::Bool(true),
            ),
            Blocker::Confirmation => {
                for (field, value) in [
                    ("serial", Value::Number(1.0)),
                    ("title", Value::str("Close terminal?")),
                    ("body", Value::str("Fixture confirmation")),
                    ("op", Value::str("terminallocal:close")),
                    ("target", Value::str("fixture-terminal")),
                ] {
                    overlay(
                        &plan,
                        &mut values,
                        "shell",
                        &["panel", "terminalClose", field],
                        value,
                    );
                }
            }
            _ => {}
        }

        // A fixed row isolates Contract routing from the producer's modal filtering.
        // The unchanged TypeScript selector test still checks configured key lookup.
        let shortcuts = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == "shortcuts")
            .unwrap();
        let mut rows = Vec::new();
        if name == "commandPalette.toggle" || name == "composer.send" {
            let row_ty = plan.type_(shortcuts.ty).elem.unwrap();
            let mut row = empty(&plan, row_ty);
            for (field, value) in [
                ("id", "fixture-guest-command"),
                ("command", name),
                ("chord", "Meta+k"),
                ("kind", "command"),
                ("target", "command"),
                ("extra", ""),
                ("label", "Fixture command"),
            ] {
                set(&plan, row_ty, &mut row, &[field], Value::str(value));
            }
            rows.push(row);
        }
        let rows = Value::list(rows);
        assert!(rows.conforms(&plan, shortcuts.ty));
        values.insert(plan.str(shortcuts.source).to_owned(), rows.clone());
        let snapshot = values.get(plan.str(data.source)).unwrap().clone();
        let mut runner = Runner::boot_stored(
            plan,
            Source {
                ready: false,
                values,
                change,
                calls: Vec::new(),
                commands: Vec::new(),
                parsed: Vec::new(),
            },
            Kernel::with_monospace(),
            Vec::new(),
            Viewport {
                width: 1280.0,
                height: 840.0,
                ..Default::default()
            },
            "/",
        )
        .unwrap();
        runner.set_time(EPOCH, 0.0).unwrap();
        runner.data().ready = true;
        runner.data_ready().unwrap();
        assert_eq!(runner.resource("data"), Some(&snapshot));
        assert_eq!(runner.resource("shortcuts"), Some(&rows));
        assert_eq!(palette_open(&runner, layout), Value::Bool(false));
        delivery(&runner);
        let mut setup = vec![setup_trace(&mut runner)];

        match blocker {
            Blocker::Confirmation => {
                let advanced = runner.advance_timed(1.0);
                assert!(advanced.error.is_none(), "{advanced:?}");
                assert_eq!(advanced.now_ms, 1.0);
                assert!(!advanced.receipts.is_empty());
                assert_eq!(
                    projected(&runner, layout, "confirmOp", "confirmationState", "op"),
                    Value::str("terminallocal:close"),
                );
                node_id(&runner, "app-confirm-cancel");
                let trace = setup_trace(&mut runner);
                assert_eq!(
                    trace
                        .hosts
                        .iter()
                        .filter(|(name, args)| name == "focus"
                            && args == &vec![Value::str("app-confirm-cancel")])
                        .count(),
                    1,
                    "{trace:?}",
                );
                setup.push(trace);
            }
            Blocker::Palette => {
                let expected = first_palette_calls(&runner);
                let view = node_test(&runner, "command-palette-trigger");
                assert!(runner.handlers_of(view).contains(&EventKind::Press));
                runner.dispatch(view, Event::Press).unwrap();
                let trace = setup_trace(&mut runner);
                assert_eq!(trace.calls, expected);
                assert_eq!(
                    trace.hosts,
                    vec![("focus".to_owned(), vec![Value::str("palette-input")],)]
                );
                assert_eq!(palette_open(&runner, layout), Value::Bool(true));
                setup.push(trace);
            }
            Blocker::Settings => {
                let view = node_test(&runner, "connection-settings");
                assert!(runner.handlers_of(view).contains(&EventKind::Press));
                runner.dispatch(view, Event::Press).unwrap();
                assert_eq!(
                    projected(&runner, layout, "settingsOpen", "settingsPageState", "open"),
                    Value::Bool(true),
                );
                let trace = setup_trace(&mut runner);
                assert!(trace.hosts.is_empty(), "{trace:?}");
                setup.push(trace);
            }
            _ => {}
        }
        delivery(&runner);
        assert_blocker(&runner, layout, blocker);
        (runner, setup)
    }

    fn assert_blocker(runner: &Runner<Source>, layout: Layout, blocker: Blocker) {
        let expected = match blocker {
            Blocker::Ssh => Some(("sshPrompt", vec!["open"], Value::Bool(true))),
            Blocker::ServerUpdate => Some((
                "data",
                vec!["serverUpdate", "confirmTitle"],
                Value::str("Update?"),
            )),
            Blocker::UpdateEverything => Some((
                "data",
                vec!["updateEverything", "confirm"],
                Value::Bool(true),
            )),
            Blocker::DeviceSetup => {
                Some(("shell", vec!["panel", "deviceSetup"], Value::Bool(true)))
            }
            Blocker::ComposerFile => {
                Some(("shell", vec!["composerFile", "open"], Value::Bool(true)))
            }
            Blocker::Modal => Some(("welcome", vec!["show"], Value::Bool(true))),
            _ => None,
        };
        if let Some((resource, path, value)) = expected {
            assert_eq!(resource_field(runner, resource, &path), value);
        }
        match blocker {
            Blocker::None => {
                assert_eq!(runner.derive("modal"), Some(&Value::Bool(false)));
                assert_eq!(palette_open(runner, layout), Value::Bool(false));
            }
            Blocker::ImagePreview => {
                let Value::List(images) = resource_field(runner, "data", &["composerImagePreview"])
                else {
                    panic!("the image-preview list")
                };
                assert_eq!(images.len(), 1);
            }
            Blocker::Permission => {
                node_id(runner, &format!("inline-permission-{KEY}-1-confirm"));
            }
            Blocker::Modal | Blocker::Palette | Blocker::Settings => {
                assert_eq!(runner.derive("modal"), Some(&Value::Bool(true)));
            }
            _ => {}
        }
    }

    #[test]
    fn configured_sidebar_uses_existing_operation_and_unsupported_command_is_inert() {
        for (bytes, layout) in plans() {
            for (open, value) in [(true, "closed"), (false, "open")] {
                let (mut runner, setup) =
                    boot_shortcut(bytes, layout, "sidebar.toggle", open, Blocker::None);
                assert_eq!(runner.resource("shortcuts"), Some(&Value::list(vec![])));
                let view = delivery(&runner);
                resize(&mut runner, view, 0.0, 1.0);
                quiet(&mut runner);
                resize(&mut runner, view, 1.0, 0.0);
                quiet(&mut runner);
                resize(&mut runner, view, 1.0, 1.0);
                let args = vec![
                    Value::str("sidebar"),
                    Value::str(""),
                    Value::str(value),
                    Value::Number(0.0),
                ];
                calls(&mut runner, vec![call("answer", "command", &args)]);
                let ticket = request(&mut runner, "localChanged");
                hosts(&mut runner, vec![]);
                assert_eq!(runner.pending(), vec![("localChanged".to_owned(), ticket)]);
                assert!(runner.queued().is_empty());
                assert_eq!(runner.data_ref().commands, vec![args]);
                // The one-shot delivery cannot issue a second newest-wins request.
                resize(&mut runner, view, 1.0, 1.0);
                calls(&mut runner, vec![]);
                assert!(runner.take_requests().is_empty());
                hosts(&mut runner, vec![]);
                assert_eq!(runner.pending(), vec![("localChanged".to_owned(), ticket)]);
                assert!(runner.queued().is_empty());
                assert_eq!(setup.len(), 1);
            }
            let (mut runner, setup) =
                boot_shortcut(bytes, layout, "composer.send", true, Blocker::None);
            let view = delivery(&runner);
            resize(&mut runner, view, 1.0, 1.0);
            quiet(&mut runner);
            assert!(runner.data_ref().commands.is_empty());
            assert_eq!(setup.len(), 1);
        }
    }

    #[test]
    fn guest_commands_cannot_bypass_each_global_guard_or_sidebar_context_guard() {
        for (bytes, layout) in plans() {
            for name in ["sidebar.toggle", "commandPalette.toggle"] {
                for blocker in [
                    Blocker::Ssh,
                    Blocker::Confirmation,
                    Blocker::ServerUpdate,
                    Blocker::UpdateEverything,
                    Blocker::ImagePreview,
                    Blocker::Permission,
                    Blocker::DeviceSetup,
                    Blocker::ComposerFile,
                ] {
                    let (mut runner, setup) = boot_shortcut(bytes, layout, name, true, blocker);
                    let before = palette_open(&runner, layout);
                    let view = delivery(&runner);
                    resize(&mut runner, view, 1.0, 1.0);
                    quiet(&mut runner);
                    assert_eq!(palette_open(&runner, layout), before, "{blocker:?}");
                    assert!(runner.data_ref().commands.is_empty(), "{blocker:?}");
                    resize(&mut runner, view, 1.0, 1.0);
                    quiet(&mut runner);
                    assert!(!setup.is_empty());
                }
            }
            // These three contextual guards belong to sidebar.toggle specifically.
            for blocker in [Blocker::Modal, Blocker::Palette, Blocker::Settings] {
                let (mut runner, setup) =
                    boot_shortcut(bytes, layout, "sidebar.toggle", true, blocker);
                let before = palette_open(&runner, layout);
                let view = delivery(&runner);
                resize(&mut runner, view, 1.0, 1.0);
                quiet(&mut runner);
                assert_eq!(palette_open(&runner, layout), before);
                assert!(runner.data_ref().commands.is_empty());
                assert!(!setup.is_empty());
            }
        }
    }

    #[test]
    fn unblocked_guest_opens_palette_once_without_restoration_geometry() {
        for (bytes, layout) in plans() {
            let (mut runner, setup) =
                boot_shortcut(bytes, layout, "commandPalette.toggle", true, Blocker::None);
            let expected = first_palette_calls(&runner);
            let view = delivery(&runner);
            resize(&mut runner, view, 0.0, 1.0);
            quiet(&mut runner);
            resize(&mut runner, view, 1.0, 1.0);
            calls(&mut runner, expected);
            assert!(runner.take_requests().is_empty());
            hosts(
                &mut runner,
                vec![("focus".to_owned(), vec![Value::str("palette-input")])],
            );
            assert!(runner.pending().is_empty());
            assert!(runner.queued().is_empty());
            assert_eq!(palette_open(&runner, layout), Value::Bool(true));
            node_id(&runner, "palette-input");
            // The same instance is already sent, so it does not close the open palette.
            resize(&mut runner, view, 1.0, 1.0);
            quiet(&mut runner);
            assert_eq!(palette_open(&runner, layout), Value::Bool(true));
            assert!(runner.data_ref().commands.is_empty());
            assert_eq!(setup.len(), 1);
        }
    }
}
