//! Paste and explicit language ranges through mounted public controls.
//! Typed replies isolate the Contract consumer. The complete TypeScript producer suite stays.
//! Host commands are observed; scripted replacement Inputs do not execute native edits.

use super::contract_keys_tests::empty;
use crate::contract_keys_tests::extraction::{
    call, element, field, fields, mutation, observation_type, resource, Call,
};
use exact_kernel::{Kernel, ViewId};
use exact_plan::{EventKind, Plan, TypesId, Value};
use exact_runner::{
    Answer, Command, ControlValue, DataError, DataSource, Delivery, Event, FieldSelection,
    InputDetails, Outcome, Request, RequestOut, Response, Runner, Store, Viewport,
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
const INTENT_URL: &str = "https://fixture.invalid/range-intent";
const FENCE: &str = "```ts\nx\n```\n- [ ] task";
const RUST_FENCE: &str = "```rust\nx\n```\n- [ ] task";

#[derive(Clone, Copy, Debug)]
enum Layout {
    Original,
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    Paste,
    Language,
}

impl Case {
    fn count(self) -> usize {
        if self == Self::Paste {
            2
        } else {
            1
        }
    }

    fn value(self, delivered: usize) -> &'static str {
        match (self, delivered) {
            (Self::Paste, 0) => "a",
            (Self::Paste, 1) => "ax",
            (Self::Paste, 2) => "axy",
            (Self::Language, 0) => FENCE,
            (Self::Language, 1) => RUST_FENCE,
            _ => panic!("bounded delivered value"),
        }
    }

    fn caret(self, delivered: usize) -> f64 {
        match self {
            Self::Paste => 1.0 + delivered as f64,
            Self::Language => {
                if delivered == 0 {
                    8.0
                } else {
                    10.0
                }
            }
        }
    }

    fn text(self, turn: usize) -> &'static str {
        match (self, turn) {
            (Self::Paste, 0) => "x",
            (Self::Paste, 1) => "y",
            (Self::Language, 0) => "rust",
            _ => panic!("bounded requested text"),
        }
    }

    fn kind(self) -> &'static str {
        if self == Self::Paste {
            "paste"
        } else {
            "language"
        }
    }

    fn range(self) -> (f64, f64) {
        if self == Self::Paste {
            (1.0, 1.0)
        } else {
            (0.0, 5.0)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Admitted,
    Request,
    Input,
    Projection,
    Receipt,
}

impl Stage {
    fn phase(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Input => "input",
            Self::Projection => "projection",
            _ => "",
        }
    }

    fn delivered(self) -> bool {
        matches!(self, Self::Projection | Self::Receipt)
    }

    fn result(self) -> bool {
        matches!(self, Self::Input | Self::Projection | Self::Receipt)
    }
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

// These minimal typed resource replies do not run the Markdown or language producer.
fn rich(plan: &Plan, args: &[Value]) -> Value {
    assert_eq!(args.len(), 6);
    let ty = resource(plan, "richComposer").0;
    let before = args[2] == Value::str(FENCE);
    let after = args[2] == Value::str(RUST_FENCE);
    let header_ty = element(plan, ty, "headers");
    let language_ty = element(plan, ty, "languages");
    let code = args[5] == Value::str("composer") && (before || after);
    let headers = if code {
        vec![fields(
            plan,
            header_ty,
            &[
                ("start", Value::Number(0.0)),
                ("end", Value::Number(if before { 5.0 } else { 7.0 })),
                ("infoStart", Value::Number(3.0)),
                ("infoEnd", Value::Number(if before { 5.0 } else { 7.0 })),
                ("info", Value::str(if before { "ts" } else { "rust" })),
                (
                    "language",
                    Value::str(if before { "typescript" } else { "rust" }),
                ),
                (
                    "label",
                    Value::str(if before { "TypeScript" } else { "Rust" }),
                ),
            ],
        )]
    } else {
        vec![]
    };
    let languages = if code {
        vec![
            fields(
                plan,
                language_ty,
                &[
                    ("id", Value::str("typescript")),
                    ("label", Value::str("TypeScript")),
                ],
            ),
            fields(
                plan,
                language_ty,
                &[("id", Value::str("rust")), ("label", Value::str("Rust"))],
            ),
        ]
    } else {
        vec![]
    };
    fields(
        plan,
        ty,
        &[
            ("generation", Value::Number(1.0)),
            ("owner", args[1].clone()),
            ("value", args[2].clone()),
            (
                "mode",
                Value::str(if args[5] == Value::str("preview") {
                    "plain"
                } else {
                    "rich"
                }),
            ),
            ("headers", Value::list(headers)),
            ("languages", Value::list(languages)),
        ],
    )
}

fn edit_reply(plan: &Plan, case: Case, turn: usize) -> Value {
    let language = case == Case::Language;
    let at = if language { 3.0 } else { case.caret(turn) };
    fields(
        plan,
        mutation(plan, "composerEdited"),
        &[
            ("owner", Value::str("draft")),
            ("value", Value::str(case.value(turn))),
            ("mode", Value::str("rich")),
            ("serial", Value::Number(turn as f64 + 1.0)),
            ("kind", Value::str("edit")),
            ("start", Value::Number(at)),
            ("end", Value::Number(if language { 5.0 } else { at })),
            ("text", Value::str(case.text(turn))),
            (
                "selectionStart",
                Value::Number(if language { 0.0 } else { at + 1.0 }),
            ),
            (
                "selectionEnd",
                Value::Number(if language { 0.0 } else { at + 1.0 }),
            ),
            ("preserveSelection", Value::Bool(language)),
        ],
    )
}

struct Source {
    schema: Plan,
    ready: bool,
    values: HashMap<String, Value>,
    snapshot: Value,
    calls: Vec<Call>,
}

impl Source {
    fn value(&self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == resource(&self.schema, "data").1 {
            return Ok(self.snapshot.clone());
        }
        if source == resource(&self.schema, "richComposer").1 {
            return Ok(rich(&self.schema, args));
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
        assert_eq!(source, "composerIntent");
        let Outcome::Response(reply) = outcome else {
            panic!("held intent response")
        };
        assert_eq!(reply.status, 200);
        assert!(reply.headers.is_empty());
        let value = Value::from_bytes(&reply.body).unwrap();
        assert!(value.conforms(&self.schema, mutation(&self.schema, "composerEdited")));
        Ok(Answer::Now(value))
    }
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
    let plan = runner.plan();
    let slot_name = if matches!(layout, Layout::Original) {
        original
    } else {
        record
    };
    let row = plan
        .slots
        .iter()
        .find(|row| plan.str(row.name) == slot_name)
        .unwrap();
    if matches!(layout, Layout::Original) {
        (row.ty, runner.slot(original).unwrap().clone())
    } else {
        let index = plan.field_index(row.ty, name).unwrap();
        let declared = plan.type_(row.ty).fields.iter().nth(index).unwrap();
        (
            plan.field(declared).ty,
            field(plan, row.ty, runner.slot(record).unwrap(), name),
        )
    }
}

fn metadata(
    runner: &Runner<Source>,
    layout: Layout,
    case: Case,
    first: usize,
    count: usize,
) -> Value {
    let ty = member(runner, layout, INTENT[1].0, "intentState", "queue").0;
    let item = runner.plan().type_(ty).elem.unwrap();
    let (start, end) = case.range();
    Value::list(
        (first..=count)
            .map(|serial| {
                fields(
                    runner.plan(),
                    item,
                    &[
                        ("serial", Value::Number(serial as f64)),
                        ("kind", Value::str(case.kind())),
                        ("text", Value::str(case.text(serial - 1))),
                        ("key", Value::str("")),
                        ("shift", Value::Bool(false)),
                        ("command", Value::Bool(false)),
                        ("start", Value::Number(start)),
                        ("end", Value::Number(end)),
                        ("inputSerial", Value::Number(1.0)),
                    ],
                )
            })
            .collect(),
    )
}

fn assert_state(runner: &Runner<Source>, layout: Layout, case: Case, turn: usize, stage: Stage) {
    let delivered = turn + usize::from(stage.delivered());
    let observed_turn = if stage == Stage::Receipt {
        turn + 1
    } else {
        turn
    };
    let first = turn + if stage == Stage::Receipt { 2 } else { 1 };
    let expected_value = if stage.result() {
        case.value(turn + 1)
    } else if turn > 0 {
        case.value(turn)
    } else {
        ""
    };
    let target = if case == Case::Language {
        if stage.delivered() {
            10.0
        } else {
            0.0
        }
    } else if stage.result() {
        case.caret(turn + 1)
    } else if turn > 0 {
        case.caret(turn)
    } else {
        0.0
    };
    let input = if stage.delivered() {
        case.caret(turn + 1)
    } else if turn > 0 {
        case.caret(turn)
    } else {
        0.0
    };
    let seen = stage != Stage::Input && (stage.delivered() || turn > 0);
    let active = if matches!(stage, Stage::Admitted | Stage::Receipt) {
        0.0
    } else {
        turn as f64 + 1.0
    };
    let expected = vec![
        Value::Number(case.count() as f64),
        metadata(runner, layout, case, first, case.count()),
        Value::Number(active),
        Value::str(stage.phase()),
        Value::str("rich"),
        Value::str(""),
        observation(
            runner.plan(),
            case.value(observed_turn),
            case.caret(observed_turn),
            observed_turn as f64 + 1.0,
        ),
        Value::str(expected_value),
        Value::Number(target),
        Value::Number(target),
        Value::Number(input),
        Value::Number(input),
        Value::Bool(case == Case::Language && stage.result()),
        Value::Bool(seen),
    ];
    let actual: Vec<_> = INTENT
        .iter()
        .map(|(old, name)| member(runner, layout, old, "intentState", name).1)
        .collect();
    assert_eq!(
        actual, expected,
        "{case:?}/{turn}/{stage:?}: full intent record"
    );
    assert_field(runner, layout, case, delivered);
    assert_eq!(
        runner.derive("composerIntentContextCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runner.derive("composerIntentReceiptCurrent"),
        Some(&Value::Bool(stage == Stage::Projection))
    );
    let ready = stage == Stage::Admitted || stage == Stage::Receipt && turn + 1 < case.count();
    assert_eq!(
        task(runner, "composerIntentReady"),
        ready.then_some(runner.now_ms() + 1.0)
    );
    assert_eq!(
        task(runner, "composerIntentReceipt"),
        (stage == Stage::Projection).then_some(runner.now_ms() + 1.0)
    );
    for name in [
        "composerIntentInvalidated",
        "composerIntentOwnerNotice",
        "composerFieldDeliver",
        "composerFieldConfirm",
        "composerFieldRestoreFocus",
    ] {
        assert_eq!(task(runner, name), None, "{name}");
    }
}

fn assert_field(runner: &Runner<Source>, layout: Layout, case: Case, delivered: usize) {
    let value = case.value(delivered);
    let caret = case.caret(delivered);
    let serial = delivered as f64 + 1.0;
    for (old, record, name, expected) in [
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
            Value::Number(caret),
        ),
        (
            "composerSelectionEnd",
            "composerSelection",
            "end",
            Value::Number(caret),
        ),
        (
            "composerFieldInputSerial",
            "fieldProtocol",
            "inputSerial",
            Value::Number(serial),
        ),
    ] {
        assert_eq!(member(runner, layout, old, record, name).1, expected);
    }
    let observed = observation(runner.plan(), value, caret, serial);
    assert_eq!(runner.derive("composerText"), Some(&Value::str(value)));
    assert_eq!(runner.derive("composerFieldSnapshot"), Some(&observed));
    assert_eq!(
        runner.derive("composerRichCurrent"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runner.derive("composerSelectionCurrent"),
        Some(&Value::Bool(true))
    );
}

fn node(runner: &Runner<Source>, id: &str) -> ViewId {
    let keys = runner.kernel().find_by_test_id(id);
    assert_eq!(keys.len(), 1, "one mounted public {id}");
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

fn task(runner: &Runner<Source>, name: &str) -> Option<f64> {
    runner
        .tasks()
        .into_iter()
        .find(|(found, _)| found == name)
        .unwrap()
        .1
}

fn query_call(runner: &Runner<Source>, name: &str, args: &[Value]) -> Call {
    call("answer", &resource(runner.plan(), name).1, args)
}

fn standing_call(runner: &Runner<Source>, name: &str) -> Call {
    query_call(runner, name, runner.resource_args(name).unwrap())
}

fn background(runner: &Runner<Source>, until: f64) -> Vec<Call> {
    assert_eq!(task(runner, "promptPreviewLifecycle"), None);
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

// Only initial boot/focus/word preparation uses finite named traffic allowances.
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
    trace
}

fn dispatch_input(
    runner: &mut Runner<Source>,
    value: &str,
    caret: u32,
    input_type: &str,
    data: &str,
) {
    let view = node(runner, "composer");
    assert!(runner.handlers_of(view).contains(&EventKind::Input));
    runner
        .dispatch(
            view,
            Event::Input(ControlValue::Edit(
                value.to_owned(),
                FieldSelection::caret(caret),
                InputDetails {
                    input_type: input_type.to_owned(),
                    data: Some(data.to_owned()),
                    cancelable: false,
                    is_composing: false,
                },
            )),
        )
        .unwrap();
}

fn boot(bytes: &[u8], layout: Layout, case: Case) -> Runner<Source> {
    let plan = Plan::decode(bytes).expect("independently compiled complete graph");
    let edit = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "composerEdited")
        .unwrap();
    assert!(edit.queue && edit.refreshes.len == 0);
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
            ("draft", Value::str(case.value(0))),
        ],
    );
    let data = Source {
        schema: Plan::decode(bytes).unwrap(),
        ready: false,
        values,
        snapshot,
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
    for name in ["rootFont", "streamClosedDialog", "streamClosedMiniDialog"] {
        assert_eq!(task(&runner, name), Some(1.0));
    }
    let warm = runner.advance_timed(2.0);
    assert!(warm.error.is_none(), "{warm:?}");
    assert_eq!(warm.now_ms, 2.0);
    let trace = preparation(&mut runner, true);
    assert!(!trace.calls.is_empty());
    let view = node(&runner, "composer");
    assert!(runner.handlers_of(view).contains(&EventKind::Focus));
    runner.dispatch(view, Event::Focus).unwrap();
    let _focus = preparation(&mut runner, false);
    dispatch_input(
        &mut runner,
        case.value(0),
        case.caret(0) as u32,
        "insertText",
        case.value(0),
    );
    let _input = preparation(&mut runner, false);
    assert_field(&runner, layout, case, 0);
    assert_eq!(task(&runner, "composerIntentOwnerNotice"), None);
    assert_eq!(task(&runner, "composerIntentReady"), None);
    assert_eq!(task(&runner, "composerIntentReceipt"), None);
    runner
}

fn admit_pastes(runner: &mut Runner<Source>, layout: Layout) {
    let observed = observation(runner.plan(), "a", 1.0, 1.0);
    for turn in 0..2 {
        let expected = vec![
            call(
                "answer",
                "composerFieldObserved",
                &[observed.clone(), Value::Bool(false)],
            ),
            standing_call(runner, "editor"),
        ];
        let view = node(runner, "composer");
        assert!(runner.handlers_of(view).contains(&EventKind::Beforeinput));
        runner
            .dispatch(
                view,
                Event::BeforeInput(
                    "a".to_owned(),
                    FieldSelection::caret(1),
                    InputDetails {
                        input_type: "insertFromPaste".to_owned(),
                        data: Some(Case::Paste.text(turn).to_owned()),
                        cancelable: true,
                        is_composing: false,
                    },
                ),
            )
            .unwrap();
        calls(runner, expected);
        no_requests(runner);
        hosts(runner, vec![command("preventDefault", vec![], Some(view))]);
        assert_eq!(
            member(runner, layout, INTENT[0].0, "intentState", "serial").1,
            Value::Number(turn as f64 + 1.0)
        );
        assert_eq!(
            member(runner, layout, INTENT[1].0, "intentState", "queue").1,
            metadata(runner, layout, Case::Paste, 1, turn + 1)
        );
        assert_field(runner, layout, Case::Paste, 0);
    }
    assert_state(runner, layout, Case::Paste, 0, Stage::Admitted);
}

fn admit_language(runner: &mut Runner<Source>, layout: Layout) {
    let args = vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str(FENCE),
        Value::Number(1.0),
        Value::str("insertText"),
        Value::str("composer"),
    ];
    assert_eq!(runner.resource_args("richComposer"), Some(args.as_slice()));
    assert_eq!(
        runner.resource("richComposer"),
        Some(&rich(runner.plan(), &args))
    );
    let view = node(runner, "composer-code-language-0");
    let ids = runner.kernel().find_by_id("field-composer-code-language-0");
    assert_eq!(ids.len(), 1);
    assert_eq!(runner.kernel().node_by_key(ids[0]).unwrap().id, view);
    assert!(runner.handlers_of(view).contains(&EventKind::Change));
    runner
        .dispatch(view, Event::Change(ControlValue::Text("rust".to_owned())))
        .unwrap();
    calls(runner, vec![]);
    no_requests(runner);
    hosts(
        runner,
        vec![command("focus", vec![Value::str("composer")], Some(view))],
    );
    assert_state(runner, layout, Case::Language, 0, Stage::Admitted);
}

fn request_args(case: Case, turn: usize) -> Vec<Value> {
    let (start, end) = if case == Case::Language {
        case.range()
    } else {
        (case.caret(turn), case.caret(turn))
    };
    vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str(case.value(turn)),
        Value::Number(turn as f64 + 1.0),
        Value::Number(start),
        Value::Number(end),
        Value::str(case.kind()),
        Value::str(case.text(turn)),
        Value::str(""),
        Value::Bool(false),
        Value::Bool(false),
        Value::Bool(false),
        Value::str("rich"),
    ]
}

fn pump(runner: &mut Runner<Source>, layout: Layout, case: Case, turn: usize) -> (u64, Vec<Value>) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task(runner, "composerIntentReady"), Some(due));
    assert_eq!(task(runner, "composerIntentReceipt"), None);
    let args = request_args(case, turn);
    let mut expected = background(runner, due);
    expected.push(call("answer", "composerIntent", &args));
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    hosts(runner, vec![]);
    let outgoing = runner.take_requests();
    assert_eq!(outgoing.len(), 1);
    let ticket = outgoing[0].ticket;
    assert_eq!(
        outgoing,
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
    assert_state(runner, layout, case, turn, Stage::Request);
    (ticket, args)
}

fn land_result(
    runner: &mut Runner<Source>,
    layout: Layout,
    case: Case,
    turn: usize,
    ticket: u64,
    args: &[Value],
) {
    let reply = edit_reply(runner.plan(), case, turn);
    let now = runner.now_ms();
    assert!(runner.fulfill(ticket, response(&reply)).unwrap().is_some());
    calls(runner, vec![call("parse", "composerIntent", args)]);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_eq!(runner.now_ms(), now);
    assert_state(runner, layout, case, turn, Stage::Request);
    let landed = runner.land_then();
    assert!(landed.error.is_none(), "{landed:?}");
    assert_eq!(landed.now_ms, now);
    assert_eq!(landed.receipts.len(), 1);
    assert_eq!(landed.receipts[0].at_ms, now);
    calls(runner, vec![]);
    no_requests(runner);
    let at = if case == Case::Language {
        3.0
    } else {
        case.caret(turn)
    };
    let mut expected = vec![command(
        "setRangeText",
        vec![
            Value::str("composer"),
            Value::str(case.text(turn)),
            Value::Number(at),
            Value::Number(if case == Case::Language { 5.0 } else { at }),
            Value::str("preserve"),
        ],
        None,
    )];
    if case == Case::Paste {
        expected.push(command(
            "setSelectionRange",
            vec![
                Value::str("composer"),
                Value::Number(at + 1.0),
                Value::Number(at + 1.0),
            ],
            None,
        ));
    }
    hosts(runner, expected);
    assert_state(runner, layout, case, turn, Stage::Input);
}

// A modeled noncancelable replacement Input carries the delivered source and selection.
// For language, caret10 is supplied; observing it does not establish native preservation.
fn deliver_input(runner: &mut Runner<Source>, layout: Layout, case: Case, turn: usize) {
    let value = case.value(turn + 1);
    let caret = case.caret(turn + 1);
    let observed = observation(runner.plan(), value, caret, turn as f64 + 2.0);
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
                Value::str(value),
                Value::Number(0.0),
            ],
        ),
        standing_call(runner, "data"),
        standing_call(runner, "editor"),
    ];
    let rich_args = vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str(value),
        Value::Number(1.0),
        Value::str("insertReplacementText"),
        Value::str("composer"),
    ];
    expected.push(query_call(runner, "richComposer", &rich_args));
    expected.push(query_call(
        runner,
        "composerFieldNext",
        &[observed, Value::Number(0.0), Value::Number(1.0)],
    ));
    dispatch_input(
        runner,
        value,
        caret as u32,
        "insertReplacementText",
        case.text(turn),
    );
    calls(runner, expected);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_eq!(
        runner.resource_args("richComposer"),
        Some(rich_args.as_slice())
    );
    assert_eq!(
        runner.resource("richComposer"),
        Some(&rich(runner.plan(), &rich_args))
    );
    assert_state(runner, layout, case, turn, Stage::Projection);
    if case == Case::Language {
        let view = node(runner, "composer-code-language-0");
        assert!(runner.handlers_of(view).contains(&EventKind::Change));
    }
}

fn receipt(runner: &mut Runner<Source>, layout: Layout, case: Case, turn: usize) {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task(runner, "composerIntentReceipt"), Some(due));
    assert_eq!(task(runner, "composerIntentReady"), None);
    let expected = background(runner, due);
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_state(runner, layout, case, turn, Stage::Receipt);
}

fn quiet_after_last(runner: &mut Runner<Source>, layout: Layout, case: Case) {
    let due = runner.now_ms() + 1.0;
    let expected = background(runner, due);
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    no_requests(runner);
    hosts(runner, vec![]);
    assert_state(runner, layout, case, case.count() - 1, Stage::Receipt);
}

#[test]
fn two_public_pastes_request_the_second_at_the_first_delivered_selection() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, Case::Paste);
        admit_pastes(&mut runner, layout);
        for turn in 0..2 {
            let (ticket, args) = pump(&mut runner, layout, Case::Paste, turn);
            land_result(&mut runner, layout, Case::Paste, turn, ticket, &args);
            deliver_input(&mut runner, layout, Case::Paste, turn);
            receipt(&mut runner, layout, Case::Paste, turn);
        }
        quiet_after_last(&mut runner, layout, Case::Paste);
    }
}

#[test]
fn mounted_language_select_keeps_explicit_header_range_and_adopts_delivered_caret() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, Case::Language);
        admit_language(&mut runner, layout);
        let (ticket, args) = pump(&mut runner, layout, Case::Language, 0);
        land_result(&mut runner, layout, Case::Language, 0, ticket, &args);
        deliver_input(&mut runner, layout, Case::Language, 0);
        receipt(&mut runner, layout, Case::Language, 0);
        quiet_after_last(&mut runner, layout, Case::Language);
    }
}

#[path = "app_extraction_composer_delivery_tests.rs"]
mod delivery;

#[path = "app_extraction_hover_tests.rs"]
mod hover;
