//! The actual app plan, through its rendered route handlers and command replies.
//! Store writes are covered in connections-routes.test.ts; native focus, spoken
//! announcements and process replacement have separate attended qualifications.

use super::composer_accessibility_tests::{first_frame, measure, set, Replies};
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::{Plan, TypesId};
use exact_runner::{Event, KeyboardEvent, Outcome, Response, Runner, Value};

fn initial(plan: &Plan, name: &str, path: &[&str]) -> (TypesId, Value) {
    let resource = plan
        .resources
        .iter()
        .find(|row| plan.str(row.name) == name)
        .unwrap();
    let mut ty = resource.ty;
    let mut value = first_frame(name);
    for name in path {
        let index = plan.field_index(ty, name).unwrap();
        let Value::Record(fields) = value else {
            panic!("fixture record")
        };
        value = fields[index].clone();
        ty = plan
            .field(plan.type_(ty).fields.iter().nth(index).unwrap())
            .ty;
    }
    (ty, value)
}

fn environment(plan: &Plan, id: &str, ids: &[&str]) -> Value {
    let (ty, mut value) = initial(plan, "connectionsPage", &["thisMachine", "environment"]);
    for (name, next) in [
        ("key", Value::str(&format!("key-{id}"))),
        ("environmentId", Value::str(id)),
        ("label", Value::str(&format!("Machine {id}"))),
        ("enabled", Value::Bool(true)),
        ("routeCount", Value::str(&format!("{} routes", ids.len()))),
        (
            "routesLabel",
            Value::str(&format!("Routes to {id}, preferred first")),
        ),
    ] {
        set(plan, ty, &mut value, &[name], next);
    }
    let routes = ids
        .iter()
        .enumerate()
        .map(|(index, route)| {
            Value::record(vec![
                Value::str(route),
                Value::str(&format!("Route {route}")),
                Value::str(&format!("https://{route}.invalid/")),
                Value::Bool(index == 0),
                Value::Bool(true),
                Value::Number((index + 1) as f64),
                Value::Bool(index == 0),
                Value::Bool(index + 1 == ids.len()),
                Value::Bool(false),
                Value::str(&format!("route:{id}:{route}")),
            ])
        })
        .collect();
    set(plan, ty, &mut value, &["routes"], Value::list(routes));
    value
}

fn host() -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let mut replies = Replies::default();
    replies.fixture(
        &plan,
        "data",
        &[
            (&["revision"], Value::Number(1.0)),
            (&["connected"], Value::Bool(true)),
        ],
    );
    replies.fixture(&plan, "welcome", &[(&["show"], Value::Bool(false))]);
    replies.fixture(
        &plan,
        "connectionsPage",
        &[(
            &["environments"],
            Value::list(vec![
                environment(&plan, "a", &["a1", "a2", "a3"]),
                environment(&plan, "b", &["b1", "b2", "b3"]),
            ]),
        )],
    );
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, replies, Box::new(measurer), 1280.0, 840.0).unwrap();
    let runner = host.runner_mut();
    runner.set_time(1_791_446_400_000.0, 0.0).unwrap();
    runner.advance(1250.0).unwrap();
    runner.act("openConnectionsPage", vec![]).unwrap();
    for environment in ["a", "b"] {
        let view = test_node(runner, &format!("environment-routes-toggle-{environment}"));
        runner.dispatch(view, Event::Press).unwrap();
    }
    assert_eq!(
        runner.derive("routeKeyboardReady"),
        Some(&Value::Bool(true))
    );
    runner.take_commands();
    runner.take_requests();
    runner.data().1.clear();
    host
}

fn test_node(runner: &Runner<Replies>, id: &str) -> u32 {
    let nodes = runner.kernel().find_by_test_id(id);
    assert_eq!(nodes.len(), 1, "one rendered {id}");
    runner.kernel().node_by_key(nodes[0]).unwrap().id
}

fn node(runner: &Runner<Replies>, id: &str) -> u32 {
    let nodes = runner.kernel().find_by_id(id);
    assert_eq!(nodes.len(), 1, "one rendered {id}");
    runner.kernel().node_by_key(nodes[0]).unwrap().id
}

fn prop(runner: &Runner<Replies>, view: u32, name: &str) -> Value {
    let value = &runner
        .kernel()
        .node(view)
        .unwrap()
        .props
        .iter()
        .find(|(id, _)| id.name() == name)
        .unwrap()
        .1;
    if let Some(text) = value.as_str() {
        Value::str(text)
    } else if let Some(number) = value.as_int() {
        Value::Number(number as f64)
    } else if let Some(number) = value.as_float() {
        Value::Number(number)
    } else {
        Value::Bool(value.as_bool().unwrap())
    }
}

fn state(runner: &Runner<Replies>, index: usize) -> &Value {
    let Value::Record(fields) = runner.slot("routeKeyboard").unwrap() else {
        panic!("route transaction")
    };
    &fields[index]
}

fn grip(environment: &str, route: &str) -> String {
    format!("environment-route-grip-{environment}-{route}")
}

fn key(runner: &mut Runner<Replies>, environment: &str, route: &str, key: &str, repeat: bool) {
    let view = node(runner, &grip(environment, route));
    let mut event = KeyboardEvent::chord(key);
    event.repeat = repeat;
    runner.dispatch(view, Event::Key(event)).unwrap();
}

fn label(runner: &Runner<Replies>, environment: &str, route: &str) -> Value {
    prop(
        runner,
        node(runner, &grip(environment, route)),
        "accessibilityLabel",
    )
}

fn answer(relaunch: bool, serial: f64, message: &str) -> Value {
    Value::record(vec![
        Value::Number(2.0),
        Value::str(message),
        Value::Bool(relaunch),
        Value::Number(serial),
    ])
}

fn ticket(runner: &mut Runner<Replies>) -> u64 {
    runner
        .take_requests()
        .into_iter()
        .find(|request| request.target == "changed")
        .unwrap()
        .ticket
}

fn land(runner: &mut Runner<Replies>, ticket: u64, result: Value) {
    runner.data().0.insert("command".into(), result);
    runner
        .fulfill(
            ticket,
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: vec![],
            }),
        )
        .unwrap();
    assert!(runner.land_then().error.is_none());
}

fn persisted_order(runner: &mut Runner<Replies>, order: &[&str]) {
    let plan = Plan::decode(PLAN).unwrap();
    runner.data().fixture(
        &plan,
        "data",
        &[
            (&["revision"], Value::Number(2.0)),
            (&["connected"], Value::Bool(true)),
        ],
    );
    runner.data().fixture(
        &plan,
        "connectionsPage",
        &[(
            &["environments"],
            Value::list(vec![
                environment(&plan, "a", order),
                environment(&plan, "b", &["b1", "b2", "b3"]),
            ]),
        )],
    );
}

#[test]
fn route_keys_move_the_rendered_rows_and_send_one_identity_based_drop() {
    let mut host = host();
    let runner = host.runner_mut();
    let first = node(runner, &grip("a", "a1"));
    assert_eq!(prop(runner, first, "tabIndex"), Value::Number(0.0));
    assert_eq!(
        label(runner, "a", "a1"),
        Value::str("Reorder Route a1, position 1")
    );
    key(runner, "a", "a1", "Space", true);
    assert_eq!(state(runner, 0), &Value::str(""));
    key(runner, "a", "a1", "Space", false);
    assert_eq!(state(runner, 0), &Value::str("lifted"));
    assert!(state(runner, 8)
        .as_str()
        .unwrap()
        .starts_with("Lifted Route a1, position 1 of 3."));
    let live = test_node(runner, "environment-route-announcement");
    assert_eq!(prop(runner, live, "text"), state(runner, 8).clone());
    assert_eq!(
        prop(runner, live, "accessibilityLive"),
        Value::str("polite")
    );
    let announcement = state(runner, 8).clone();
    for chord in ["ArrowUp", "ArrowLeft", "ArrowRight"] {
        key(runner, "a", "a1", chord, false);
        assert_eq!(state(runner, 8), &announcement);
    }
    key(runner, "b", "b1", "ArrowDown", false);
    assert_eq!(
        label(runner, "b", "b1"),
        Value::str("Reorder Route b1, position 1")
    );
    key(runner, "a", "a1", "ArrowDown", false);
    assert_eq!(
        node(runner, &grip("a", "a1")),
        first,
        "the keyed grip survives preview movement"
    );
    assert_eq!(
        label(runner, "a", "a1"),
        Value::str("Reorder Route a1, position 2")
    );
    assert_eq!(
        label(runner, "a", "a2"),
        Value::str("Reorder Route a2, position 1")
    );
    assert_eq!(
        state(runner, 8),
        &Value::str("Moved Route a1 to position 2 of 3.")
    );
    let commands = runner.take_commands();
    assert!(commands.iter().any(
        |command| command.name == "focus" && command.args == vec![Value::str(&grip("a", "a1"))]
    ));
    let list = node(runner, "environment-routes-list-a");
    runner
        .dispatch(
            list,
            Event::ReorderDrop {
                item: "a3".into(),
                before: Some("a1".into()),
            },
        )
        .unwrap();
    assert!(
        runner.data_ref().1.is_empty(),
        "a queued pointer drop cannot overlap the keyboard transaction"
    );
    key(runner, "a", "a1", "Space", true);
    assert_eq!(state(runner, 0), &Value::str("lifted"));
    key(runner, "a", "a1", "Space", false);
    assert_eq!(state(runner, 0), &Value::str("saving"));
    assert_eq!(
        runner.data_ref().1,
        vec![vec![
            Value::str("environment-route-move"),
            Value::str("key-a"),
            Value::str("a1>a3\na1\na2\na3"),
            Value::Number(1.0)
        ]]
    );
    key(runner, "a", "a1", "Space", false);
    key(runner, "b", "b1", "Space", false);
    assert_eq!(runner.data_ref().1.len(), 1);
    let ticket = ticket(runner);
    persisted_order(runner, &["a2", "a1", "a3"]);
    land(runner, ticket, answer(false, 1.0, ""));
    assert_eq!(state(runner, 0), &Value::str(""));
    assert_eq!(
        state(runner, 8),
        &Value::str("Dropped Route a1 at position 2 of 3.")
    );
    assert_eq!(
        label(runner, "a", "a1"),
        Value::str("Reorder Route a1, position 2")
    );
}

#[test]
fn route_cancel_blur_and_unchanged_drop_never_send_a_write() {
    for ending in ["Escape", "blur", "unchanged"] {
        let mut host = host();
        let runner = host.runner_mut();
        let back = test_node(runner, "close-settings");
        assert_eq!(
            prop(runner, back, "accessibilityKeyShortcuts"),
            Value::str("Escape")
        );
        key(runner, "a", "a2", "Space", false);
        // Native registered shortcuts run before the focused grip's handler.
        // Back must release Escape so the route transaction can cancel itself.
        assert_eq!(
            prop(runner, back, "accessibilityKeyShortcuts"),
            Value::str("")
        );
        key(runner, "a", "a2", "ArrowUp", false);
        if ending == "unchanged" {
            key(runner, "a", "a2", "ArrowDown", false);
            key(runner, "a", "a2", "Enter", false);
        } else if ending == "blur" {
            let view = node(runner, &grip("a", "a2"));
            runner.dispatch(view, Event::Blur).unwrap();
        } else {
            key(runner, "a", "a2", ending, false);
        }
        assert_eq!(state(runner, 0), &Value::str(""), "{ending}");
        assert_eq!(runner.slot("settingsOpen"), Some(&Value::Bool(true)));
        assert_eq!(
            prop(runner, back, "accessibilityKeyShortcuts"),
            Value::str("Escape")
        );
        assert_eq!(
            label(runner, "a", "a2"),
            Value::str("Reorder Route a2, position 2")
        );
        assert!(runner.data_ref().1.is_empty(), "{ending}");
        assert!(state(runner, 8)
            .as_str()
            .unwrap()
            .contains(if ending == "unchanged" {
                "Order unchanged"
            } else {
                "Canceled"
            }));
    }
}

#[test]
fn route_remove_dialog_owns_its_shortcut_and_cancel_keeps_settings_and_routes() {
    let mut host = host();
    let runner = host.runner_mut();
    let opener = node(runner, "environment-route-remove-a-a2");
    runner.dispatch(opener, Event::Press).unwrap();
    let dialog = test_node(runner, "route-remove-dialog");
    let cancel = test_node(runner, "route-remove-cancel");
    // Modal from the commit after the focus entered it (dialog-trap.contract), as DialogOpenFocus's focus on Cancel
    // reaches the popup's `focusin`.
    assert_eq!(
        prop(runner, dialog, "accessibilityModal"),
        Value::Bool(false)
    );
    runner
        .dispatch(
            dialog,
            Event::Focusin {
                target: "route-remove-cancel".to_owned(),
                related_target: None,
            },
        )
        .unwrap();
    assert_eq!(
        prop(runner, dialog, "accessibilityModal"),
        Value::Bool(true)
    );
    assert_eq!(
        prop(runner, cancel, "accessibilityKeyShortcuts"),
        Value::str("Escape")
    );
    // The native shortcut filter admits only descendants of the active modal.
    let inside_dialog = |view| {
        let mut current = Some(view);
        while let Some(id) = current {
            if id == dialog {
                return true;
            }
            current = runner.kernel().node(id).unwrap().parent;
        }
        false
    };
    assert!(inside_dialog(cancel));
    assert!(!inside_dialog(test_node(runner, "close-settings")));
    runner.dispatch(cancel, Event::Press).unwrap();
    assert_eq!(runner.slot("connectionRemove"), Some(&Value::str("")));
    assert_eq!(runner.slot("settingsOpen"), Some(&Value::Bool(true)));
    assert!(runner
        .kernel()
        .find_by_test_id("route-remove-dialog")
        .is_empty());
    assert_eq!(node(runner, "environment-route-remove-a-a2"), opener);
    for (index, route) in ["a1", "a2", "a3"].into_iter().enumerate() {
        assert_eq!(
            label(runner, "a", route),
            Value::str(&format!("Reorder Route {route}, position {}", index + 1))
        );
    }
    assert!(runner.data_ref().1.is_empty(), "cancel cannot write routes");
}

#[test]
fn route_boundary_repeat_and_idle_pointer_drop_keep_their_own_semantics() {
    let mut host = host();
    let runner = host.runner_mut();
    key(runner, "a", "a3", "Enter", false);
    assert_eq!(state(runner, 0), &Value::str(""));
    key(runner, "a", "a3", "Space", false);
    let announcement = state(runner, 8).clone();
    key(runner, "a", "a3", "ArrowDown", true);
    assert_eq!(state(runner, 8), &announcement);
    key(runner, "a", "a3", "ArrowUp", true);
    assert_eq!(
        label(runner, "a", "a3"),
        Value::str("Reorder Route a3, position 2")
    );
    key(runner, "a", "a3", "Escape", false);
    assert!(runner.data_ref().1.is_empty());
    let list = node(runner, "environment-routes-list-b");
    runner
        .dispatch(
            list,
            Event::ReorderDrop {
                item: "b3".into(),
                before: Some("b1".into()),
            },
        )
        .unwrap();
    assert_eq!(
        runner.data_ref().1,
        vec![vec![
            Value::str("environment-route-move"),
            Value::str("key-b"),
            Value::str("b3>b1"),
            Value::Number(0.0)
        ]]
    );
}

#[test]
fn route_owner_loss_cancels_without_a_write() {
    for ending in ["window", "hidden", "page", "collapse", "busy", "routes"] {
        let mut host = host();
        let runner = host.runner_mut();
        key(runner, "a", "a1", "Space", false);
        key(runner, "a", "a1", "ArrowDown", false);
        match ending {
            "window" | "hidden" => {
                let mut page = runner.page();
                page.has_focus = ending != "window";
                page.hidden = ending == "hidden";
                runner.set_page(page).unwrap();
            }
            "page" => {
                runner
                    .act("coreNavigate", vec![Value::str("appearance")])
                    .unwrap();
            }
            "collapse" => {
                let view = test_node(runner, "environment-routes-toggle-a");
                runner.dispatch(view, Event::Press).unwrap();
            }
            "busy" => {
                runner
                    .act(
                        "connectionOp",
                        vec![
                            Value::str("environment-enable"),
                            Value::str("key-b"),
                            Value::str("false"),
                        ],
                    )
                    .unwrap();
            }
            "routes" => {
                let plan = Plan::decode(PLAN).unwrap();
                runner.data().fixture(
                    &plan,
                    "connectionsPage",
                    &[(
                        &["environments"],
                        Value::list(vec![
                            environment(&plan, "a", &["a2", "a1", "a3"]),
                            environment(&plan, "b", &["b1", "b2", "b3"]),
                        ]),
                    )],
                );
                runner.advance(runner.now_ms() + 2.0).unwrap();
                runner.act("tick", vec![]).unwrap();
                assert_eq!(
                    runner.derive("routeKeyboardOwned"),
                    Some(&Value::Bool(false)),
                    "the refreshed route order invalidates the captured baseline"
                );
            }
            _ => unreachable!(),
        }
        runner.advance(runner.now_ms() + 2.0).unwrap();
        assert_eq!(state(runner, 0), &Value::str(""), "{ending}");
        assert!(
            runner
                .data_ref()
                .1
                .iter()
                .all(|args| args[0] != Value::str("environment-route-move")),
            "{ending}"
        );
    }
}

#[test]
fn route_completion_requires_its_own_live_request_and_owner() {
    for outcome in ["success", "failure", "wrong-request", "owner-lost"] {
        let mut host = host();
        let runner = host.runner_mut();
        key(runner, "a", "a1", "Space", false);
        key(runner, "a", "a1", "ArrowDown", false);
        key(runner, "a", "a1", "Space", false);
        let request = ticket(runner);
        assert_eq!(
            prop(
                runner,
                test_node(runner, "close-settings"),
                "accessibilityKeyShortcuts"
            ),
            Value::str("")
        );
        runner.take_commands();
        if outcome == "success" {
            persisted_order(runner, &["a2", "a1", "a3"]);
        }
        if outcome == "owner-lost" {
            let view = test_node(runner, "environment-routes-toggle-a");
            runner.dispatch(view, Event::Press).unwrap();
        }
        land(
            runner,
            request,
            answer(
                false,
                if outcome == "wrong-request" { 0.0 } else { 1.0 },
                if outcome == "failure" {
                    "The route store refused the write."
                } else {
                    ""
                },
            ),
        );
        match outcome {
            "success" => assert!(state(runner, 8).as_str().unwrap().starts_with("Dropped ")),
            "failure" => assert!(state(runner, 8)
                .as_str()
                .unwrap()
                .contains("did not finish")),
            "wrong-request" => {
                assert_eq!(state(runner, 0), &Value::str("saving"));
                assert!(state(runner, 8).as_str().unwrap().starts_with("Saving "));
                key(runner, "a", "a1", "Space", false);
                assert_eq!(
                    runner.data_ref().1.len(),
                    1,
                    "only a lifted-to-saving transition can submit"
                );
                runner.advance(runner.now_ms() + 2.0).unwrap();
                assert_eq!(state(runner, 8), &Value::str(""));
            }
            "owner-lost" => assert_eq!(state(runner, 8), &Value::str("")),
            _ => unreachable!(),
        }
        assert_eq!(state(runner, 0), &Value::str(""));
        assert!(
            !runner
                .take_commands()
                .iter()
                .any(|command| command.name == "focus"),
            "completion cannot restore focus after ownership changes"
        );
    }
}

#[test]
fn an_old_drop_reply_cannot_complete_a_new_saving_transaction() {
    let mut host = host();
    let runner = host.runner_mut();
    key(runner, "a", "a1", "Space", false);
    key(runner, "a", "a1", "ArrowDown", false);
    key(runner, "a", "a1", "Space", false);
    let old = ticket(runner);
    let view = node(runner, &grip("a", "a1"));
    runner.dispatch(view, Event::Blur).unwrap();
    land(runner, old, answer(false, 1.0, ""));
    runner.data().0.remove("command");
    key(runner, "a", "a2", "Space", false);
    key(runner, "a", "a2", "ArrowDown", false);
    key(runner, "a", "a2", "Space", false);
    let new = ticket(runner);
    assert_eq!(state(runner, 1), &Value::Number(3.0));
    let saving = state(runner, 8).clone();
    runner.take_commands();
    assert!(runner
        .fulfill(
            old,
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: vec![]
            })
        )
        .unwrap()
        .is_none());
    assert_eq!(state(runner, 8), &saving);
    land(runner, new, answer(false, 1.0, ""));
    assert_eq!(state(runner, 0), &Value::str("saving"));
    assert_eq!(
        state(runner, 8),
        &saving,
        "an older receipt cannot announce a new drop"
    );
    assert!(!runner
        .take_commands()
        .iter()
        .any(|command| command.name == "focus"));
    runner.advance(runner.now_ms() + 2.0).unwrap();
    assert_eq!(state(runner, 0), &Value::str(""));
    assert_eq!(state(runner, 8), &Value::str(""));
}

#[test]
fn settings_reply_emits_public_relaunch_only_after_confirmed_persistence() {
    for (relaunch, message) in [
        (true, ""),
        (true, "A later preference save failed."),
        (false, ""),
        (false, "Desktop settings write failed."),
    ] {
        let mut host = host();
        let runner = host.runner_mut();
        runner
            .act(
                "connectionOp",
                vec![
                    Value::str("local-environment"),
                    Value::str(""),
                    Value::str("off"),
                ],
            )
            .unwrap();
        let request = ticket(runner);
        assert!(!runner
            .take_commands()
            .iter()
            .any(|command| command.name == "relaunch"));
        land(runner, request, answer(relaunch, 0.0, message));
        assert_eq!(
            runner
                .take_commands()
                .iter()
                .filter(|command| command.name == "relaunch")
                .count(),
            usize::from(relaunch)
        );
        assert!(runner.land_then().error.is_none());
        assert!(
            !runner
                .take_commands()
                .iter()
                .any(|command| command.name == "relaunch"),
            "a handled reply cannot relaunch twice"
        );
    }
}
