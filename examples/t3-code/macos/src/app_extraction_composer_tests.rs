//! Composer field ACK and deferred focus through the compiled app plan.
//! Typed field commands/ACKs isolate the Contract consumer from the TypeScript queue producer.
//! Native image removal, its confirmation handoff, and execution of host edits
//! are outside this case.

use super::composer_accessibility_tests::set;
use super::contract_keys_tests::empty;
use crate::contract_keys_tests::extraction::{
    call, element, field, fields, mutation, resource, Call,
};
use exact_kernel::{Kernel, ViewId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::geometry::{GeometryAnswer, GeometryLinks, Scrolled};
use exact_runner::{
    Answer, Command, ControlValue, DataError, DataSource, Delivery, Event, FieldSelection,
    InputDetails, Outcome, Request, RequestOut, Response, Runner, RunnerLinks, Store, Viewport,
};
use std::cell::RefCell;
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
const TARGET: &str = "preview-image-next";
const TOPIC: &str = "t3.notify";

#[derive(Clone, Copy, Debug)]
enum Layout {
    Original,
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Changed {
    Source,
    Owner,
    Snapshot,
    Generation,
    Composition,
    Cover,
}

#[derive(Clone, Copy, Debug)]
enum Stale {
    Source,
    Serial,
    Selection,
}

fn image(plan: &Plan, id: &str) -> Value {
    let (ty, _) = resource(plan, "data");
    fields(
        plan,
        element(plan, ty, "snapshotDrafts"),
        &[("id", Value::str(id)), ("name", Value::str(id))],
    )
}

// These are resource reply constructors, never direct assignments to Runner state.
fn snapshot(plan: &Plan, images: &[&str], change: Option<Changed>) -> Value {
    let (ty, _) = resource(plan, "data");
    fields(
        plan,
        ty,
        &[
            (
                "revision",
                Value::Number(if change == Some(Changed::Generation) {
                    2.0
                } else {
                    1.0
                }),
            ),
            ("connected", Value::Bool(true)),
            ("sidebarOpen", Value::Bool(true)),
            (
                "composerOwner",
                Value::str(if change == Some(Changed::Owner) {
                    "another-draft"
                } else {
                    "draft"
                }),
            ),
            (
                "snapshotOwner",
                Value::str(if change == Some(Changed::Snapshot) {
                    "another-snapshot"
                } else {
                    "snapshot"
                }),
            ),
            ("draft", Value::str("after")),
            (
                "snapshotDrafts",
                Value::list(images.iter().map(|id| image(plan, id)).collect()),
            ),
        ],
    )
}

struct Source {
    schema: Plan,
    ready: bool,
    values: HashMap<String, Value>,
    snapshot: Value,
    hold_snapshot: bool,
    generation_changes: bool,
    second_command: bool,
    ack_replies: usize,
    preview_observation: Option<Value>,
    calls: Vec<Call>,
}

impl Source {
    fn record(&mut self, method: &'static str, source: &str, args: &[Value]) {
        self.calls.push(call(method, source, args));
    }

    fn observation_field(&self, observation: &Value, name: &str) -> Value {
        let observation_ty = self
            .schema
            .mutations
            .iter()
            .find(|row| self.schema.str(row.name) == "composerFieldAck")
            .unwrap()
            .ty;
        // The ACK and observation share named scalar fields, but not record order.
        let ack_index = self.schema.field_index(observation_ty, name).unwrap();
        let ack_field = self
            .schema
            .type_(observation_ty)
            .fields
            .iter()
            .nth(ack_index)
            .unwrap();
        let kind = self.schema.field(ack_field).ty;
        let observation_value = match name {
            "generation" => 0,
            "snapshotOwner" => 1,
            "owner" => 2,
            "value" => 3,
            "start" => 4,
            "end" => 5,
            "inputSerial" => 9,
            _ => panic!("unregistered observation field"),
        };
        let Value::Record(values) = observation else {
            panic!("field observation")
        };
        assert_eq!(
            values.len(),
            10,
            "the declared ComposerFieldObservation fields"
        );
        let value = values[observation_value].clone();
        assert!(value.conforms(&self.schema, kind));
        value
    }

    fn next(&self, args: &[Value]) -> Value {
        assert_eq!(args.len(), 3);
        let (ty, _) = resource(&self.schema, "composerFieldNext");
        let after = args[1].as_number().unwrap();
        let serial = self
            .observation_field(&args[0], "inputSerial")
            .as_number()
            .unwrap();
        let seq = if after == 0.0 && serial >= 1.0 {
            1.0
        } else if self.second_command && self.ack_replies == 1 && after == 1.0 {
            2.0
        } else {
            after
        };
        let issued = seq > after;
        let first = seq == 1.0;
        fields(
            &self.schema,
            ty,
            &[
                ("generation", self.observation_field(&args[0], "generation")),
                (
                    "snapshotOwner",
                    self.observation_field(&args[0], "snapshotOwner"),
                ),
                ("owner", self.observation_field(&args[0], "owner")),
                (
                    "value",
                    if issued {
                        self.observation_field(&args[0], "value")
                    } else {
                        Value::str("")
                    },
                ),
                ("seq", Value::Number(seq)),
                ("ready", Value::Bool(issued)),
                (
                    "start",
                    Value::Number(if issued && !first { 2.0 } else { 0.0 }),
                ),
                (
                    "end",
                    Value::Number(if !issued {
                        0.0
                    } else if first {
                        6.0
                    } else {
                        2.0
                    }),
                ),
                (
                    "text",
                    Value::str(if first && issued { "after" } else { "" }),
                ),
                (
                    "selectionStart",
                    Value::Number(if issued { 2.0 } else { 0.0 }),
                ),
                (
                    "selectionEnd",
                    Value::Number(if issued { 2.0 } else { 0.0 }),
                ),
            ],
        )
    }

    fn value(&self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let (_, snapshot_source) = resource(&self.schema, "data");
        let (_, field_source) = resource(&self.schema, "composerFieldNext");
        let (rich_ty, rich_source) = resource(&self.schema, "richComposer");
        if source == snapshot_source {
            return Ok(self.snapshot.clone());
        }
        if source == field_source {
            return Ok(self.next(args));
        }
        if source == rich_source {
            assert_eq!(args.len(), 6);
            let (data_ty, _) = resource(&self.schema, "data");
            let revision = field(&self.schema, data_ty, &self.snapshot, "revision");
            let generation = if self.generation_changes
                && revision == Value::Number(2.0)
                && args[1] == Value::str("draft")
            {
                4.0
            } else {
                3.0
            };
            return Ok(fields(
                &self.schema,
                rich_ty,
                &[
                    ("generation", Value::Number(generation)),
                    ("owner", args[1].clone()),
                    ("value", args[2].clone()),
                    ("mode", Value::str("rich")),
                ],
            ));
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
        self.record("query", source, args);
        self.value(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.record("answer", source, args);
        let (_, snapshot_source) = resource(&self.schema, "data");
        let (_, field_source) = resource(&self.schema, "composerFieldNext");
        if source == snapshot_source || source == field_source {
            store.observe_topic(TOPIC);
        }
        if source == snapshot_source && self.hold_snapshot {
            return Ok(Answer::Later(Request::get(
                "https://fixture.invalid/snapshot",
            )));
        }
        if source == "composerFieldAcknowledged" {
            return Ok(Answer::Later(Request::get("https://fixture.invalid/ack")));
        }
        if source == "composerFieldObserved" {
            let ty = mutation(&self.schema, "composerFieldChanged");
            return Ok(Answer::Now(fields(
                &self.schema,
                ty,
                &[
                    ("value", self.observation_field(&args[0], "value")),
                    ("start", self.observation_field(&args[0], "start")),
                    ("end", self.observation_field(&args[0], "end")),
                ],
            )));
        }
        if source == "command" {
            assert_eq!(args.len(), 4);
            assert_eq!(
                args[0],
                Value::str("draft"),
                "only the public input's draft write"
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
            assert_eq!(args.len(), 1);
            self.preview_observation = Some(args[0].clone());
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
        self.record("parse", source, args);
        let Outcome::Response(reply) = outcome else {
            panic!("a real held-ticket response")
        };
        assert_eq!(reply.status, 200);
        assert!(reply.headers.is_empty());
        let value = Value::from_bytes(&reply.body).unwrap();
        let (data_ty, data_source) = resource(&self.schema, "data");
        if source == "composerFieldAcknowledged" {
            assert!(value.conforms(&self.schema, mutation(&self.schema, "composerFieldAck")));
            self.ack_replies += 1;
            self.hold_snapshot = true;
        } else {
            assert_eq!(source, data_source);
            assert!(value.conforms(&self.schema, data_ty));
            self.snapshot = value.clone();
            self.hold_snapshot = false;
        }
        Ok(Answer::Now(value))
    }
}

#[derive(Default)]
struct GeometryState {
    frames: Vec<String>,
    zero_target: bool,
}

thread_local! {
    static GEOMETRY: RefCell<GeometryState> = RefCell::new(GeometryState::default());
}

fn frame(kernel: &Kernel, _: &Scrolled, view: ViewId) -> GeometryAnswer {
    let node = kernel.node(view).unwrap();
    let id = node
        .props
        .iter()
        .find(|(id, _)| id.name() == "id")
        .and_then(|(_, value)| value.as_str())
        .unwrap_or("")
        .to_owned();
    if !id.is_empty() {
        let keys = kernel.find_by_id(&id);
        assert_eq!(keys.len(), 1);
        assert_eq!(kernel.node_by_key(keys[0]).unwrap().id, view);
    }
    let zero = GEOMETRY.with(|state| {
        let mut state = state.borrow_mut();
        state.frames.push(id.clone());
        state.zero_target && id == TARGET
    });
    GeometryAnswer {
        width: if zero { 0.0 } else { 20.0 },
        height: 20.0,
        ..Default::default()
    }
}

fn measure(kernel: &mut Kernel, scrolled: &Scrolled, view: ViewId) -> GeometryAnswer {
    frame(kernel, scrolled, view)
}

fn point(_: &Kernel, _: &Scrolled, _: f64, _: f64) -> Option<ViewId> {
    None
}
fn points(_: &Kernel, _: &Scrolled, _: f64, _: f64) -> Vec<ViewId> {
    vec![]
}
fn shown(_: &Kernel, _: &Scrolled, _: ViewId, _: bool) -> bool {
    true
}
static GEOMETRY_LINKS: GeometryLinks = GeometryLinks::new(frame, measure, point, points, shown);

fn geometry_take() -> Vec<String> {
    GEOMETRY.with(|state| std::mem::take(&mut state.borrow_mut().frames))
}

fn slot(runner: &Runner<Source>, layout: Layout, name: &str) -> Value {
    let projection = match name {
        "composerFieldInputSerial" => Some(("fieldProtocol", "inputSerial")),
        "composerFieldAfter" => Some(("fieldProtocol", "after")),
        "composerFieldAcked" => Some(("fieldProtocol", "acked")),
        "composerFieldEffects" => Some(("fieldProtocol", "effects")),
        "composerFieldFocusDone" => Some(("fieldProtocol", "focusDone")),
        "composerFieldFocus" => Some(("fieldProtocol", "focus")),
        "composerSelectionOwner" => Some(("composerSelection", "owner")),
        "composerSelectionValue" => Some(("composerSelection", "value")),
        "composerSelectionStart" => Some(("composerSelection", "start")),
        "composerSelectionEnd" => Some(("composerSelection", "end")),
        "composerComposing" => Some(("composerDraft", "composing")),
        "modelsOpen" => Some(("composerMenu", "modelsOpen")),
        _ => None,
    };
    let Some((record, member)) = projection.filter(|_| matches!(layout, Layout::Selected)) else {
        return runner.slot(name).unwrap().clone();
    };
    let plan = runner.plan();
    let row = plan
        .slots
        .iter()
        .find(|row| plan.str(row.name) == record)
        .unwrap();
    field(plan, row.ty, runner.slot(record).unwrap(), member)
}

fn focus_seq(runner: &Runner<Source>, layout: Layout) -> Value {
    field(
        runner.plan(),
        mutation(runner.plan(), "composerFieldAck"),
        &slot(runner, layout, "composerFieldFocus"),
        "seq",
    )
}

fn node(runner: &Runner<Source>, id: &str) -> u32 {
    let keys = runner.kernel().find_by_test_id(id);
    assert_eq!(keys.len(), 1, "one mounted {id}");
    runner.kernel().node_by_key(keys[0]).unwrap().id
}

fn node_id(runner: &Runner<Source>, id: &str) -> u32 {
    let keys = runner.kernel().find_by_id(id);
    assert_eq!(keys.len(), 1, "one mounted id {id}");
    runner.kernel().node_by_key(keys[0]).unwrap().id
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

fn command(name: &str, args: Vec<Value>) -> Command {
    Command {
        name: name.to_owned(),
        args,
        source: None,
    }
}

fn response(value: &Value) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: value.to_bytes(),
    })
}

fn task_due(runner: &Runner<Source>, name: &str) -> Option<f64> {
    runner
        .tasks()
        .into_iter()
        .find(|(found, _)| found == name)
        .unwrap()
        .1
}

#[derive(Debug)]
struct Setup {
    phase: &'static str,
    calls: Vec<Call>,
    hosts: Vec<Command>,
}

// Preparations retain their full traffic and admit only these named production sources.
// They do not execute the TypeScript field bridge or any host command.
fn setup_trace(runner: &mut Runner<Source>, phase: &'static str) -> Setup {
    let trace = Setup {
        phase,
        calls: std::mem::take(&mut runner.data().calls),
        hosts: runner.take_commands(),
    };
    for observed in &trace.calls {
        assert_eq!(observed.method, "answer", "{trace:?}");
        let known = runner
            .plan()
            .resources
            .iter()
            .any(|row| runner.plan().str(row.source) == observed.source)
            || [
                "command",
                "composerFieldObserved",
                "transportPump",
                "composerChipObserved",
            ]
            .contains(&observed.source.as_str());
        assert!(known, "{trace:?}");
        if observed.source == "command" {
            assert_eq!(observed.args[0], Value::str("draft"), "{trace:?}");
        }
    }
    let expected = if trace.phase == "boot" {
        vec![
            command("setRootFontSize", vec![Value::Number(16.0)]),
            command("close", vec![Value::str("server-browser-dialog-")]),
            command("close", vec![Value::str("server-browser-dialog-")]),
        ]
    } else {
        vec![]
    };
    assert_eq!(trace.hosts, expected, "{trace:?}");
    trace
}

fn setup(runner: &mut Runner<Source>, phase: &'static str) -> Setup {
    no_requests(runner);
    setup_trace(runner, phase)
}

fn setup_held_ack(runner: &mut Runner<Source>, ticket: u64) -> Setup {
    assert!(
        runner.take_requests().is_empty(),
        "all outgoing preparation requests"
    );
    assert_eq!(
        runner.pending(),
        vec![("composerFieldAck".to_owned(), ticket)]
    );
    assert!(runner.queued().is_empty());
    setup_trace(runner, "stale-ack-observation")
}

fn input(runner: &mut Runner<Source>, value: &str, at: u32, composing: bool) {
    let view = node(runner, "composer");
    assert!(runner.handlers_of(view).contains(&EventKind::Input));
    runner
        .dispatch(
            view,
            Event::Input(ControlValue::Edit(
                value.to_owned(),
                FieldSelection::caret(at),
                InputDetails {
                    input_type: "insertReplacementText".to_owned(),
                    data: Some(value.to_owned()),
                    cancelable: false,
                    is_composing: composing,
                },
            )),
        )
        .unwrap();
}

fn select(runner: &mut Runner<Source>, at: u32) {
    let view = node(runner, "composer");
    assert!(runner
        .handlers_of(view)
        .contains(&EventKind::Selectionchange));
    runner
        .dispatch(
            view,
            Event::FieldSelectionChange("after".to_owned(), FieldSelection::caret(at)),
        )
        .unwrap();
}

fn boot(bytes: &[u8], second_command: bool, generation_changes: bool) -> Runner<Source> {
    GEOMETRY.with(|state| *state.borrow_mut() = GeometryState::default());
    let plan = Plan::decode(bytes).expect("one independently compiled complete graph");
    assert!(
        plan.mutations
            .iter()
            .find(|row| plan.str(row.name) == "composerFieldAck")
            .unwrap()
            .queue
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
    let mut initial = snapshot(&plan, &["removed", "next"], None);
    let (ty, _) = resource(&plan, "data");
    set(&plan, ty, &mut initial, &["draft"], Value::str("before"));
    let data = Source {
        schema: Plan::decode(bytes).unwrap(),
        ready: false,
        values,
        snapshot: initial,
        hold_snapshot: false,
        generation_changes,
        second_command,
        ack_replies: 0,
        preview_observation: None,
        calls: vec![],
    };
    let links = RunnerLinks {
        geometry: Some(&GEOMETRY_LINKS),
        ..RunnerLinks::ALL
    };
    let mut runner = Runner::boot_with_delivery_linked(
        links,
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
    assert_eq!(task_due(&runner, "rootFont"), Some(1.0));
    assert_eq!(task_due(&runner, "streamClosedDialog"), Some(1.0));
    assert_eq!(task_due(&runner, "streamClosedMiniDialog"), Some(1.0));
    let mounted = runner.advance_timed(2.0);
    assert!(mounted.error.is_none(), "{mounted:?}");
    assert_eq!(mounted.now_ms, 2.0);
    let boot_trace = setup(&mut runner, "boot");
    assert!(!boot_trace.calls.is_empty());
    assert!(runner.watched_topics().iter().any(|topic| topic == TOPIC));
    node(&runner, "composer");
    node_id(&runner, TARGET);
    assert_eq!(task_due(&runner, "composerFieldDeliver"), None);
    geometry_take();
    runner
}

fn source_call(runner: &Runner<Source>, name: &str) -> Call {
    let (_, source) = resource(runner.plan(), name);
    call("answer", &source, runner.resource_args(name).unwrap())
}

fn background_calls(runner: &Runner<Source>, until: f64) -> Vec<Call> {
    let mut expected = Vec::new();
    if let Some(due) = task_due(runner, "transportAnswered").filter(|due| *due <= until) {
        expected.push(call(
            "answer",
            "transportPump",
            &[Value::Number(EPOCH + due), Value::str("")],
        ));
    }
    if task_due(runner, "promptPreviewLifecycle").is_some_and(|due| due <= until) {
        // A snapshot-owner change also gives the hidden preview a new owner.
        // Copy its settled earlier observation; only this lifecycle's identity/serial change.
        let Value::Record(old) = runner.data_ref().preview_observation.as_ref().unwrap() else {
            panic!("the warm preview observation")
        };
        let mut observation = old.to_vec();
        assert_eq!(observation.len(), 10);
        let plan = runner.plan();
        let (data_ty, _) = resource(plan, "data");
        observation[1] = field(
            plan,
            data_ty,
            runner.resource("data").unwrap(),
            "snapshotOwner",
        );
        let (preview_ty, _) = resource(plan, "richPromptPreview");
        observation[2] = field(
            plan,
            preview_ty,
            runner.resource("richPromptPreview").unwrap(),
            "owner",
        );
        observation[9] = Value::Number(old[9].as_number().unwrap() + 1.0);
        expected.push(call(
            "answer",
            "composerChipObserved",
            &[Value::record(observation)],
        ));
        expected.push(source_call(runner, "chip"));
    }
    expected
}

fn apply(runner: &mut Runner<Source>, layout: Layout, seq: f64, first: bool) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task_due(runner, "composerFieldDeliver"), Some(due));
    let mut expected = background_calls(runner, due);
    let mut next = runner.resource_args("composerFieldNext").unwrap().to_vec();
    assert_eq!(next.len(), 3);
    next[1] = Value::Number(seq);
    let (_, source) = resource(runner.plan(), "composerFieldNext");
    expected.push(call("answer", &source, &next));
    geometry_take();
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_requests(runner);
    hosts(
        runner,
        vec![
            command(
                "setRangeText",
                vec![
                    Value::str("composer"),
                    Value::str(if first { "after" } else { "" }),
                    Value::Number(if first { 0.0 } else { 2.0 }),
                    Value::Number(if first { 6.0 } else { 2.0 }),
                    Value::str("preserve"),
                ],
            ),
            command(
                "setSelectionRange",
                vec![
                    Value::str("composer"),
                    Value::Number(2.0),
                    Value::Number(2.0),
                ],
            ),
        ],
    );
    assert_eq!(geometry_take(), vec!["composer".to_owned()]);
    assert_eq!(
        slot(runner, layout, "composerFieldAfter"),
        Value::Number(seq)
    );
}

fn ack_ticket(runner: &mut Runner<Source>, layout: Layout, seq: f64) -> (u64, Vec<Value>) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task_due(runner, "composerFieldConfirm"), Some(due));
    let observation = runner.resource_args("composerFieldNext").unwrap()[0].clone();
    let args = vec![observation, Value::Number(seq), Value::Bool(true)];
    let mut expected = background_calls(runner, due);
    expected.push(call("answer", "composerFieldAcknowledged", &args));
    geometry_take();
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    hosts(runner, vec![]);
    assert_eq!(geometry_take(), vec!["composer".to_owned()]);
    let requests = runner.take_requests();
    assert_eq!(requests.len(), 1);
    let ticket = requests[0].ticket;
    assert_eq!(
        requests,
        vec![RequestOut {
            ticket,
            target: "composerFieldAck".to_owned(),
            request: Request::get("https://fixture.invalid/ack"),
            forced: false,
        }]
    );
    assert_eq!(
        runner.pending(),
        vec![("composerFieldAck".to_owned(), ticket)]
    );
    assert!(runner.queued().is_empty());
    assert_eq!(
        slot(runner, layout, "composerFieldAcked"),
        Value::Number(seq)
    );
    (ticket, args)
}

fn begin(
    bytes: &[u8],
    layout: Layout,
    second: bool,
    generation_changes: bool,
) -> (Runner<Source>, u64, Vec<Value>) {
    let mut runner = boot(bytes, second, generation_changes);
    input(&mut runner, "before", 2, false);
    let _trace = setup(&mut runner, "initial-input");
    assert_eq!(
        slot(&runner, layout, "composerFieldInputSerial"),
        Value::Number(1.0)
    );
    assert_eq!(
        slot(&runner, layout, "composerSelectionStart"),
        Value::Number(2.0)
    );
    apply(&mut runner, layout, 1.0, true);
    // A synthesized independent native input acknowledges the commanded replacement.
    // Runner does not execute setRangeText or assume its host supplied this event.
    input(&mut runner, "after", 2, false);
    let _trace = setup(&mut runner, "generated-input");
    assert_eq!(
        slot(&runner, layout, "composerFieldInputSerial"),
        Value::Number(2.0)
    );
    let (ticket, args) = ack_ticket(&mut runner, layout, 1.0);
    assert_eq!(
        slot(&runner, layout, "composerFieldEffects"),
        Value::Number(0.0)
    );
    assert_eq!(focus_seq(&runner, layout), Value::Number(0.0));
    (runner, ticket, args)
}

fn ack(
    runner: &Runner<Source>,
    seq: f64,
    applied: bool,
    focus: &str,
    then: &str,
    serial: f64,
) -> Value {
    fields(
        runner.plan(),
        mutation(runner.plan(), "composerFieldAck"),
        &[
            ("applied", Value::Bool(applied)),
            ("value", Value::str("after")),
            ("start", Value::Number(2.0)),
            ("end", Value::Number(2.0)),
            ("generation", Value::Number(3.0)),
            ("snapshotOwner", Value::str("snapshot")),
            ("owner", Value::str("draft")),
            ("seq", Value::Number(seq)),
            ("then", Value::str(then)),
            ("focus", Value::str(focus)),
            (
                "removedImage",
                Value::str(if focus.is_empty() { "" } else { "removed" }),
            ),
            ("inputSerial", Value::Number(serial)),
        ],
    )
}

// The async snapshot read retains its previous value while the ACK continuation runs.
fn land_ack(runner: &mut Runner<Source>, ticket: u64, args: &[Value], answer: Value) -> u64 {
    let mut expected = vec![call("parse", "composerFieldAcknowledged", args)];
    for row in &runner.plan().resources {
        let name = runner.plan().str(row.name);
        if [
            "data",
            "editor",
            "chip",
            "richComposer",
            "shell",
            "composerFieldNext",
        ]
        .contains(&name)
        {
            expected.push(source_call(runner, name));
        }
    }
    assert_eq!(expected.len(), 7);
    let now = runner.now_ms();
    geometry_take();
    assert!(runner.fulfill(ticket, response(&answer)).unwrap().is_some());
    calls(runner, expected);
    hosts(runner, vec![]);
    assert!(geometry_take().is_empty());
    let requests = runner.take_requests();
    assert_eq!(requests.len(), 1);
    let data_ticket = requests[0].ticket;
    assert_eq!(
        requests,
        vec![RequestOut {
            ticket: data_ticket,
            target: "data".to_owned(),
            request: Request::get("https://fixture.invalid/snapshot"),
            forced: true,
        }]
    );
    assert_eq!(runner.pending(), vec![("data".to_owned(), data_ticket)]);
    assert!(runner.queued().is_empty());
    let continuation = runner.land_then();
    assert!(continuation.error.is_none(), "{continuation:?}");
    assert_eq!(continuation.now_ms, now);
    assert_eq!(continuation.receipts.len(), 1);
    assert_eq!(continuation.receipts[0].at_ms, now);
    calls(runner, vec![]);
    assert!(runner.take_requests().is_empty());
    hosts(runner, vec![]);
    assert!(geometry_take().is_empty());
    data_ticket
}

fn accepted(bytes: &[u8], layout: Layout, second: bool, generation: bool) -> (Runner<Source>, u64) {
    let (mut runner, ticket, args) = begin(bytes, layout, second, generation);
    let answer = ack(&runner, 1.0, true, TARGET, "", 2.0);
    let data_ticket = land_ack(&mut runner, ticket, &args, answer.clone());
    assert_eq!(
        slot(&runner, layout, "composerFieldEffects"),
        Value::Number(1.0)
    );
    assert_eq!(slot(&runner, layout, "composerFieldFocus"), answer);
    assert_eq!(
        slot(&runner, layout, "composerFieldFocusDone"),
        Value::Number(0.0)
    );
    assert_eq!(
        runner.derive("composerFieldFocusCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(task_due(&runner, "composerFieldRestoreFocus"), None);
    (runner, data_ticket)
}

// This is an actual read response, not an injected resource value or state reset.
// Context-change replies have their own retained preparation trace.
fn land_snapshot(
    runner: &mut Runner<Source>,
    ticket: u64,
    images: &[&str],
    change: Option<Changed>,
) -> Setup {
    let value = snapshot(runner.plan(), images, change);
    let (_, source) = resource(runner.plan(), "data");
    let args = runner.resource_args("data").unwrap().to_vec();
    geometry_take();
    assert!(runner.fulfill(ticket, response(&value)).unwrap().is_some());
    assert!(runner.take_requests().is_empty());
    assert!(runner.pending().is_empty());
    assert!(runner.queued().is_empty());
    let trace = Setup {
        phase: "snapshot-response",
        calls: std::mem::take(&mut runner.data().calls),
        hosts: runner.take_commands(),
    };
    assert_eq!(trace.calls.first(), Some(&call("parse", &source, &args)));
    for observed in &trace.calls[1..] {
        assert_eq!(observed.method, "answer", "{trace:?}");
        assert!(
            runner
                .plan()
                .resources
                .iter()
                .any(|row| runner.plan().str(row.source) == observed.source),
            "{trace:?}"
        );
    }
    assert!(trace.hosts.is_empty(), "{trace:?}");
    assert!(geometry_take().is_empty());
    if change.is_none() {
        assert_eq!(trace.calls, vec![call("parse", &source, &args)]);
    }
    trace
}

#[derive(Clone, Copy, Debug)]
enum FocusCase {
    Current,
    Invalid,
    Missing,
    ZeroWidth,
}

fn focus_step(runner: &mut Runner<Source>, layout: Layout, seq: f64, case: FocusCase) {
    let current = !matches!(case, FocusCase::Invalid);
    let current_value = Value::Bool(current);
    let expected_current = Some(&current_value);
    assert_eq!(
        runner.derive("composerFieldFocusCurrent"),
        expected_current,
        "{case:?}"
    );
    assert_eq!(focus_seq(runner, layout), Value::Number(seq));
    let previous = slot(runner, layout, "composerFieldFocus");
    let due = runner.now_ms() + 1.0;
    assert_eq!(task_due(runner, "composerFieldRestoreFocus"), Some(due));
    let expected = background_calls(runner, due);
    let expected_geometry = if matches!(case, FocusCase::Current | FocusCase::ZeroWidth) {
        vec![TARGET.to_owned()]
    } else {
        vec![]
    };
    geometry_take();
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_requests(runner);
    hosts(
        runner,
        if matches!(case, FocusCase::Current) {
            vec![command("focus", vec![Value::str(TARGET)])]
        } else {
            vec![]
        },
    );
    // A missing ID is rejected before this callback; silence is not an attempted-read count.
    assert_eq!(geometry_take(), expected_geometry, "{case:?}");
    assert_eq!(
        runner.derive("composerFieldFocusCurrent"),
        expected_current,
        "{case:?}"
    );
    assert_eq!(slot(runner, layout, "composerFieldFocus"), previous);
    assert_eq!(focus_seq(runner, layout), Value::Number(seq));
    assert_eq!(
        slot(runner, layout, "composerFieldFocusDone"),
        Value::Number(seq)
    );
    assert_eq!(task_due(runner, "composerFieldRestoreFocus"), None);
    let repeated_calls = background_calls(runner, due + 1.0);
    let repeat = runner.advance_timed(due + 1.0);
    assert!(repeat.error.is_none(), "{repeat:?}");
    calls(runner, repeated_calls);
    no_requests(runner);
    hosts(runner, vec![]);
    assert!(geometry_take().is_empty());
    assert_eq!(slot(runner, layout, "composerFieldFocus"), previous);
    assert_eq!(
        slot(runner, layout, "composerFieldFocusDone"),
        Value::Number(seq)
    );
    assert_eq!(task_due(runner, "composerFieldRestoreFocus"), None);
}

fn wait_step(runner: &mut Runner<Source>, layout: Layout, seq: f64, data_ticket: u64) {
    let previous = slot(runner, layout, "composerFieldFocus");
    assert_eq!(focus_seq(runner, layout), Value::Number(seq));
    assert_eq!(
        slot(runner, layout, "composerFieldFocusDone"),
        Value::Number(0.0)
    );
    assert_eq!(
        runner.derive("composerFieldFocusCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(task_due(runner, "composerFieldRestoreFocus"), None);
    let due = runner.now_ms() + 1.0;
    let expected = background_calls(runner, due);
    geometry_take();
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    assert!(runner.take_requests().is_empty());
    assert_eq!(runner.pending(), vec![("data".to_owned(), data_ticket)]);
    assert!(runner.queued().is_empty());
    hosts(runner, vec![]);
    assert!(geometry_take().is_empty());
    assert_eq!(slot(runner, layout, "composerFieldFocus"), previous);
    assert_eq!(focus_seq(runner, layout), Value::Number(seq));
    assert_eq!(
        slot(runner, layout, "composerFieldFocusDone"),
        Value::Number(0.0)
    );
    assert_eq!(
        runner.derive("composerFieldFocusCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(task_due(runner, "composerFieldRestoreFocus"), None);
}

fn change_public(runner: &mut Runner<Source>, change: Changed) {
    match change {
        Changed::Source => input(runner, "new manual input", 4, false),
        Changed::Composition => input(runner, "after", 2, true),
        Changed::Cover => {
            let view = node(runner, "connection-settings");
            runner.dispatch(view, Event::Press).unwrap();
        }
        _ => panic!("a data context changes in the actual pending snapshot response"),
    }
}

fn assert_context(runner: &Runner<Source>, layout: Layout, change: Changed) {
    match change {
        Changed::Source => assert_eq!(
            runner.derive("composerText"),
            Some(&Value::str("new manual input")),
        ),
        Changed::Owner => assert_eq!(
            runner.derive("richComposerOwner"),
            Some(&Value::str("another-draft")),
        ),
        Changed::Snapshot => {
            let (ty, _) = resource(runner.plan(), "data");
            assert_eq!(
                field(
                    runner.plan(),
                    ty,
                    runner.resource("data").unwrap(),
                    "snapshotOwner"
                ),
                Value::str("another-snapshot")
            );
        }
        Changed::Generation => {
            let (ty, _) = resource(runner.plan(), "richComposer");
            let value = field(
                runner.plan(),
                ty,
                runner.resource("richComposer").unwrap(),
                "generation",
            );
            assert_eq!(value, Value::Number(4.0));
        }
        Changed::Composition => {
            assert_eq!(slot(runner, layout, "composerComposing"), Value::Bool(true),)
        }
        Changed::Cover => assert_eq!(
            runner.derive("composerFieldLive"),
            Some(&Value::Bool(false)),
        ),
    }
}

fn second_ticket(
    mut runner: Runner<Source>,
    layout: Layout,
    data_ticket: u64,
) -> (Runner<Source>, u64, Vec<Value>) {
    // Boundary-only second edit: its empty replacement gets no generated Input.
    // A supplied applied ACK tests the consumer boundary, not TS queue acceptance.
    let _trace = land_snapshot(&mut runner, data_ticket, &["removed", "next"], None);
    apply(&mut runner, layout, 2.0, false);
    let (ticket, args) = ack_ticket(&mut runner, layout, 2.0);
    assert_eq!(
        slot(&runner, layout, "composerFieldInputSerial"),
        Value::Number(2.0)
    );
    (runner, ticket, args)
}

#[test]
fn image_presence_delays_focus_until_a_resource_reply_removes_it() {
    for (bytes, layout) in plans() {
        let (mut runner, ticket) = accepted(bytes, layout, false, false);
        wait_step(&mut runner, layout, 1.0, ticket);
        assert_eq!(
            slot(&runner, layout, "composerFieldFocusDone"),
            Value::Number(0.0)
        );
        let _trace = land_snapshot(&mut runner, ticket, &["next"], None);
        focus_step(&mut runner, layout, 1.0, FocusCase::Current);
        assert_eq!(
            slot(&runner, layout, "composerFieldEffects"),
            Value::Number(1.0)
        );
    }
}

#[test]
fn a_current_applied_ack_replaces_the_previous_deferred_focus() {
    for (bytes, layout) in plans() {
        let (runner, data_ticket) = accepted(bytes, layout, true, false);
        let (mut runner, ticket, args) = second_ticket(runner, layout, data_ticket);
        let previous = slot(&runner, layout, "composerFieldFocus");
        let answer = ack(&runner, 2.0, true, TARGET, "", 2.0);
        let data_ticket = land_ack(&mut runner, ticket, &args, answer.clone());
        assert_ne!(answer, previous);
        assert_eq!(slot(&runner, layout, "composerFieldFocus"), answer);
        assert_eq!(
            slot(&runner, layout, "composerFieldEffects"),
            Value::Number(2.0)
        );
        assert_eq!(
            slot(&runner, layout, "composerFieldFocusDone"),
            Value::Number(0.0)
        );
        wait_step(&mut runner, layout, 2.0, data_ticket);
        let _trace = land_snapshot(&mut runner, data_ticket, &["next"], None);
        focus_step(&mut runner, layout, 2.0, FocusCase::Current);
        assert_eq!(
            slot(&runner, layout, "composerFieldEffects"),
            Value::Number(2.0)
        );
    }
}

#[test]
fn six_public_context_changes_retire_pending_focus() {
    for (bytes, layout) in plans() {
        for change in [
            Changed::Source,
            Changed::Owner,
            Changed::Snapshot,
            Changed::Generation,
            Changed::Composition,
            Changed::Cover,
        ] {
            let (mut runner, ticket) =
                accepted(bytes, layout, false, change == Changed::Generation);
            match change {
                Changed::Source | Changed::Composition | Changed::Cover => {
                    // Input can change serial/selection as well as its named context.
                    change_public(&mut runner, change);
                    // A draft write can refresh and replace the held snapshot request.
                    let requests = runner.take_requests();
                    let current = if requests.is_empty() {
                        ticket
                    } else {
                        assert_eq!(requests.len(), 1);
                        let current = requests[0].ticket;
                        assert_eq!(
                            requests,
                            vec![RequestOut {
                                ticket: current,
                                target: "data".to_owned(),
                                request: Request::get("https://fixture.invalid/snapshot"),
                                forced: true,
                            }]
                        );
                        current
                    };
                    let trace = Setup {
                        phase: "public-context",
                        calls: std::mem::take(&mut runner.data().calls),
                        hosts: runner.take_commands(),
                    };
                    assert!(trace.hosts.is_empty(), "{trace:?}");
                    for observed in &trace.calls {
                        assert_eq!(observed.method, "answer", "{trace:?}");
                        assert!(
                            runner
                                .plan()
                                .resources
                                .iter()
                                .any(|row| runner.plan().str(row.source) == observed.source)
                                || ["command", "composerFieldObserved"]
                                    .contains(&observed.source.as_str()),
                            "{trace:?}"
                        );
                    }
                    let _trace = land_snapshot(&mut runner, current, &["next"], None);
                }
                _ => {
                    let _trace = land_snapshot(&mut runner, ticket, &["next"], Some(change));
                }
            }
            assert_context(&runner, layout, change);
            assert_eq!(
                runner.derive("composerFieldFocusCurrent"),
                Some(&Value::Bool(false)),
                "{change:?}"
            );
            focus_step(&mut runner, layout, 1.0, FocusCase::Invalid);
        }
    }
}

#[test]
fn same_value_input_and_a_changed_caret_retire_focus() {
    for (bytes, layout) in plans() {
        for serial_change in [true, false] {
            let (mut runner, ticket) = accepted(bytes, layout, false, false);
            // Land the image-preserving resource reply before the public observation changes.
            let _trace = land_snapshot(&mut runner, ticket, &["removed", "next"], None);
            if serial_change {
                input(&mut runner, "after", 2, false);
            } else {
                select(&mut runner, 4);
            }
            let _trace = setup(&mut runner, "public-observation");
            assert_eq!(
                slot(&runner, layout, "composerFieldInputSerial"),
                Value::Number(if serial_change { 3.0 } else { 2.0 })
            );
            assert_eq!(
                slot(&runner, layout, "composerSelectionStart"),
                Value::Number(if serial_change { 2.0 } else { 4.0 })
            );
            assert_eq!(
                runner.derive("composerFieldFocusCurrent"),
                Some(&Value::Bool(false))
            );
            focus_step(&mut runner, layout, 1.0, FocusCase::Invalid);
        }
    }
}

#[test]
fn a_newer_shaped_ack_retires_the_older_focus_without_an_input_serial_change() {
    for (bytes, layout) in plans() {
        let (runner, data_ticket) = accepted(bytes, layout, true, false);
        let (mut runner, ticket, args) = second_ticket(runner, layout, data_ticket);
        let answer = ack(&runner, 2.0, true, "", "", 2.0);
        let data_ticket = land_ack(&mut runner, ticket, &args, answer);
        assert_eq!(
            slot(&runner, layout, "composerFieldEffects"),
            Value::Number(2.0)
        );
        assert_eq!(
            slot(&runner, layout, "composerFieldInputSerial"),
            Value::Number(2.0)
        );
        assert_eq!(focus_seq(&runner, layout), Value::Number(1.0));
        let _trace = land_snapshot(&mut runner, data_ticket, &["next"], None);
        assert_eq!(
            runner.derive("composerFieldFocusCurrent"),
            Some(&Value::Bool(false))
        );
        focus_step(&mut runner, layout, 1.0, FocusCase::Invalid);
    }
}

#[test]
fn missing_and_mounted_zero_width_targets_refuse_focus() {
    for (bytes, layout) in plans() {
        for missing in [true, false] {
            let (mut runner, ticket) = accepted(bytes, layout, false, false);
            let images: &[&str] = if missing { &[] } else { &["next"] };
            let _trace = land_snapshot(&mut runner, ticket, images, None);
            if missing {
                assert!(runner.kernel().find_by_id(TARGET).is_empty());
            } else {
                node_id(&runner, TARGET);
                GEOMETRY.with(|state| state.borrow_mut().zero_target = true);
            }
            focus_step(
                &mut runner,
                layout,
                1.0,
                if missing {
                    FocusCase::Missing
                } else {
                    FocusCase::ZeroWidth
                },
            );
        }
    }
}

#[test]
fn three_stale_ack_variants_cannot_open_models_or_replace_existing_focus() {
    for (bytes, layout) in plans() {
        for stale in [Stale::Source, Stale::Serial, Stale::Selection] {
            let (runner, data_ticket) = accepted(bytes, layout, true, false);
            let (mut runner, ticket, args) = second_ticket(runner, layout, data_ticket);
            let previous = slot(&runner, layout, "composerFieldFocus");
            match stale {
                Stale::Source => input(&mut runner, "new input", 3, false),
                Stale::Serial => input(&mut runner, "after", 2, false),
                Stale::Selection => select(&mut runner, 3),
            }
            let _trace = setup_held_ack(&mut runner, ticket);
            assert_eq!(
                slot(&runner, layout, "composerFieldInputSerial"),
                Value::Number(if matches!(stale, Stale::Selection) {
                    2.0
                } else {
                    3.0
                })
            );
            assert_eq!(
                slot(&runner, layout, "composerSelectionStart"),
                Value::Number(if matches!(stale, Stale::Serial) {
                    2.0
                } else {
                    3.0
                })
            );
            let answer = ack(&runner, 2.0, true, "", "models", 2.0);
            let data_ticket = land_ack(&mut runner, ticket, &args, answer);
            assert_eq!(
                slot(&runner, layout, "composerFieldEffects"),
                Value::Number(1.0)
            );
            assert_eq!(slot(&runner, layout, "composerFieldFocus"), previous);
            assert_eq!(
                slot(&runner, layout, "modelsOpen"),
                Value::Bool(false),
                "{stale:?}"
            );
            let _trace = land_snapshot(&mut runner, data_ticket, &["removed", "next"], None);
            focus_step(&mut runner, layout, 1.0, FocusCase::Invalid);
        }
    }
}

#[test]
fn non_applied_ack_preserves_prior_focus_even_with_adversarial_effect_fields() {
    for (bytes, layout) in plans() {
        for adversarial in [true, false] {
            let (runner, data_ticket) = accepted(bytes, layout, true, false);
            let (mut runner, ticket, args) = second_ticket(runner, layout, data_ticket);
            let previous = slot(&runner, layout, "composerFieldFocus");
            let mut answer = ack(
                &runner,
                2.0,
                false,
                if adversarial { "queued-target" } else { "" },
                if adversarial { "models" } else { "" },
                if adversarial { 2.0 } else { -1.0 },
            );
            if !adversarial {
                let ty = mutation(runner.plan(), "composerFieldAck");
                set(
                    runner.plan(),
                    ty,
                    &mut answer,
                    &["reason"],
                    Value::str("field acknowledgement stale"),
                );
            }
            let data_ticket = land_ack(&mut runner, ticket, &args, answer);
            assert_eq!(
                slot(&runner, layout, "composerFieldEffects"),
                Value::Number(1.0)
            );
            assert_eq!(slot(&runner, layout, "composerFieldFocus"), previous);
            assert_eq!(slot(&runner, layout, "modelsOpen"), Value::Bool(false));
            let _trace = land_snapshot(&mut runner, data_ticket, &["removed", "next"], None);
            assert_eq!(task_due(&runner, "composerFieldRestoreFocus"), None);
            let due = runner.now_ms() + 1.0;
            let expected = background_calls(&runner, due);
            geometry_take();
            let advanced = runner.advance_timed(due);
            assert!(advanced.error.is_none(), "{advanced:?}");
            calls(&mut runner, expected);
            no_requests(&mut runner);
            hosts(&mut runner, vec![]);
            assert!(geometry_take().is_empty());
        }
    }
}
