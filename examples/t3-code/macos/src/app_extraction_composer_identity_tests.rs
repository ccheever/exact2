//! Identity and stale-serial consumer cases through the compiled app plan.
//! The TypeScript producer suite stays byte-identical and remains required.
//! The empty field command, non-applied ACK and identity replies are typed fixtures;
//! observed host edits and controlled geometry do not execute native operations.

use super::contract_keys_tests::empty;
use crate::contract_keys_tests::extraction::task;
use crate::contract_keys_tests::extraction::{
    call, field, fields, identity_edit_reply as edit_reply, identity_next_command as next_command,
    identity_observation as observation, identity_refusal as refusal,
    identity_snapshot as snapshot, mutation, observation_type, resource, Call,
};
use exact_kernel::{Kernel, ViewId};
use exact_plan::{EventKind, Plan, TypesId, Value};
use exact_runner::geometry::{GeometryAnswer, GeometryLinks, Scrolled};
use exact_runner::{
    Answer, Command, ControlValue, DataError, DataSource, Delivery, Event, FieldSelection,
    InputDetails, KeyboardEvent, Outcome, Request, RequestOut, Response, Runner, RunnerLinks,
    Store, Viewport,
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
const INTENT_URL: &str = "https://fixture.invalid/identity-intent";
const ACK_URL: &str = "https://fixture.invalid/identity-ack";
const DATA_URL: &str = "https://fixture.invalid/identity-snapshot";
const RICH_URL: &str = "https://fixture.invalid/identity-rich";
const NOTICE: &str = "The prompt changed. Please try that edit again.";
const PREVIEW: &str = concat!(
    "Use $frontend-design to fix the flaky test in ",
    "[surface.test.ts](apps/web/src/terminal/ghostty/surface.test.ts) and align the header with ",
    "[SettingsPanels.tsx](apps/web/src/components/settings/SettingsPanels.tsx) before shipping.",
);

#[derive(Clone, Copy, Debug)]
enum Layout {
    Original,
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Change {
    Owner,
    Generation,
    Mode,
    Snapshot,
}

impl Change {
    fn snapshot(self) -> &'static str {
        if self == Self::Snapshot {
            "next"
        } else {
            "snapshot"
        }
    }

    fn generation(self) -> f64 {
        if self == Self::Generation {
            2.0
        } else {
            1.0
        }
    }

    fn owner_timer(self) -> bool {
        matches!(self, Self::Generation | Self::Snapshot)
    }

    fn context(self) -> bool {
        self == Self::Owner
    }
}

// Constructors only build local typed reply Values, never Runner state or resource assignments.

fn rich(plan: &Plan, args: &[Value], change: Option<Change>) -> Value {
    assert_eq!(args.len(), 6);
    let changed = if args[5] == Value::str("composer") {
        change
    } else {
        None
    };
    fields(
        plan,
        resource(plan, "richComposer").0,
        &[
            (
                "generation",
                Value::Number(if changed == Some(Change::Generation) {
                    2.0
                } else {
                    1.0
                }),
            ),
            (
                "owner",
                if changed == Some(Change::Owner) {
                    Value::str("other")
                } else {
                    args[1].clone()
                },
            ),
            ("value", args[2].clone()),
            (
                "mode",
                Value::str(if changed == Some(Change::Mode) {
                    "plain"
                } else {
                    "rich"
                }),
            ),
        ],
    )
}

struct Source {
    schema: Plan,
    ready: bool,
    values: HashMap<String, Value>,
    snapshot: Value,
    change: Option<Change>,
    ack_answered: bool,
    data_delivered: bool,
    rich_delivered: bool,
    preview: Option<Value>,
    calls: Vec<Call>,
}

impl Source {
    fn value(&self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == resource(&self.schema, "data").1 {
            return Ok(self.snapshot.clone());
        }
        if source == resource(&self.schema, "richComposer").1 {
            return Ok(rich(
                &self.schema,
                args,
                if self.rich_delivered {
                    self.change
                } else {
                    None
                },
            ));
        }
        if source == resource(&self.schema, "composerFieldNext").1 {
            return Ok(next_command(&self.schema, args, self.change.is_some()));
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
        if source == "composerFieldAcknowledged" {
            return Ok(Answer::Later(Request::get(ACK_URL)));
        }
        if self.ack_answered && !self.data_delivered && source == resource(&self.schema, "data").1 {
            return Ok(Answer::Later(Request::get(DATA_URL)));
        }
        if self.ack_answered
            && !self.rich_delivered
            && source == resource(&self.schema, "richComposer").1
            && args[5] == Value::str("composer")
        {
            return Ok(Answer::Later(Request::get(RICH_URL)));
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
            assert_eq!(args[0], Value::str("draft"));
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
            self.preview = Some(args[0].clone());
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
            panic!("held request response")
        };
        assert_eq!(reply.status, 200);
        assert!(reply.headers.is_empty());
        let value = Value::from_bytes(&reply.body).unwrap();
        if source == "composerIntent" {
            assert!(value.conforms(&self.schema, mutation(&self.schema, "composerEdited")));
        } else if source == "composerFieldAcknowledged" {
            assert!(self.change.is_some() && !self.ack_answered);
            assert_eq!(value, refusal(&self.schema));
            self.ack_answered = true;
        } else if source == resource(&self.schema, "data").1 {
            assert!(self.ack_answered && !self.data_delivered);
            assert!(value.conforms(&self.schema, resource(&self.schema, "data").0));
            self.snapshot = value.clone();
            self.data_delivered = true;
        } else {
            assert_eq!(source, resource(&self.schema, "richComposer").1);
            assert!(self.ack_answered && !self.rich_delivered);
            assert_eq!(value, rich(&self.schema, args, self.change));
            self.rich_delivered = true;
        }
        Ok(Answer::Now(value))
    }
}

thread_local! { static FRAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }

fn frame(kernel: &Kernel, _: &Scrolled, view: ViewId) -> GeometryAnswer {
    let node = kernel.node(view).unwrap();
    let id = node
        .props
        .iter()
        .find(|(id, _)| id.name() == "id")
        .and_then(|(_, value)| value.as_str())
        .unwrap_or("")
        .to_owned();
    let keys = kernel.find_by_id(&id);
    assert_eq!(keys.len(), 1);
    assert_eq!(kernel.node_by_key(keys[0]).unwrap().id, view);
    FRAMES.with(|frames| frames.borrow_mut().push(id));
    GeometryAnswer {
        width: 20.0,
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

fn frames(expected: &[&str]) {
    let actual = FRAMES.with(|frames| std::mem::take(&mut *frames.borrow_mut()));
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>()
    );
}

fn command(name: &str, args: Vec<Value>, source: Option<ViewId>) -> Command {
    Command {
        name: name.to_owned(),
        args,
        source,
    }
}

fn node(runner: &Runner<Source>) -> ViewId {
    let keys = runner.kernel().find_by_test_id("composer");
    assert_eq!(keys.len(), 1);
    runner.kernel().node_by_key(keys[0]).unwrap().id
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

fn pending(runner: &Runner<Source>, expected: &[(&str, u64)]) {
    assert_eq!(
        runner.pending(),
        expected
            .iter()
            .map(|(name, ticket)| ((*name).to_owned(), *ticket))
            .collect::<Vec<_>>()
    );
    assert!(runner.queued().is_empty());
}

fn no_outgoing(runner: &mut Runner<Source>) {
    assert!(
        runner.take_requests().is_empty(),
        "complete outgoing vector"
    );
}

fn query_call(runner: &Runner<Source>, name: &str, args: &[Value]) -> Call {
    call("answer", &resource(runner.plan(), name).1, args)
}

fn standing_call(runner: &Runner<Source>, name: &str) -> Call {
    query_call(runner, name, runner.resource_args(name).unwrap())
}

fn background(runner: &Runner<Source>, until: f64) -> Vec<Call> {
    let mut expected = vec![];
    if let Some(due) = task(runner, "transportAnswered").filter(|due| *due <= until) {
        expected.push(call(
            "answer",
            "transportPump",
            &[Value::Number(EPOCH + due), Value::str("")],
        ));
    }
    expected
}

const INTENT: [(&str, &str); 14] = [
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
) -> (TypesId, Value) {
    crate::contract_keys_tests::extraction::logical_member(
        runner,
        matches!(layout, Layout::Original),
        original,
        record,
        name,
    )
}

fn intent_value(runner: &Runner<Source>, layout: Layout, index: usize) -> Value {
    member(
        runner,
        layout,
        INTENT[index].0,
        "intentState",
        INTENT[index].1,
    )
    .1
}

fn metadata(runner: &Runner<Source>, layout: Layout, first: usize) -> Value {
    let ty = member(runner, layout, INTENT[1].0, "intentState", "queue").0;
    let item = runner.plan().type_(ty).elem.unwrap();
    Value::list(
        (first..=2)
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
            .collect(),
    )
}

fn intent_state(runner: &Runner<Source>, layout: Layout, first: usize, phase: &str, notice: &str) {
    let second = first == 2;
    let edited = second || phase == "input" || phase == "projection";
    let received = second || phase == "projection";
    let expected = vec![
        Value::Number(2.0),
        metadata(runner, layout, first),
        Value::Number(if phase.is_empty() { 0.0 } else { first as f64 }),
        Value::str(phase),
        Value::str("rich"),
        Value::str(notice),
        observation(
            runner.plan(),
            "snapshot",
            1.0,
            if second { "word\n" } else { "word" },
            if second { 2.0 } else { 1.0 },
        ),
        Value::str(if edited { "word\n" } else { "" }),
        Value::Number(if edited { 5.0 } else { 0.0 }),
        Value::Number(if edited { 5.0 } else { 0.0 }),
        Value::Number(if received { 5.0 } else { 0.0 }),
        Value::Number(if received { 5.0 } else { 0.0 }),
        Value::Bool(false),
        Value::Bool(received),
    ];
    let actual: Vec<_> = (0..INTENT.len())
        .map(|index| intent_value(runner, layout, index))
        .collect();
    assert_eq!(actual, expected, "complete fixed intent record");
}

fn field_state(runner: &Runner<Source>, layout: Layout, change: Option<Change>, serial: f64) {
    let value = if serial == 1.0 { "word" } else { "word\n" };
    let owner = change.map_or("snapshot", Change::snapshot);
    let generation = change.map_or(1.0, Change::generation);
    let snapshot = observation(runner.plan(), owner, generation, value, serial);
    assert_eq!(runner.derive("composerText"), Some(&Value::str(value)));
    assert_eq!(runner.derive("composerFieldSnapshot"), Some(&snapshot));
    assert_eq!(
        runner.derive("composerSelectionCurrent"),
        Some(&Value::Bool(true))
    );
    for (original, record, name, expected) in [
        ("draft", "composerDraft", "value", Value::str(value)),
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
            Value::Bool(false),
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
            Value::str(value),
        ),
        (
            "composerSelectionStart",
            "composerSelection",
            "start",
            Value::Number(3.0 + serial),
        ),
        (
            "composerSelectionEnd",
            "composerSelection",
            "end",
            Value::Number(3.0 + serial),
        ),
        (
            "composerFieldInputSerial",
            "fieldProtocol",
            "inputSerial",
            Value::Number(serial),
        ),
    ] {
        assert_eq!(member(runner, layout, original, record, name).1, expected);
    }
}

fn field_protocol(runner: &Runner<Source>, layout: Layout, acked: f64) {
    crate::contract_keys_tests::extraction::identity_field_protocol(
        runner,
        matches!(layout, Layout::Original),
        acked,
    )
}

// Only initial mount and initial word/caret setup permit these finite named source calls.
// Every measured field, metadata, response, continuation and timer phase below is exact.
#[derive(Debug)]
struct Preparation {
    calls: Vec<Call>,
    hosts: Vec<Command>,
}

fn preparation(runner: &mut Runner<Source>, boot: bool) -> Preparation {
    no_outgoing(runner);
    pending(runner, &[]);
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
    assert_eq!(
        trace.hosts,
        if boot {
            vec![
                command("setRootFontSize", vec![Value::Number(16.0)], None),
                command("close", vec![Value::str("server-browser-dialog-")], None),
                command("close", vec![Value::str("server-browser-dialog-")], None),
            ]
        } else {
            vec![]
        },
        "{trace:?}"
    );
    frames(&[]);
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

fn boot(bytes: &[u8], layout: Layout, change: Option<Change>) -> Runner<Source> {
    FRAMES.with(|frames| frames.borrow_mut().clear());
    let plan = Plan::decode(bytes).expect("independently compiled complete graph");
    let edit = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "composerEdited")
        .unwrap();
    assert!(edit.queue && edit.refreshes.len == 0);
    let ack = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "composerFieldAck")
        .unwrap();
    assert!(ack.queue);
    let range = ack.refreshes;
    let mut refreshes: Vec<_> = plan.mutation_refreshes
        [range.start as usize..(range.start + range.len) as usize]
        .iter()
        .map(|row| plan.str(plan.resources[row.resource.0 as usize].name))
        .collect();
    refreshes.sort_unstable();
    assert_eq!(
        refreshes,
        vec![
            "chip",
            "composerFieldNext",
            "data",
            "editor",
            "richComposer",
            "shell",
        ]
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
    let data = Source {
        schema: Plan::decode(bytes).unwrap(),
        ready: false,
        values,
        snapshot: snapshot(&plan, "snapshot"),
        change,
        ack_answered: false,
        data_delivered: false,
        rich_delivered: false,
        preview: None,
        calls: vec![],
    };
    let mut runner = Runner::boot_with_delivery_linked(
        RunnerLinks {
            geometry: Some(&GEOMETRY_LINKS),
            ..RunnerLinks::ALL
        },
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
    for name in ["rootFont", "streamClosedDialog", "streamClosedMiniDialog"] {
        assert_eq!(task(&runner, name), Some(1.0));
    }
    let warm = runner.advance_timed(2.0);
    assert!(warm.error.is_none(), "{warm:?}");
    assert_eq!(warm.now_ms, 2.0);
    let boot_trace = preparation(&mut runner, true);
    assert!(!boot_trace.calls.is_empty());
    let view = node(&runner);
    assert!(runner.handlers_of(view).contains(&EventKind::Focus));
    runner.dispatch(view, Event::Focus).unwrap();
    let _focus = preparation(&mut runner, false);
    dispatch_input(&mut runner, "word", 4);
    let _input = preparation(&mut runner, false);
    field_state(&runner, layout, None, 1.0);
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(task(&runner, "composerIntentOwnerNotice"), None);
    assert_eq!(task(&runner, "promptPreviewLifecycle"), None);
    assert_eq!(task(&runner, "promptPreviewHidden"), None);
    assert_eq!(
        task(&runner, "composerFieldDeliver"),
        change.map(|_| runner.now_ms() + 1.0)
    );
    runner
}

fn expect_requests(runner: &mut Runner<Source>, recipe: &[(&str, &str, bool)]) -> Vec<u64> {
    let actual = runner.take_requests();
    assert_eq!(actual.len(), recipe.len());
    let expected: Vec<_> = actual
        .iter()
        .zip(recipe)
        .map(|(observed, (target, url, forced))| RequestOut {
            ticket: observed.ticket,
            target: (*target).to_owned(),
            request: Request::get(*url),
            forced: *forced,
        })
        .collect();
    assert_eq!(
        actual, expected,
        "complete independently specified outgoing vector"
    );
    actual.into_iter().map(|request| request.ticket).collect()
}

fn field_setup(runner: &mut Runner<Source>, layout: Layout) -> (u64, Vec<Value>) {
    let observed = observation(runner.plan(), "snapshot", 1.0, "word", 1.0);
    let mut field_args = runner.resource_args("composerFieldNext").unwrap().to_vec();
    assert_eq!(
        field_args,
        vec![observed.clone(), Value::Number(0.0), Value::Number(1.0),]
    );
    field_args[1] = Value::Number(1.0);
    let apply_at = runner.now_ms() + 1.0;
    assert_eq!(task(runner, "composerFieldDeliver"), Some(apply_at));
    let mut expected = background(runner, apply_at);
    expected.push(query_call(runner, "composerFieldNext", &field_args));
    let advanced = runner.advance_timed(apply_at);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, apply_at);
    calls(runner, expected);
    no_outgoing(runner);
    pending(runner, &[]);
    hosts(
        runner,
        vec![
            command(
                "setRangeText",
                vec![
                    Value::str("composer"),
                    Value::str(""),
                    Value::Number(4.0),
                    Value::Number(4.0),
                    Value::str("preserve"),
                ],
                None,
            ),
            command(
                "setSelectionRange",
                vec![
                    Value::str("composer"),
                    Value::Number(4.0),
                    Value::Number(4.0),
                ],
                None,
            ),
        ],
    );
    frames(&["composer"]);
    field_protocol(runner, layout, 0.0);
    field_state(runner, layout, None, 1.0);
    let confirm_at = apply_at + 1.0;
    assert_eq!(task(runner, "composerFieldConfirm"), Some(confirm_at));
    let args = vec![observed, Value::Number(1.0), Value::Bool(true)];
    let mut expected = background(runner, confirm_at);
    expected.push(call("answer", "composerFieldAcknowledged", &args));
    let confirmed = runner.advance_timed(confirm_at);
    assert!(confirmed.error.is_none(), "{confirmed:?}");
    assert_eq!(confirmed.now_ms, confirm_at);
    calls(runner, expected);
    hosts(runner, vec![]);
    frames(&["composer"]);
    let ticket = expect_requests(runner, &[("composerFieldAck", ACK_URL, false)])[0];
    pending(runner, &[("composerFieldAck", ticket)]);
    field_protocol(runner, layout, 1.0);
    field_state(runner, layout, None, 1.0);
    assert_eq!(task(runner, "composerFieldConfirm"), None);
    (ticket, args)
}

fn keys(runner: &mut Runner<Source>, layout: Layout, held: &[(&str, u64)]) {
    for serial in 1..=2 {
        let view = node(runner);
        assert!(runner.handlers_of(view).contains(&EventKind::Key));
        runner
            .dispatch(view, Event::Key(KeyboardEvent::chord("Shift+Enter")))
            .unwrap();
        calls(runner, vec![]);
        no_outgoing(runner);
        pending(runner, held);
        hosts(
            runner,
            vec![
                command("preventDefault", vec![], Some(view)),
                command("stopPropagation", vec![], Some(view)),
            ],
        );
        frames(&[]);
        assert_eq!(
            intent_value(runner, layout, 0),
            Value::Number(f64::from(serial))
        );
        let (ty, actual) = member(runner, layout, INTENT[1].0, "intentState", "queue");
        let item = runner.plan().type_(ty).elem.unwrap();
        let expected = Value::list(
            (1..=serial)
                .map(|value| {
                    fields(
                        runner.plan(),
                        item,
                        &[
                            ("serial", Value::Number(f64::from(value))),
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
                .collect(),
        );
        assert_eq!(actual, expected);
        assert_eq!(intent_value(runner, layout, 2), Value::Number(0.0));
        assert_eq!(intent_value(runner, layout, 3), Value::str(""));
    }
    assert_eq!(intent_value(runner, layout, 5), Value::str(""));
    assert_eq!(
        intent_value(runner, layout, 6),
        observation(runner.plan(), "snapshot", 1.0, "word", 1.0)
    );
}

fn intent_args(index: usize) -> Vec<Value> {
    let second = index == 1;
    assert!(index <= 1);
    let caret = if second { 5.0 } else { 4.0 };
    vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str(if second { "word\n" } else { "word" }),
        Value::Number(if second { 2.0 } else { 1.0 }),
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

fn pump(
    runner: &mut Runner<Source>,
    layout: Layout,
    index: usize,
    held: &[(&str, u64)],
) -> (u64, Vec<Value>) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task(runner, "composerIntentReady"), Some(due));
    assert_eq!(task(runner, "composerIntentReceipt"), None);
    let args = intent_args(index);
    let mut expected = background(runner, due);
    expected.push(call("answer", "composerIntent", &args));
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    hosts(runner, vec![]);
    frames(&[]);
    let ticket = expect_requests(runner, &[("composerEdited", INTENT_URL, false)])[0];
    let mut next = held.to_vec();
    next.push(("composerEdited", ticket));
    pending(runner, &next);
    intent_state(runner, layout, index + 1, "request", "");
    assert_eq!(task(runner, "composerIntentReady"), None);
    (ticket, args)
}

fn continuation(runner: &mut Runner<Source>, held: &[(&str, u64)], expected_hosts: Vec<Command>) {
    let now = runner.now_ms();
    let landed = runner.land_then();
    assert!(landed.error.is_none(), "{landed:?}");
    assert_eq!(landed.now_ms, now);
    assert_eq!(landed.receipts.len(), 1);
    assert_eq!(landed.receipts[0].at_ms, now);
    calls(runner, vec![]);
    no_outgoing(runner);
    pending(runner, held);
    hosts(runner, expected_hosts);
    frames(&[]);
}

struct Refresh {
    data_ticket: u64,
    rich_ticket: u64,
    data_args: Vec<Value>,
    original_rich_args: Vec<Value>,
    carried_rich_args: Vec<Value>,
}

fn ack_refresh(
    runner: &mut Runner<Source>,
    layout: Layout,
    ack: u64,
    args: &[Value],
    intent: u64,
) -> Refresh {
    intent_state(runner, layout, 1, "request", "");
    field_protocol(runner, layout, 1.0);
    pending(
        runner,
        &[("composerFieldAck", ack), ("composerEdited", intent)],
    );
    let data_args = runner.resource_args("data").unwrap().to_vec();
    let rich_args = vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str("word"),
        Value::Number(1.0),
        Value::str("insertText"),
        Value::str("composer"),
    ];
    assert_eq!(
        runner.resource_args("richComposer"),
        Some(rich_args.as_slice())
    );
    let mut expected = vec![call("parse", "composerFieldAcknowledged", args)];
    // FieldNext is declared earlier but awaits the rich projection's settlement.
    for name in [
        "data",
        "editor",
        "chip",
        "richComposer",
        "shell",
        "composerFieldNext",
    ] {
        expected.push(standing_call(runner, name));
    }
    // Request effects enqueue in row order, separately from the dependency-ordered calls.
    let mut order = vec![];
    for row in &runner.plan().resources {
        let name = runner.plan().str(row.name);
        if ["data", "richComposer"].contains(&name) {
            order.push(name.to_owned());
        }
    }
    assert_eq!(expected.len(), 7);
    let now = runner.now_ms();
    assert!(runner
        .fulfill(ack, response(&refusal(runner.plan())))
        .unwrap()
        .is_some());
    calls(runner, expected);
    hosts(runner, vec![]);
    frames(&[]);
    let recipes: Vec<_> = order
        .iter()
        .map(|name| {
            (
                name.as_str(),
                if name == "data" { DATA_URL } else { RICH_URL },
                true,
            )
        })
        .collect();
    let tickets = expect_requests(runner, &recipes);
    let data_index = order.iter().position(|name| name == "data").unwrap();
    let rich_index = order
        .iter()
        .position(|name| name == "richComposer")
        .unwrap();
    let mut held = vec![("composerEdited", intent)];
    held.extend(
        order
            .iter()
            .zip(&tickets)
            .map(|(name, ticket)| (name.as_str(), *ticket)),
    );
    pending(runner, &held);
    assert_eq!(runner.now_ms(), now);
    intent_state(runner, layout, 1, "request", "");
    field_state(runner, layout, None, 1.0);
    continuation(runner, &held, vec![]);
    intent_state(runner, layout, 1, "request", "");
    field_protocol(runner, layout, 1.0);
    assert_eq!(task(runner, "composerIntentOwnerNotice"), None);
    assert_eq!(task(runner, "composerIntentInvalidated"), None);
    Refresh {
        data_ticket: tickets[data_index],
        rich_ticket: tickets[rich_index],
        data_args,
        original_rich_args: rich_args.clone(),
        carried_rich_args: rich_args,
    }
}

fn land_snapshot(
    runner: &mut Runner<Source>,
    layout: Layout,
    change: Change,
    refresh: &mut Refresh,
    intent: u64,
) {
    let next = snapshot(runner.plan(), change.snapshot());
    let (_, data_source) = resource(runner.plan(), "data");
    let mut expected = vec![call("parse", &data_source, &refresh.data_args)];
    if change == Change::Snapshot {
        let mut preview_args = runner.resource_args("richPromptPreview").unwrap().to_vec();
        assert_eq!(preview_args.len(), 6);
        assert_eq!(preview_args[0], Value::str("snapshot"));
        assert_eq!(preview_args[1], Value::str("snapshot:prompt-preview"));
        preview_args[0] = Value::str("next");
        preview_args[1] = Value::str("next:prompt-preview");
        refresh.carried_rich_args = vec![
            Value::str("next"),
            Value::str("draft"),
            Value::str("word"),
            Value::Number(1.0),
            Value::str("insertText"),
            Value::str("composer"),
        ];
        let field_args = vec![
            observation(runner.plan(), "next", 1.0, "word", 1.0),
            Value::Number(1.0),
            Value::Number(1.0),
        ];
        expected.extend([
            query_call(runner, "richComposer", &refresh.carried_rich_args),
            query_call(runner, "richPromptPreview", &preview_args),
            query_call(runner, "composerFieldNext", &field_args),
        ]);
        assert_eq!(expected.len(), 4);
    }
    let now = runner.now_ms();
    assert!(runner
        .fulfill(refresh.data_ticket, response(&next))
        .unwrap()
        .is_some());
    calls(runner, expected);
    hosts(runner, vec![]);
    frames(&[]);
    no_outgoing(runner);
    pending(
        runner,
        &[
            ("composerEdited", intent),
            ("richComposer", refresh.rich_ticket),
        ],
    );
    assert_eq!(runner.now_ms(), now);
    assert_eq!(
        runner.resource_args("richComposer"),
        Some(refresh.original_rich_args.as_slice()),
        "displayed answer still has old args"
    );
    // The same Request on a nonforced argument change keeps its ticket and changes p.args.
    // It is unrelated to a watched-topic ask_again; the later parse asserts the carried args.
    intent_state(runner, layout, 1, "request", "");
    field_state(
        runner,
        layout,
        if change == Change::Snapshot {
            Some(Change::Snapshot)
        } else {
            None
        },
        1.0,
    );
    field_protocol(runner, layout, 1.0);
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runner.derive("composerIntentContextCurrent"),
        Some(&Value::Bool(change != Change::Snapshot))
    );
    assert_eq!(
        task(runner, "composerIntentOwnerNotice"),
        (change == Change::Snapshot).then_some(now + 1.0)
    );
    assert_eq!(
        task(runner, "composerIntentInvalidated"),
        (change == Change::Snapshot).then_some(now + 1.0)
    );
    assert_eq!(
        task(runner, "promptPreviewLifecycle"),
        (change == Change::Snapshot).then_some(now + 1.0)
    );
}

fn land_rich(
    runner: &mut Runner<Source>,
    layout: Layout,
    change: Change,
    refresh: &Refresh,
    intent: u64,
) {
    let (ty, source) = resource(runner.plan(), "richComposer");
    let reply = rich(runner.plan(), &refresh.carried_rich_args, Some(change));
    assert!(reply.conforms(runner.plan(), ty));
    let mut expected = vec![call("parse", &source, &refresh.carried_rich_args)];
    if change == Change::Generation {
        expected.push(query_call(
            runner,
            "composerFieldNext",
            &[
                observation(runner.plan(), "snapshot", 2.0, "word", 1.0),
                Value::Number(1.0),
                Value::Number(1.0),
            ],
        ));
    }
    let now = runner.now_ms();
    assert!(runner
        .fulfill(refresh.rich_ticket, response(&reply))
        .unwrap()
        .is_some());
    calls(runner, expected);
    no_outgoing(runner);
    pending(runner, &[("composerEdited", intent)]);
    hosts(runner, vec![]);
    frames(&[]);
    assert_eq!(runner.now_ms(), now);
    assert_eq!(
        runner.resource_args("richComposer"),
        Some(refresh.carried_rich_args.as_slice())
    );
    assert_eq!(runner.resource("richComposer"), Some(&reply));
    intent_state(runner, layout, 1, "request", "");
    field_state(runner, layout, Some(change), 1.0);
    field_protocol(runner, layout, 1.0);
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(change != Change::Owner))
    );
    assert_eq!(
        runner.derive("composerIntentContextCurrent"),
        Some(&Value::Bool(change.context()))
    );
    assert_eq!(task(runner, "composerIntentReady"), None);
    assert_eq!(task(runner, "composerIntentReceipt"), None);
    assert_eq!(
        task(runner, "composerIntentOwnerNotice"),
        change.owner_timer().then_some(now + 1.0)
    );
    assert_eq!(
        task(runner, "composerIntentInvalidated"),
        (!change.context()).then_some(now + 1.0)
    );
    assert_eq!(
        task(runner, "promptPreviewLifecycle"),
        (change == Change::Snapshot).then_some(now + 1.0)
    );
}

fn late_edit(
    runner: &mut Runner<Source>,
    layout: Layout,
    change: Change,
    ticket: u64,
    args: &[Value],
) {
    intent_state(runner, layout, 1, "request", "");
    let now = runner.now_ms();
    let reply = edit_reply(runner.plan());
    assert!(runner.fulfill(ticket, response(&reply)).unwrap().is_some());
    calls(runner, vec![call("parse", "composerIntent", args)]);
    no_outgoing(runner);
    pending(runner, &[]);
    hosts(runner, vec![]);
    frames(&[]);
    assert_eq!(runner.now_ms(), now);
    intent_state(runner, layout, 1, "request", "");
    continuation(runner, &[], vec![]);
    intent_state(runner, layout, 3, "", NOTICE);
    field_state(runner, layout, Some(change), 1.0);
    field_protocol(runner, layout, 1.0);
    assert_eq!(task(runner, "composerIntentInvalidated"), None);
    assert_eq!(task(runner, "composerIntentReady"), None);
    assert_eq!(task(runner, "composerIntentReceipt"), None);
    assert_eq!(
        task(runner, "composerIntentOwnerNotice"),
        change.owner_timer().then_some(now + 1.0)
    );
}

fn identity_tick(runner: &mut Runner<Source>, layout: Layout, change: Change) {
    let due = runner.now_ms() + 1.0;
    let mut expected = background(runner, due);
    if change == Change::Snapshot {
        let ty = observation_type(runner.plan());
        let previous = fields(
            runner.plan(),
            ty,
            &[
                ("generation", Value::Number(1.0)),
                ("snapshotOwner", Value::str("snapshot")),
                ("owner", Value::str("snapshot:prompt-preview")),
                ("value", Value::str(PREVIEW)),
                ("inputSerial", Value::Number(2.0)),
            ],
        );
        assert_eq!(
            runner.data_ref().preview,
            Some(previous),
            "known boot observation"
        );
        let observation = fields(
            runner.plan(),
            ty,
            &[
                ("generation", Value::Number(1.0)),
                ("snapshotOwner", Value::str("next")),
                ("owner", Value::str("next:prompt-preview")),
                ("value", Value::str(PREVIEW)),
                ("inputSerial", Value::Number(3.0)),
            ],
        );
        expected.push(call("answer", "composerChipObserved", &[observation]));
        expected.push(standing_call(runner, "chip"));
    }
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_outgoing(runner);
    pending(runner, &[]);
    hosts(runner, vec![]);
    frames(&[]);
    intent_state(
        runner,
        layout,
        3,
        "",
        if change.owner_timer() { "" } else { NOTICE },
    );
    field_state(runner, layout, Some(change), 1.0);
    field_protocol(runner, layout, 1.0);
    for name in [
        "composerIntentOwnerNotice",
        "composerIntentInvalidated",
        "composerIntentReady",
        "composerIntentReceipt",
        "promptPreviewLifecycle",
    ] {
        assert_eq!(task(runner, name), None, "{name}");
    }
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(change != Change::Owner))
    );
    assert_eq!(
        runner.derive("composerIntentContextCurrent"),
        Some(&Value::Bool(change.context()))
    );
}

#[test]
fn identity_replies_reject_the_held_edit_before_identity_timers_run() {
    for (bytes, layout) in plans() {
        for change in [
            Change::Owner,
            Change::Generation,
            Change::Mode,
            Change::Snapshot,
        ] {
            let mut runner = boot(bytes, layout, Some(change));
            let (ack, ack_args) = field_setup(&mut runner, layout);
            keys(&mut runner, layout, &[("composerFieldAck", ack)]);
            let (intent, intent_args) = pump(&mut runner, layout, 0, &[("composerFieldAck", ack)]);
            let mut refresh = ack_refresh(&mut runner, layout, ack, &ack_args, intent);
            land_snapshot(&mut runner, layout, change, &mut refresh, intent);
            land_rich(&mut runner, layout, change, &refresh, intent);
            late_edit(&mut runner, layout, change, intent, &intent_args);
            identity_tick(&mut runner, layout, change);
        }
    }
}

fn first_edit(runner: &mut Runner<Source>, layout: Layout, ticket: u64, args: &[Value]) {
    let reply = edit_reply(runner.plan());
    assert!(runner.fulfill(ticket, response(&reply)).unwrap().is_some());
    calls(runner, vec![call("parse", "composerIntent", args)]);
    no_outgoing(runner);
    pending(runner, &[]);
    hosts(runner, vec![]);
    frames(&[]);
    intent_state(runner, layout, 1, "request", "");
    continuation(
        runner,
        &[],
        vec![
            command(
                "setRangeText",
                vec![
                    Value::str("composer"),
                    Value::str("\n"),
                    Value::Number(4.0),
                    Value::Number(4.0),
                    Value::str("preserve"),
                ],
                None,
            ),
            command(
                "setSelectionRange",
                vec![
                    Value::str("composer"),
                    Value::Number(5.0),
                    Value::Number(5.0),
                ],
                None,
            ),
        ],
    );
    intent_state(runner, layout, 1, "input", "");
    assert_eq!(task(runner, "composerIntentReceipt"), None);
}

fn accepted_input(runner: &mut Runner<Source>, layout: Layout) {
    let observed = observation(runner.plan(), "snapshot", 1.0, "word\n", 2.0);
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
                Value::str("word\n"),
                Value::Number(0.0),
            ],
        ),
        standing_call(runner, "data"),
        standing_call(runner, "editor"),
    ];
    let rich_args = vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str("word\n"),
        Value::Number(1.0),
        Value::str("insertText"),
        Value::str("composer"),
    ];
    expected.push(query_call(runner, "richComposer", &rich_args));
    expected.push(query_call(
        runner,
        "composerFieldNext",
        &[observed, Value::Number(0.0), Value::Number(1.0)],
    ));
    dispatch_input(runner, "word\n", 5);
    calls(runner, expected);
    no_outgoing(runner);
    pending(runner, &[]);
    hosts(runner, vec![]);
    frames(&[]);
    intent_state(runner, layout, 1, "projection", "");
    field_state(runner, layout, None, 2.0);
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(task(runner, "composerIntentReady"), None);
    assert_eq!(
        task(runner, "composerIntentReceipt"),
        Some(runner.now_ms() + 1.0)
    );
}

fn first_receipt(runner: &mut Runner<Source>, layout: Layout) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task(runner, "composerIntentReceipt"), Some(due));
    let expected = background(runner, due);
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_outgoing(runner);
    pending(runner, &[]);
    hosts(runner, vec![]);
    frames(&[]);
    intent_state(runner, layout, 2, "", "");
    field_state(runner, layout, None, 2.0);
    assert_eq!(task(runner, "composerIntentReceipt"), None);
    assert_eq!(task(runner, "composerIntentReady"), Some(due + 1.0));
}

// The old private repeated callback is unavailable through fulfill/land_then.
// This is a distinct real second ticket carrying the first reply's stale serial.
#[test]
fn a_live_second_ticket_with_the_first_payload_cannot_apply_a_second_edit() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, None);
        keys(&mut runner, layout, &[]);
        let (first, args) = pump(&mut runner, layout, 0, &[]);
        first_edit(&mut runner, layout, first, &args);
        accepted_input(&mut runner, layout);
        first_receipt(&mut runner, layout);
        let (second, current_args) = pump(&mut runner, layout, 1, &[]);
        assert_ne!(first, second);
        let now = runner.now_ms();
        let stale = edit_reply(runner.plan());
        assert!(runner.fulfill(second, response(&stale)).unwrap().is_some());
        calls(
            &mut runner,
            vec![call("parse", "composerIntent", &current_args)],
        );
        no_outgoing(&mut runner);
        pending(&runner, &[]);
        hosts(&mut runner, vec![]);
        frames(&[]);
        intent_state(&runner, layout, 2, "request", "");
        continuation(&mut runner, &[], vec![]);
        intent_state(&runner, layout, 2, "request", "");
        field_state(&runner, layout, None, 2.0);
        assert_eq!(runner.now_ms(), now);
        assert_eq!(
            runner.derive("composerIntentContextCurrent"),
            Some(&Value::Bool(true))
        );
        for name in [
            "composerIntentReady",
            "composerIntentReceipt",
            "composerIntentInvalidated",
        ] {
            assert_eq!(task(&runner, name), None);
        }
        let due = now + 1.0;
        let expected = background(&runner, due);
        let advanced = runner.advance_timed(due);
        assert!(advanced.error.is_none(), "{advanced:?}");
        assert_eq!(advanced.now_ms, due);
        calls(&mut runner, expected);
        no_outgoing(&mut runner);
        pending(&runner, &[]);
        hosts(&mut runner, vec![]);
        frames(&[]);
        intent_state(&runner, layout, 2, "request", "");
        field_state(&runner, layout, None, 2.0);
    }
}
