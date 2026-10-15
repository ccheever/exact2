//! What moved from the module's native readers and menu
//! construction into the compiled plan, read from the plan as the host reads it.
//! The window's root removes the standard tab items by `menuRemove`, the surface
//! launcher's available rows declare their letters as `accessibilityKeyShortcuts`
//! (R8KeysLauncher is gone), a sidebar row's press carries its own click modifiers
//! (T3Sidebar's click monitor is gone), and the keybinding recorder is a Contract
//! capture handler (T3KeyRecorder is gone).

use super::composer_accessibility_tests::{measure, set, Replies};
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::{Plan, TypeKind, TypesId};
use exact_runner::{Event, Runner, Value};

const EPOCH: f64 = 1_791_446_400_000.0;

fn empty(plan: &Plan, ty: TypesId) -> Value {
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
                .map(|field| empty(plan, plan.field(field).ty))
                .collect(),
        ),
    }
}

/// The element type of `resource`'s list field `path` (a record), empty.
fn element(plan: &Plan, resource: &str, path: &[&str]) -> (TypesId, Value) {
    let row = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == resource)
        .unwrap();
    let mut ty = row.ty;
    for name in path {
        let field = plan
            .type_(ty)
            .fields
            .iter()
            .find(|field| plan.str(plan.field(*field).name) == *name)
            .unwrap();
        ty = plan.field(field).ty;
    }
    let elem = plan.type_(ty).elem.unwrap();
    (elem, empty(plan, elem))
}

fn surface(plan: &Plan, id: &str, shortcut: &str, available: bool) -> Value {
    let (ty, mut value) = element(plan, "shell", &["surfaces"]);
    for (name, field) in [
        ("id", Value::str(id)),
        ("label", Value::str(id)),
        ("icon", Value::str("files")),
        ("shortcut", Value::str(shortcut)),
        ("letter", Value::str(&shortcut.to_lowercase())),
        ("available", Value::Bool(available)),
        (
            "reason",
            Value::str(if available { "" } else { "Not here" }),
        ),
    ] {
        set(plan, ty, &mut value, &[name], field);
    }
    value
}

fn host() -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let (thread_type, mut thread) = element(&plan, "data", &["threads"]);
    for (name, value) in [
        ("id", Value::str("fixture-thread")),
        ("title", Value::str("Fixture thread")),
        ("label", Value::str("Fixture thread")),
        ("section", Value::str("active")),
        ("hoverKey", Value::str("active|fixture-thread")),
        ("titleColor", Value::str("#000000")),
        ("idleTitleColor", Value::str("#000000")),
        ("titleWeight", Value::Number(400.0)),
        ("idleTitleWeight", Value::Number(400.0)),
    ] {
        set(&plan, thread_type, &mut thread, &[name], value);
    }
    let surfaces = vec![
        surface(&plan, "browser", "B", true),
        surface(&plan, "terminal", "T", false),
        surface(&plan, "files", "F", true),
    ];
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
    // The shell's baked answer is no record to start from (it waits on the data): an empty one, filled in.
    let shell = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == "shell")
        .unwrap();
    let mut view = empty(&plan, shell.ty);
    for (path, value) in [
        (&["surfaces"][..], Value::list(surfaces)),
        (&["panel", "launcher"][..], Value::Bool(true)),
    ] {
        set(&plan, shell.ty, &mut view, path, value);
    }
    replies.0.insert(plan.str(shell.source).to_owned(), view);
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

fn node(runner: &Runner<Replies>, test_id: &str) -> Option<u32> {
    let nodes = runner.kernel().find_by_test_id(test_id);
    assert!(nodes.len() <= 1, "at most one rendered {test_id}");
    nodes
        .first()
        .map(|key| runner.kernel().node_by_key(*key).unwrap().id)
}

fn prop(runner: &Runner<Replies>, test_id: &str, name: &str) -> Option<String> {
    let id = node(runner, test_id).unwrap_or_else(|| panic!("{test_id} is rendered"));
    runner
        .kernel()
        .node(id)
        .unwrap()
        .props
        .iter()
        .find(|(prop, _)| prop.name() == name)
        .and_then(|(_, value)| value.as_str().map(str::to_owned))
}

#[test]
fn the_windows_root_removes_the_standard_tab_items() {
    let mut host = host();
    let runner = host.runner_mut();
    assert_eq!(
        prop(runner, "t3-code", "menuRemove").as_deref(),
        Some("show-tab-bar show-all-tabs")
    );
}

#[test]
fn the_launchers_available_rows_declare_their_letters_and_it_takes_the_focus() {
    let mut host = host();
    let runner = host.runner_mut();
    assert_eq!(
        prop(runner, "surface-browser", "accessibilityKeyShortcuts").as_deref(),
        Some("b Shift+B")
    );
    assert_eq!(
        prop(runner, "surface-files", "accessibilityKeyShortcuts").as_deref(),
        Some("f Shift+F")
    );
    // An unavailable row draws no button, so no shortcut answers its letter.
    assert_eq!(
        prop(runner, "surface-terminal", "accessibilityKeyShortcuts"),
        None
    );
    let chooser = node(runner, "surface-chooser").expect("the launcher is shown");
    let props = &runner.kernel().node(chooser).unwrap().props;
    assert!(
        props.iter().any(|(prop, _)| prop.name() == "autofocus"),
        "the launcher focuses itself as it mounts (focusOnMount)"
    );
}

#[test]
fn a_sidebar_row_press_carries_its_own_click_modifiers() {
    // A fresh window for each press: the first leaves its command out (the busy guard), as the window's does.
    for (held, how) in [
        ("Control+Alt", "click"),
        ("Meta+Shift", "click:meta"),
        ("Shift", "click:shift"),
    ] {
        let mut host = host();
        let runner = host.runner_mut();
        let row = node(runner, "thread-fixture-thread").expect("the row is shown");
        runner.dispatch(row, Event::press(held).unwrap()).unwrap();
        let calls = runner.data_ref().1.clone();
        assert_eq!(calls.len(), 1, "{held}");
        assert_eq!(
            calls[0][..3],
            [
                Value::str("select-thread"),
                Value::str("fixture-thread"),
                Value::str(how)
            ],
            "{held}"
        );
    }
}

/// Settings › Keybindings' new row with its recorder up: the Contract focus target (key-recorder.contract), not the
/// native T3KeyRecorder view.
fn recording() -> exact_apple::Host<Replies> {
    let mut host = host();
    let runner = host.runner_mut();
    runner.act("openSettings", vec![]).unwrap();
    runner
        .act("coreNavigate", vec![Value::str("keybindings")])
        .unwrap();
    runner.act("newKeybinding", vec![]).unwrap();
    let new = Value::str("new");
    runner
        .act(
            "recordKeybinding",
            vec![new, Value::str("chat.new"), Value::str(""), Value::str("")],
        )
        .unwrap();
    runner.take_commands();
    host
}

fn keydown(key: &str, code: &str, held: &str) -> Event {
    let mut event = exact_runner::KeyboardEvent::chord(&format!("{held}{key}"));
    event.code = code.to_owned();
    Event::Keycapture(event)
}

#[test]
fn the_keybinding_recorder_captures_a_chord_by_its_physical_key_and_escape_restores() {
    let mut host = recording();
    let runner = host.runner_mut();
    let recorder = node(runner, "keybinding-recorder").expect("the recorder is up");
    assert!(
        runner
            .kernel()
            .node(recorder)
            .unwrap()
            .props
            .iter()
            .any(|(prop, _)| prop.name() == "autofocus"),
        "it takes the keyboard as it mounts"
    );
    // A modifier alone records nothing; the key is taken either way (its default cancelled, no ancestor hears it).
    runner
        .dispatch(recorder, keydown("Meta", "MetaLeft", "Meta+"))
        .unwrap();
    let names = |runner: &mut Runner<Replies>| {
        runner
            .take_commands()
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(runner), ["preventDefault", "stopPropagation"]);
    assert_eq!(runner.slot("keybindingRecording"), Some(&Value::Bool(true)));
    // Korean 2-Set's ㅠ on the B key with ⌘⇧: the physical Latin letter.
    runner
        .dispatch(recorder, keydown("ㅠ", "KeyB", "Meta+Shift+"))
        .unwrap();
    assert_eq!(
        runner.slot("keybindingKey"),
        Some(&Value::str("mod+shift+b"))
    );
    assert_eq!(
        runner.slot("keybindingRecording"),
        Some(&Value::Bool(false))
    );
    assert!(
        node(runner, "keybinding-recorder").is_none(),
        "recording ended"
    );
    // Recording again, Escape gives the row its binding back (the new row's is none).
    runner
        .act(
            "recordKeybinding",
            vec![
                Value::str("new"),
                Value::str("chat.new"),
                Value::str("mod+shift+b"),
                Value::str(""),
            ],
        )
        .unwrap();
    let recorder = node(runner, "keybinding-recorder").expect("the recorder is up again");
    runner
        .dispatch(recorder, keydown("Escape", "Escape", ""))
        .unwrap();
    assert_eq!(runner.slot("keybindingKey"), Some(&Value::str("")));
    assert_eq!(
        runner.slot("keybindingRecording"),
        Some(&Value::Bool(false))
    );
}
