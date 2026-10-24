//! Composer intent ordering and invalidation through mounted public handlers.
//! Replies are typed consumer fixtures. The unchanged TypeScript suite still tests the producer,
//! projection and source session. Host commands are observed, not executed.

use super::composer_accessibility_tests::set;
use super::contract_keys_tests::empty;
use crate::contract_keys_tests::extraction::{
    call, field, fields, mutation, observation_type, resource, Call,
};
use exact_kernel::{Kernel, ViewId};
use exact_plan::{EventKind, Plan, TypesId, Value};
use exact_runner::{
    Answer, Command, ControlValue, DataError, DataSource, Delivery, Event, FieldSelection,
    InputDetails, KeyboardEvent, Outcome, Request, RequestOut, Response, Runner, Store, Viewport,
};
use std::collections::HashMap;

fn plans() -> impl Iterator<Item = (&'static [u8], Layout)> {
    crate::contract_keys_tests::extraction_plans().map(|(bytes, original)| {
        (
            bytes,
            if original {
                Layout::Original
            } else {
                Layout::Selected
            },
        )
    })
}
const EPOCH: f64 = 1_791_446_400_000.0;
const INTENT_URL: &str = "https://fixture.invalid/intent";
const PROJECTION_URL: &str = "https://fixture.invalid/projection";

#[derive(Clone, Copy, Debug)]
enum Layout {
    Original,
    Selected,
}

// Reply constructors change local Values, never Runner slots or resources.

fn rich(plan: &Plan, owner: &Value, value: &Value) -> Value {
    fields(
        plan,
        resource(plan, "richComposer").0,
        &[
            ("generation", Value::Number(1.0)),
            ("owner", owner.clone()),
            ("value", value.clone()),
            ("mode", Value::str("rich")),
        ],
    )
}

fn observation(plan: &Plan, value: &str, caret: f64, serial: f64) -> Value {
    fields(
        plan,
        observation_type(plan),
        &[
            ("generation", Value::Number(1.0)),
            ("snapshotOwner", Value::str("snapshot")),
            ("owner", Value::str("draft")),
            ("value", Value::str(value)),
            ("start", Value::Number(caret)),
            ("end", Value::Number(caret)),
            ("composing", Value::Bool(false)),
            ("live", Value::Bool(true)),
            ("focused", Value::Bool(true)),
            ("inputSerial", Value::Number(serial)),
        ],
    )
}

fn edit_reply(plan: &Plan, index: usize) -> Value {
    let value = format!("word{}", "\n".repeat(index));
    let at = 4.0 + index as f64;
    fields(
        plan,
        mutation(plan, "composerEdited"),
        &[
            ("owner", Value::str("draft")),
            ("value", Value::str(&value)),
            ("mode", Value::str("rich")),
            ("serial", Value::Number(index as f64 + 1.0)),
            ("kind", Value::str("edit")),
            ("start", Value::Number(at)),
            ("end", Value::Number(at)),
            ("text", Value::str("\n")),
            ("selectionStart", Value::Number(at + 1.0)),
            ("selectionEnd", Value::Number(at + 1.0)),
            ("preserveSelection", Value::Bool(false)),
        ],
    )
}

struct Source {
    schema: Plan,
    ready: bool,
    values: HashMap<String, Value>,
    snapshot: Value,
    delay_projection: bool,
    projection_delivered: bool,
    calls: Vec<Call>,
}

impl Source {
    fn value(&self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == resource(&self.schema, "data").1 {
            return Ok(self.snapshot.clone());
        }
        if source == resource(&self.schema, "richComposer").1 {
            assert_eq!(args.len(), 6);
            return Ok(rich(&self.schema, &args[1], &args[2]));
        }
        self.values
            .get(source)
            .cloned()
            .ok_or_else(|| DataError::UnknownSource(source.to_owned()))
    }
}

impl DataSource for Source {
    fn ready(&self) -> bool {
        self.ready
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls.push(call("query", source, args));
        self.value(source, args)
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        self.calls.push(call("answer", source, args));
        if source == "composerIntent" {
            return Ok(Answer::Later(Request::get(INTENT_URL)));
        }
        if source == resource(&self.schema, "richComposer").1
            && self.delay_projection
            && !self.projection_delivered
            && args[1] == Value::str("draft")
            && args[2] == Value::str("word\n")
            && args[5] == Value::str("composer")
        {
            return Ok(Answer::Later(Request::get(PROJECTION_URL)));
        }
        if source == "composerFieldObserved" {
            let ty = observation_type(&self.schema);
            return Ok(Answer::Now(fields(
                &self.schema,
                mutation(&self.schema, "composerFieldChanged"),
                &[
                    ("value", field(&self.schema, ty, &args[0], "value")),
                    ("start", field(&self.schema, ty, &args[0], "start")),
                    ("end", field(&self.schema, ty, &args[0], "end")),
                ],
            )));
        }
        if source == "command" {
            assert_eq!(args.len(), 4);
            assert_eq!(
                args[0],
                Value::str("draft"),
                "only public field persistence"
            );
            return Ok(Answer::Now(empty(
                &self.schema,
                mutation(&self.schema, "localChanged"),
            )));
        }
        if source == "transportPump" {
            return Ok(Answer::Now(empty(
                &self.schema,
                mutation(&self.schema, "transportPumped"),
            )));
        }
        if source == "composerChipObserved" {
            return Ok(Answer::Now(empty(
                &self.schema,
                mutation(&self.schema, "chipObserved"),
            )));
        }
        self.value(source, args).map(Answer::Now)
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.calls.push(call("parse", source, args));
        let Outcome::Response(reply) = outcome else {
            panic!("one held request response")
        };
        assert_eq!(reply.status, 200);
        assert!(reply.headers.is_empty());
        let value = Value::from_bytes(&reply.body).unwrap();
        if source == "composerIntent" {
            assert!(value.conforms(&self.schema, mutation(&self.schema, "composerEdited")));
        } else {
            assert_eq!(source, resource(&self.schema, "richComposer").1);
            assert!(self.delay_projection && !self.projection_delivered);
            assert_eq!(args[1], Value::str("draft"));
            assert_eq!(args[2], Value::str("word\n"));
            assert_eq!(args[5], Value::str("composer"));
            assert!(value.conforms(&self.schema, resource(&self.schema, "richComposer").0));
            self.projection_delivered = true;
        }
        Ok(Answer::Now(value))
    }
}

fn slot_type(plan: &Plan, name: &str) -> TypesId {
    plan.slots
        .iter()
        .find(|row| plan.str(row.name) == name)
        .unwrap()
        .ty
}

fn logical(runner: &Runner<Source>, layout: Layout, name: &str) -> (TypesId, Value) {
    let projected = match name {
        "composerIntentSerial" => ("intentState", "serial"),
        "composerIntentQueue" => ("intentState", "queue"),
        "composerIntentActive" => ("intentState", "active"),
        "composerIntentPhase" => ("intentState", "phase"),
        "composerIntentNotice" => ("intentState", "notice"),
        "composerIntentObserved" => ("intentState", "observed"),
        "composerIntentCaretSeen" => ("intentState", "caretSeen"),
        "composerFieldInputSerial" => ("fieldProtocol", "inputSerial"),
        "composerSelectionStart" => ("composerSelection", "start"),
        "composerSelectionEnd" => ("composerSelection", "end"),
        _ => panic!("unregistered logical assertion {name}"),
    };
    if matches!(layout, Layout::Original) {
        return (
            slot_type(runner.plan(), name),
            runner.slot(name).unwrap().clone(),
        );
    }
    let plan = runner.plan();
    let ty = slot_type(plan, projected.0);
    let index = plan.field_index(ty, projected.1).unwrap();
    let member = plan.type_(ty).fields.iter().nth(index).unwrap();
    (
        plan.field(member).ty,
        field(plan, ty, runner.slot(projected.0).unwrap(), projected.1),
    )
}

fn state(runner: &Runner<Source>, layout: Layout, name: &str) -> Value {
    logical(runner, layout, name).1
}

fn assert_queue(runner: &Runner<Source>, layout: Layout, first: usize, count: usize) {
    let (ty, actual) = logical(runner, layout, "composerIntentQueue");
    let item = runner.plan().type_(ty).elem.unwrap();
    let expected = (first..=count)
        .map(|serial| {
            fields(
                runner.plan(),
                item,
                &[
                    ("serial", Value::Number(serial as f64)),
                    ("kind", Value::str("key")),
                    ("text", Value::str("")),
                    ("key", Value::str("Enter")),
                    ("shift", Value::Bool(true)),
                    ("command", Value::Bool(false)),
                    ("start", Value::Number(4.0)),
                    ("end", Value::Number(4.0)),
                    ("inputSerial", Value::Number(1.0)),
                ],
            )
        })
        .collect();
    assert_eq!(actual, Value::list(expected), "complete ordered metadata");
}

fn node(runner: &Runner<Source>) -> ViewId {
    let keys = runner.kernel().find_by_test_id("composer");
    assert_eq!(keys.len(), 1, "one mounted public composer");
    runner.kernel().node_by_key(keys[0]).unwrap().id
}

fn command(name: &str, args: Vec<Value>, source: Option<ViewId>) -> Command {
    Command {
        name: name.to_owned(),
        args,
        source,
    }
}

fn response(value: &Value) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: value.to_bytes(),
    })
}

fn calls(runner: &mut Runner<Source>, expected: Vec<Call>) {
    assert_eq!(
        std::mem::take(&mut runner.data().calls),
        expected,
        "all ordered data calls"
    );
}

fn hosts(runner: &mut Runner<Source>, expected: Vec<Command>) {
    assert_eq!(
        runner.take_commands(),
        expected,
        "all ordered host commands"
    );
}

fn no_requests(runner: &mut Runner<Source>) {
    assert!(runner.take_requests().is_empty(), "all outgoing requests");
    assert!(runner.pending().is_empty());
    assert!(runner.queued().is_empty());
}

fn task_due(runner: &Runner<Source>, name: &str) -> Option<f64> {
    runner
        .tasks()
        .into_iter()
        .find(|(found, _)| found == name)
        .unwrap()
        .1
}

fn source_call(runner: &Runner<Source>, name: &str) -> Call {
    call(
        "answer",
        &resource(runner.plan(), name).1,
        runner.resource_args(name).unwrap(),
    )
}

// Only the initial mount and word/caret preparation use finite source allowances.
// Their complete traces are retained; witness events below use fixed full expectations.
#[derive(Debug)]
struct Preparation {
    calls: Vec<Call>,
    hosts: Vec<Command>,
}

fn preparation(runner: &mut Runner<Source>, boot: bool) -> Preparation {
    no_requests(runner);
    let trace = Preparation {
        calls: std::mem::take(&mut runner.data().calls),
        hosts: runner.take_commands(),
    };
    for observed in &trace.calls {
        assert_eq!(observed.method, "answer", "{trace:?}");
        assert!(
            runner
                .plan()
                .resources
                .iter()
                .any(|row| runner.plan().str(row.source) == observed.source)
                || [
                    "command",
                    "composerFieldObserved",
                    "transportPump",
                    "composerChipObserved"
                ]
                .contains(&observed.source.as_str()),
            "{trace:?}"
        );
        if observed.source == "command" {
            assert_eq!(observed.args[0], Value::str("draft"), "{trace:?}");
        }
    }
    let expected = if boot {
        vec![
            command("setRootFontSize", vec![Value::Number(16.0)], None),
            command("close", vec![Value::str("server-browser-dialog-")], None),
            command("close", vec![Value::str("server-browser-dialog-")], None),
        ]
    } else {
        vec![]
    };
    assert_eq!(trace.hosts, expected, "{trace:?}");
    trace
}

fn dispatch_input(runner: &mut Runner<Source>, value: &str, caret: u32) {
    let view = node(runner);
    assert!(runner.handlers_of(view).contains(&EventKind::Input));
    runner
        .dispatch(
            view,
            Event::Input(ControlValue::Edit(
                value.to_owned(),
                FieldSelection::caret(caret),
                InputDetails {
                    input_type: "insertText".to_owned(),
                    data: Some(value.to_owned()),
                    cancelable: false,
                    is_composing: false,
                },
            )),
        )
        .unwrap();
}

fn boot(bytes: &[u8], layout: Layout, lag: bool) -> Runner<Source> {
    let plan = Plan::decode(bytes).expect("one independently compiled complete graph");
    let row = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "composerEdited")
        .unwrap();
    assert!(row.queue);
    assert_eq!(
        row.refreshes.len, 0,
        "the intent reply does not refresh ACK resources"
    );
    let mut values = HashMap::new();
    for row in &plan.resources {
        if !exact_plan::runner_owned_source(plan.str(row.source)) {
            let value = empty(&plan, row.ty);
            if let Some(previous) = values.insert(plan.str(row.source).to_owned(), value.clone()) {
                assert_eq!(previous, value);
            }
        }
    }
    let snapshot = fields(
        &plan,
        resource(&plan, "data").0,
        &[
            ("revision", Value::Number(1.0)),
            ("connected", Value::Bool(true)),
            ("sidebarOpen", Value::Bool(true)),
            ("snapshotOwner", Value::str("snapshot")),
            ("composerOwner", Value::str("draft")),
            ("draft", Value::str("word")),
        ],
    );
    let data = Source {
        schema: Plan::decode(bytes).unwrap(),
        ready: false,
        values,
        snapshot,
        delay_projection: lag,
        projection_delivered: false,
        calls: vec![],
    };
    let mut runner = Runner::boot_with_delivery(
        plan,
        data,
        Kernel::with_monospace(),
        None,
        vec![],
        Delivery::default(),
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
    for task in ["rootFont", "streamClosedDialog", "streamClosedMiniDialog"] {
        assert_eq!(task_due(&runner, task), Some(1.0));
    }
    let warm = runner.advance_timed(2.0);
    assert!(warm.error.is_none(), "{warm:?}");
    assert_eq!(warm.now_ms, 2.0);
    let boot_trace = preparation(&mut runner, true);
    assert!(!boot_trace.calls.is_empty());
    let view = node(&runner);
    assert!(runner.handlers_of(view).contains(&EventKind::Focus));
    runner.dispatch(view, Event::Focus).unwrap();
    let _focus_trace = preparation(&mut runner, false);
    dispatch_input(&mut runner, "word", 4);
    let _input_trace = preparation(&mut runner, false);
    assert_eq!(
        state(&runner, layout, "composerFieldInputSerial"),
        Value::Number(1.0)
    );
    assert_eq!(
        state(&runner, layout, "composerSelectionStart"),
        Value::Number(4.0)
    );
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(task_due(&runner, "composerIntentReady"), None);
    assert_eq!(task_due(&runner, "composerIntentReceipt"), None);
    runner
}

fn background(runner: &Runner<Source>, until: f64) -> Vec<Call> {
    assert_eq!(task_due(runner, "promptPreviewLifecycle"), None);
    let mut expected = Vec::new();
    if let Some(due) = task_due(runner, "transportAnswered").filter(|due| *due <= until) {
        expected.push(call(
            "answer",
            "transportPump",
            &[Value::Number(EPOCH + due), Value::str("")],
        ));
    }
    expected
}

fn keys(runner: &mut Runner<Source>, layout: Layout, count: usize) {
    for serial in 1..=count {
        let view = node(runner);
        assert!(runner.handlers_of(view).contains(&EventKind::Key));
        runner
            .dispatch(view, Event::Key(KeyboardEvent::chord("Shift+Enter")))
            .unwrap();
        calls(runner, vec![]);
        no_requests(runner);
        hosts(
            runner,
            vec![
                command("preventDefault", vec![], Some(view)),
                command("stopPropagation", vec![], Some(view)),
            ],
        );
        assert_eq!(
            state(runner, layout, "composerIntentSerial"),
            Value::Number(serial as f64)
        );
        assert_eq!(state(runner, layout, "composerIntentPhase"), Value::str(""));
        assert_eq!(
            state(runner, layout, "composerIntentActive"),
            Value::Number(0.0)
        );
        assert_queue(runner, layout, 1, serial);
    }
    assert_eq!(
        state(runner, layout, "composerIntentNotice"),
        Value::str("")
    );
    assert_eq!(
        state(runner, layout, "composerIntentObserved"),
        observation(runner.plan(), "word", 4.0, 1.0)
    );
    assert_eq!(
        task_due(runner, "composerIntentReady"),
        Some(runner.now_ms() + 1.0)
    );
}

fn intent_args(index: usize) -> Vec<Value> {
    let caret = 4.0 + index as f64;
    vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str(&format!("word{}", "\n".repeat(index))),
        Value::Number(index as f64 + 1.0),
        Value::Number(caret),
        Value::Number(caret),
        Value::str("key"),
        Value::str(""),
        Value::str("Enter"),
        Value::Bool(true),
        Value::Bool(false),
        Value::Bool(false),
        Value::str("rich"),
    ]
}

fn pump(runner: &mut Runner<Source>, layout: Layout, index: usize) -> (u64, Vec<Value>) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task_due(runner, "composerIntentReady"), Some(due));
    let args = intent_args(index);
    let mut expected = background(runner, due);
    expected.push(call("answer", "composerIntent", &args));
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    hosts(runner, vec![]);
    let requests = runner.take_requests();
    assert_eq!(requests.len(), 1);
    let ticket = requests[0].ticket;
    assert_eq!(
        requests,
        vec![RequestOut {
            ticket,
            target: "composerEdited".to_owned(),
            request: Request::get(INTENT_URL),
            forced: false,
        }]
    );
    assert_eq!(
        runner.pending(),
        vec![("composerEdited".to_owned(), ticket)]
    );
    assert!(runner.queued().is_empty());
    assert_eq!(
        state(runner, layout, "composerIntentPhase"),
        Value::str("request")
    );
    assert_eq!(
        state(runner, layout, "composerIntentActive"),
        Value::Number(index as f64 + 1.0)
    );
    assert_eq!(task_due(runner, "composerIntentReceipt"), None);
    (ticket, args)
}

fn land_edit(
    runner: &mut Runner<Source>,
    layout: Layout,
    index: usize,
    ticket: u64,
    args: &[Value],
) {
    let now = runner.now_ms();
    let answer = edit_reply(runner.plan(), index);
    assert!(runner.fulfill(ticket, response(&answer)).unwrap().is_some());
    calls(runner, vec![call("parse", "composerIntent", args)]);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_eq!(runner.now_ms(), now);
    assert_eq!(
        state(runner, layout, "composerIntentPhase"),
        Value::str("request")
    );
    let landed = runner.land_then();
    assert!(landed.error.is_none(), "{landed:?}");
    assert_eq!(landed.now_ms, now);
    assert_eq!(landed.receipts.len(), 1);
    assert_eq!(landed.receipts[0].at_ms, now);
    calls(runner, vec![]);
    no_requests(runner);
    let at = 4.0 + index as f64;
    hosts(
        runner,
        vec![
            command(
                "setRangeText",
                vec![
                    Value::str("composer"),
                    Value::str("\n"),
                    Value::Number(at),
                    Value::Number(at),
                    Value::str("preserve"),
                ],
                None,
            ),
            command(
                "setSelectionRange",
                vec![
                    Value::str("composer"),
                    Value::Number(at + 1.0),
                    Value::Number(at + 1.0),
                ],
                None,
            ),
        ],
    );
    assert_eq!(
        state(runner, layout, "composerIntentPhase"),
        Value::str("input")
    );
    assert_eq!(task_due(runner, "composerIntentReady"), None);
    assert_eq!(task_due(runner, "composerIntentReceipt"), None);
}

struct Projection {
    ticket: u64,
    args: Vec<Value>,
}

// Settlement asks forced data/editor, the new rich value, then its dependent observation.
// Expected arguments use standing queries and the declared next Input fields.
fn input(
    runner: &mut Runner<Source>,
    layout: Layout,
    index: usize,
    caret: u32,
    lag: bool,
) -> Option<Projection> {
    let value = format!("word{}", "\n".repeat(index + 1));
    let serial = index as f64 + 2.0;
    let observed = observation(runner.plan(), &value, f64::from(caret), serial);
    let mut expected = vec![
        call(
            "answer",
            "composerFieldObserved",
            &[observed.clone(), Value::Bool(true)],
        ),
        call(
            "answer",
            "command",
            &[
                Value::str("draft"),
                Value::str(""),
                Value::str(&value),
                Value::Number(0.0),
            ],
        ),
        source_call(runner, "data"),
        source_call(runner, "editor"),
    ];
    let (_, rich_source) = resource(runner.plan(), "richComposer");
    let mut rich_args = runner.resource_args("richComposer").unwrap().to_vec();
    assert_eq!(rich_args.len(), 6);
    rich_args[2] = Value::str(&value);
    rich_args[4] = Value::str("insertText");
    expected.push(call("answer", &rich_source, &rich_args));
    let (_, field_source) = resource(runner.plan(), "composerFieldNext");
    let mut field_args = runner.resource_args("composerFieldNext").unwrap().to_vec();
    assert_eq!(field_args.len(), 3);
    field_args[0] = observed;
    expected.push(call("answer", &field_source, &field_args));
    dispatch_input(runner, &value, caret);
    calls(runner, expected);
    hosts(runner, vec![]);
    assert_eq!(
        state(runner, layout, "composerFieldInputSerial"),
        Value::Number(serial)
    );
    assert_eq!(
        state(runner, layout, "composerSelectionStart"),
        Value::Number(f64::from(caret))
    );
    assert_eq!(
        state(runner, layout, "composerSelectionEnd"),
        Value::Number(f64::from(caret))
    );
    assert_eq!(runner.derive("composerText"), Some(&Value::str(&value)));
    assert_eq!(
        state(runner, layout, "composerIntentPhase"),
        Value::str("projection")
    );
    assert_eq!(task_due(runner, "composerIntentReady"), None);
    if lag {
        let requests = runner.take_requests();
        assert_eq!(requests.len(), 1);
        let ticket = requests[0].ticket;
        assert_eq!(
            requests,
            vec![RequestOut {
                ticket,
                target: "richComposer".to_owned(),
                request: Request::get(PROJECTION_URL),
                forced: false,
            }]
        );
        assert_eq!(runner.pending(), vec![("richComposer".to_owned(), ticket)]);
        assert!(runner.queued().is_empty());
        assert_eq!(
            runner.derive("composerRichCurrent"),
            Some(&Value::Bool(false))
        );
        assert_eq!(task_due(runner, "composerIntentReceipt"), None);
        Some(Projection {
            ticket,
            args: rich_args,
        })
    } else {
        no_requests(runner);
        assert_eq!(
            runner.derive("composerRichCurrent"),
            Some(&Value::Bool(true))
        );
        None
    }
}

fn project(runner: &mut Runner<Source>, projection: &Projection) {
    let (ty, source) = resource(runner.plan(), "richComposer");
    // The displayed answer retains old arguments while this request waits.
    let args = &projection.args;
    assert_eq!(
        args,
        &vec![
            Value::str("snapshot"),
            Value::str("draft"),
            Value::str("word\n"),
            Value::Number(1.0),
            Value::str("insertText"),
            Value::str("composer"),
        ]
    );
    let answer = rich(runner.plan(), &Value::str("draft"), &Value::str("word\n"));
    assert!(answer.conforms(runner.plan(), ty));
    let now = runner.now_ms();
    assert!(runner
        .fulfill(projection.ticket, response(&answer))
        .unwrap()
        .is_some());
    calls(runner, vec![call("parse", &source, args)]);
    hosts(runner, vec![]);
    no_requests(runner);
    assert_eq!(runner.now_ms(), now);
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(true))
    );
}

fn move_caret(runner: &mut Runner<Source>, layout: Layout, caret: u32, changed: bool) {
    let observed = observation(runner.plan(), "word\n", f64::from(caret), 2.0);
    let mut expected = vec![
        call(
            "answer",
            "composerFieldObserved",
            &[observed.clone(), Value::Bool(false)],
        ),
        source_call(runner, "editor"),
    ];
    if changed {
        let (_, source) = resource(runner.plan(), "composerFieldNext");
        let mut args = runner.resource_args("composerFieldNext").unwrap().to_vec();
        args[0] = observed;
        expected.push(call("answer", &source, &args));
    }
    let view = node(runner);
    assert!(runner
        .handlers_of(view)
        .contains(&EventKind::Selectionchange));
    runner
        .dispatch(
            view,
            Event::FieldSelectionChange("word\n".to_owned(), FieldSelection::caret(caret)),
        )
        .unwrap();
    calls(runner, expected);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_eq!(
        state(runner, layout, "composerFieldInputSerial"),
        Value::Number(2.0)
    );
    assert_eq!(
        state(runner, layout, "composerSelectionStart"),
        Value::Number(f64::from(caret))
    );
    assert_eq!(
        state(runner, layout, "composerSelectionEnd"),
        Value::Number(f64::from(caret))
    );
    assert_eq!(
        state(runner, layout, "composerIntentPhase"),
        Value::str("projection")
    );
}

fn quiet(
    runner: &mut Runner<Source>,
    phase: &str,
    layout: Layout,
    projection: Option<&Projection>,
) {
    assert_eq!(task_due(runner, "composerIntentReady"), None);
    assert_eq!(task_due(runner, "composerIntentReceipt"), None);
    let due = runner.now_ms() + 1.0;
    let expected = background(runner, due);
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    hosts(runner, vec![]);
    assert!(runner.take_requests().is_empty());
    assert_eq!(
        runner.pending(),
        projection
            .map(|held| vec![("richComposer".to_owned(), held.ticket)])
            .unwrap_or_default()
    );
    assert!(runner.queued().is_empty());
    assert_eq!(
        state(runner, layout, "composerIntentPhase"),
        Value::str(phase)
    );
    assert_eq!(task_due(runner, "composerIntentReady"), None);
    assert_eq!(task_due(runner, "composerIntentReceipt"), None);
}

fn receipt(runner: &mut Runner<Source>, layout: Layout, index: usize, count: usize) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task_due(runner, "composerIntentReceipt"), Some(due));
    assert_eq!(task_due(runner, "composerIntentReady"), None);
    let expected = background(runner, due);
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_queue(runner, layout, index + 2, count);
    assert_eq!(state(runner, layout, "composerIntentPhase"), Value::str(""));
    assert_eq!(
        state(runner, layout, "composerIntentActive"),
        Value::Number(0.0)
    );
    assert_eq!(
        state(runner, layout, "composerIntentNotice"),
        Value::str("")
    );
    assert_eq!(
        state(runner, layout, "composerIntentObserved"),
        observation(
            runner.plan(),
            &format!("word{}", "\n".repeat(index + 1)),
            5.0 + index as f64,
            index as f64 + 2.0,
        )
    );
    assert_eq!(task_due(runner, "composerIntentReceipt"), None);
    assert_eq!(
        task_due(runner, "composerIntentReady"),
        (index + 1 < count).then_some(due + 1.0)
    );
}

#[test]
fn two_or_three_suppressed_keys_wait_for_each_resulting_value_and_caret() {
    for (bytes, layout) in plans() {
        for count in [2, 3] {
            let mut runner = boot(bytes, layout, false);
            keys(&mut runner, layout, count);
            for index in 0..count {
                let (ticket, args) = pump(&mut runner, layout, index);
                land_edit(&mut runner, layout, index, ticket, &args);
                assert!(input(&mut runner, layout, index, (5 + index) as u32, false).is_none());
                receipt(&mut runner, layout, index, count);
            }
            assert_eq!(
                runner.derive("composerText"),
                Some(&Value::str(&format!("word{}", "\n".repeat(count))))
            );
            quiet(&mut runner, "", layout, None);
        }
    }
}

#[test]
fn a_result_waits_for_input_and_that_input_waits_for_its_projection() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, true);
        keys(&mut runner, layout, 2);
        let (ticket, args) = pump(&mut runner, layout, 0);
        land_edit(&mut runner, layout, 0, ticket, &args);
        quiet(&mut runner, "input", layout, None);
        assert_queue(&runner, layout, 1, 2);
        assert_eq!(
            state(&runner, layout, "composerFieldInputSerial"),
            Value::Number(1.0)
        );
        let projection = input(&mut runner, layout, 0, 5, true).unwrap();
        quiet(&mut runner, "projection", layout, Some(&projection));
        assert_queue(&runner, layout, 1, 2);
        project(&mut runner, &projection);
        receipt(&mut runner, layout, 0, 2);
        let (_ticket, _args) = pump(&mut runner, layout, 1);
    }
}

#[test]
fn intermediate_selection_waits_for_the_requested_final_caret() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, false);
        keys(&mut runner, layout, 2);
        let (ticket, args) = pump(&mut runner, layout, 0);
        land_edit(&mut runner, layout, 0, ticket, &args);
        assert!(input(&mut runner, layout, 0, 4, false).is_none());
        assert_eq!(
            state(&runner, layout, "composerIntentCaretSeen"),
            Value::Bool(false)
        );
        move_caret(&mut runner, layout, 4, false);
        quiet(&mut runner, "projection", layout, None);
        assert_queue(&runner, layout, 1, 2);
        move_caret(&mut runner, layout, 5, true);
        assert_eq!(
            state(&runner, layout, "composerIntentCaretSeen"),
            Value::Bool(true)
        );
        receipt(&mut runner, layout, 0, 2);
        let (_ticket, _args) = pump(&mut runner, layout, 1);
    }
}

mod manual_invalidation {
    //! Six additive invalidation witnesses follow the retained four-case prefix.
    //! BeforeInput changes stored composition; a selection event cannot carry that flag.
    //! These public event outcomes do not isolate every guard from overlapping changes.

    use super::*;

    const NOTICE: &str = "The prompt changed. Please try that edit again.";

    #[derive(Clone, Copy, Debug)]
    enum Stage {
        Request,
        Projection,
    }

    impl Stage {
        fn value(self) -> &'static str {
            match self {
                Self::Request => "word",
                Self::Projection => "word\n",
            }
        }

        fn caret(self) -> u32 {
            match self {
                Self::Request => 4,
                Self::Projection => 5,
            }
        }

        fn serial(self) -> f64 {
            match self {
                Self::Request => 1.0,
                Self::Projection => 2.0,
            }
        }

        fn projection(self) -> bool {
            matches!(self, Self::Projection)
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Change {
        Input,
        Selection,
        Composition,
    }

    const INTENT_MEMBERS: [(&str, &str); 14] = [
        ("composerIntentSerial", "serial"),
        ("composerIntentQueue", "queue"),
        ("composerIntentActive", "active"),
        ("composerIntentPhase", "phase"),
        ("composerIntentMode", "mode"),
        ("composerIntentNotice", "notice"),
        ("composerIntentObserved", "observed"),
        ("composerIntentExpected", "expected"),
        ("composerIntentStart", "start"),
        ("composerIntentEnd", "end"),
        ("composerIntentInputStart", "inputStart"),
        ("composerIntentInputEnd", "inputEnd"),
        ("composerIntentPreserves", "preserves"),
        ("composerIntentCaretSeen", "caretSeen"),
    ];

    fn member(
        runner: &Runner<Source>,
        layout: Layout,
        original: &str,
        record: &str,
        name: &str,
    ) -> Value {
        if matches!(layout, Layout::Original) {
            runner.slot(original).unwrap().clone()
        } else {
            field(
                runner.plan(),
                slot_type(runner.plan(), record),
                runner.slot(record).unwrap(),
                name,
            )
        }
    }

    fn queue(runner: &Runner<Source>, layout: Layout, cleared: bool) -> Value {
        let (ty, _) = logical(runner, layout, "composerIntentQueue");
        let item = runner.plan().type_(ty).elem.unwrap();
        let values = if cleared {
            vec![]
        } else {
            (1..=2)
                .map(|serial| {
                    fields(
                        runner.plan(),
                        item,
                        &[
                            ("serial", Value::Number(f64::from(serial))),
                            ("kind", Value::str("key")),
                            ("text", Value::str("")),
                            ("key", Value::str("Enter")),
                            ("shift", Value::Bool(true)),
                            ("command", Value::Bool(false)),
                            ("start", Value::Number(4.0)),
                            ("end", Value::Number(4.0)),
                            ("inputSerial", Value::Number(1.0)),
                        ],
                    )
                })
                .collect()
        };
        Value::list(values)
    }

    fn assert_intent(runner: &Runner<Source>, layout: Layout, stage: Stage, cleared: bool) {
        let projected = stage.projection();
        let phase = if cleared {
            ""
        } else if projected {
            "projection"
        } else {
            "request"
        };
        let at = if projected { 5.0 } else { 0.0 };
        let expected = vec![
            Value::Number(2.0),
            queue(runner, layout, cleared),
            Value::Number(if cleared { 0.0 } else { 1.0 }),
            Value::str(phase),
            Value::str("rich"),
            Value::str(if cleared { NOTICE } else { "" }),
            observation(runner.plan(), "word", 4.0, 1.0),
            Value::str(if projected { "word\n" } else { "" }),
            Value::Number(at),
            Value::Number(at),
            Value::Number(at),
            Value::Number(at),
            Value::Bool(false),
            Value::Bool(projected),
        ];
        let actual: Vec<_> = INTENT_MEMBERS
            .iter()
            .map(|(original, name)| member(runner, layout, original, "intentState", name))
            .collect();
        assert_eq!(
            actual, expected,
            "complete intent record: {stage:?}, cleared={cleared}"
        );
    }

    fn observed(
        runner: &Runner<Source>,
        value: &str,
        caret: u32,
        serial: f64,
        composing: bool,
    ) -> Value {
        let mut next = observation(runner.plan(), value, f64::from(caret), serial);
        set(
            runner.plan(),
            observation_type(runner.plan()),
            &mut next,
            &["composing"],
            Value::Bool(composing),
        );
        next
    }

    fn assert_field(runner: &Runner<Source>, layout: Layout, stage: Stage, change: Option<Change>) {
        let input = matches!(change, Some(Change::Input));
        let composing = matches!(change, Some(Change::Composition));
        let value = format!("{}{}", stage.value(), if input { "x" } else { "" });
        let caret = if matches!(change, Some(Change::Selection)) {
            0
        } else {
            stage.caret() + if input { 1 } else { 0 }
        };
        let serial = stage.serial() + if input { 1.0 } else { 0.0 };
        assert_eq!(runner.derive("composerText"), Some(&Value::str(&value)));
        assert_eq!(
            state(runner, layout, "composerFieldInputSerial"),
            Value::Number(serial)
        );
        for (original, record, name, expected) in [
            ("draft", "composerDraft", "value", Value::str(&value)),
            ("draftOwner", "composerDraft", "owner", Value::str("draft")),
            (
                "composerFocused",
                "composerDraft",
                "focused",
                Value::Bool(true),
            ),
            (
                "composerComposing",
                "composerDraft",
                "composing",
                Value::Bool(composing),
            ),
            (
                "composerSelectionOwner",
                "composerSelection",
                "owner",
                Value::str("draft"),
            ),
            (
                "composerSelectionValue",
                "composerSelection",
                "value",
                Value::str(&value),
            ),
            (
                "composerSelectionStart",
                "composerSelection",
                "start",
                Value::Number(f64::from(caret)),
            ),
            (
                "composerSelectionEnd",
                "composerSelection",
                "end",
                Value::Number(f64::from(caret)),
            ),
        ] {
            assert_eq!(member(runner, layout, original, record, name), expected);
        }
        let snapshot = observed(runner, &value, caret, serial, composing);
        assert_eq!(runner.derive("composerFieldSnapshot"), Some(&snapshot));
        assert_eq!(
            runner.derive("composerSelectionCurrent"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            runner.derive("composerRichCurrent"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            runner.derive("composerIntentContextCurrent"),
            Some(&Value::Bool(!composing))
        );
    }

    fn assert_pending(runner: &Runner<Source>, stage: Stage, ticket: u64) {
        let expected = if stage.projection() {
            vec![]
        } else {
            vec![("composerEdited".to_owned(), ticket)]
        };
        assert_eq!(runner.pending(), expected);
        assert!(runner.queued().is_empty());
    }

    fn assert_canceled(runner: &Runner<Source>) {
        for task in [
            "composerIntentReady",
            "composerIntentReceipt",
            "composerIntentInvalidated",
        ] {
            assert_eq!(task_due(runner, task), None, "{task}");
        }
    }

    fn manual_input(runner: &mut Runner<Source>, stage: Stage) {
        let value = format!("{}x", stage.value());
        let caret = stage.caret() + 1;
        let snapshot = observed(runner, &value, caret, stage.serial() + 1.0, false);
        let mut expected = vec![
            call(
                "answer",
                "composerFieldObserved",
                &[snapshot.clone(), Value::Bool(true)],
            ),
            call(
                "answer",
                "command",
                &[
                    Value::str("draft"),
                    Value::str(""),
                    Value::str(&value),
                    Value::Number(0.0),
                ],
            ),
            source_call(runner, "data"),
            source_call(runner, "editor"),
        ];
        let mut rich_args = runner.resource_args("richComposer").unwrap().to_vec();
        assert_eq!(rich_args.len(), 6);
        rich_args[2] = Value::str(&value);
        rich_args[4] = Value::str("insertText");
        expected.push(call(
            "answer",
            &resource(runner.plan(), "richComposer").1,
            &rich_args,
        ));
        let mut field_args = runner.resource_args("composerFieldNext").unwrap().to_vec();
        assert_eq!(field_args.len(), 3);
        field_args[0] = snapshot;
        expected.push(call(
            "answer",
            &resource(runner.plan(), "composerFieldNext").1,
            &field_args,
        ));
        dispatch_input(runner, &value, caret);
        calls(runner, expected);
    }

    fn selection_or_composition(runner: &mut Runner<Source>, stage: Stage, change: Change) {
        let composing = matches!(change, Change::Composition);
        let caret = if composing { stage.caret() } else { 0 };
        let snapshot = observed(runner, stage.value(), caret, stage.serial(), composing);
        let mut expected = vec![
            call(
                "answer",
                "composerFieldObserved",
                &[snapshot.clone(), Value::Bool(false)],
            ),
            source_call(runner, "editor"),
        ];
        let mut args = runner.resource_args("composerFieldNext").unwrap().to_vec();
        assert_eq!(args.len(), 3);
        args[0] = snapshot;
        expected.push(call(
            "answer",
            &resource(runner.plan(), "composerFieldNext").1,
            &args,
        ));
        let view = node(runner);
        let event = if composing {
            assert!(runner.handlers_of(view).contains(&EventKind::Beforeinput));
            Event::BeforeInput(
                stage.value().to_owned(),
                FieldSelection::caret(caret),
                InputDetails {
                    input_type: "insertCompositionText".to_owned(),
                    data: None,
                    cancelable: false,
                    is_composing: true,
                },
            )
        } else {
            assert!(runner
                .handlers_of(view)
                .contains(&EventKind::Selectionchange));
            Event::FieldSelectionChange(stage.value().to_owned(), FieldSelection::caret(caret))
        };
        runner.dispatch(view, event).unwrap();
        calls(runner, expected);
    }

    fn invalidate(
        runner: &mut Runner<Source>,
        layout: Layout,
        stage: Stage,
        change: Change,
        ticket: u64,
    ) {
        let now = runner.now_ms();
        assert_intent(runner, layout, stage, false);
        assert_field(runner, layout, stage, None);
        assert_pending(runner, stage, ticket);
        assert_eq!(task_due(runner, "composerIntentReady"), None);
        assert_eq!(
            task_due(runner, "composerIntentReceipt"),
            stage.projection().then_some(now + 1.0)
        );
        match change {
            Change::Input => manual_input(runner, stage),
            Change::Selection | Change::Composition => {
                selection_or_composition(runner, stage, change);
            }
        }
        assert_eq!(runner.now_ms(), now, "invalidate before the receipt timer");
        assert!(
            runner.take_requests().is_empty(),
            "all material-event outgoing requests"
        );
        hosts(runner, vec![]);
        assert_pending(runner, stage, ticket);
        assert_intent(runner, layout, stage, true);
        assert_field(runner, layout, stage, Some(change));
        assert_canceled(runner);
    }

    fn assert_cleared(runner: &Runner<Source>, layout: Layout, stage: Stage, change: Change) {
        assert_intent(runner, layout, stage, true);
        assert_field(runner, layout, stage, Some(change));
        assert_canceled(runner);
    }

    fn late_request(
        runner: &mut Runner<Source>,
        layout: Layout,
        change: Change,
        ticket: u64,
        args: &[Value],
    ) {
        let now = runner.now_ms();
        let answer = edit_reply(runner.plan(), 0);
        assert!(runner.fulfill(ticket, response(&answer)).unwrap().is_some());
        calls(runner, vec![call("parse", "composerIntent", args)]);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_eq!(runner.now_ms(), now);
        assert_cleared(runner, layout, Stage::Request, change);
        let continuation = runner.land_then();
        assert!(continuation.error.is_none(), "{continuation:?}");
        assert_eq!(continuation.now_ms, now);
        assert_eq!(continuation.receipts.len(), 1);
        assert_eq!(continuation.receipts[0].at_ms, now);
        calls(runner, vec![]);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_cleared(runner, layout, Stage::Request, change);
    }

    // The projection-stage reply has already landed. Reusing its ticket is a Runner drop,
    // not another composerEditDone invocation like the old hand-called fixture supplied.
    fn duplicate(
        runner: &mut Runner<Source>,
        layout: Layout,
        stage: Stage,
        change: Change,
        ticket: u64,
    ) {
        let now = runner.now_ms();
        let answer = edit_reply(runner.plan(), 0);
        assert!(runner.fulfill(ticket, response(&answer)).unwrap().is_none());
        calls(runner, vec![]);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_eq!(runner.now_ms(), now);
        assert_cleared(runner, layout, stage, change);
        let continuation = runner.land_then();
        assert!(continuation.error.is_none(), "{continuation:?}");
        assert_eq!(continuation.now_ms, now);
        assert!(continuation.receipts.is_empty());
        calls(runner, vec![]);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_cleared(runner, layout, stage, change);
    }

    #[test]
    fn six_manual_changes_retire_waiting_intents_before_late_or_spent_replies() {
        for (bytes, layout) in plans() {
            for stage in [Stage::Request, Stage::Projection] {
                for change in [Change::Input, Change::Selection, Change::Composition] {
                    let mut runner = boot(bytes, layout, false);
                    keys(&mut runner, layout, 2);
                    let (ticket, args) = pump(&mut runner, layout, 0);
                    if stage.projection() {
                        land_edit(&mut runner, layout, 0, ticket, &args);
                        assert!(input(&mut runner, layout, 0, 5, false).is_none());
                    }
                    invalidate(&mut runner, layout, stage, change, ticket);
                    if !stage.projection() {
                        late_request(&mut runner, layout, change, ticket, &args);
                    }
                    duplicate(&mut runner, layout, stage, change, ticket);
                    quiet(&mut runner, "", layout, None);
                    assert_cleared(&runner, layout, stage, change);
                }
            }
        }
    }
}
