//! The compiled app's command-completion actions, using the existing plan fixture.
//! Native field selection and row-drop input remain separate AppKit qualifications.

use super::composer_accessibility_tests::{measure, Replies};
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::Plan;
use exact_runner::Value;

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
