//! Mounted PR hover routing and queued composer line breaks.
//! Both explicit artifact paths also exercise the independently compiled original graph.
//! Fixture requests are observed only; this test runs no external executor.

use crate::composer_accessibility_tests::{measure, set, Replies};
use crate::contract_keys_tests::empty;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::{Plan, TypesId, Value};
use exact_runner::{Answer, DataError, DataSource, Event, Outcome, Request, Runner, Store};

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
const OWNER: &str = "owned-pr";
const LINKED: &str = "linked-pr";
const LINK_URL: &str = "https://example.invalid/pull/2";
const URL_ONLY: &str = "https://example.invalid/commit/abc";

#[derive(Clone, Copy, Debug)]
enum Layout {
    Original,
    Extracted,
}

#[derive(Debug, PartialEq)]
struct DataCall {
    method: &'static str,
    source: String,
    args: Vec<Value>,
}

#[derive(Default)]
struct TracedReplies {
    inner: Replies,
    calls: Vec<DataCall>,
}

impl DataSource for TracedReplies {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls.push(DataCall {
            method: "query",
            source: source.to_owned(),
            args: args.to_vec(),
        });
        self.inner.query(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.calls.push(DataCall {
            method: "answer",
            source: source.to_owned(),
            args: args.to_vec(),
        });
        self.inner.answer(store, source, args)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.calls.push(DataCall {
            method: "parse",
            source: source.to_owned(),
            args: args.to_vec(),
        });
        self.inner.parse(store, source, args, outcome)
    }
}

fn resource_type(plan: &Plan, name: &str) -> TypesId {
    plan.resources
        .iter()
        .find(|row| plan.str(row.name) == name)
        .unwrap()
        .ty
}

fn element(plan: &Plan, mut ty: TypesId, path: &[&str]) -> TypesId {
    for name in path {
        let index = plan.field_index(ty, name).unwrap();
        let field = plan.type_(ty).fields.iter().nth(index).unwrap();
        ty = plan.field(field).ty;
    }
    plan.type_(ty).elem.expect("a fixture list field")
}

fn answer(replies: &mut TracedReplies, plan: &Plan, name: &str, value: Value) {
    let row = plan
        .resources
        .iter()
        .find(|row| plan.str(row.name) == name)
        .unwrap();
    assert!(value.conforms(plan, row.ty), "{name}'s authored type");
    replies
        .inner
        .0
        .insert(plan.str(row.source).to_owned(), value);
}

fn boot(bytes: &[u8]) -> exact_apple::Host<TracedReplies> {
    let plan = Plan::decode(bytes).expect("a complete compiled app plan");
    let mut replies = TracedReplies::default();
    let data_ty = resource_type(&plan, "data");
    let mut data = empty(&plan, data_ty);
    for (path, value) in [
        (&["revision"][..], Value::Number(1.0)),
        (&["connected"][..], Value::Bool(true)),
        (&["threadId"][..], Value::str("fixture-thread")),
        (&["snapshotOwner"][..], Value::str("fixture-snapshot")),
    ] {
        set(&plan, data_ty, &mut data, path, value);
    }
    answer(&mut replies, &plan, "data", data.clone());

    let shell_ty = resource_type(&plan, "shell");
    let mut shell = empty(&plan, shell_ty);
    for (path, value) in [
        (&["panel", "open"][..], Value::Bool(true)),
        (&["panel", "kind"][..], Value::str("pull-request")),
        (&["panel", "key"][..], Value::str("fixture-panel")),
        (&["panel", "pr", "selected"][..], Value::str(OWNER)),
        (
            &["panel", "pr", "surface"][..],
            Value::str("fixture-surface"),
        ),
        (&["panel", "pr", "tab"][..], Value::str("summary")),
    ] {
        set(&plan, shell_ty, &mut shell, path, value);
    }
    answer(&mut replies, &plan, "shell", shell.clone());
    let welcome_ty = resource_type(&plan, "welcome");
    answer(&mut replies, &plan, "welcome", empty(&plan, welcome_ty));

    let detail_ty = resource_type(&plan, "prDetail");
    let chip_ty = element(&plan, detail_ty, &["md", "chips"]);
    let markdown_ty = resource_type(&plan, "prMarkdown");
    let doc_ty = element(&plan, markdown_ty, &["documents"]);
    let block_ty = element(&plan, doc_ty, &["blocks"]);
    let run_ty = element(&plan, block_ty, &["runs"]);
    let mut chips = Vec::new();
    let mut runs = Vec::new();
    for (id, href, target) in [("linked", LINK_URL, LINKED), ("url-only", URL_ONLY, "")] {
        let mut chip = empty(&plan, chip_ty);
        for (field, value) in [
            ("id", Value::str(id)),
            ("href", Value::str(href)),
            ("kind", Value::str("pr-link")),
            ("target", Value::str(target)),
        ] {
            set(&plan, chip_ty, &mut chip, &[field], value);
        }
        chips.push(chip);
        let mut run = empty(&plan, run_ty);
        for (field, value) in [
            ("id", Value::str(id)),
            ("text", Value::str(id)),
            ("href", Value::str(href)),
            ("kind", Value::str("link-start")),
        ] {
            set(&plan, run_ty, &mut run, &[field], value);
        }
        runs.push(run);
    }

    // A nonempty source body makes prMarkdown's cache key differ from an empty baked answer.
    let body_ty = element(&plan, detail_ty, &["bodies"]);
    let mut source_body = empty(&plan, body_ty);
    set(
        &plan,
        body_ty,
        &mut source_body,
        &["id"],
        Value::str("fixture-body"),
    );
    set(
        &plan,
        body_ty,
        &mut source_body,
        &["body"],
        Value::str(&format!("[linked]({LINK_URL}) [url-only]({URL_ONLY})")),
    );
    let mut detail = empty(&plan, detail_ty);
    for (path, value) in [
        (&["ref"][..], Value::str(OWNER)),
        (&["phase"][..], Value::str("content")),
        (&["open"][..], Value::Bool(true)),
        (&["hasBody"][..], Value::Bool(true)),
        (&["bodyId"][..], Value::str("fixture-body")),
        (&["bodies"][..], Value::list(vec![source_body])),
        (&["md", "chips"][..], Value::list(chips)),
    ] {
        set(&plan, detail_ty, &mut detail, path, value);
    }
    answer(&mut replies, &plan, "prDetail", detail.clone());

    let mut block = empty(&plan, block_ty);
    for (field, value) in [
        ("id", Value::str("fixture-block")),
        ("kind", Value::str("paragraph")),
        ("flow", Value::Bool(true)),
        ("runs", Value::list(runs)),
    ] {
        set(&plan, block_ty, &mut block, &[field], value);
    }
    let mut doc = empty(&plan, doc_ty);
    set(&plan, doc_ty, &mut doc, &["id"], Value::str("fixture-body"));
    set(
        &plan,
        doc_ty,
        &mut doc,
        &["blocks"],
        Value::list(vec![block]),
    );
    let mut markdown = empty(&plan, markdown_ty);
    set(
        &plan,
        markdown_ty,
        &mut markdown,
        &["documents"],
        Value::list(vec![doc]),
    );
    answer(&mut replies, &plan, "prMarkdown", markdown.clone());

    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(bytes, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    let runner = host.runner_mut();
    runner.set_time(EPOCH, 0.0).unwrap();
    runner.advance(1250.0).unwrap();
    for (name, expected) in [
        ("data", data),
        ("shell", shell),
        ("prDetail", detail),
        ("prMarkdown", markdown),
    ] {
        assert_eq!(runner.resource(name), Some(&expected), "{name}");
    }
    node(runner, "pr-link-linked");
    node(runner, "pr-link-url-only");
    // Startup traffic is outside the event witness; no requests are fulfilled.
    runner.take_requests();
    runner.take_commands();
    runner.data().calls.clear();
    runner.data().inner.1.clear();
    host
}

fn node(runner: &Runner<TracedReplies>, test_id: &str) -> u32 {
    let keys = runner.kernel().find_by_test_id(test_id);
    assert_eq!(keys.len(), 1, "one mounted {test_id}");
    runner.kernel().node_by_key(keys[0]).unwrap().id
}

#[derive(Debug, PartialEq)]
struct Hover {
    wait: f64,
    end: f64,
    tip: Value,
    on: String,
    open: f64,
    until: f64,
}

fn hover(runner: &Runner<TracedReplies>, layout: Layout) -> Hover {
    let values: Vec<Value> = match layout {
        Layout::Original => [
            "hoverWait",
            "hoverEnd",
            "hoverTip",
            "hoverOn",
            "hoverOpen",
            "hoverUntil",
        ]
        .map(|name| runner.slot(name).unwrap().clone())
        .to_vec(),
        Layout::Extracted => {
            let plan = runner.plan();
            let slot = plan
                .slots
                .iter()
                .find(|slot| plan.str(slot.name) == "hoverState")
                .unwrap();
            let Value::Record(fields) = runner.slot("hoverState").unwrap() else {
                panic!("the extracted root hover record")
            };
            ["wait", "end", "tip", "on", "open", "until"]
                .map(|name| fields[plan.field_index(slot.ty, name).unwrap()].clone())
                .to_vec()
        }
    };
    Hover {
        wait: values[0].as_number().unwrap(),
        end: values[1].as_number().unwrap(),
        tip: values[2].clone(),
        on: values[3].as_str().unwrap().to_owned(),
        open: values[4].as_number().unwrap(),
        until: values[5].as_number().unwrap(),
    }
}

fn tip_field(runner: &Runner<TracedReplies>, layout: Layout, tip: &Value, name: &str) -> Value {
    let plan = runner.plan();
    let slot_name = match layout {
        Layout::Original => "hoverTip",
        Layout::Extracted => "hoverState",
    };
    let slot = plan
        .slots
        .iter()
        .find(|slot| plan.str(slot.name) == slot_name)
        .unwrap();
    let ty = match layout {
        Layout::Original => slot.ty,
        Layout::Extracted => {
            let index = plan.field_index(slot.ty, "tip").unwrap();
            plan.field(plan.type_(slot.ty).fields.iter().nth(index).unwrap())
                .ty
        }
    };
    let Value::Record(fields) = tip else {
        panic!("HoverTip is a record")
    };
    fields[plan.field_index(ty, name).unwrap()].clone()
}

fn route(runner: &Runner<TracedReplies>) -> Vec<Value> {
    [
        "utilityPage",
        "prSelected",
        "prPanelOpen",
        "prTab",
        "commandPending",
        "foldHold",
    ]
    .map(|name| runner.slot(name).unwrap().clone())
    .to_vec()
}

fn event(
    runner: &mut Runner<TracedReplies>,
    test_id: &str,
    event: Event,
    expected: Option<(&str, Vec<Value>)>,
) {
    runner.dispatch(node(runner, test_id), event).unwrap();
    let calls = std::mem::take(&mut runner.data().calls);
    let requests: Vec<_> = runner
        .take_requests()
        .into_iter()
        .map(|request| (request.target, request.request, request.forced))
        .collect();
    match expected {
        Some((target, args)) => {
            assert_eq!(
                calls,
                vec![DataCall {
                    method: "answer",
                    source: "command".to_owned(),
                    args,
                }]
            );
            assert_eq!(
                requests,
                vec![(
                    target.to_owned(),
                    Request::get("https://fixture.invalid/"),
                    false
                )]
            );
        }
        None => {
            assert!(calls.is_empty(), "all data calls: {calls:?}");
            assert!(requests.is_empty(), "all requests: {requests:?}");
        }
    }
    assert!(
        runner.take_commands().is_empty(),
        "the ordered host vector is empty"
    );
}

fn preview(inside: bool) -> Option<(&'static str, Vec<Value>)> {
    Some((
        "localChanged",
        vec![
            Value::str("chatlocal:pr-preview"),
            Value::str(LINKED),
            Value::str(if inside { "1" } else { "0" }),
            Value::Number(0.0),
        ],
    ))
}

#[test]
fn extraction_preserves_hover_then_pr_dispatch_and_guards() {
    for (bytes, layout) in plans() {
        let mut host = boot(bytes);
        let runner = host.runner_mut();
        let initial = hover(runner, layout);
        let initial_route = vec![
            Value::str(""),
            Value::str(""),
            Value::Bool(true),
            Value::str("summary"),
            Value::Bool(false),
            Value::Bool(false),
        ];
        assert_eq!(route(runner), initial_route);
        assert_eq!(initial.on, "");
        assert_eq!(
            tip_field(runner, layout, &initial.tip, "key"),
            Value::str("")
        );

        // PrLinkRun.hover calls hoverTipAt before PrdBody.textLocal in this one delivery.
        event(runner, "pr-link-linked", Event::Hover(true), preview(true));
        let entered = hover(runner, layout);
        assert_eq!(entered.wait, initial.wait);
        assert_eq!(entered.on, "trigger");
        assert_eq!(entered.open, initial.wait + 350.0);
        assert_eq!(entered.end, entered.open);
        assert_eq!(entered.until, initial.until);
        for (field, expected) in [
            ("key", "pr-preview-linked"),
            ("kind", "pr-preview"),
            ("text", LINK_URL),
            ("testId", "pull-request-link-preview"),
        ] {
            assert_eq!(
                tip_field(runner, layout, &entered.tip, field),
                Value::str(expected)
            );
        }
        assert_eq!(route(runner), initial_route);

        event(
            runner,
            "pr-link-linked",
            Event::Hover(false),
            preview(false),
        );
        let left = hover(runner, layout);
        assert_eq!(left.wait, initial.wait);
        assert_eq!(left.tip, entered.tip);
        assert_eq!(left.on, "");
        assert_eq!(left.open, entered.open);
        assert_eq!(left.end, initial.wait);
        assert_eq!(left.until, initial.wait);
        assert_eq!(route(runner), initial_route);

        // The hover mutation's guard is false, but the later panel-local dispatch still runs.
        event(
            runner,
            "pr-link-linked",
            Event::Hover(false),
            preview(false),
        );
        assert_eq!(hover(runner, layout), left);
        assert_eq!(route(runner), initial_route);

        // A URL-only chip updates its tip and skips the panel-preview dispatch.
        event(runner, "pr-link-url-only", Event::Hover(true), None);
        let url = hover(runner, layout);
        assert_eq!(url.wait, initial.wait);
        assert_eq!(url.on, "trigger");
        assert_eq!(url.open, initial.wait);
        assert_eq!(url.end, initial.wait);
        assert_eq!(url.until, left.until);
        assert_eq!(
            tip_field(runner, layout, &url.tip, "kind"),
            Value::str("tip")
        );
        assert_eq!(
            tip_field(runner, layout, &url.tip, "text"),
            Value::str(URL_ONLY)
        );
        assert_eq!(route(runner), initial_route);

        // Press uses the same textLocal wrapper's retained PR action branch.
        event(
            runner,
            "pr-link-linked",
            Event::Press,
            Some((
                "changed",
                vec![
                    Value::str("pageslocal:pr-act-link-open"),
                    Value::str(OWNER),
                    Value::str(&format!("\n{LINKED}\n{LINK_URL}\n")),
                    Value::Number(0.0),
                ],
            )),
        );
        let mut pending_route = initial_route.clone();
        pending_route[4] = Value::Bool(true);
        assert_eq!(route(runner), pending_route);
        assert_eq!(hover(runner, layout), url);
        event(runner, "pr-link-linked", Event::Press, None);
        assert_eq!(route(runner), pending_route);
        assert_eq!(hover(runner, layout), url);
    }
}

// Two public beforeinput line breaks share the FIFO already used by the paste witnesses.
// Setup uses a named extra Input to reach word/caret4; its serial2 differs from the old helper0.
// Replies and replacement Inputs are scripted consumer boundaries, without native editing or undo.
mod rapid_line_breaks {
    use super::super::*;

    fn value(delivered: usize) -> &'static str {
        match delivered {
            0 => "word",
            1 => "word\n",
            2 => "word\n\n",
            _ => panic!("two delivered line breaks"),
        }
    }

    fn queued(runner: &Runner<Source>, layout: Layout, first: usize, admitted: usize) -> Value {
        let ty = member(runner, layout, INTENT[1].0, "intentState", "queue").0;
        let item = runner.plan().type_(ty).elem.unwrap();
        Value::list(
            (first..=admitted)
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
                            ("inputSerial", Value::Number(2.0)),
                        ],
                    )
                })
                .collect(),
        )
    }

    fn field_state(runner: &Runner<Source>, layout: Layout, delivered: usize) {
        let value = value(delivered);
        let caret = 4.0 + delivered as f64;
        let serial = 2.0 + delivered as f64;
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
            assert_eq!(member(runner, layout, old, container, name).1, expected);
        }
        assert_eq!(runner.derive("composerText"), Some(&Value::str(value)));
        assert_eq!(
            runner.derive("composerFieldSnapshot"),
            Some(&observation(runner.plan(), value, caret, serial))
        );
        for name in ["composerRichCurrent", "composerSelectionCurrent"] {
            assert_eq!(runner.derive(name), Some(&Value::Bool(true)), "{name}");
        }
    }

    fn state(runner: &Runner<Source>, layout: Layout, turn: usize, stage: Stage, admitted: usize) {
        let delivered = turn + usize::from(stage.delivered());
        let observed = turn + usize::from(stage == Stage::Receipt);
        let first = turn + if stage == Stage::Receipt { 2 } else { 1 };
        let result_value = if stage.result() {
            value(turn + 1)
        } else if turn > 0 {
            value(turn)
        } else {
            ""
        };
        let target = if stage.result() {
            5.0 + turn as f64
        } else if turn > 0 {
            4.0 + turn as f64
        } else {
            0.0
        };
        let input = if stage.delivered() {
            5.0 + turn as f64
        } else if turn > 0 {
            4.0 + turn as f64
        } else {
            0.0
        };
        let active = if matches!(stage, Stage::Admitted | Stage::Receipt) {
            0.0
        } else {
            turn as f64 + 1.0
        };
        let actual: Vec<_> = INTENT
            .iter()
            .map(|(old, name)| member(runner, layout, old, "intentState", name).1)
            .collect();
        assert_eq!(
            actual,
            vec![
                Value::Number(admitted as f64),
                queued(runner, layout, first, admitted),
                Value::Number(active),
                Value::str(stage.phase()),
                Value::str("rich"),
                Value::str(""),
                observation(
                    runner.plan(),
                    value(observed),
                    4.0 + observed as f64,
                    2.0 + observed as f64
                ),
                Value::str(result_value),
                Value::Number(target),
                Value::Number(target),
                Value::Number(input),
                Value::Number(input),
                Value::Bool(false),
                Value::Bool(stage != Stage::Input && (stage.delivered() || turn > 0)),
            ],
            "turn{turn}/{stage:?}: all fourteen intent members"
        );
        field_state(runner, layout, delivered);
        assert_eq!(
            runner.derive("composerIntentContextCurrent"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            runner.derive("composerIntentBaseCurrent"),
            Some(&Value::Bool(stage != Stage::Projection))
        );
        assert_eq!(
            runner.derive("composerIntentReceiptCurrent"),
            Some(&Value::Bool(stage == Stage::Projection))
        );
        let ready = stage == Stage::Admitted || stage == Stage::Receipt && turn + 1 < admitted;
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
            "composerIntentFormatQuiet",
            "composerFieldDeliver",
            "composerFieldConfirm",
            "composerFieldRestoreFocus",
        ] {
            assert_eq!(task(runner, name), None, "{name}");
        }
        assert!(runner
            .kernel()
            .find_by_test_id("composer-intent-notice")
            .is_empty());
    }

    fn boot_word(bytes: &[u8], layout: Layout) -> Runner<Source> {
        let mut runner = boot(bytes, layout, Case::Paste);
        dispatch_input(&mut runner, "word", 4, "insertText", "word");
        let _word = preparation(&mut runner, false);
        field_state(&runner, layout, 0);
        for name in [
            "composerIntentReady",
            "composerIntentReceipt",
            "composerIntentOwnerNotice",
        ] {
            assert_eq!(task(&runner, name), None, "{name}");
        }
        runner
    }

    fn admit_two(runner: &mut Runner<Source>, layout: Layout) {
        let now = runner.now_ms();
        let observed = observation(runner.plan(), "word", 4.0, 2.0);
        for admitted in 1..=2 {
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
                        "word".to_owned(),
                        FieldSelection::caret(4),
                        InputDetails {
                            input_type: "insertLineBreak".to_owned(),
                            data: None,
                            cancelable: true,
                            is_composing: false,
                        },
                    ),
                )
                .unwrap();
            assert_eq!(runner.now_ms(), now);
            calls(runner, expected);
            no_requests(runner);
            hosts(runner, vec![command("preventDefault", vec![], Some(view))]);
            state(runner, layout, 0, Stage::Admitted, admitted);
        }
    }

    fn args(turn: usize) -> Vec<Value> {
        vec![
            Value::str("snapshot"),
            Value::str("draft"),
            Value::str(value(turn)),
            Value::Number(turn as f64 + 1.0),
            Value::Number(4.0 + turn as f64),
            Value::Number(4.0 + turn as f64),
            Value::str("key"),
            Value::str(""),
            Value::str("Enter"),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
            Value::str("rich"),
        ]
    }

    fn request(runner: &mut Runner<Source>, layout: Layout, turn: usize) -> u64 {
        let due = runner.now_ms() + 1.0;
        assert_eq!(task(runner, "composerIntentReady"), Some(due));
        let mut expected = background(runner, due);
        expected.push(call("answer", "composerIntent", &args(turn)));
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
        state(runner, layout, turn, Stage::Request, 2);
        ticket
    }

    fn held_tick(runner: &mut Runner<Source>, layout: Layout, turn: usize, ticket: u64) {
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
            vec![("composerEdited".to_owned(), ticket)]
        );
        assert!(runner.queued().is_empty());
        state(runner, layout, turn, Stage::Request, 2);
    }

    fn reply(plan: &Plan, turn: usize) -> Value {
        let at = 4.0 + turn as f64;
        fields(
            plan,
            mutation(plan, "composerEdited"),
            &[
                ("owner", Value::str("draft")),
                ("value", Value::str(value(turn))),
                ("mode", Value::str("rich")),
                ("serial", Value::Number(turn as f64 + 1.0)),
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

    fn land(runner: &mut Runner<Source>, layout: Layout, turn: usize, ticket: u64) {
        let now = runner.now_ms();
        let answer = reply(runner.plan(), turn);
        assert!(runner.fulfill(ticket, response(&answer)).unwrap().is_some());
        calls(runner, vec![call("parse", "composerIntent", &args(turn))]);
        no_requests(runner);
        hosts(runner, vec![]);
        assert_eq!(runner.now_ms(), now);
        state(runner, layout, turn, Stage::Request, 2);
        let landed = runner.land_then();
        assert!(landed.error.is_none(), "{landed:?}");
        assert_eq!(landed.now_ms, now);
        assert_eq!(landed.receipts.len(), 1);
        assert_eq!(landed.receipts[0].at_ms, now);
        calls(runner, vec![]);
        no_requests(runner);
        let at = 4.0 + turn as f64;
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
        state(runner, layout, turn, Stage::Input, 2);
    }

    fn input(runner: &mut Runner<Source>, layout: Layout, turn: usize) {
        let delivered = turn + 1;
        let value = value(delivered);
        let at = 4.0 + delivered as f64;
        let observed = observation(runner.plan(), value, at, 2.0 + delivered as f64);
        let rich_args = vec![
            Value::str("snapshot"),
            Value::str("draft"),
            Value::str(value),
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
                    Value::str(value),
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
        dispatch_input(runner, value, at as u32, "insertReplacementText", "\n");
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
        state(runner, layout, turn, Stage::Projection, 2);
    }

    fn receipt(runner: &mut Runner<Source>, layout: Layout, turn: usize) {
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
        state(runner, layout, turn, Stage::Receipt, 2);
    }

    #[test]
    fn two_public_beforeinput_line_breaks_wait_for_each_input_and_receipt_in_order() {
        for (bytes, layout) in plans() {
            let mut runner = boot_word(bytes, layout);
            admit_two(&mut runner, layout);
            let mut tickets = vec![];
            for turn in 0..2 {
                let ticket = request(&mut runner, layout, turn);
                assert!(!tickets.contains(&ticket));
                tickets.push(ticket);
                held_tick(&mut runner, layout, turn, ticket);
                land(&mut runner, layout, turn, ticket);
                input(&mut runner, layout, turn);
                receipt(&mut runner, layout, turn);
            }
            assert_eq!(tickets.len(), 2);
            let due = runner.now_ms() + 1.0;
            let expected = background(&runner, due);
            let quiet = runner.advance_timed(due);
            assert!(quiet.error.is_none(), "{quiet:?}");
            assert_eq!(quiet.now_ms, due);
            calls(&mut runner, expected);
            no_requests(&mut runner);
            hosts(&mut runner, vec![]);
            state(&runner, layout, 1, Stage::Receipt, 2);
        }
    }
}
