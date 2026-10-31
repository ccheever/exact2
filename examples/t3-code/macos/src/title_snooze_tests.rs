//! The compiled plan's root actions, run as the window runs
//! them. Thread title › Snooze › Custom… (`ui:custom-snooze`, shell.ts) reaches
//! the sidebar's Custom snooze command for the item's own thread, with the
//! window's wall time and "title" as its origin, as the row's Custom does with
//! none. The data module is a stand-in that records each `command` request and
//! leaves it pending, so the actions' own busy guard is what is exercised.
//! The title menu's state is the window's view-only state
//! (`titleMenuOpen#1`, T3Window in app-window.contract): the view's pick is the
//! window's action, which closes the menu and then runs this root half
//! (`titleMenuPick`), the one that sends.

use super::composer_accessibility_tests::{set, Replies};
use super::PLAN;
use exact_apple::measure::{CMetrics, CRequest, CallbackMeasurer};
use exact_plan::{Plan, TypeKind, TypesId};
use exact_runner::{
    Answer, DataError, DataSource, Event, KeyboardEvent, Request, Runner, Store, Value,
};

#[derive(Default)]
struct Commands {
    calls: Vec<Vec<Value>>,
}

impl DataSource for Commands {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source == "command" {
            self.calls.push(args.to_vec());
        }
        Ok(Answer::Later(Request::get("https://fixture.invalid/")))
    }
}

extern "C" fn measure(_: *mut std::ffi::c_void, _: *const CRequest) -> CMetrics {
    CMetrics::default()
}

const EPOCH: f64 = 1_791_446_400_000.0;

fn host() -> exact_apple::Host<Commands> {
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, Commands::default(), Box::new(measurer), 1280.0, 840.0)
            .unwrap();
    let runner = host.runner_mut();
    runner.set_time(EPOCH, 0.0).unwrap();
    runner.advance(1250.0).unwrap();
    runner.data().calls.clear();
    host
}

fn pick(op: &str, target: &str) -> Vec<Value> {
    vec![
        Value::str(op),
        Value::str(target),
        Value::str(""),
        Value::str(""),
        Value::str(""),
        Value::Bool(false),
    ]
}

fn command(op: &str, target: &str, value: &str) -> Vec<Value> {
    vec![
        Value::str(op),
        Value::str(target),
        Value::str(value),
        Value::Number(EPOCH + 1250.0),
    ]
}

#[test]
fn title_custom_snooze_asks_the_sidebar_command_for_its_own_thread() {
    let mut host = host();
    let runner = host.runner_mut();
    runner
        .act("titleMenuPick", pick("ui:custom-snooze", "thread-b"))
        .unwrap();
    assert_eq!(runner.slot("titleMenuOpen#1"), Some(&Value::Bool(false)));
    assert_eq!(
        runner.data_ref().calls,
        vec![command("sidebar:snooze:custom", "thread-b", "title")]
    );
    // The request is still out: a second pick waits, as every sidebar command does.
    runner
        .act("titleMenuPick", pick("ui:custom-snooze", "thread-a"))
        .unwrap();
    assert_eq!(runner.data_ref().calls.len(), 1);
}

#[test]
fn the_sidebar_rows_custom_snooze_is_unchanged() {
    let mut host = host();
    let runner = host.runner_mut();
    let args = vec![
        Value::str("snooze:custom"),
        Value::str("thread-b"),
        Value::str(""),
    ];
    runner.act("sidebarRun", args).unwrap();
    assert_eq!(
        runner.data_ref().calls,
        vec![command("sidebar:snooze:custom", "thread-b", "")]
    );
}

fn empty_record(plan: &Plan, ty: TypesId) -> Value {
    match plan.type_(ty).kind {
        TypeKind::Number => Value::Number(0.0),
        TypeKind::Bool => Value::Bool(false),
        TypeKind::String => Value::str(""),
        TypeKind::Unit => Value::Unit,
        TypeKind::Option => Value::NONE,
        TypeKind::List => Value::list(vec![]),
        TypeKind::Record => Value::record(
            plan.type_(ty)
                .fields
                .iter()
                .map(|field| empty_record(plan, plan.field(field).ty))
                .collect(),
        ),
    }
}

fn snooze_host() -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let data = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == "data")
        .unwrap();
    let threads = plan
        .type_(data.ty)
        .fields
        .iter()
        .find(|field| plan.str(plan.field(*field).name) == "threads")
        .unwrap();
    let thread_type = plan.type_(plan.field(threads).ty).elem.unwrap();
    let mut thread = empty_record(&plan, thread_type);
    for (name, value) in [
        ("id", Value::str("fixture-thread")),
        ("title", Value::str("Fixture thread")),
        ("label", Value::str("Fixture thread")),
        ("projectName", Value::str("Fixture project")),
        ("section", Value::str("active")),
        ("hoverKey", Value::str("active|fixture-thread")),
        ("card", Value::Bool(true)),
        ("canSnooze", Value::Bool(true)),
        ("titleColor", Value::str("#000000")),
        ("idleTitleColor", Value::str("#000000")),
        ("titleWeight", Value::Number(400.0)),
        ("idleTitleWeight", Value::Number(400.0)),
    ] {
        set(&plan, thread_type, &mut thread, &[name], value);
    }
    let presets = [("hour", "In an hour"), ("tomorrow", "Tomorrow")]
        .into_iter()
        .map(|(id, label)| {
            Value::record(vec![
                Value::str(id),
                Value::str(label),
                Value::str(""),
                Value::str(""),
            ])
        })
        .collect();
    set(
        &plan,
        thread_type,
        &mut thread,
        &["snoozePresets"],
        Value::list(presets),
    );
    let mut replies = Replies::default();
    replies.fixture(
        &plan,
        "data",
        &[
            (&["revision"], Value::Number(1.0)),
            (&["connected"], Value::Bool(true)),
            (&["sidebarOpen"], Value::Bool(true)),
            (&["threads"], Value::list(vec![thread])),
        ],
    );
    replies.fixture(&plan, "welcome", &[(&["show"], Value::Bool(false))]);
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    let runner = host.runner_mut();
    runner.set_time(EPOCH, 0.0).unwrap();
    runner.advance(1250.0).unwrap();
    runner.take_commands();
    runner.data().1.clear();
    host
}

fn node(runner: &Runner<Replies>, id: &str) -> u32 {
    let nodes = runner.kernel().find_by_id(id);
    assert_eq!(nodes.len(), 1, "one rendered {id}");
    runner.kernel().node_by_key(nodes[0]).unwrap().id
}

fn actions_hidden(runner: &Runner<Replies>, trigger: u32) -> bool {
    let mut parent = runner.kernel().node(trigger).unwrap().parent;
    while let Some(view) = parent {
        let node = runner.kernel().node(view).unwrap();
        if let Some((_, value)) = node
            .props
            .iter()
            .find(|(id, _)| id.name() == "accessibilityElementsHidden")
        {
            return value.as_bool().unwrap();
        }
        parent = node.parent;
    }
    panic!("snooze action slot has an explicit accessibilityElementsHidden property")
}

#[test]
fn snooze_keyboard_opening_names_its_trigger_and_an_actual_row() {
    let mut host = snooze_host();
    let runner = host.runner_mut();
    let trigger = node(runner, "snooze-trigger-fixture-thread");
    for key in ["Enter", " ", "Space", "ArrowDown", "ArrowUp"] {
        runner
            .dispatch(trigger, Event::Key(KeyboardEvent::chord(key)))
            .unwrap();
        let commands = runner.take_commands();
        assert_eq!(
            commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["preventDefault", "stopPropagation", "showPopover", "focus"],
            "{key}"
        );
        assert_eq!(
            commands[2].args,
            vec![
                Value::str("snooze-fixture-thread"),
                Value::str("snooze-trigger-fixture-thread")
            ]
        );
        let target = if key == "ArrowUp" {
            "snooze-fixture-thread-custom"
        } else {
            "snooze-fixture-thread-hour"
        };
        node(runner, target);
        assert_eq!(commands[3].args, vec![Value::str(target)]);
        let mut repeated = KeyboardEvent::chord(key);
        repeated.repeat = true;
        runner.dispatch(trigger, Event::Key(repeated)).unwrap();
        assert_eq!(
            runner
                .take_commands()
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["preventDefault", "stopPropagation"]
        );
    }
    for chord in [
        "Meta+Enter",
        "Control+ArrowDown",
        "Alt+ArrowUp",
        "Shift+Space",
    ] {
        runner
            .dispatch(trigger, Event::Key(KeyboardEvent::chord(chord)))
            .unwrap();
        assert!(runner.take_commands().is_empty(), "{chord}");
    }
}

#[test]
fn snooze_toggle_pins_the_trigger_across_row_focus_and_releases_on_close() {
    let mut host = snooze_host();
    let runner = host.runner_mut();
    let trigger = node(runner, "snooze-trigger-fixture-thread");
    let popup = node(runner, "snooze-fixture-thread");
    assert!(actions_hidden(runner, trigger));
    runner
        .dispatch(
            popup,
            Event::Toggle {
                old_open: false,
                new_open: true,
            },
        )
        .unwrap();
    assert!(!actions_hidden(runner, trigger));
    let first = node(runner, "snooze-fixture-thread-hour");
    let next = node(runner, "snooze-fixture-thread-tomorrow");
    runner.dispatch(first, Event::Focus).unwrap();
    runner.dispatch(first, Event::Blur).unwrap();
    runner.dispatch(next, Event::Focus).unwrap();
    for (view, event) in runner.focus_bubble(next, Some(first), true) {
        runner.dispatch(view, event).unwrap();
    }
    assert!(!actions_hidden(runner, trigger));
    assert!(runner.take_commands().is_empty());

    // The observed row focus, including pointer focus, is KeyMenu's position.
    let keys = node(runner, "snooze-fixture-thread-keys");
    runner
        .dispatch(keys, Event::Key(KeyboardEvent::chord("ArrowDown")))
        .unwrap();
    let commands = runner.take_commands();
    assert_eq!(commands.last().unwrap().name, "focus");
    assert_eq!(
        commands.last().unwrap().args,
        vec![Value::str("snooze-fixture-thread-custom")]
    );
    assert!(!actions_hidden(runner, trigger));

    // Swift owns visibility and focus restoration. Its actual closed toggle
    // releases the pin even when no menu-row blur is available (light dismiss).
    runner
        .dispatch(
            popup,
            Event::Toggle {
                old_open: true,
                new_open: false,
            },
        )
        .unwrap();
    assert_eq!(node(runner, "snooze-trigger-fixture-thread"), trigger);
    assert!(actions_hidden(runner, trigger));
    assert!(runner.take_commands().is_empty());
}

/// With the
/// pointer outside, Escape returned the focus to the trigger and the closed
/// toggle then hid the actions under it (`aria-hidden`), where the macOS host
/// dropped the focus. As the reference's `has-[:focus-visible]` does, the
/// actions stay shown while one of them holds the focus.
#[test]
fn snooze_escape_keeps_the_returned_trigger_shown_until_focus_leaves_the_actions() {
    let mut host = snooze_host();
    let runner = host.runner_mut();
    let trigger = node(runner, "snooze-trigger-fixture-thread");
    let popup = node(runner, "snooze-fixture-thread");
    let first = node(runner, "snooze-fixture-thread-hour");
    let toggle = |runner: &mut Runner<Replies>, open: bool| {
        runner
            .dispatch(
                popup,
                Event::Toggle {
                    old_open: !open,
                    new_open: open,
                },
            )
            .unwrap();
    };
    let focus_path = |runner: &mut Runner<Replies>, from: u32, to: Option<u32>, entering: bool| {
        for (view, event) in runner.focus_bubble(from, to, entering) {
            runner.dispatch(view, event).unwrap();
        }
    };
    // ↓ on the focused trigger: open, then the first row takes the focus.
    focus_path(runner, trigger, None, true);
    toggle(runner, true);
    focus_path(runner, trigger, Some(first), false);
    focus_path(runner, first, Some(trigger), true);
    assert!(!actions_hidden(runner, trigger));
    // Escape with the pointer on the composer: the host focuses the trigger, then the toggle closes.
    focus_path(runner, first, Some(trigger), false);
    focus_path(runner, trigger, Some(first), true);
    toggle(runner, false);
    assert_eq!(node(runner, "snooze-trigger-fixture-thread"), trigger);
    assert!(!actions_hidden(runner, trigger));
    assert!(runner.take_commands().is_empty());
    // Tab on to something outside the actions releases them.
    let row = node(runner, "thread-fixture-thread");
    focus_path(runner, trigger, Some(row), false);
    assert!(actions_hidden(runner, trigger));
}
