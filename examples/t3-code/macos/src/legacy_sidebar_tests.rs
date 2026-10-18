//! The compiled app's command-completion actions, using the existing plan fixture.
//! Native field selection and row-drop input remain separate AppKit qualifications.

use super::composer_accessibility_tests::{measure, Replies};
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::Plan;
use exact_runner::{Event, KeyboardEvent, Runner, Value};

fn host(rename: &str, message: &str) -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let mut replies = Replies::default();
    replies.fixture(
        &plan,
        "data",
        &[
            (&["revision"], Value::Number(1.0)),
            (&["sidebar", "renameId"], Value::str(rename)),
        ],
    );
    // The refreshed snapshot deliberately has no navigation state. The reply
    // carries its destination, so neither its timing nor an older route can win.
    replies.0.insert(
        "command".to_owned(),
        Value::record(vec![
            Value::Number(2.0),
            Value::str(message),
            Value::Bool(false),
            Value::Number(0.0),
        ]),
    );
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    host.runner_mut()
        .set_time(1_791_446_400_000.0, 0.0)
        .unwrap();
    host.runner_mut().advance(1250.0).unwrap();
    host.runner_mut().take_commands();
    host
}

#[test]
fn project_settings_reply_selects_its_member_without_navigation_in_the_snapshot() {
    for project in ["p-local", "p-remote", "project:with:colons"] {
        let mut host = host("", &format!("sidebar:project-settings:{project}"));
        let runner = host.runner_mut();
        runner
            .act(
                "sidebarRun",
                vec![
                    Value::str("legacy-project-menu"),
                    Value::str(project),
                    Value::str(""),
                ],
            )
            .unwrap();
        assert!(runner.land_then().error.is_none());
        assert_eq!(runner.slot("settingsOpen"), Some(&Value::Bool(true)));
        assert_eq!(runner.slot("settingsRoute"), Some(&Value::str("projects")));
        assert_eq!(runner.slot("settingsProjectId"), Some(&Value::str(project)));
        assert_eq!(runner.slot("commandPending"), Some(&Value::Bool(false)));
    }
}

#[test]
fn an_active_rename_refuses_late_composer_focus_but_plain_click_keeps_it() {
    for (rename, focuses) in [("thread-b", false), ("", true)] {
        let mut host = host(rename, "focus:composer");
        let runner = host.runner_mut();
        runner
            .act(
                "command",
                vec![
                    Value::str("select-thread"),
                    Value::str("thread-b"),
                    Value::str("click"),
                    Value::Number(0.0),
                ],
            )
            .unwrap();
        assert!(runner.land_then().error.is_none());
        let commands = runner.take_commands();
        assert_eq!(
            commands
                .iter()
                .any(|command| command.name == "focus"
                    && command.args == vec![Value::str("composer")]),
            focuses
        );
    }
}

fn options_snapshot(replies: &mut Replies, plan: &Plan, count: f64) {
    replies.fixture(
        plan,
        "data",
        &[
            (&["revision"], Value::Number(count)),
            (&["connected"], Value::Bool(true)),
            (&["sidebarOpen"], Value::Bool(true)),
            (&["legacy", "enabled"], Value::Bool(true)),
            (&["legacy", "previewCount"], Value::Number(count)),
            (&["legacy", "projectSort"], Value::str("updated_at")),
            (&["legacy", "threadSort"], Value::str("updated_at")),
        ],
    );
}

fn options_host() -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let mut replies = Replies::default();
    options_snapshot(&mut replies, &plan, 5.0);
    replies.fixture(&plan, "welcome", &[(&["show"], Value::Bool(false))]);
    replies.0.insert(
        "command".to_owned(),
        Value::record(vec![
            Value::Number(15.0),
            Value::str(""),
            Value::Bool(false),
            Value::Number(0.0),
        ]),
    );
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    let runner = host.runner_mut();
    runner.set_time(1_791_446_400_000.0, 0.0).unwrap();
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

fn focus_path(runner: &mut Runner<Replies>, from: u32, to: Option<u32>, entering: bool) {
    for (view, event) in runner.focus_bubble(from, to, entering) {
        runner.dispatch(view, event).unwrap();
    }
}

#[test]
fn options_keyboard_opening_names_its_source_and_an_actual_endpoint_once() {
    let mut host = options_host();
    let runner = host.runner_mut();
    let trigger = node(runner, "legacy-options");
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
                Value::str("legacy-sidebar-options"),
                Value::str("legacy-options")
            ]
        );
        let target = if key == "ArrowUp" {
            "legacy-sort-threads-created_at"
        } else {
            "legacy-sort-projects-updated_at"
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
fn options_hover_focus_drives_arrows_and_the_whole_popup_owns_focus_departure() {
    let mut host = options_host();
    let runner = host.runner_mut();
    let manual = node(runner, "legacy-sort-projects-manual");
    runner.dispatch(manual, Event::Hover(true)).unwrap();
    let commands = runner.take_commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "focus");
    assert_eq!(
        commands[0].args,
        vec![Value::str("legacy-sort-projects-manual")]
    );
    runner.dispatch(manual, Event::Focus).unwrap();
    focus_path(runner, manual, None, true);
    assert!(runner.take_commands().is_empty());
    let keys = node(runner, "legacy-sidebar-options-keys");
    runner
        .dispatch(keys, Event::Key(KeyboardEvent::chord("ArrowDown")))
        .unwrap();
    let commands = runner.take_commands();
    assert_eq!(commands.last().unwrap().name, "focus");
    assert_eq!(
        commands.last().unwrap().args,
        vec![Value::str("legacy-sort-threads-updated_at")]
    );

    let inside = [
        "legacy-sort-projects-manual",
        "legacy-preview-count",
        "legacy-preview-increase",
        "legacy-preview-decrease",
        "legacy-sort-threads-created_at",
    ];
    for pair in inside.windows(2) {
        let from = node(runner, pair[0]);
        let to = node(runner, pair[1]);
        focus_path(runner, from, Some(to), false);
        focus_path(runner, to, Some(from), true);
        assert!(
            runner.take_commands().is_empty(),
            "{} → {}",
            pair[0],
            pair[1]
        );
    }
    for outside in [Some(node(runner, "legacy-options")), None] {
        let count = node(runner, "legacy-preview-count");
        focus_path(runner, count, outside, false);
        let commands = runner.take_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "hidePopover");
        assert_eq!(commands[0].args, vec![Value::str("legacy-sidebar-options")]);
    }
}

#[test]
fn options_count_reconciles_the_clamped_reply_without_replacing_the_input() {
    let mut host = options_host();
    let runner = host.runner_mut();
    let input = node(runner, "legacy-preview-count");
    let value = |runner: &Runner<Replies>| {
        runner
            .kernel()
            .node(input)
            .unwrap()
            .props
            .iter()
            .find(|(id, _)| id.name() == "value")
            .unwrap()
            .1
            .as_str()
            .unwrap()
            .to_owned()
    };
    runner.dispatch(input, Event::Input("99".into())).unwrap();
    assert_eq!(value(runner), "99");
    // The TypeScript data test owns the 1–15 arithmetic. Here its reply must
    // reconcile the same focused input rather than remount a count-keyed row.
    options_snapshot(runner.data(), &Plan::decode(PLAN).unwrap(), 15.0);
    runner.dispatch(input, Event::Change("99".into())).unwrap();
    assert!(runner.land_then().error.is_none());
    assert_eq!(node(runner, "legacy-preview-count"), input);
    assert_eq!(value(runner), "15");
    assert_eq!(
        runner.data_ref().1,
        vec![vec![
            Value::str("sidebarlocal:legacy-preview"),
            Value::str(""),
            Value::str("99"),
            Value::Number(0.0)
        ]]
    );
    assert!(!runner
        .take_commands()
        .iter()
        .any(|c| c.name == "hidePopover" || c.name == "focus"));
}

/// Tab from Visible threads went to the
/// increase stepper, the clamp to 15 then disabled it, and the focus stayed
/// on a disabled button that took no further Tab. Base UI's steppers
/// (useNumberFieldButton, tabIndex -1) are no Tab stops and a press leaves
/// the focus in the field, so the field is the popup's last stop and Tab
/// leaves the popup (which then hides, as above).
#[test]
fn options_steppers_are_no_tab_stops_so_a_clamp_never_strands_the_focus() {
    let mut host = options_host();
    let runner = host.runner_mut();
    let prop = |runner: &Runner<Replies>, id: &str, name: &str| {
        let view = node(runner, id);
        runner
            .kernel()
            .node(view)
            .unwrap()
            .props
            .iter()
            .find(|(prop, _)| prop.name() == name)
            .map(|(_, value)| format!("{value:?}"))
    };
    let input = node(runner, "legacy-preview-count");
    runner.dispatch(input, Event::Input("22".into())).unwrap();
    options_snapshot(runner.data(), &Plan::decode(PLAN).unwrap(), 15.0);
    runner.dispatch(input, Event::Change("22".into())).unwrap();
    assert!(runner.land_then().error.is_none());
    assert_eq!(
        prop(runner, "legacy-preview-increase", "disabled"),
        Some("Bool(true)".to_owned())
    );
    for id in ["legacy-preview-decrease", "legacy-preview-increase"] {
        let tab_index = prop(runner, id, "tabIndex");
        assert!(
            matches!(tab_index.as_deref(), Some("Int(-1)" | "Float(-1.0)")),
            "{id}: {tab_index:?}"
        );
        // A clicked button takes no focus on the Mac whatever its tabindex, so the field keeps it with no retainFocus.
        assert_eq!(prop(runner, id, "retainFocus"), None, "{id}");
    }
    assert_eq!(prop(runner, "legacy-preview-count", "tabIndex"), None);
}
