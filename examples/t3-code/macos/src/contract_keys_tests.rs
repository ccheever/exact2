//! What moved from the module's native readers and menu
//! construction into the compiled plan, read from the plan as the host reads it.
//! The window's root removes the standard tab items by `menuRemove`, the surface
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

/// Ordinary runs exercise the baked plan. Differential runs require both complete artifacts.
pub(super) fn extraction_plans() -> impl Iterator<Item = (&'static [u8], bool)> {
    static PLANS: std::sync::OnceLock<(Option<Vec<u8>>, Vec<u8>)> = std::sync::OnceLock::new();
    let (original, selected) = PLANS.get_or_init(|| {
        let pair = (
            std::env::var_os("EXACT_T3_EXTRACTION_ORIGINAL_PLAN"),
            std::env::var_os("EXACT_T3_EXTRACTION_SELECTED_PLAN"),
        );
        let read = |path: std::ffi::OsString| {
            let bytes = std::fs::read(&path).expect("read the supplied extraction plan");
            Plan::decode(&bytes).expect("a valid complete extraction plan");
            bytes
        };
        match pair {
            (None, None) => (None, PLAN.to_vec()),
            (Some(original), Some(selected)) => (Some(read(original)), read(selected)),
            _ => panic!("differential extraction tests require both plan paths"),
        }
    });
    original
        .as_deref()
        .map(|bytes| (bytes, true))
        .into_iter()
        .chain(std::iter::once((selected.as_slice(), false)))
}

pub(super) fn slot_field<'a, D: exact_runner::DataSource>(
    runner: &'a Runner<D>,
    slot: &str,
    name: &str,
) -> Option<&'a Value> {
    let plan = runner.plan();
    let row = plan
        .slots
        .iter()
        .find(|row| row.owner.is_none() && plan.str(row.name) == slot)?;
    let index = plan.field_index(row.ty, name)?;
    let Value::Record(fields) = runner.slot(slot)? else {
        panic!("record slot {slot}")
    };
    fields.get(index)
}

pub(super) fn grip(environment: &str, route: &str) -> String {
    format!("environment-route-grip-{environment}-{route}")
}

pub(super) fn empty(plan: &Plan, ty: TypesId) -> Value {
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
fn the_keybinding_recorder_captures_the_layout_character_and_escape_restores() {
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
    assert_eq!(
        slot_field(runner, "keybindingEditor", "recording"),
        Some(&Value::Bool(true))
    );
    // Settings records the layout's character: Korean 2-Set's ㅠ at KeyB with ⌘⇧ stays ㅠ.
    runner
        .dispatch(recorder, keydown("ㅠ", "KeyB", "Meta+Shift+"))
        .unwrap();
    assert_eq!(
        slot_field(runner, "keybindingEditor", "key"),
        Some(&Value::str("mod+shift+ㅠ"))
    );
    assert_eq!(
        slot_field(runner, "keybindingEditor", "recording"),
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
    assert_eq!(
        slot_field(runner, "keybindingEditor", "key"),
        Some(&Value::str(""))
    );
    assert_eq!(
        slot_field(runner, "keybindingEditor", "recording"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        slot_field(runner, "settingsPageState", "open"),
        Some(&Value::Bool(true))
    );
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
        slot_field(runner, "keybindingEditor", "recording"),
        Some(&Value::Bool(false))
    );
    assert!(!modal(runner, "keybinding-capture-new"));
    // Its onFocus starts recording (a click or Tab): the recorder takes its place, focused (autofocus) and modal.
    focusin(runner, "keybinding-capture-new", "keybinding-field-new");
    assert_eq!(
        slot_field(runner, "keybindingEditor", "recording"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        slot_field(runner, "keybindingEditor", "previous"),
        Some(&Value::str("new"))
    );
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
    assert_eq!(
        slot_field(runner, "keybindingEditor", "recording"),
        Some(&Value::Bool(true))
    );
    // Escape restores ⌘N; the row is no longer dirty, so the pill comes back in the field's place, as the
    // reference's KeybindingKeyControl renders it (showPill), and Settings stays.
    runner
        .dispatch(field, keydown("Escape", "Escape", ""))
        .unwrap();
    assert_eq!(
        slot_field(runner, "keybindingEditor", "key"),
        Some(&Value::str("mod+n"))
    );
    assert!(node(runner, "keybinding-recorder-chat-new").is_none());
    assert!(node(runner, "edit-keybinding-chat-new").is_some());
    assert_eq!(
        slot_field(runner, "settingsPageState", "open"),
        Some(&Value::Bool(true))
    );
}

pub(super) mod extraction {
    use super::empty;
    use crate::composer_accessibility_tests::set;
    use exact_plan::{Plan, TypesId, Value};
    use exact_runner::Runner;

    #[derive(Clone, Debug, PartialEq)]
    pub(crate) struct Call {
        pub(crate) method: &'static str,
        pub(crate) source: String,
        pub(crate) args: Vec<Value>,
    }

    pub(crate) fn call(method: &'static str, source: &str, args: &[Value]) -> Call {
        Call {
            method,
            source: source.to_owned(),
            args: args.to_vec(),
        }
    }

    pub(crate) fn resource(plan: &Plan, name: &str) -> (TypesId, String) {
        let row = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap();
        (row.ty, plan.str(row.source).to_owned())
    }

    pub(crate) fn mutation(plan: &Plan, name: &str) -> TypesId {
        plan.mutations
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap()
            .ty
    }

    pub(crate) fn field(plan: &Plan, ty: TypesId, value: &Value, name: &str) -> Value {
        let Value::Record(values) = value else {
            panic!("record field {name}")
        };
        values[plan.field_index(ty, name).unwrap()].clone()
    }

    pub(crate) fn fields(plan: &Plan, ty: TypesId, entries: &[(&str, Value)]) -> Value {
        let mut value = empty(plan, ty);
        for (name, next) in entries {
            set(plan, ty, &mut value, &[*name], next.clone());
        }
        assert!(value.conforms(plan, ty));
        value
    }

    pub(crate) fn element(plan: &Plan, ty: TypesId, name: &str) -> TypesId {
        let index = plan.field_index(ty, name).unwrap();
        let member = plan.type_(ty).fields.iter().nth(index).unwrap();
        plan.type_(plan.field(member).ty).elem.unwrap()
    }

    pub(crate) fn observation_type(plan: &Plan) -> TypesId {
        plan.derives
            .iter()
            .find(|row| plan.str(row.name) == "composerFieldSnapshot")
            .unwrap()
            .ty
    }

    pub(crate) fn identity_observation(
        plan: &Plan,
        owner: &str,
        generation: f64,
        value: &str,
        serial: f64,
    ) -> Value {
        let caret = 3.0 + serial;
        fields(
            plan,
            observation_type(plan),
            &[
                ("generation", Value::Number(generation)),
                ("snapshotOwner", Value::str(owner)),
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

    pub(crate) fn identity_snapshot(plan: &Plan, owner: &str) -> Value {
        fields(
            plan,
            resource(plan, "data").0,
            &[
                ("revision", Value::Number(1.0)),
                ("connected", Value::Bool(true)),
                ("sidebarOpen", Value::Bool(true)),
                ("snapshotOwner", Value::str(owner)),
                ("composerOwner", Value::str("draft")),
                ("draft", Value::str("word")),
            ],
        )
    }

    pub(crate) fn identity_next_command(plan: &Plan, args: &[Value], enabled: bool) -> Value {
        assert_eq!(args.len(), 3);
        let ty = observation_type(plan);
        let read = |name| field(plan, ty, &args[0], name);
        let after = args[1].as_number().unwrap();
        let ready = enabled && after == 0.0 && read("inputSerial") == Value::Number(1.0);
        fields(
            plan,
            resource(plan, "composerFieldNext").0,
            &[
                ("generation", read("generation")),
                ("snapshotOwner", read("snapshotOwner")),
                ("owner", read("owner")),
                ("value", if ready { read("value") } else { Value::str("") }),
                ("seq", Value::Number(if ready { 1.0 } else { after })),
                ("ready", Value::Bool(ready)),
                ("start", Value::Number(if ready { 4.0 } else { 0.0 })),
                ("end", Value::Number(if ready { 4.0 } else { 0.0 })),
                (
                    "selectionStart",
                    Value::Number(if ready { 4.0 } else { 0.0 }),
                ),
                ("selectionEnd", Value::Number(if ready { 4.0 } else { 0.0 })),
            ],
        )
    }

    pub(crate) fn identity_refusal(plan: &Plan) -> Value {
        fields(
            plan,
            mutation(plan, "composerFieldAck"),
            &[
                ("applied", Value::Bool(false)),
                ("reason", Value::str("field acknowledgement stale")),
                ("value", Value::str("word")),
                ("start", Value::Number(4.0)),
                ("end", Value::Number(4.0)),
                ("generation", Value::Number(1.0)),
                ("snapshotOwner", Value::str("snapshot")),
                ("owner", Value::str("draft")),
                ("seq", Value::Number(1.0)),
                ("inputSerial", Value::Number(-1.0)),
            ],
        )
    }

    pub(crate) fn identity_edit_reply(plan: &Plan) -> Value {
        fields(
            plan,
            mutation(plan, "composerEdited"),
            &[
                ("owner", Value::str("draft")),
                ("value", Value::str("word")),
                ("mode", Value::str("rich")),
                ("serial", Value::Number(1.0)),
                ("kind", Value::str("edit")),
                ("start", Value::Number(4.0)),
                ("end", Value::Number(4.0)),
                ("text", Value::str("\n")),
                ("selectionStart", Value::Number(5.0)),
                ("selectionEnd", Value::Number(5.0)),
                ("preserveSelection", Value::Bool(false)),
            ],
        )
    }

    pub(crate) fn logical_member<D: exact_runner::DataSource>(
        runner: &Runner<D>,
        original_layout: bool,
        original: &str,
        record: &str,
        name: &str,
    ) -> (TypesId, Value) {
        let plan = runner.plan();
        let slot_name = if original_layout { original } else { record };
        let row = plan
            .slots
            .iter()
            .find(|row| plan.str(row.name) == slot_name)
            .unwrap();
        if original_layout {
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

    pub(crate) fn task<D: exact_runner::DataSource>(runner: &Runner<D>, name: &str) -> Option<f64> {
        runner
            .tasks()
            .into_iter()
            .find(|(found, _)| found == name)
            .unwrap()
            .1
    }

    pub(crate) fn identity_field_protocol<D: exact_runner::DataSource>(
        runner: &Runner<D>,
        original_layout: bool,
        acked: f64,
    ) {
        for (original, name, expected) in [
            ("composerFieldAfter", "after", 1.0),
            ("composerFieldAcked", "acked", acked),
            ("composerFieldEffects", "effects", 0.0),
            ("composerFieldFocusDone", "focusDone", 0.0),
        ] {
            assert_eq!(
                logical_member(runner, original_layout, original, "fieldProtocol", name).1,
                Value::Number(expected)
            );
        }
        assert_eq!(
            logical_member(
                runner,
                original_layout,
                "composerFieldAdmitted",
                "fieldProtocol",
                "admitted"
            )
            .1,
            Value::Bool(true)
        );
        let args = vec![
            identity_observation(runner.plan(), "snapshot", 1.0, "word", 1.0),
            Value::Number(0.0),
            Value::Number(1.0),
        ];
        assert_eq!(
            logical_member(
                runner,
                original_layout,
                "composerFieldTarget",
                "fieldProtocol",
                "target"
            )
            .1,
            identity_next_command(runner.plan(), &args, true)
        );
        assert_eq!(
            logical_member(
                runner,
                original_layout,
                "composerFieldFocus",
                "fieldProtocol",
                "focus"
            )
            .1,
            fields(
                runner.plan(),
                mutation(runner.plan(), "composerFieldAck"),
                &[
                    ("generation", Value::Number(-1.0)),
                    ("inputSerial", Value::Number(-1.0)),
                ]
            )
        );
        assert_eq!(task(runner, "composerFieldDeliver"), None);
        assert_eq!(task(runner, "composerFieldRestoreFocus"), None);
    }
}

#[derive(Clone, Copy)]
struct PlacementCase {
    name: &'static str,
    target: Option<f64>,
    row: Option<f64>,
    port: (f64, f64),
    tries: f64,
    miss: f64,
    smooth: bool,
    reduced: bool,
    scroll: Option<(&'static str, f64)>,
    next_tries: f64,
    next_miss: f64,
    next_smooth: bool,
    settled: bool,
}

thread_local! {
    static PLACEMENT_GEOMETRY: std::cell::RefCell<(Vec<u32>, [exact_runner::geometry::GeometryAnswer; 3])> =
        Default::default();
}

fn placement_frame(
    _: &exact_kernel::Kernel,
    _: &exact_runner::geometry::Scrolled,
    view: u32,
) -> exact_runner::geometry::GeometryAnswer {
    PLACEMENT_GEOMETRY.with(|state| {
        let mut state = state.borrow_mut();
        state.0.push(view);
        state.1[view as usize - 1]
    })
}

fn placement_slot(
    plan: &Plan,
    original: bool,
    citation: bool,
    field: &str,
) -> (usize, Option<usize>) {
    let flat = match (citation, field) {
        (true, "key") => "citeKey",
        (true, "nonce") => "citeNonce",
        (true, "done") => "citeDone",
        (true, "tries") => "citeTries",
        (true, "smooth") => "citeSmooth",
        (true, "miss") => "citeMiss",
        (false, "tries") => "findTries",
        (false, "miss") => "findMiss",
        (false, "placing") => "findPlacing",
        _ => panic!("unknown placement field"),
    };
    let name = if original {
        flat
    } else if citation {
        "citationPlacement"
    } else {
        "findPlacement"
    };
    let (index, slot) = plan
        .slots
        .iter()
        .enumerate()
        .find(|(_, slot)| slot.owner.is_none() && plan.str(slot.name) == name)
        .expect("placement root slot");
    (
        index,
        (!original).then(|| {
            plan.field_index(slot.ty, field)
                .expect("placement record field")
        }),
    )
}

fn placement_assign(
    plan: &Plan,
    slots: &mut [Value],
    original: bool,
    citation: bool,
    field: &str,
    value: Value,
) {
    let (index, member) = placement_slot(plan, original, citation, field);
    if let Some(member) = member {
        let Value::Record(fields) = &slots[index] else {
            panic!("placement record")
        };
        let mut fields = fields.to_vec();
        fields[member] = value;
        slots[index] = Value::record(fields);
    } else {
        slots[index] = value;
    }
}

fn placement_case(plan: &Plan, original: bool, citation: bool, case: PlacementCase) {
    use exact_kernel::{Kernel, NodeType, Op, PropId};
    use exact_runner::geometry::{GeometryAnswer, GeometryEnv, GeometryLinks, Scrolled};
    use exact_runner::held::Held;
    use exact_runner::vm::{self, Env, Strings};

    let key = "fixture-entry";
    let mut slots: Vec<_> = plan.slots.iter().map(|slot| empty(plan, slot.ty)).collect();
    let initial = if citation {
        vec![
            ("key", Value::str(key)),
            ("nonce", Value::Number(7.0)),
            ("done", Value::Number(3.0)),
            ("tries", Value::Number(case.tries)),
            ("smooth", Value::Bool(case.smooth)),
            ("miss", Value::Number(case.miss)),
        ]
    } else {
        vec![
            ("tries", Value::Number(case.tries)),
            ("miss", Value::Number(case.miss)),
            ("placing", Value::Bool(true)),
        ]
    };
    for (field, value) in &initial {
        placement_assign(plan, &mut slots, original, citation, field, value.clone());
    }
    let mut resources: Vec<_> = plan
        .resources
        .iter()
        .map(|row| Some(Held::new(empty(plan, row.ty))))
        .collect();
    for (name, field, value) in [
        (
            "viewport",
            "prefersReducedMotion",
            Value::Bool(case.reduced),
        ),
        ("threadFind", "entry", Value::str(key)),
    ] {
        let (index, row) = plan
            .resources
            .iter()
            .enumerate()
            .find(|(_, row)| plan.str(row.name) == name)
            .expect("placement resource");
        let mut record = empty(plan, row.ty);
        set(plan, row.ty, &mut record, &[field], value);
        resources[index] = Some(Held::new(record));
    }

    let mut kernel = Kernel::with_monospace();
    let mut ops = Vec::new();
    for (index, id) in [
        if citation {
            format!("cite-{key}")
        } else {
            "thread-find-active".to_owned()
        },
        format!("row-{key}"),
        "transcript".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        let view = index as u32 + 1;
        ops.push(Op::CreateView {
            id: view,
            node_type: NodeType::View,
        });
        ops.push(Op::SetProp {
            id: view,
            prop: PropId::Id,
            value: id.into(),
        });
    }
    kernel.apply(0, 1, &ops).unwrap();
    let answer = |y: Option<f64>| {
        y.map_or(GeometryAnswer::UNAVAILABLE, |y| GeometryAnswer {
            y,
            width: 600.0,
            height: 100.0,
            ..Default::default()
        })
    };
    PLACEMENT_GEOMETRY.with(|state| {
        *state.borrow_mut() = (
            vec![],
            [
                answer(case.target),
                answer(case.row),
                GeometryAnswer {
                    y: case.port.0,
                    height: case.port.1,
                    width: 600.0,
                    ..Default::default()
                },
            ],
        )
    });
    static LINKS: GeometryLinks = GeometryLinks::new(
        placement_frame,
        |kernel, scrolled, view| placement_frame(kernel, scrolled, view),
        |_, _, _, _| None,
        |_, _, _, _| vec![],
        |_, _, _, _| true,
    );
    let scrolled = Scrolled::default();
    let geometry = GeometryEnv::new(&kernel, &scrolled, &LINKS, &[]);
    let strings = Strings::new(plan);
    let derives = vec![None; plan.derives.len()];
    let pending_resources = vec![false; plan.resources.len()];
    let failed_resources = vec![None; plan.resources.len()];
    let failed_why = vec![None; plan.resources.len()];
    let pending_mutations = vec![false; plan.mutations.len()];
    let failed_mutations = vec![None; plan.mutations.len()];
    let env = Env {
        plan,
        strings: &strings,
        router: None,
        lists: None,
        format: None,
        geometry: Some(&geometry),
        slots: &slots,
        derives: &derives,
        resources: &resources,
        params: &[],
        frames: &[],
        now_ms: 0.0,
        pending_resources: &pending_resources,
        failed_resources: &failed_resources,
        failed_why: &failed_why,
        pending_mutations: &pending_mutations,
        failed_mutations: &failed_mutations,
        store_dependent_derives: &[],
        store_dependent_resources: &[],
    };
    let name = if citation { "citePlace" } else { "findPlace" };
    let action = plan
        .actions
        .iter()
        .find(|action| plan.str(action.name) == name)
        .expect("compiled placement action");
    assert_eq!(action.params.len, 0);
    let allowed: Vec<_> = action.writes.iter().map(|id| plan.write(id).slot).collect();
    let outcome = vm::eval(plan.code(action.body), &env, &allowed)
        .unwrap_or_else(|error| panic!("{} original={original}: {error:?}", case.name));
    PLACEMENT_GEOMETRY.with(|state| assert_eq!(state.borrow().0, [1, 2, 3], "{}", case.name));
    let expected_commands = case.scroll.map_or_else(Vec::new, |(behavior, offset)| {
        vec![(
            "scrollIntoView".to_owned(),
            vec![
                Value::str("transcript"),
                Value::str(key),
                Value::NONE,
                Value::NONE,
                Value::str(behavior),
                Value::NONE,
                Value::Number(offset),
            ],
        )]
    });
    assert_eq!(
        outcome.commands, expected_commands,
        "{} original={original}",
        case.name
    );
    assert!(
        outcome.sends.is_empty() && outcome.refreshes.is_empty() && outcome.row_writes.is_empty()
    );
    assert_eq!(outcome.value, Value::Unit);
    let expected_names = if !original {
        vec![if citation {
            "citationPlacement"
        } else {
            "findPlacement"
        }]
    } else if case.target.is_none() || case.row.is_none() {
        vec![if citation { "citeTries" } else { "findTries" }]
    } else if case.settled {
        vec![if citation { "citeDone" } else { "findPlacing" }]
    } else if citation && case.next_smooth {
        vec!["citeSmooth", "citeMiss", "citeTries"]
    } else if citation {
        vec!["citeMiss", "citeTries"]
    } else {
        vec!["findMiss", "findTries"]
    };
    let mut actual_names: Vec<_> = outcome
        .writes
        .iter()
        .map(|(index, _)| plan.str(plan.slots[*index as usize].name))
        .collect();
    let mut expected_names = expected_names;
    actual_names.sort_unstable();
    expected_names.sort_unstable();
    assert_eq!(
        actual_names, expected_names,
        "{} original={original}",
        case.name
    );
    let mut expected_slots = slots.clone();
    placement_assign(
        plan,
        &mut expected_slots,
        original,
        citation,
        "tries",
        Value::Number(case.next_tries),
    );
    placement_assign(
        plan,
        &mut expected_slots,
        original,
        citation,
        "miss",
        Value::Number(case.next_miss),
    );
    if citation {
        placement_assign(
            plan,
            &mut expected_slots,
            original,
            true,
            "smooth",
            Value::Bool(case.next_smooth),
        );
        placement_assign(
            plan,
            &mut expected_slots,
            original,
            true,
            "done",
            Value::Number(if case.settled { 7.0 } else { 3.0 }),
        );
    } else {
        placement_assign(
            plan,
            &mut expected_slots,
            original,
            false,
            "placing",
            Value::Bool(!case.settled),
        );
    }
    let mut actual_slots = slots;
    for (index, value) in outcome.writes {
        actual_slots[index as usize] = value;
    }
    assert_eq!(
        actual_slots, expected_slots,
        "{} original={original}",
        case.name
    );
}

#[test]
fn extraction_placement_citation_geometry_and_retries() {
    let base = PlacementCase {
        name: "first smooth",
        target: Some(430.0),
        row: Some(400.0),
        port: (50.0, 600.0),
        tries: 0.0,
        miss: 1_000_000.0,
        smooth: false,
        reduced: false,
        scroll: Some(("smooth", -88.0)),
        next_tries: 1.0,
        next_miss: 262.0,
        next_smooth: true,
        settled: false,
    };
    let cases = [
        base,
        PlacementCase {
            name: "missing target",
            target: None,
            smooth: true,
            scroll: Some(("instant", -120.0)),
            next_miss: base.miss,
            ..base
        },
        PlacementCase {
            name: "missing row",
            row: None,
            smooth: true,
            scroll: Some(("instant", -120.0)),
            next_miss: base.miss,
            ..base
        },
        PlacementCase {
            name: "reduced motion",
            reduced: true,
            scroll: Some(("instant", -88.0)),
            next_smooth: false,
            ..base
        },
        PlacementCase {
            name: "retry",
            tries: 1.0,
            scroll: Some(("instant", -88.0)),
            next_tries: 2.0,
            next_smooth: false,
            ..base
        },
        PlacementCase {
            name: "short port",
            port: (0.0, 300.0),
            target: Some(700.0),
            row: Some(700.0),
            scroll: Some(("smooth", -98.0)),
            next_miss: 602.0,
            ..base
        },
        PlacementCase {
            name: "positive tolerance",
            target: Some(169.0),
            scroll: None,
            next_tries: 0.0,
            next_miss: base.miss,
            next_smooth: false,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "negative tolerance",
            target: Some(167.0),
            scroll: None,
            next_tries: 0.0,
            next_miss: base.miss,
            next_smooth: false,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "unchanged miss",
            miss: 262.0,
            scroll: None,
            next_tries: 0.0,
            next_miss: 262.0,
            next_smooth: false,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "half point",
            miss: 262.5,
            scroll: None,
            next_tries: 0.0,
            next_miss: 262.5,
            next_smooth: false,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "over half point",
            miss: 262.5001,
            ..base
        },
    ];
    for (bytes, original) in extraction_plans() {
        let plan = Plan::decode(bytes).unwrap();
        for case in cases {
            placement_case(&plan, original, true, case);
        }
    }
}

#[test]
fn extraction_placement_find_geometry_and_retries() {
    let base = PlacementCase {
        name: "find offset",
        target: Some(430.0),
        row: Some(400.0),
        port: (50.0, 600.0),
        tries: 0.0,
        miss: 1_000_000.0,
        smooth: false,
        reduced: false,
        scroll: Some(("instant", -66.0)),
        next_tries: 1.0,
        next_miss: 284.0,
        next_smooth: false,
        settled: false,
    };
    let cases = [
        base,
        PlacementCase {
            name: "missing target",
            target: None,
            scroll: None,
            next_miss: base.miss,
            ..base
        },
        PlacementCase {
            name: "missing row",
            row: None,
            scroll: None,
            next_miss: base.miss,
            ..base
        },
        PlacementCase {
            name: "negative miss",
            target: Some(100.0),
            row: Some(130.0),
            scroll: Some(("instant", -126.0)),
            next_miss: -46.0,
            ..base
        },
        PlacementCase {
            name: "positive tolerance",
            target: Some(147.0),
            scroll: None,
            next_tries: 0.0,
            next_miss: base.miss,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "negative tolerance",
            target: Some(145.0),
            scroll: None,
            next_tries: 0.0,
            next_miss: base.miss,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "unchanged miss",
            miss: 284.0,
            scroll: None,
            next_tries: 0.0,
            next_miss: 284.0,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "half point",
            miss: 284.5,
            scroll: None,
            next_tries: 0.0,
            next_miss: 284.5,
            settled: true,
            ..base
        },
        PlacementCase {
            name: "over half point",
            miss: 284.5001,
            ..base
        },
    ];
    for (bytes, original) in extraction_plans() {
        let plan = Plan::decode(bytes).unwrap();
        for case in cases {
            placement_case(&plan, original, false, case);
        }
    }
}
