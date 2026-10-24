//! Handled outdent and one admitted send through public composer keys.
//! Typed block/reply boundaries do not execute the TypeScript producer or native editor.
//! Snapshot text is current while the local draft remains empty/unset at input serial0.

use super::*;

const SEND_URL: &str = "https://fixture.invalid/composer-send";
const CODE: &str = "```ts\nx\n```";
const REFUSED: &str = "That edit could not be applied. Please try again.";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scenario {
    Outdent,
    Send,
}

impl Scenario {
    fn value(self) -> &'static str {
        if self == Self::Outdent {
            CODE
        } else {
            "word"
        }
    }
    fn caret(self) -> f64 {
        if self == Self::Outdent {
            6.0
        } else {
            4.0
        }
    }
    fn key(self, serial: usize) -> &'static str {
        if self == Self::Outdent && serial == 1 {
            "Tab"
        } else {
            "Enter"
        }
    }
    fn chord(self, serial: usize) -> &'static str {
        if self == Self::Send {
            "Enter"
        } else if serial == 1 {
            "Shift+Tab"
        } else {
            "Shift+Enter"
        }
    }
}

fn projection(plan: &Plan, args: &[Value]) -> Value {
    assert_eq!(args.len(), 6);
    let ty = resource(plan, "richComposer").0;
    let blocks = if args[5] == Value::str("composer") && args[2] == Value::str(CODE) {
        let block_ty = element(plan, ty, "blocks");
        vec![fields(
            plan,
            block_ty,
            &[
                ("kind", Value::str("code")),
                ("start", Value::Number(0.0)),
                ("end", Value::Number(11.0)),
                ("contentStart", Value::Number(6.0)),
                ("bodyStart", Value::Number(6.0)),
                ("bodyEnd", Value::Number(7.0)),
                ("bodyLine", Value::Bool(true)),
            ],
        )]
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
            ("blocks", Value::list(blocks)),
        ],
    )
}

struct DeliverySource {
    inner: Source,
}
type Host = Runner<DeliverySource>;

impl DeliverySource {
    fn value(&self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == resource(&self.inner.schema, "richComposer").1 {
            Ok(projection(&self.inner.schema, args))
        } else {
            self.inner.value(source, args)
        }
    }
}

impl DataSource for DeliverySource {
    fn ready(&self) -> bool {
        self.inner.ready
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.inner.calls.push(call("query", source, args));
        self.value(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if source == resource(&self.inner.schema, "richComposer").1 {
            self.inner.calls.push(call("answer", source, args));
            return Ok(Answer::Now(projection(&self.inner.schema, args)));
        }
        if source == "command" && args.first() == Some(&Value::str("send")) {
            self.inner.calls.push(call("answer", source, args));
            assert_eq!(
                args,
                &[
                    Value::str("send"),
                    Value::str(""),
                    Value::str("word"),
                    Value::Number(0.0)
                ]
            );
            return Ok(Answer::Later(Request::get(SEND_URL)));
        }
        self.inner.answer(store, source, args)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.inner.parse(store, source, args, outcome)
    }
}

fn member(
    runner: &Host,
    layout: Layout,
    old: &str,
    container: &str,
    name: &str,
) -> (TypesId, Value) {
    let plan = runner.plan();
    let slot = if matches!(layout, Layout::Original) {
        old
    } else {
        container
    };
    let row = plan
        .slots
        .iter()
        .find(|row| plan.str(row.name) == slot)
        .unwrap();
    if matches!(layout, Layout::Original) {
        (row.ty, runner.slot(old).unwrap().clone())
    } else {
        let index = plan.field_index(row.ty, name).unwrap();
        let declared = plan.type_(row.ty).fields.iter().nth(index).unwrap();
        (
            plan.field(declared).ty,
            field(plan, row.ty, runner.slot(container).unwrap(), name),
        )
    }
}

fn node(runner: &Host) -> ViewId {
    let nodes = runner.kernel().find_by_test_id("composer");
    assert_eq!(nodes.len(), 1);
    runner.kernel().node_by_key(nodes[0]).unwrap().id
}

fn task(runner: &Host, name: &str) -> Option<f64> {
    runner
        .tasks()
        .into_iter()
        .find(|(found, _)| found == name)
        .unwrap()
        .1
}

fn calls(runner: &mut Host, expected: Vec<Call>) {
    assert_eq!(
        std::mem::take(&mut runner.data().inner.calls),
        expected,
        "all data calls"
    );
}

fn hosts(runner: &mut Host, expected: Vec<Command>) {
    assert_eq!(runner.take_commands(), expected, "all host commands");
}

fn traffic(runner: &mut Host, pending: Vec<(String, u64)>) {
    assert!(runner.take_requests().is_empty(), "all outgoing requests");
    assert_eq!(runner.pending(), pending);
    assert!(runner.queued().is_empty());
}

fn background(runner: &Host, due: f64) -> Vec<Call> {
    assert_eq!(task(runner, "promptPreviewLifecycle"), None);
    if let Some(at) = task(runner, "transportAnswered").filter(|at| *at <= due) {
        vec![call(
            "answer",
            "transportPump",
            &[Value::Number(EPOCH + at), Value::str("")],
        )]
    } else {
        vec![]
    }
}

// Initial boot/focus/selection preparation has finite source-name allowances.
fn preparation(runner: &mut Host, boot: bool) {
    traffic(runner, vec![]);
    let seen = std::mem::take(&mut runner.data().inner.calls);
    for item in &seen {
        assert_eq!(item.method, "answer", "{seen:?}");
        assert!(
            runner
                .plan()
                .resources
                .iter()
                .any(|row| runner.plan().str(row.source) == item.source)
                || [
                    "composerFieldObserved",
                    "transportPump",
                    "composerChipObserved"
                ]
                .contains(&item.source.as_str()),
            "{seen:?}"
        );
    }
    hosts(
        runner,
        if boot {
            vec![
                command("setRootFontSize", vec![Value::Number(16.0)], None),
                command("close", vec![Value::str("server-browser-dialog-")], None),
                command("close", vec![Value::str("server-browser-dialog-")], None),
            ]
        } else {
            vec![]
        },
    );
}

fn boot(bytes: &[u8], layout: Layout, scenario: Scenario) -> Host {
    let plan = Plan::decode(bytes).expect("complete independently compiled source graph");
    let intent = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "composerEdited")
        .unwrap();
    assert!(intent.queue && intent.refreshes.len == 0);
    let changed = plan
        .mutations
        .iter()
        .find(|row| plan.str(row.name) == "changed")
        .unwrap();
    assert!(!changed.queue && changed.refreshes.len > 0);
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
            ("draft", Value::str(scenario.value())),
            ("canSend", Value::Bool(scenario == Scenario::Send)),
        ],
    );
    let source = DeliverySource {
        inner: Source {
            schema: Plan::decode(bytes).unwrap(),
            ready: false,
            values,
            snapshot,
            calls: vec![],
        },
    };
    let mut runner = Runner::boot_with_delivery(
        plan,
        source,
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
    runner.data().inner.ready = true;
    runner.data_ready().unwrap();
    for name in ["rootFont", "streamClosedDialog", "streamClosedMiniDialog"] {
        assert_eq!(task(&runner, name), Some(1.0));
    }
    let warm = runner.advance_timed(2.0);
    assert!(warm.error.is_none(), "{warm:?}");
    assert_eq!(warm.now_ms, 2.0);
    preparation(&mut runner, true);
    let view = node(&runner);
    assert!(runner.handlers_of(view).contains(&EventKind::Focus));
    runner.dispatch(view, Event::Focus).unwrap();
    preparation(&mut runner, false);
    assert!(runner
        .handlers_of(view)
        .contains(&EventKind::Selectionchange));
    runner
        .dispatch(
            view,
            Event::FieldSelectionChange(
                scenario.value().to_owned(),
                FieldSelection::caret(scenario.caret() as u32),
            ),
        )
        .unwrap();
    preparation(&mut runner, false);
    field_state(&runner, layout, scenario, false);
    runner
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    Admitted(usize),
    Request(usize),
    Handled,
    Sent,
}

fn queue(runner: &Host, layout: Layout, scenario: Scenario, first: usize, count: usize) -> Value {
    let ty = member(runner, layout, INTENT[1].0, "intentState", "queue").0;
    let item = runner.plan().type_(ty).elem.unwrap();
    Value::list(
        (first..=count)
            .map(|serial| {
                fields(
                    runner.plan(),
                    item,
                    &[
                        ("serial", Value::Number(serial as f64)),
                        ("kind", Value::str("key")),
                        ("text", Value::str("")),
                        ("key", Value::str(scenario.key(serial))),
                        ("shift", Value::Bool(scenario == Scenario::Outdent)),
                        ("command", Value::Bool(false)),
                        ("start", Value::Number(scenario.caret())),
                        ("end", Value::Number(scenario.caret())),
                        ("inputSerial", Value::Number(0.0)),
                    ],
                )
            })
            .collect(),
    )
}

fn field_state(runner: &Host, layout: Layout, scenario: Scenario, sent: bool) {
    for (old, container, name, expected) in [
        ("draft", "composerDraft", "value", Value::str("")),
        (
            "draftOwner",
            "composerDraft",
            "owner",
            Value::str("__unset__"),
        ),
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
            "composerTyping",
            "composerDraft",
            "typing",
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
            Value::str(scenario.value()),
        ),
        (
            "composerSelectionStart",
            "composerSelection",
            "start",
            Value::Number(scenario.caret()),
        ),
        (
            "composerSelectionEnd",
            "composerSelection",
            "end",
            Value::Number(scenario.caret()),
        ),
        (
            "composerFieldInputSerial",
            "fieldProtocol",
            "inputSerial",
            Value::Number(0.0),
        ),
    ] {
        assert_eq!(member(runner, layout, old, container, name).1, expected);
    }
    assert_eq!(runner.slot("commandPending"), Some(&Value::Bool(sent)));
    assert_eq!(
        runner.slot("jumpRequest"),
        Some(&Value::Number(if sent { 1.0 } else { 0.0 }))
    );
    assert_eq!(
        runner.derive("composerText"),
        Some(&Value::str(scenario.value()))
    );
    assert_eq!(
        runner.derive("composerFieldSnapshot"),
        Some(&observation(
            runner.plan(),
            scenario.value(),
            scenario.caret(),
            0.0
        ))
    );
    for name in ["composerRichCurrent", "composerSelectionCurrent"] {
        assert_eq!(runner.derive(name), Some(&Value::Bool(true)), "{name}");
    }
    assert_eq!(
        runner.derive("composerInCode"),
        Some(&Value::Bool(scenario == Scenario::Outdent))
    );
    let data_ty = resource(runner.plan(), "data").0;
    assert_eq!(
        field(
            runner.plan(),
            data_ty,
            runner.resource("data").unwrap(),
            "canSend"
        ),
        Value::Bool(scenario == Scenario::Send)
    );
    let editor_ty = resource(runner.plan(), "editor").0;
    assert_eq!(
        field(
            runner.plan(),
            editor_ty,
            runner.resource("editor").unwrap(),
            "overLimit"
        ),
        Value::Bool(false)
    );
}

fn state(runner: &Host, layout: Layout, scenario: Scenario, mark: Mark) {
    let admitted = if let Mark::Admitted(count) = mark {
        count
    } else {
        2
    };
    let first = if matches!(mark, Mark::Handled | Mark::Request(2)) {
        2
    } else {
        1
    };
    let active = if let Mark::Request(serial) = mark {
        serial as f64
    } else {
        0.0
    };
    let metadata = if mark == Mark::Sent {
        Value::list(vec![])
    } else {
        queue(runner, layout, scenario, first, admitted)
    };
    let message = if mark == Mark::Sent { REFUSED } else { "" };
    let actual: Vec<_> = INTENT
        .iter()
        .map(|(old, name)| member(runner, layout, old, "intentState", name).1)
        .collect();
    assert_eq!(
        actual,
        vec![
            Value::Number(admitted as f64),
            metadata,
            Value::Number(active),
            Value::str(if matches!(mark, Mark::Request(_)) {
                "request"
            } else {
                ""
            }),
            Value::str("rich"),
            Value::str(message),
            observation(runner.plan(), scenario.value(), scenario.caret(), 0.0),
            Value::str(""),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Bool(false),
            Value::Bool(false),
        ],
        "{scenario:?}/{mark:?}: all intent members"
    );
    field_state(runner, layout, scenario, mark == Mark::Sent);
    for name in ["composerIntentContextCurrent", "composerIntentBaseCurrent"] {
        assert_eq!(runner.derive(name), Some(&Value::Bool(true)), "{name}");
    }
    assert_eq!(
        runner.derive("composerIntentReceiptCurrent"),
        Some(&Value::Bool(false))
    );
    let ready = matches!(mark, Mark::Admitted(_) | Mark::Handled);
    assert_eq!(
        task(runner, "composerIntentReady"),
        ready.then_some(runner.now_ms() + 1.0)
    );
    for name in [
        "composerIntentReceipt",
        "composerIntentInvalidated",
        "composerIntentOwnerNotice",
        "composerIntentFormatQuiet",
        "composerFieldDeliver",
        "composerFieldConfirm",
        "composerFieldRestoreFocus",
    ] {
        assert_eq!(task(runner, name), None, "{name}");
    }
    let nodes = runner.kernel().find_by_test_id("composer-intent-notice");
    if message.is_empty() {
        assert!(nodes.is_empty());
    } else {
        assert_eq!(nodes.len(), 1);
        let node = runner.kernel().node_by_key(nodes[0]).unwrap();
        assert_eq!(node.props.str(exact_kernel::PropId::Text), Some(message));
        assert_eq!(
            node.props.str(exact_kernel::PropId::AccessibilityRole),
            Some("status")
        );
    }
}

fn admit(runner: &mut Host, layout: Layout, scenario: Scenario) {
    let now = runner.now_ms();
    for serial in 1..=2 {
        let view = node(runner);
        assert!(runner.handlers_of(view).contains(&EventKind::Key));
        runner
            .dispatch(view, Event::key(scenario.chord(serial)))
            .unwrap();
        assert_eq!(runner.now_ms(), now);
        calls(runner, vec![]);
        traffic(runner, vec![]);
        hosts(
            runner,
            vec![
                command("preventDefault", vec![], Some(view)),
                command("stopPropagation", vec![], Some(view)),
            ],
        );
        state(runner, layout, scenario, Mark::Admitted(serial));
    }
}

fn args(scenario: Scenario, serial: usize) -> Vec<Value> {
    vec![
        Value::str("snapshot"),
        Value::str("draft"),
        Value::str(scenario.value()),
        Value::Number(serial as f64),
        Value::Number(scenario.caret()),
        Value::Number(scenario.caret()),
        Value::str("key"),
        Value::str(""),
        Value::str(scenario.key(serial)),
        Value::Bool(scenario == Scenario::Outdent),
        Value::Bool(false),
        Value::Bool(false),
        Value::str("rich"),
    ]
}

fn request(runner: &mut Host, layout: Layout, scenario: Scenario, serial: usize) -> u64 {
    let due = runner.now_ms() + 1.0;
    assert_eq!(task(runner, "composerIntentReady"), Some(due));
    let mut expected = background(runner, due);
    expected.push(call("answer", "composerIntent", &args(scenario, serial)));
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
    state(runner, layout, scenario, Mark::Request(serial));
    ticket
}

fn result(plan: &Plan, scenario: Scenario) -> Value {
    fields(
        plan,
        mutation(plan, "composerEdited"),
        &[
            ("owner", Value::str("draft")),
            ("value", Value::str(scenario.value())),
            ("mode", Value::str("rich")),
            ("serial", Value::Number(1.0)),
            (
                "kind",
                Value::str(if scenario == Scenario::Outdent {
                    "handled"
                } else {
                    "send"
                }),
            ),
        ],
    )
}

fn fulfill_first(runner: &mut Host, layout: Layout, scenario: Scenario, ticket: u64) {
    let now = runner.now_ms();
    let reply = result(runner.plan(), scenario);
    assert!(runner.fulfill(ticket, response(&reply)).unwrap().is_some());
    calls(
        runner,
        vec![call("parse", "composerIntent", &args(scenario, 1))],
    );
    traffic(runner, vec![]);
    hosts(runner, vec![]);
    assert_eq!(runner.now_ms(), now);
    state(runner, layout, scenario, Mark::Request(1));
}

fn then_first(runner: &mut Host, layout: Layout, scenario: Scenario) -> Option<u64> {
    let now = runner.now_ms();
    let landed = runner.land_then();
    assert!(landed.error.is_none(), "{landed:?}");
    assert_eq!(landed.now_ms, now);
    assert_eq!(landed.receipts.len(), 1);
    assert_eq!(landed.receipts[0].at_ms, now);
    hosts(runner, vec![]);
    if scenario == Scenario::Outdent {
        calls(runner, vec![]);
        traffic(runner, vec![]);
        state(runner, layout, scenario, Mark::Handled);
        None
    } else {
        calls(
            runner,
            vec![call(
                "answer",
                "command",
                &[
                    Value::str("send"),
                    Value::str(""),
                    Value::str("word"),
                    Value::Number(0.0),
                ],
            )],
        );
        let outgoing = runner.take_requests();
        assert_eq!(outgoing.len(), 1);
        let ticket = outgoing[0].ticket;
        assert_eq!(
            outgoing,
            vec![RequestOut {
                ticket,
                target: "changed".to_owned(),
                request: Request::get(SEND_URL),
                forced: false,
            }]
        );
        assert_eq!(runner.pending(), vec![("changed".to_owned(), ticket)]);
        assert!(runner.queued().is_empty());
        state(runner, layout, scenario, Mark::Sent);
        Some(ticket)
    }
}

fn quiet(
    runner: &mut Host,
    layout: Layout,
    scenario: Scenario,
    mark: Mark,
    pending: Vec<(String, u64)>,
) {
    let due = runner.now_ms() + 1.0;
    let expected = background(runner, due);
    let advanced = runner.advance_timed(due);
    assert!(advanced.error.is_none(), "{advanced:?}");
    assert_eq!(advanced.now_ms, due);
    calls(runner, expected);
    traffic(runner, pending);
    hosts(runner, vec![]);
    state(runner, layout, scenario, mark);
}

#[test]
fn public_noop_outdent_consumes_only_first_metadata_and_releases_next_key_without_input() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, Scenario::Outdent);
        admit(&mut runner, layout, Scenario::Outdent);
        let first = request(&mut runner, layout, Scenario::Outdent, 1);
        fulfill_first(&mut runner, layout, Scenario::Outdent, first);
        assert_eq!(then_first(&mut runner, layout, Scenario::Outdent), None);
        let second = request(&mut runner, layout, Scenario::Outdent, 2);
        assert_ne!(first, second);
        quiet(
            &mut runner,
            layout,
            Scenario::Outdent,
            Mark::Request(2),
            vec![("composerEdited".to_owned(), second)],
        );
    }
}

#[test]
fn one_send_is_admitted_and_queued_repeat_refuses_visibly_while_real_send_ticket_is_held() {
    for (bytes, layout) in plans() {
        let mut runner = boot(bytes, layout, Scenario::Send);
        admit(&mut runner, layout, Scenario::Send);
        let first = request(&mut runner, layout, Scenario::Send, 1);
        fulfill_first(&mut runner, layout, Scenario::Send, first);
        let sent = then_first(&mut runner, layout, Scenario::Send).unwrap();
        assert_ne!(first, sent);
        let reply = result(runner.plan(), Scenario::Send);
        assert!(runner.fulfill(first, response(&reply)).unwrap().is_none());
        calls(&mut runner, vec![]);
        traffic(&mut runner, vec![("changed".to_owned(), sent)]);
        hosts(&mut runner, vec![]);
        state(&runner, layout, Scenario::Send, Mark::Sent);
        let now = runner.now_ms();
        let duplicate = runner.land_then();
        assert!(duplicate.error.is_none(), "{duplicate:?}");
        assert_eq!(duplicate.now_ms, now);
        assert!(duplicate.receipts.is_empty());
        calls(&mut runner, vec![]);
        traffic(&mut runner, vec![("changed".to_owned(), sent)]);
        hosts(&mut runner, vec![]);
        state(&runner, layout, Scenario::Send, Mark::Sent);
        quiet(
            &mut runner,
            layout,
            Scenario::Send,
            Mark::Sent,
            vec![("changed".to_owned(), sent)],
        );
    }
}

// The old helper calls private actions. These cases use mounted controls and typed replies.
// Their FENCE/caret8 and a/caret1 setups differ from the old fence-only/caret11 and word/caret4.
// The first replacement Input is scripted. No producer, native edit, undo or focus is executed.
mod bounded_admission {
    use super::super::*;

    const CHANGED: &str = "```ts\nx\n\n```\n- [ ] task";
    const STALE: &str = "The prompt changed. Please try that edit again.";
    const FULL: &str = "Too many edits are waiting. Please try that edit again.";

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Boundary {
        One,
        Admitted,
        Request,
        Input,
        Projection,
        Receipt,
        Refused,
    }

    fn queue_item(
        runner: &Runner<Source>,
        layout: Layout,
        serial: usize,
        language: bool,
        at: f64,
    ) -> Value {
        let list = member(runner, layout, INTENT[1].0, "intentState", "queue").0;
        let ty = runner.plan().type_(list).elem.unwrap();
        fields(
            runner.plan(),
            ty,
            &[
                ("serial", Value::Number(serial as f64)),
                (
                    "kind",
                    Value::str(if language { "language" } else { "key" }),
                ),
                ("text", Value::str(if language { "rust" } else { "" })),
                ("key", Value::str(if language { "" } else { "Enter" })),
                ("shift", Value::Bool(!language)),
                ("command", Value::Bool(false)),
                ("start", Value::Number(if language { 0.0 } else { at })),
                ("end", Value::Number(if language { 5.0 } else { at })),
                ("inputSerial", Value::Number(1.0)),
            ],
        )
    }

    fn record(runner: &Runner<Source>, layout: Layout) -> Vec<Value> {
        INTENT
            .iter()
            .map(|(old, name)| member(runner, layout, old, "intentState", name).1)
            .collect()
    }

    fn field_state(
        runner: &Runner<Source>,
        layout: Layout,
        value: &str,
        at: f64,
        serial: f64,
        base_current: bool,
    ) {
        for (old, container, name, expected) in [
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
                Value::Number(at),
            ),
            (
                "composerSelectionEnd",
                "composerSelection",
                "end",
                Value::Number(at),
            ),
            (
                "composerFieldInputSerial",
                "fieldProtocol",
                "inputSerial",
                Value::Number(serial),
            ),
        ] {
            assert_eq!(member(runner, layout, old, container, name).1, expected);
        }
        assert_eq!(runner.derive("composerText"), Some(&Value::str(value)));
        assert_eq!(
            runner.derive("composerFieldSnapshot"),
            Some(&observation(runner.plan(), value, at, serial))
        );
        for name in [
            "composerRichCurrent",
            "composerSelectionCurrent",
            "composerIntentContextCurrent",
        ] {
            assert_eq!(runner.derive(name), Some(&Value::Bool(true)), "{name}");
        }
        assert_eq!(
            runner.derive("composerIntentBaseCurrent"),
            Some(&Value::Bool(base_current))
        );
    }

    fn idle_tasks(runner: &Runner<Source>) {
        for name in [
            "composerIntentInvalidated",
            "composerIntentOwnerNotice",
            "composerIntentFormatQuiet",
            "composerFieldDeliver",
            "composerFieldConfirm",
            "composerFieldRestoreFocus",
        ] {
            assert_eq!(task(runner, name), None, "{name}");
        }
    }

    fn notice(runner: &Runner<Source>, expected: &str) {
        let found = runner.kernel().find_by_test_id("composer-intent-notice");
        if expected.is_empty() {
            assert!(found.is_empty());
        } else {
            assert_eq!(found.len(), 1);
            let node = runner.kernel().node_by_key(found[0]).unwrap();
            assert_eq!(node.props.str(exact_kernel::PropId::Text), Some(expected));
            assert_eq!(
                node.props.str(exact_kernel::PropId::AccessibilityRole),
                Some("status")
            );
        }
    }

    fn stale_state(runner: &Runner<Source>, layout: Layout, boundary: Boundary) {
        let delivered = matches!(
            boundary,
            Boundary::Projection | Boundary::Receipt | Boundary::Refused
        );
        let done = matches!(boundary, Boundary::Receipt | Boundary::Refused);
        let result = matches!(boundary, Boundary::Input) || delivered;
        let queue = match boundary {
            Boundary::One => vec![queue_item(runner, layout, 1, false, 8.0)],
            Boundary::Receipt => vec![queue_item(runner, layout, 2, true, 8.0)],
            Boundary::Refused => vec![],
            _ => vec![
                queue_item(runner, layout, 1, false, 8.0),
                queue_item(runner, layout, 2, true, 8.0),
            ],
        };
        let phase = match boundary {
            Boundary::Request => "request",
            Boundary::Input => "input",
            Boundary::Projection => "projection",
            _ => "",
        };
        let value = if delivered { CHANGED } else { FENCE };
        let at = if delivered { 9.0 } else { 8.0 };
        let serial = if delivered { 2.0 } else { 1.0 };
        let expected_notice = if boundary == Boundary::Refused {
            STALE
        } else {
            ""
        };
        let observed = observation(
            runner.plan(),
            if done { CHANGED } else { FENCE },
            if done { 9.0 } else { 8.0 },
            if done { 2.0 } else { 1.0 },
        );
        assert_eq!(
            record(runner, layout),
            vec![
                Value::Number(if boundary == Boundary::One { 1.0 } else { 2.0 }),
                Value::list(queue),
                Value::Number(
                    if matches!(
                        boundary,
                        Boundary::Request | Boundary::Input | Boundary::Projection
                    ) {
                        1.0
                    } else {
                        0.0
                    }
                ),
                Value::str(phase),
                Value::str("rich"),
                Value::str(expected_notice),
                observed,
                Value::str(if result { CHANGED } else { "" }),
                Value::Number(if result { 9.0 } else { 0.0 }),
                Value::Number(if result { 9.0 } else { 0.0 }),
                Value::Number(if delivered { 9.0 } else { 0.0 }),
                Value::Number(if delivered { 9.0 } else { 0.0 }),
                Value::Bool(false),
                Value::Bool(delivered),
            ],
            "{boundary:?}: complete intent state"
        );
        field_state(
            runner,
            layout,
            value,
            at,
            serial,
            boundary != Boundary::Projection,
        );
        assert_eq!(
            runner.derive("composerIntentReceiptCurrent"),
            Some(&Value::Bool(boundary == Boundary::Projection))
        );
        let ready = matches!(
            boundary,
            Boundary::One | Boundary::Admitted | Boundary::Receipt
        );
        assert_eq!(
            task(runner, "composerIntentReady"),
            ready.then_some(runner.now_ms() + 1.0)
        );
        assert_eq!(
            task(runner, "composerIntentReceipt"),
            (boundary == Boundary::Projection).then_some(runner.now_ms() + 1.0)
        );
        idle_tasks(runner);
        notice(runner, expected_notice);
    }

    fn key(runner: &mut Runner<Source>) {
        let view = node(runner, "composer");
        assert!(runner.handlers_of(view).contains(&EventKind::Key));
        runner.dispatch(view, Event::key("Shift+Enter")).unwrap();
        calls(runner, vec![]);
        no_requests(runner);
        hosts(
            runner,
            vec![
                command("preventDefault", vec![], Some(view)),
                command("stopPropagation", vec![], Some(view)),
            ],
        );
    }

    fn language(runner: &mut Runner<Source>) {
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
    }

    fn first_args() -> Vec<Value> {
        vec![
            Value::str("snapshot"),
            Value::str("draft"),
            Value::str(FENCE),
            Value::Number(1.0),
            Value::Number(8.0),
            Value::Number(8.0),
            Value::str("key"),
            Value::str(""),
            Value::str("Enter"),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
            Value::str("rich"),
        ]
    }

    fn first_reply(plan: &Plan) -> Value {
        fields(
            plan,
            mutation(plan, "composerEdited"),
            &[
                ("owner", Value::str("draft")),
                ("value", Value::str(FENCE)),
                ("serial", Value::Number(1.0)),
                ("mode", Value::str("rich")),
                ("kind", Value::str("edit")),
                ("start", Value::Number(8.0)),
                ("end", Value::Number(8.0)),
                ("text", Value::str("\n")),
                ("selectionStart", Value::Number(9.0)),
                ("selectionEnd", Value::Number(9.0)),
                ("preserveSelection", Value::Bool(false)),
            ],
        )
    }

    fn take_first(runner: &mut Runner<Source>, layout: Layout) -> (u64, Vec<Value>) {
        let due = runner.now_ms() + 1.0;
        assert_eq!(task(runner, "composerIntentReady"), Some(due));
        let args = first_args();
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
        stale_state(runner, layout, Boundary::Request);
        (ticket, args)
    }

    fn land_first(runner: &mut Runner<Source>, layout: Layout, ticket: u64, args: &[Value]) {
        let reply = first_reply(runner.plan());
        let now = runner.now_ms();
        assert!(runner.fulfill(ticket, response(&reply)).unwrap().is_some());
        calls(runner, vec![call("parse", "composerIntent", args)]);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_eq!(runner.now_ms(), now);
        stale_state(runner, layout, Boundary::Request);
        let landed = runner.land_then();
        assert!(landed.error.is_none(), "{landed:?}");
        assert_eq!(landed.now_ms, now);
        assert_eq!(landed.receipts.len(), 1);
        assert_eq!(landed.receipts[0].at_ms, now);
        calls(runner, vec![]);
        no_requests(runner);
        hosts(
            runner,
            vec![
                command(
                    "setRangeText",
                    vec![
                        Value::str("composer"),
                        Value::str("\n"),
                        Value::Number(8.0),
                        Value::Number(8.0),
                        Value::str("preserve"),
                    ],
                    None,
                ),
                command(
                    "setSelectionRange",
                    vec![
                        Value::str("composer"),
                        Value::Number(9.0),
                        Value::Number(9.0),
                    ],
                    None,
                ),
            ],
        );
        stale_state(runner, layout, Boundary::Input);
    }

    fn changed_input(runner: &mut Runner<Source>, layout: Layout) {
        let observed = observation(runner.plan(), CHANGED, 9.0, 2.0);
        let rich_args = vec![
            Value::str("snapshot"),
            Value::str("draft"),
            Value::str(CHANGED),
            Value::Number(1.0),
            Value::str("insertReplacementText"),
            Value::str("composer"),
        ];
        let expected = vec![
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
                    Value::str(CHANGED),
                    Value::Number(0.0),
                ],
            ),
            standing_call(runner, "data"),
            standing_call(runner, "editor"),
            query_call(runner, "richComposer", &rich_args),
            query_call(
                runner,
                "composerFieldNext",
                &[observed, Value::Number(0.0), Value::Number(1.0)],
            ),
        ];
        dispatch_input(runner, CHANGED, 9, "insertReplacementText", "\n");
        calls(runner, expected);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_eq!(
            runner.resource_args("richComposer"),
            Some(rich_args.as_slice())
        );
        // This minimal reply echoes identity but does not claim a real post-edit parser/catalog.
        assert_eq!(
            runner.resource("richComposer"),
            Some(&rich(runner.plan(), &rich_args))
        );
        stale_state(runner, layout, Boundary::Projection);
    }

    fn tick(runner: &mut Runner<Source>) {
        let due = runner.now_ms() + 1.0;
        let expected = background(runner, due);
        let advanced = runner.advance_timed(due);
        assert!(advanced.error.is_none(), "{advanced:?}");
        assert_eq!(advanced.now_ms, due);
        calls(runner, expected);
        no_requests(runner);
        hosts(runner, vec![]);
    }

    #[test]
    fn prior_edit_receipt_leaves_explicit_metadata_stale_and_next_ready_refuses_it() {
        for (bytes, layout) in plans() {
            let mut runner = boot(bytes, layout, Case::Language);
            key(&mut runner);
            stale_state(&runner, layout, Boundary::One);
            language(&mut runner);
            stale_state(&runner, layout, Boundary::Admitted);
            let (ticket, args) = take_first(&mut runner, layout);
            land_first(&mut runner, layout, ticket, &args);
            changed_input(&mut runner, layout);
            assert_eq!(
                task(&runner, "composerIntentReceipt"),
                Some(runner.now_ms() + 1.0)
            );
            tick(&mut runner);
            stale_state(&runner, layout, Boundary::Receipt);
            let queued = member(&runner, layout, INTENT[1].0, "intentState", "queue").1;
            assert_eq!(
                queued,
                Value::list(vec![queue_item(&runner, layout, 2, true, 8.0)])
            );
            assert_eq!(
                task(&runner, "composerIntentReady"),
                Some(runner.now_ms() + 1.0)
            );
            tick(&mut runner);
            stale_state(&runner, layout, Boundary::Refused);
            tick(&mut runner);
            stale_state(&runner, layout, Boundary::Refused);
        }
    }

    fn bounded_state(
        runner: &Runner<Source>,
        layout: Layout,
        count: usize,
        refused: bool,
        requesting: bool,
    ) {
        let queue = Value::list(
            (1..=count)
                .map(|serial| queue_item(runner, layout, serial, false, 1.0))
                .collect(),
        );
        let message = if refused { FULL } else { "" };
        assert_eq!(
            record(runner, layout),
            vec![
                Value::Number(count as f64),
                queue,
                Value::Number(if requesting { 1.0 } else { 0.0 }),
                Value::str(if requesting { "request" } else { "" }),
                Value::str("rich"),
                Value::str(message),
                observation(runner.plan(), "a", 1.0, 1.0),
                Value::str(""),
                Value::Number(0.0),
                Value::Number(0.0),
                Value::Number(0.0),
                Value::Number(0.0),
                Value::Bool(false),
                Value::Bool(false),
            ],
            "exact admitted metadata and all preserved members"
        );
        field_state(runner, layout, "a", 1.0, 1.0, true);
        assert_eq!(
            runner.derive("composerIntentReceiptCurrent"),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            task(runner, "composerIntentReady"),
            (!requesting).then_some(runner.now_ms() + 1.0)
        );
        assert_eq!(task(runner, "composerIntentReceipt"), None);
        idle_tasks(runner);
        notice(runner, message);
    }

    fn first_bounded_request(runner: &mut Runner<Source>, layout: Layout) {
        let due = runner.now_ms() + 1.0;
        assert_eq!(task(runner, "composerIntentReady"), Some(due));
        let args = vec![
            Value::str("snapshot"),
            Value::str("draft"),
            Value::str("a"),
            Value::Number(1.0),
            Value::Number(1.0),
            Value::Number(1.0),
            Value::str("key"),
            Value::str(""),
            Value::str("Enter"),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
            Value::str("rich"),
        ];
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
        bounded_state(runner, layout, 32, true, true);
    }

    #[test]
    fn public_key33_reports_refusal_without_evicting_or_renumbering_first32() {
        for (bytes, layout) in plans() {
            let mut runner = boot(bytes, layout, Case::Paste);
            let now = runner.now_ms();
            for count in 1..=32 {
                key(&mut runner);
                assert_eq!(runner.now_ms(), now);
                bounded_state(&runner, layout, count, false, false);
            }
            let expected_queue = Value::list(
                (1..=32)
                    .map(|serial| queue_item(&runner, layout, serial, false, 1.0))
                    .collect(),
            );
            key(&mut runner);
            assert_eq!(runner.now_ms(), now);
            bounded_state(&runner, layout, 32, true, false);
            assert_eq!(
                member(&runner, layout, INTENT[1].0, "intentState", "queue").1,
                expected_queue,
            );
            first_bounded_request(&mut runner, layout);
        }
    }
}
