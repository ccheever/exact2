//! What moved from the module's native readers and menu
//! construction into the compiled plan, read from the plan as the host reads it.
//! The window's root removes the host's menu items the reference bar lacks by `menuRemove`, the surface
//! launcher's available rows declare their letters as `accessibilityKeyShortcuts`
//! (the native launcher's key re-post is gone), a sidebar row's press carries its own click modifiers
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
    // Settings › Keybindings with one binding, Chat: New on ⌘N.
    let (row_type, mut row) = element(&plan, "bindings", &["rows"]);
    for (name, value) in [
        ("id", Value::str("chat-new")),
        ("command", Value::str("chat.new")),
        ("label", Value::str("Chat: New")),
        ("key", Value::str("mod+n")),
        ("source", Value::str("Default")),
        ("aria", Value::str("⌘N")),
        ("defaultKey", Value::str("mod+n")),
    ] {
        set(&plan, row_type, &mut row, &[name], value);
    }
    let (group_type, mut group) = element(&plan, "bindings", &["groups"]);
    for (name, value) in [
        ("id", Value::str("chat")),
        ("title", Value::str("Chat")),
        ("rows", Value::list(vec![row.clone()])),
    ] {
        set(&plan, group_type, &mut group, &[name], value);
    }
    replies.fixture(
        &plan,
        "bindings",
        &[
            (&["available"], Value::Bool(true)),
            (&["writable"], Value::Bool(true)),
            (&["draftValid"], Value::Bool(true)),
            (&["rows"], Value::list(vec![row])),
            (&["groups"], Value::list(vec![group])),
        ],
    );
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
fn the_windows_root_removes_the_host_items_the_reference_bar_lacks() {
    let mut host = host();
    let runner = host.runner_mut();
    assert_eq!(
        prop(runner, "t3-code", "menuRemove").as_deref(),
        Some(concat!(
            "show-tab-bar show-all-tabs develop back paste-and-match-style find ",
            "spelling-and-grammar substitutions transformations app-help"
        ))
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

/// Settings › Keybindings, its new row added.
fn adding() -> exact_apple::Host<Replies> {
    let mut host = host();
    let runner = host.runner_mut();
    runner.act("openSettings", vec![]).unwrap();
    runner
        .act("coreNavigate", vec![Value::str("keybindings")])
        .unwrap();
    runner.act("newKeybinding", vec![]).unwrap();
    runner.take_commands();
    host
}

/// Settings › Keybindings' new row with its recorder up: the Contract focus target (key-recorder.contract), not the
/// native T3KeyRecorder view.
fn recording() -> exact_apple::Host<Replies> {
    let mut host = adding();
    let runner = host.runner_mut();
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
fn the_keybinding_recorder_captures_a_chord_and_escape_restores() {
    let mut host = recording();
    let runner = host.runner_mut();
    let recorder = node(runner, "keybinding-recorder-new").expect("the recorder is up");
    assert!(
        runner
            .kernel()
            .node(recorder)
            .unwrap()
            .props
            .iter()
            .any(
                |(prop, value)| prop.name() == "autofocus" && format!("{value:?}").contains("true")
            ),
        "recording, it takes the keyboard as it mounts"
    );
    // KeybindingsSettings.tsx's Input: a text field named for its command ("Keybinding for New thread"), the prompt
    // its placeholder, so VoiceOver reads both as the focus lands in it.
    let field = runner.kernel().node(recorder).unwrap();
    assert_eq!(format!("{:?}", field.node_type), "TextInput");
    let text = |name: &str| {
        field
            .props
            .iter()
            .find(|(prop, _)| prop.name() == name)
            .and_then(|(_, value)| value.as_str().map(str::to_owned))
    };
    // (The fixture's page data names no draft command, so the name's command part is empty here.)
    assert!(text("accessibilityLabel").is_some_and(|label| label.starts_with("Keybinding for ")));
    assert_eq!(text("placeholder").as_deref(), Some("Press shortcut"));
    // Editable, as the reference's Input is: AppKit serves a non-editable NSTextField as AXStaticText.
    let editable = field
        .props
        .iter()
        .find(|(prop, _)| prop.name() == "editable")
        .map(|(_, value)| format!("{value:?}"));
    assert!(
        editable
            .as_deref()
            .is_none_or(|value| !value.contains("false")),
        "the recorder is editable: {editable:?}"
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
    // ⌘⇧B on a US layout. Settings records the layout's character (keybinding-defaults.test.ts holds the other
    // layouts, Korean 2-Set's "ㅠ" among them), so the event names the character, not this machine's input source.
    runner
        .dispatch(recorder, keydown("B", "KeyB", "Meta+Shift+"))
        .unwrap();
    assert_eq!(
        runner.slot("keybindingKey"),
        Some(&Value::str("mod+shift+b"))
    );
    assert_eq!(
        runner.slot("keybindingRecording"),
        Some(&Value::Bool(false))
    );
    names(runner);
    // Recording ends, but the field keeps the focus and goes on taking keys (KeybindingsSettings.tsx's Input stays
    // mounted and focused): the same recorder shows the draft.
    assert_eq!(
        node(runner, "keybinding-recorder-new"),
        Some(recorder),
        "the recorder keeps the focus after a chord"
    );
    // Escape on it gives the row its binding back (the new row's is none) and is the recorder's alone: Settings stays.
    runner
        .dispatch(recorder, keydown("Escape", "Escape", ""))
        .unwrap();
    assert_eq!(
        names(runner),
        ["preventDefault", "stopPropagation"],
        "no ancestor hears that Escape"
    );
    assert_eq!(runner.slot("keybindingKey"), Some(&Value::str("")));
    assert_eq!(
        runner.slot("keybindingRecording"),
        Some(&Value::Bool(false))
    );
    assert_eq!(runner.slot("settingsOpen"), Some(&Value::Bool(true)));
    // The new row's field (NewKeybindingKeyInput) stays a text field, "Unassigned", once the focus leaves it.
    let recorder = node(runner, "keybinding-recorder-new")
        .expect("the unassigned row's field still has the focus");
    runner.dispatch(recorder, Event::Blur).unwrap();
    assert!(node(runner, "keybinding-recorder-new").is_none(), "left");
    assert_eq!(
        prop(runner, "keybinding-field-new", "placeholder").as_deref(),
        Some("Unassigned")
    );
    assert!(!modal(runner, "keybinding-capture-new"));
}

/// Whether the node is the window's modal scope (`aria-modal`).
fn modal(runner: &Runner<Replies>, test_id: &str) -> bool {
    let id = node(runner, test_id).unwrap_or_else(|| panic!("{test_id} is rendered"));
    runner
        .kernel()
        .node(id)
        .unwrap()
        .props
        .iter()
        .any(|(prop, value)| {
            prop.name() == "accessibilityModal" && format!("{value:?}").contains("true")
        })
}

/// The focus entering `field`, as its ancestor `row` hears it (`focusin` bubbles; the host delivers it to each).
fn focusin(runner: &mut Runner<Replies>, row: &str, field: &str) {
    let id = node(runner, row).unwrap_or_else(|| panic!("{row} is rendered"));
    runner
        .dispatch(
            id,
            Event::Focusin {
                target: field.to_owned(),
                related_target: None,
            },
        )
        .unwrap();
}

#[test]
fn the_new_keybinding_row_is_a_text_field_that_records_once_it_has_the_focus() {
    // KeybindingsSettings.tsx: "Add keybinding" mounts NewKeybindingSettingsRow, whose Input does not autofocus, so the
    // focus stays on the button; the field is an Input from the start, "Unassigned" until it takes the focus.
    let mut host = adding();
    let runner = host.runner_mut();
    let field = node(runner, "keybinding-field-new").expect("the new row's field is shown");
    assert_eq!(
        format!("{:?}", runner.kernel().node(field).unwrap().node_type),
        "TextInput"
    );
    assert!(
        !runner
            .kernel()
            .node(field)
            .unwrap()
            .props
            .iter()
            .any(
                |(prop, value)| prop.name() == "autofocus" && format!("{value:?}").contains("true")
            ),
        "the field does not take the focus from Add keybinding"
    );
    assert_eq!(
        prop(runner, "keybinding-field-new", "placeholder").as_deref(),
        Some("Unassigned")
    );
    assert_eq!(
        runner.slot("keybindingRecording"),
        Some(&Value::Bool(false))
    );
    assert!(!modal(runner, "keybinding-capture-new"));
    // Its onFocus starts recording (a click or Tab): the recorder takes its place, focused (autofocus) and modal.
    focusin(runner, "keybinding-capture-new", "keybinding-field-new");
    assert_eq!(runner.slot("keybindingRecording"), Some(&Value::Bool(true)));
    assert_eq!(runner.slot("keybindingPrevious"), Some(&Value::str("new")));
    assert_eq!(
        prop(runner, "keybinding-recorder-new", "placeholder").as_deref(),
        Some("Press shortcut")
    );
    assert!(
        modal(runner, "keybinding-recording-new"),
        "recording, it scopes the window's shortcuts"
    );
    // A press elsewhere in the window lets go of the field, as a browser blurs the reference's input.
    let root = node(runner, "t3-code").unwrap();
    runner.take_commands();
    runner
        .dispatch(
            root,
            Event::Pointerdown(exact_runner::PointerEvent {
                offset_x: 4.0,
                offset_y: 4.0,
                buttons: 1.0,
                pressure: 0.5,
                pointer_type: "mouse".to_owned(),
                pointer_id: 1.0,
                client_x: 4.0,
                client_y: 4.0,
                held: Default::default(),
            }),
        )
        .unwrap();
    let blurs = runner
        .take_commands()
        .into_iter()
        .filter(|command| command.name == "blur")
        .map(|command| format!("{:?}", command.args))
        .collect::<Vec<_>>();
    assert_eq!(blurs.len(), 1, "{blurs:?}");
    assert!(blurs[0].contains("keybinding-recorder-new"), "{blurs:?}");
}

#[test]
fn an_existing_bindings_field_mounts_recording_and_escape_gives_the_pill_back() {
    let mut host = adding();
    let runner = host.runner_mut();
    runner.act("cancelKeybinding", vec![]).unwrap();
    let pill = node(runner, "edit-keybinding-chat-new").expect("the binding's pill");
    runner.dispatch(pill, Event::Press).unwrap();
    // autoFocus={isRecording}: the field takes the focus as the pill gives way to it.
    let field = node(runner, "keybinding-recorder-chat-new").expect("the field replaces the pill");
    assert!(runner
        .kernel()
        .node(field)
        .unwrap()
        .props
        .iter()
        .any(|(prop, value)| prop.name() == "autofocus" && format!("{value:?}").contains("true")));
    assert_eq!(runner.slot("keybindingRecording"), Some(&Value::Bool(true)));
    // Escape restores ⌘N; the row is no longer dirty, so the pill comes back in the field's place, as the
    // reference's KeybindingKeyControl renders it (showPill), and Settings stays.
    runner
        .dispatch(field, keydown("Escape", "Escape", ""))
        .unwrap();
    assert_eq!(runner.slot("keybindingKey"), Some(&Value::str("mod+n")));
    assert!(node(runner, "keybinding-recorder-chat-new").is_none());
    assert!(node(runner, "edit-keybinding-chat-new").is_some());
    assert_eq!(runner.slot("settingsOpen"), Some(&Value::Bool(true)));
}
