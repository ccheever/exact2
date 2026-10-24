//! Undo routing, modifier capture and scheduled-editor continuations.
//! Ordinary runs use the baked plan. Both explicit artifact paths enable differential tests.
//! No request produced here is executed by an external network client.

use super::composer_accessibility_tests::{measure, set, Replies};
use super::contract_keys_tests::empty;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Event, KeyboardEvent, Outcome, Request, Runner, Store,
};

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
const UNDO_UNTIL: f64 = EPOCH + 9_000.0;

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

fn boot(bytes: &[u8]) -> exact_apple::Host<TracedReplies> {
    let plan = Plan::decode(bytes).expect("a complete compiled app plan");
    let data = plan
        .resources
        .iter()
        .find(|row| plan.str(row.name) == "data")
        .expect("the app's Snapshot resource");
    let mut snapshot = empty(&plan, data.ty);
    for (path, value) in [
        (&["revision"][..], Value::Number(1.0)),
        (&["connected"][..], Value::Bool(true)),
        (&["sidebarOpen"][..], Value::Bool(true)),
        (&["sidebar", "undoUntil"][..], Value::Number(UNDO_UNTIL)),
    ] {
        set(&plan, data.ty, &mut snapshot, path, value);
    }
    assert!(snapshot.conforms(&plan, data.ty));

    // Keep one authored dispatch row; keyboardDispatch's selection is tested separately.
    let shortcuts = plan
        .resources
        .iter()
        .find(|row| plan.str(row.name) == "shortcuts")
        .expect("the app's KeyDispatch list resource");
    let row = Value::record(vec![
        Value::str("undo-fixture"),
        Value::str("thread.undo"),
        Value::str("Meta+z"),
        Value::str("command"),
        Value::str("sidebar:undo"),
        Value::str("z"),
        Value::str("Undo"),
    ]);
    assert_eq!(plan.str(shortcuts.source), "keyboardDispatch");
    let element = plan.type_(shortcuts.ty).elem.expect("a list");
    assert!(row.conforms(&plan, element));
    let rows = Value::list(vec![row]);

    // Do not call Replies::fixture/first_frame: those use the crate's baked PLAN.
    let mut replies = TracedReplies::default();
    replies
        .inner
        .0
        .insert(plan.str(data.source).to_owned(), snapshot.clone());
    replies
        .inner
        .0
        .insert(plan.str(shortcuts.source).to_owned(), rows.clone());
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(bytes, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    let runner = host.runner_mut();
    runner.set_time(EPOCH, 0.0).unwrap();
    runner.advance(1250.0).unwrap();
    assert_eq!(runner.resource("data"), Some(&snapshot));
    assert_eq!(runner.resource("shortcuts"), Some(&rows));
    assert_eq!(runner.slot("commandPending"), Some(&Value::Bool(false)));
    let startup = runner.take_requests();
    assert!(
        startup
            .iter()
            .all(|request| request.target != "data" && request.target != "shortcuts"),
        "the two witness resources must be answered by the fixture"
    );
    runner.take_commands();
    runner.data().inner.1.clear();
    runner.data().calls.clear();
    host
}

fn node(runner: &Runner<TracedReplies>, test_id: &str) -> u32 {
    let keys = runner.kernel().find_by_test_id(test_id);
    assert_eq!(keys.len(), 1, "one mounted {test_id}");
    runner.kernel().node_by_key(keys[0]).unwrap().id
}

fn undo_state(runner: &Runner<TracedReplies>, layout: Layout) -> (String, f64) {
    let (held, released) = match layout {
        Layout::Original => (
            runner.slot("undoHeldKey").expect("the original scalar"),
            runner
                .slot("undoReleasedUntil")
                .expect("the original scalar"),
        ),
        Layout::Extracted => {
            let plan = runner.plan();
            let slot = plan
                .slots
                .iter()
                .find(|slot| plan.str(slot.name) == "modifierOwnership")
                .expect("the extracted root record");
            let Value::Record(fields) = runner
                .slot("modifierOwnership")
                .expect("the extracted root record value")
            else {
                panic!("modifierOwnership is a record")
            };
            (
                &fields[plan.field_index(slot.ty, "undoHeldKey").unwrap()],
                &fields[plan.field_index(slot.ty, "undoReleasedUntil").unwrap()],
            )
        }
    };
    (
        held.as_str().expect("a held-key string").to_owned(),
        released.as_number().expect("a release deadline"),
    )
}

fn modifier_meta(runner: &Runner<TracedReplies>, layout: Layout, speed: bool) -> bool {
    let (slot_name, member) = match layout {
        Layout::Original => (
            if speed {
                "speedModifierState"
            } else {
                "shortcutModifiers"
            },
            None,
        ),
        Layout::Extracted => (
            "modifierOwnership",
            Some(if speed { "speed" } else { "shortcut" }),
        ),
    };
    let plan = runner.plan();
    let slot = plan
        .slots
        .iter()
        .find(|slot| plan.str(slot.name) == slot_name)
        .expect("the layout's modifier slot");
    let value = runner.slot(slot_name).expect("the layout's modifier value");
    let (ty, value) = if let Some(member) = member {
        let index = plan.field_index(slot.ty, member).unwrap();
        let field = plan.type_(slot.ty).fields.iter().nth(index).unwrap();
        let Value::Record(fields) = value else {
            panic!("the extracted ownership slot is a record")
        };
        (plan.field(field).ty, &fields[index])
    } else {
        (slot.ty, value)
    };
    let Value::Record(fields) = value else {
        panic!("the shortcut modifier state is a record")
    };
    match &fields[plan.field_index(ty, "metaKey").unwrap()] {
        Value::Bool(held) => *held,
        _ => panic!("metaKey is a bool"),
    }
}

fn capture(
    runner: &mut Runner<TracedReplies>,
    wire: &str,
    expected_calls: &[DataCall],
) -> Vec<String> {
    let event = KeyboardEvent::parse(wire).expect("an exact KeyboardEvent wire");
    runner
        .dispatch(node(runner, "t3-code"), Event::Keycapture(event))
        .unwrap();
    let calls = std::mem::take(&mut runner.data().calls);
    assert_eq!(calls.as_slice(), expected_calls, "all data calls: {wire}");
    assert!(
        runner.take_requests().is_empty(),
        "no outgoing requests: {wire}"
    );
    runner
        .take_commands()
        .into_iter()
        .map(|command| {
            assert!(
                command.args.is_empty(),
                "these cancellation commands take no arguments"
            );
            command.name
        })
        .collect()
}

#[test]
fn extraction_preserves_undo_repeats_and_staged_modifier_updates() {
    for (bytes, layout) in plans() {
        let mut host = boot(bytes);
        let runner = host.runner_mut();
        assert_eq!(
            undo_state(runner, layout),
            (String::new(), 0.0),
            "{layout:?}"
        );
        runner
            .dispatch(node(runner, "shortcut-thread.undo"), Event::Press)
            .unwrap();
        assert_eq!(undo_state(runner, layout), ("z".to_owned(), 0.0));
        assert!(runner.take_commands().is_empty());
        let undo_args = vec![
            Value::str("sidebar:undo"),
            Value::str("z"),
            Value::str(""),
            Value::Number(0.0),
        ];
        assert_eq!(runner.data_ref().inner.1, vec![undo_args.clone()]);
        assert_eq!(
            std::mem::take(&mut runner.data().calls),
            vec![DataCall {
                method: "answer",
                source: "command".to_owned(),
                args: undo_args,
            }]
        );
        // Tickets are allocator identities; the complete ordered request payloads are compared.
        let requests: Vec<_> = runner
            .take_requests()
            .into_iter()
            .map(|request| (request.target, request.request, request.forced))
            .collect();
        assert_eq!(
            requests,
            vec![(
                "changed".to_owned(),
                Request::get("https://fixture.invalid/"),
                false,
            )]
        );

        for wire in ["Meta+z\nKeyZ\ntrue", "Meta+z\nKeyW\ntrue"] {
            assert_eq!(
                capture(runner, wire, &[]),
                vec!["preventDefault".to_owned(), "stopPropagation".to_owned()],
                "{layout:?}: {wire}"
            );
            assert_eq!(undo_state(runner, layout), ("z".to_owned(), 0.0));
        }
        assert!(capture(runner, "Meta+x\nKeyX\ntrue", &[]).is_empty());
        assert!(capture(runner, "z\nKeyZ\ntrue", &[]).is_empty());
        assert_eq!(undo_state(runner, layout), ("z".to_owned(), 0.0));

        // Only a modifier's own key can set its flag.
        assert!(capture(runner, "Meta+Meta\nMetaLeft\nfalse", &[]).is_empty());
        assert_eq!(undo_state(runner, layout), ("z".to_owned(), 0.0));
        assert!(modifier_meta(runner, layout, false));
        assert!(modifier_meta(runner, layout, true));

        // keyboardDispatch's 22nd argument is undoClaimed; all its other inputs stay fixed.
        let mut released_args = runner.resource_args("shortcuts").unwrap().to_vec();
        assert_eq!(released_args.len(), 24);
        assert_eq!(released_args[21], Value::Bool(true));
        released_args[21] = Value::Bool(false);
        let released_call = DataCall {
            method: "answer",
            source: "keyboardDispatch".to_owned(),
            args: released_args,
        };
        // This delivery releases Undo and clears both previously true modifier flags.
        assert!(capture(runner, "z\nKeyZ\nfalse", &[released_call]).is_empty());
        assert_eq!(
            undo_state(runner, layout),
            (String::new(), UNDO_UNTIL),
            "the later modifier copy must preserve the cleared key and released claim"
        );
        assert!(!modifier_meta(runner, layout, false));
        assert!(!modifier_meta(runner, layout, true));
        assert!(capture(runner, "Meta+z\nKeyZ\ntrue", &[]).is_empty());
        assert_eq!(undo_state(runner, layout), (String::new(), UNDO_UNTIL));
        assert_eq!(
            runner.data_ref().inner.1.len(),
            1,
            "capture never sends a second Undo"
        );
    }
}

// Scheduled editor writes use public events and the real mutation continuation.
mod webhook_completion {
    use super::super::composer_accessibility_tests::set;
    use super::super::contract_keys_tests::empty;
    use super::{plans, DataCall, Layout, EPOCH};
    use exact_kernel::Kernel;
    use exact_plan::{EventKind, Plan, TypesId, Value};
    use exact_runner::{
        Answer, Command, DataError, DataSource, Event, KeyboardEvent, Outcome, Request, RequestOut,
        Response, Runner, Store, Viewport,
    };
    use std::collections::HashMap;

    const KEEP: &str = "task-webhook-keep";
    const SUBJECT: &str = "environment|task";

    struct Source {
        ready: bool,
        values: HashMap<String, Value>,
        scheduled_source: String,
        scheduled_closed: Value,
        scheduled_open: Value,
        reply_type: TypesId,
        calls: Vec<DataCall>,
    }

    impl Source {
        fn record(&mut self, method: &'static str, source: &str, args: &[Value]) {
            self.calls.push(DataCall {
                method,
                source: source.to_owned(),
                args: args.to_vec(),
            });
        }

        fn value(&self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            if source == self.scheduled_source {
                assert_eq!(args.len(), 13);
                return Ok(if args[2] == Value::str("task") {
                    assert_eq!(args[3], Value::str(SUBJECT));
                    self.scheduled_open.clone()
                } else {
                    assert_eq!(args[2], Value::str(""));
                    self.scheduled_closed.clone()
                });
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
            _: &mut Store,
            source: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            self.record("answer", source, args);
            if source == "command" {
                Ok(Answer::Later(Request::get(
                    "https://fixture.invalid/webhook",
                )))
            } else {
                self.value(source, args).map(Answer::Now)
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
            assert_eq!(source, "command");
            let Outcome::Response(response) = outcome else {
                panic!("a real reply")
            };
            assert_eq!(response.status, 200);
            assert!(response.headers.is_empty());
            Ok(Answer::Now(Value::from_bytes(&response.body).unwrap()))
        }
    }

    fn resource(plan: &Plan, name: &str) -> (TypesId, String) {
        let row = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap();
        (row.ty, plan.str(row.source).to_owned())
    }

    fn element(plan: &Plan, ty: TypesId, name: &str) -> TypesId {
        let index = plan.field_index(ty, name).unwrap();
        let field = plan.type_(ty).fields.iter().nth(index).unwrap();
        plan.type_(plan.field(field).ty).elem.unwrap()
    }

    fn fields(plan: &Plan, ty: TypesId, fields: &[(&str, Value)]) -> Value {
        let mut value = empty(plan, ty);
        for (name, next) in fields {
            set(plan, ty, &mut value, &[*name], next.clone());
        }
        assert!(value.conforms(plan, ty));
        value
    }

    fn overlay(
        plan: &Plan,
        values: &mut HashMap<String, Value>,
        name: &str,
        fields: &[(&str, Value)],
    ) {
        let (ty, source) = resource(plan, name);
        let mut value = values[&source].clone();
        for (name, next) in fields {
            set(plan, ty, &mut value, &[*name], next.clone());
        }
        assert!(value.conforms(plan, ty));
        values.insert(source, value);
    }

    fn slot(runner: &Runner<Source>, layout: Layout, flat: &str) -> Value {
        let projection = match flat {
            "settingsOpen" => Some(("settingsPageState", "open")),
            "settingsRoute" => Some(("settingsPageState", "route")),
            "settingsRefresh" => Some(("settingsPageState", "refresh")),
            "keybindingSaving" => Some(("keybindingEditor", "saving")),
            "keybindingOpen" => Some(("keybindingEditor", "open")),
            "keybindingPrevious" => Some(("keybindingEditor", "previous")),
            "keybindingRecording" => Some(("keybindingEditor", "recording")),
            "keybindingDrafts" => Some(("keybindingEditor", "drafts")),
            _ => None,
        };
        let Some((record, field)) = projection.filter(|_| matches!(layout, Layout::Extracted))
        else {
            return runner.slot(flat).unwrap().clone();
        };
        let plan = runner.plan();
        let row = plan
            .slots
            .iter()
            .find(|row| plan.str(row.name) == record)
            .unwrap();
        let Value::Record(values) = runner.slot(record).unwrap() else {
            panic!("owner record")
        };
        values[plan.field_index(row.ty, field).unwrap()].clone()
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

    fn prop(runner: &Runner<Source>, view: u32, name: &str) -> Value {
        let node = runner.kernel().node(view).unwrap();
        let value = &node
            .props
            .iter()
            .find(|(id, _)| id.name() == name)
            .unwrap()
            .1;
        if let Some(text) = value.as_str() {
            Value::str(text)
        } else {
            Value::Bool(value.as_bool().expect("the disabled boolean"))
        }
    }

    fn call(method: &'static str, source: &str, args: &[Value]) -> DataCall {
        DataCall {
            method,
            source: source.to_owned(),
            args: args.to_vec(),
        }
    }

    fn calls(runner: &mut Runner<Source>, expected: Vec<DataCall>) {
        assert_eq!(std::mem::take(&mut runner.data().calls), expected);
    }

    fn hosts(runner: &mut Runner<Source>, expected: Vec<Command>) {
        assert_eq!(
            runner.take_commands(),
            expected,
            "the complete ordered host commands"
        );
    }

    fn no_requests(runner: &mut Runner<Source>) {
        assert!(
            runner.take_requests().is_empty(),
            "the complete outgoing request vector"
        );
        assert!(runner.pending().is_empty());
        assert!(runner.queued().is_empty());
    }

    #[derive(Debug)]
    struct Setup {
        phase: &'static str,
        calls: Vec<DataCall>,
        hosts: Vec<Command>,
    }

    // Setup is a separate retained trace with finite named host allowances.
    // Public menu keys request a popover; no host executor or hit testing runs here.
    fn setup(runner: &mut Runner<Source>, phase: &'static str) -> Setup {
        no_requests(runner);
        let trace = Setup {
            phase,
            calls: std::mem::take(&mut runner.data().calls),
            hosts: runner.take_commands(),
        };
        for call in &trace.calls {
            assert_eq!(call.method, "answer", "{trace:?}");
            assert!(
                runner
                    .plan()
                    .resources
                    .iter()
                    .any(|row| runner.plan().str(row.source) == call.source),
                "{trace:?}"
            );
        }
        for command in &trace.hosts {
            let allowed = match trace.phase {
                "boot" => {
                    command.name == "setRootFontSize" && command.args == vec![Value::Number(16.0)]
                }
                "task-menu-key" => ["preventDefault", "stopPropagation", "showPopover", "focus"]
                    .contains(&command.name.as_str()),
                "settings-open" | "scheduled-page" | "editor-open" | "draft-input" => false,
                _ => panic!("an unregistered setup phase"),
            };
            assert!(allowed, "the complete setup host trace: {trace:?}");
        }
        trace
    }

    fn press(runner: &mut Runner<Source>, id: &str) {
        let view = node(runner, id);
        assert!(runner.handlers_of(view).contains(&EventKind::Press), "{id}");
        runner.dispatch(view, Event::Press).unwrap();
    }

    fn boot(bytes: &[u8], layout: Layout) -> (Runner<Source>, Vec<Setup>) {
        let plan = Plan::decode(bytes).expect("a separately compiled complete app graph");
        let mutation = plan
            .mutations
            .iter()
            .find(|row| plan.str(row.name) == "changed")
            .unwrap();
        assert!(!mutation.queue);
        let reply_type = mutation.ty;
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
        overlay(
            &plan,
            &mut values,
            "data",
            &[
                ("revision", Value::Number(1.0)),
                ("connected", Value::Bool(true)),
                ("sidebarOpen", Value::Bool(true)),
            ],
        );
        overlay(
            &plan,
            &mut values,
            "settings",
            &[("environmentId", Value::str("environment"))],
        );
        let (nav_ty, _) = resource(&plan, "settingsNavigation");
        let nav_item = fields(
            &plan,
            element(&plan, nav_ty, "items"),
            &[
                ("id", Value::str("scheduled-tasks")),
                ("route", Value::str("scheduled-tasks")),
                ("title", Value::str("Scheduled tasks")),
            ],
        );
        overlay(
            &plan,
            &mut values,
            "settingsNavigation",
            &[
                ("items", Value::list(vec![nav_item])),
                ("count", Value::Number(1.0)),
            ],
        );
        let (page_ty, scheduled_source) = resource(&plan, "scheduled");
        let task = fields(
            &plan,
            element(&plan, page_ty, "tasks"),
            &[
                ("id", Value::str("task")),
                ("environmentId", Value::str("environment")),
                ("title", Value::str("Fixture")),
                ("prompt", Value::str("Review")),
                ("webhook", Value::Bool(true)),
                ("enabled", Value::Bool(true)),
                ("first", Value::Bool(true)),
            ],
        );
        let section = fields(
            &plan,
            element(&plan, page_ty, "sections"),
            &[
                ("id", Value::str("environment")),
                ("label", Value::str("Fixture machine")),
                ("state", Value::str("ready")),
                ("tasks", Value::list(vec![task.clone()])),
            ],
        );
        let draft = fields(
            &plan,
            element(&plan, page_ty, "editors"),
            &[
                ("key", Value::str("environment:task")),
                ("id", Value::str("task")),
                ("environmentId", Value::str("environment")),
                ("title", Value::str("Fixture")),
                ("prompt", Value::str("Review")),
                ("enabled", Value::Bool(true)),
                ("address", Value::str("https://fixture.invalid/hooks/token")),
                ("copyable", Value::Bool(true)),
                ("scheduleMode", Value::str("webhook")),
                ("signatureEncoding", Value::str("hex")),
                ("intervalMinutes", Value::str("60")),
                ("timeOfDay", Value::str("09:00")),
                ("monday", Value::Bool(true)),
                ("projectId", Value::str("project")),
                ("workspaceMode", Value::str("root")),
                ("baseRef", Value::str("main")),
                ("startFromOrigin", Value::Bool(true)),
                ("modelKey", Value::str("codex:model")),
            ],
        );
        overlay(
            &plan,
            &mut values,
            "scheduled",
            &[
                ("available", Value::Bool(true)),
                ("writable", Value::Bool(true)),
                ("environment", Value::str("Fixture machine")),
                ("tasks", Value::list(vec![task])),
                ("sections", Value::list(vec![section])),
            ],
        );
        let scheduled_closed = values[&scheduled_source].clone();
        let mut scheduled_open = scheduled_closed.clone();
        set(
            &plan,
            page_ty,
            &mut scheduled_open,
            &["editors"],
            Value::list(vec![draft]),
        );
        assert!(scheduled_open.conforms(&plan, page_ty));
        let mut runner = Runner::boot_stored(
            plan,
            Source {
                ready: false,
                values,
                scheduled_source,
                scheduled_closed,
                scheduled_open,
                reply_type,
                calls: Vec::new(),
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
        let mut traces = vec![setup(&mut runner, "boot")];
        press(&mut runner, "connection-settings");
        assert_eq!(slot(&runner, layout, "settingsOpen"), Value::Bool(true));
        traces.push(setup(&mut runner, "settings-open"));
        press(&mut runner, "settings-scheduled-tasks");
        assert_eq!(
            slot(&runner, layout, "settingsRoute"),
            Value::str("scheduled-tasks")
        );
        traces.push(setup(&mut runner, "scheduled-page"));
        let trigger = node(&runner, "task-actions-task");
        assert!(runner.handlers_of(trigger).contains(&EventKind::Key));
        runner
            .dispatch(
                trigger,
                Event::Key(KeyboardEvent::parse("ArrowDown\nArrowDown\nfalse").unwrap()),
            )
            .unwrap();
        traces.push(setup(&mut runner, "task-menu-key"));
        press(&mut runner, "task-edit-task");
        traces.push(setup(&mut runner, "editor-open"));
        assert_eq!(slot(&runner, layout, "restEditor"), Value::str("task"));
        assert_eq!(slot(&runner, layout, "restSubject"), Value::str(SUBJECT));
        node(&runner, "scheduled-task-dialog");
        for (id, text) in [("task-title", "Edited"), ("task-prompt", "Draft")] {
            let view = node(&runner, id);
            runner.dispatch(view, Event::Input(text.into())).unwrap();
            assert_eq!(prop(&runner, view, "value"), Value::str(text));
        }
        traces.push(setup(&mut runner, "draft-input"));
        assert_eq!(slot(&runner, layout, "settingsRefresh"), Value::Number(0.0));
        assert_eq!(slot(&runner, layout, "commandPending"), Value::Bool(false));
        assert_eq!(slot(&runner, layout, "pendingModal"), Value::str(""));
        assert_eq!(slot(&runner, layout, "lineageStopping"), Value::str(""));
        for (name, value) in [
            ("keybindingSaving", Value::str("")),
            ("keybindingOpen", Value::Bool(false)),
            ("keybindingPrevious", Value::str("")),
            ("keybindingRecording", Value::Bool(false)),
            ("keybindingDrafts", Value::list(vec![])),
        ] {
            assert_eq!(slot(&runner, layout, name), value);
        }
        (runner, traces)
    }

    fn payload(op: &str) -> String {
        if op != "save" {
            return format!("action={op}-webhook&id=task&environmentId=environment");
        }
        concat!(
            "action=save&id=task&environmentId=environment&title=Edited&prompt=Draft",
            "&enabled=true&scheduleMode=webhook&signatureEnabled=false&signatureHeader=",
            "&signatureEncoding=hex&signaturePrefix=&signatureSecret=&maxDeliveryAgeMinutes=",
            "&intervalMinutes=60&timeOfDay=09%3A00&sunday=false&monday=true&tuesday=false",
            "&wednesday=false&thursday=false&friday=false&saturday=false&projectId=project",
            "&threadId=&workspaceMode=root&baseRef=main&startFromOrigin=true&path=",
            "&modelKey=codex%3Amodel",
        )
        .to_owned()
    }

    fn source_call(runner: &Runner<Source>, name: &str) -> DataCall {
        let (_, source) = resource(runner.plan(), name);
        call("answer", &source, runner.resource_args(name).unwrap())
    }

    fn completion_calls(runner: &Runner<Source>, closes: bool, success: bool) -> Vec<DataCall> {
        if !success {
            return vec![];
        }
        let mut expected = Vec::new();
        for row in &runner.plan().resources {
            let name = runner.plan().str(row.name);
            let (arity, index) = match name {
                "bindings" => (10, 7),
                "scheduled" => (13, 6),
                "sourceControl" => (8, 3),
                "integrations" => (9, 3),
                "storageSettings" => (9, 3),
                "archives" => (9, 4),
                "diagnostics" => (7, 4),
                "snapshotSettings" => (4, 1),
                "settingsCore" => (14, 7),
                "projectsSettings" => (6, 4),
                "sbPicker" if closes => (7, usize::MAX),
                _ => continue,
            };
            let mut args = runner.resource_args(name).unwrap().to_vec();
            assert_eq!(args.len(), arity, "{name}");
            if name == "sbPicker" {
                assert_eq!(args[0], Value::str("task"));
                args[0] = Value::str("");
            } else {
                assert_eq!(args[index], Value::Number(0.0), "{name}");
                args[index] = Value::Number(1.0);
                if name == "scheduled" && closes {
                    assert_eq!(args[2], Value::str("task"));
                    args[2] = Value::str("");
                }
            }
            expected.push(call("answer", runner.plan().str(row.source), &args));
        }
        assert_eq!(expected.len(), if closes { 11 } else { 10 });
        expected
    }

    fn no_marker_in_dialog(runner: &Runner<Source>, dialog: u32) {
        let mut pending = vec![dialog];
        while let Some(view) = pending.pop() {
            let node = runner.kernel().node(view).unwrap();
            for (_, value) in node.props.iter() {
                if let Some(text) = value.as_str() {
                    assert!(
                        !text.contains(KEEP) && !text.contains("toasted:"),
                        "internal marker rendered in the dialog"
                    );
                }
            }
            pending.extend(node.children());
        }
    }

    #[test]
    fn mounted_webhook_copy_rotate_and_save_use_the_real_completion() {
        for (bytes, layout) in plans() {
            for (op, message) in [
                ("copy", KEEP),
                ("rotate", KEEP),
                ("save", ""),
                ("copy", "toasted:Could not copy webhook URL."),
                ("rotate", "toasted:Could not rotate webhook URL."),
            ] {
                let (mut runner, setup) = boot(bytes, layout);
                let dialog = node(&runner, "scheduled-task-dialog");
                let title = node(&runner, "task-title");
                let prompt = node(&runner, "task-prompt");
                if op == "rotate" {
                    press(&mut runner, "task-webhook-rotate");
                    calls(&mut runner, vec![]);
                    no_requests(&mut runner);
                    hosts(&mut runner, vec![]);
                    node(&runner, "webhook-rotate-dialog");
                    let cancel = node_id(&runner, "task-webhook-rotate-cancel");
                    assert!(runner.handlers_of(cancel).contains(&EventKind::Press));
                    runner.dispatch(cancel, Event::Press).unwrap();
                    calls(&mut runner, vec![]);
                    no_requests(&mut runner);
                    hosts(&mut runner, vec![]);
                    assert!(runner
                        .kernel()
                        .find_by_test_id("webhook-rotate-dialog")
                        .is_empty());
                    assert_eq!(node(&runner, "scheduled-task-dialog"), dialog);
                    press(&mut runner, "task-webhook-rotate");
                    calls(&mut runner, vec![]);
                    no_requests(&mut runner);
                    hosts(&mut runner, vec![]);
                    node(&runner, "webhook-rotate-dialog");
                }
                press(
                    &mut runner,
                    match op {
                        "copy" => "task-webhook-copy",
                        "rotate" => "task-webhook-rotate-confirm",
                        "save" => "save-scheduled-task",
                        _ => unreachable!(),
                    },
                );
                let args = vec![
                    Value::str("rest:task"),
                    Value::str("environment:"),
                    Value::str(&payload(op)),
                    Value::Number(0.0),
                ];
                calls(&mut runner, vec![call("answer", "command", &args)]);
                let requests = runner.take_requests();
                assert_eq!(requests.len(), 1);
                let ticket = requests[0].ticket;
                assert_eq!(
                    requests,
                    vec![RequestOut {
                        ticket,
                        target: "changed".to_owned(),
                        request: Request::get("https://fixture.invalid/webhook"),
                        forced: false,
                    }],
                    "the complete outgoing request vector"
                );
                hosts(&mut runner, vec![]);
                assert_eq!(runner.pending(), vec![("changed".to_owned(), ticket)]);
                assert!(runner.queued().is_empty());
                assert_eq!(slot(&runner, layout, "commandPending"), Value::Bool(true));
                assert_eq!(slot(&runner, layout, "pendingModal"), Value::str("rest"));
                assert_eq!(slot(&runner, layout, "restEditor"), Value::str("task"));
                assert!(runner
                    .kernel()
                    .find_by_test_id("webhook-rotate-dialog")
                    .is_empty());
                assert_eq!(
                    prop(&runner, node(&runner, "task-webhook-copy"), "disabled"),
                    Value::Bool(true)
                );

                let success = message.is_empty() || message == KEEP;
                let closes = message.is_empty();
                let completion = completion_calls(&runner, closes, success);
                let snapshot = source_call(&runner, "data");
                let reply_ty = runner.data_ref().reply_type;
                let reply = fields(
                    runner.plan(),
                    reply_ty,
                    &[
                        ("revision", Value::Number(1.0)),
                        ("message", Value::str(message)),
                    ],
                );
                let response = Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: reply.to_bytes(),
                });
                assert!(runner.fulfill(ticket, response).unwrap().is_some());
                calls(&mut runner, vec![call("parse", "command", &args), snapshot]);
                no_requests(&mut runner);
                hosts(&mut runner, vec![]);
                assert_eq!(slot(&runner, layout, "commandPending"), Value::Bool(true));
                assert_eq!(slot(&runner, layout, "settingsRefresh"), Value::Number(0.0));
                let advanced = runner.land_then();
                assert!(advanced.error.is_none(), "{advanced:?}");
                assert_eq!(advanced.now_ms, 0.0);
                assert_eq!(advanced.receipts.len(), 1, "one real completion commit");
                assert_eq!(advanced.receipts[0].at_ms, 0.0);
                calls(&mut runner, completion);
                no_requests(&mut runner);
                hosts(&mut runner, vec![]);
                assert_eq!(slot(&runner, layout, "commandPending"), Value::Bool(false));
                assert_eq!(slot(&runner, layout, "pendingModal"), Value::str(""));
                assert_eq!(slot(&runner, layout, "lineageStopping"), Value::str(""));
                assert_eq!(
                    slot(&runner, layout, "settingsRefresh"),
                    Value::Number(if success { 1.0 } else { 0.0 })
                );
                assert_eq!(slot(&runner, layout, "modalError"), Value::str(""));
                if closes {
                    assert_eq!(slot(&runner, layout, "restEditor"), Value::str(""));
                    for id in ["scheduled-task-dialog", "task-title", "task-prompt"] {
                        assert!(runner.kernel().find_by_test_id(id).is_empty(), "{id}");
                    }
                } else {
                    assert_eq!(slot(&runner, layout, "restEditor"), Value::str("task"));
                    assert_eq!(node(&runner, "scheduled-task-dialog"), dialog);
                    assert_eq!(node(&runner, "task-title"), title);
                    assert_eq!(node(&runner, "task-prompt"), prompt);
                    assert_eq!(prop(&runner, title, "value"), Value::str("Edited"));
                    assert_eq!(prop(&runner, prompt, "value"), Value::str("Draft"));
                    assert_eq!(
                        prop(&runner, node(&runner, "task-webhook-copy"), "disabled"),
                        Value::Bool(false)
                    );
                    no_marker_in_dialog(&runner, dialog);
                    assert!(runner
                        .kernel()
                        .find_by_test_id("scheduled-task-error")
                        .is_empty());
                }
                // A second settlement cannot increment refresh or consume the same reply again.
                let repeated = runner.land_then();
                assert!(repeated.error.is_none(), "{repeated:?}");
                assert_eq!(repeated.now_ms, 0.0);
                assert!(repeated.receipts.is_empty());
                calls(&mut runner, vec![]);
                no_requests(&mut runner);
                hosts(&mut runner, vec![]);
                assert_eq!(
                    slot(&runner, layout, "settingsRefresh"),
                    Value::Number(if success { 1.0 } else { 0.0 })
                );
                assert_eq!(setup.len(), 6);
            }
        }
    }
}
