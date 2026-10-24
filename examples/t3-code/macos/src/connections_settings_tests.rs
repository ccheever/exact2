//! The actual app plan, through its rendered route handlers and command replies.
//! Store writes are covered in connections-routes.test.ts; native focus, spoken
//! announcements and process replacement have separate attended qualifications.

use super::composer_accessibility_tests::{first_frame, measure, set, Replies};
use super::contract_keys_tests::{grip, slot_field};
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
        assert_eq!(
            slot_field(runner, "settingsPageState", "open"),
            Some(&Value::Bool(true))
        );
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
    assert_eq!(
        slot_field(runner, "settingsPageState", "open"),
        Some(&Value::Bool(true))
    );
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

mod extraction_settings_jump {
    use super::super::composer_accessibility_tests::set;
    use exact_kernel::{Kernel, PropId, ViewId};
    use exact_plan::{Plan, TypeKind, TypesId};
    use exact_runner::geometry::{GeometryAnswer, GeometryLinks, Scrolled};
    use exact_runner::{
        Answer, DataError, DataSource, Delivery, DeviceLinks, Event, Outcome, Request, Response,
        Runner, RunnerLinks, Store, Value, Viewport,
    };
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    const TARGET: &str = "setting-fixture-target";
    const CORE: &str = "settingsCore";

    // Construct replies from this run's plan, without baked-plan fixture helpers.
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

    fn field_type(plan: &Plan, mut ty: TypesId, path: &[&str]) -> TypesId {
        for name in path {
            let index = plan.field_index(ty, name).expect("fixture field");
            let field = plan.type_(ty).fields.iter().nth(index).unwrap();
            ty = plan.field(field).ty;
        }
        ty
    }

    fn resource_type(plan: &Plan, name: &str) -> TypesId {
        plan.resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .expect("fixture resource")
            .ty
    }

    fn resource_source(plan: &Plan, name: &str) -> String {
        let row = plan
            .resources
            .iter()
            .find(|row| plan.str(row.name) == name)
            .expect("fixture resource");
        plan.str(row.source).to_owned()
    }

    fn record_field(plan: &Plan, ty: TypesId, value: &Value, name: &str) -> Value {
        let index = plan.field_index(ty, name).expect("projected field");
        let Value::Record(fields) = value else {
            panic!("projected record")
        };
        fields[index].clone()
    }

    fn core_value(plan: &Plan, route: &str, ready: bool) -> Value {
        let ty = resource_type(plan, CORE);
        let mut core = empty(plan, ty);
        for (name, value) in [
            ("route", Value::str(route)),
            ("ready", Value::Bool(ready)),
            ("kind", Value::str("environment")),
        ] {
            set(plan, ty, &mut core, &[name], value);
        }
        if ready {
            let section_ty = plan
                .type_(field_type(plan, ty, &["sections"]))
                .elem
                .unwrap();
            let row_ty = plan
                .type_(field_type(plan, section_ty, &["rows"]))
                .elem
                .unwrap();
            let mut row = empty(plan, row_ty);
            for (name, value) in [
                ("id", Value::str("fixture-target")),
                ("title", Value::str("Fixture setting")),
                ("label", Value::str("Fixture action")),
                ("kind", Value::str("action")),
            ] {
                set(plan, row_ty, &mut row, &[name], value);
            }
            let mut section = empty(plan, section_ty);
            set(
                plan,
                section_ty,
                &mut section,
                &["id"],
                Value::str("fixture-section"),
            );
            set(
                plan,
                section_ty,
                &mut section,
                &["rows"],
                Value::list(vec![row]),
            );
            set(
                plan,
                ty,
                &mut core,
                &["sections"],
                Value::list(vec![section]),
            );
        }
        core
    }

    fn navigation(plan: &Plan) -> Value {
        let ty = resource_type(plan, "settingsNavigation");
        let item_ty = plan.type_(field_type(plan, ty, &["items"])).elem.unwrap();
        let mut items = Vec::new();
        for (id, route, target) in [
            ("appearance", "appearance", ""),
            ("fixture-jump", "general", TARGET),
            ("general", "general", ""),
        ] {
            let mut item = empty(plan, item_ty);
            for (name, value) in [
                ("id", Value::str(id)),
                ("route", Value::str(route)),
                ("target", Value::str(target)),
                ("title", Value::str(id)),
                ("section", Value::str("Fixture")),
            ] {
                set(plan, item_ty, &mut item, &[name], value);
            }
            items.push(item);
        }
        let mut nav = empty(plan, ty);
        set(plan, ty, &mut nav, &["items"], Value::list(items));
        set(plan, ty, &mut nav, &["searching"], Value::Bool(true));
        set(plan, ty, &mut nav, &["count"], Value::Number(3.0));
        nav
    }

    #[derive(Default)]
    struct SettingsData {
        ready: bool,
        general_ready: bool,
        values: BTreeMap<String, Value>,
        core_source: String,
        general_waiting: Value,
        general_landed: Value,
        appearance: Value,
        calls: Vec<(String, Vec<Value>)>,
        parses: Vec<(String, Vec<Value>)>,
    }

    impl SettingsData {
        fn new(plan: &Plan) -> Self {
            let mut values = BTreeMap::new();
            let mut types = BTreeMap::new();
            for row in &plan.resources {
                let source = plan.str(row.source);
                if exact_plan::runner_owned_source(source) {
                    continue;
                }
                let value = empty(plan, row.ty);
                if let Some(prior) = values.get(source) {
                    assert_eq!(types.get(source), Some(&row.ty), "source type: {source}");
                    assert_eq!(prior, &value, "source default: {source}");
                } else {
                    types.insert(source.to_owned(), row.ty);
                    values.insert(source.to_owned(), value);
                }
            }
            for (name, fields) in [
                (
                    "data",
                    vec![
                        (vec!["revision"], Value::Number(1.0)),
                        (vec!["connected"], Value::Bool(true)),
                        (vec!["sidebarOpen"], Value::Bool(true)),
                        (vec!["look", "mode"], Value::str("light")),
                    ],
                ),
                ("welcome", vec![(vec!["show"], Value::Bool(false))]),
                (
                    "settings",
                    vec![
                        (vec!["environmentId"], Value::str("fixture-environment")),
                        (vec!["available"], Value::Bool(true)),
                    ],
                ),
            ] {
                let ty = resource_type(plan, name);
                let source = resource_source(plan, name);
                let value = values.get_mut(&source).expect("seeded source");
                for (path, next) in fields {
                    set(plan, ty, value, &path, next);
                }
            }
            values.insert(
                resource_source(plan, "settingsNavigation"),
                navigation(plan),
            );
            Self {
                core_source: resource_source(plan, CORE),
                general_waiting: core_value(plan, "general", false),
                general_landed: core_value(plan, "general", true),
                appearance: core_value(plan, "appearance", false),
                values,
                ..Default::default()
            }
        }
    }

    impl DataSource for SettingsData {
        fn ready(&self) -> bool {
            self.ready
        }

        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            if source == self.core_source {
                assert_eq!(args.len(), 14, "complete SettingsCore source ABI");
                return Ok(match args[4].as_str().expect("route") {
                    "general" if self.general_ready => self.general_landed.clone(),
                    "general" => self.general_waiting.clone(),
                    "appearance" => self.appearance.clone(),
                    route => panic!("unexpected Settings route: {route}"),
                });
            }
            self.values
                .get(source)
                .cloned()
                .ok_or_else(|| DataError::UnknownSource(source.to_owned()))
        }

        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            self.calls.push((source.to_owned(), args.to_vec()));
            if source == self.core_source
                && !self.general_ready
                && args.get(4).and_then(Value::as_str) == Some("general")
                && args.get(5).and_then(Value::as_str) == Some(TARGET)
            {
                assert_eq!(args.get(6), Some(&Value::Bool(true)));
                return Ok(Answer::Later(Request::get(
                    "https://fixture.invalid/settings",
                )));
            }
            self.query(source, args).map(Answer::Now)
        }

        fn parse(
            &mut self,
            _: &mut Store,
            source: &str,
            args: &[Value],
            outcome: Outcome,
        ) -> Result<Answer, DataError> {
            assert_eq!(source, self.core_source);
            assert_eq!(args.get(5).and_then(Value::as_str), Some(TARGET));
            let Outcome::Response(response) = outcome else {
                panic!("fixture response")
            };
            assert_eq!(response.status, 200);
            assert_eq!(response.body, b"ready");
            self.parses.push((source.to_owned(), args.to_vec()));
            self.general_ready = true;
            self.query(source, args).map(Answer::Now)
        }
    }

    #[derive(Default)]
    struct Boxes {
        strict: bool,
        reads: Vec<String>,
    }

    thread_local! {
        static BOXES: RefCell<Boxes> = RefCell::new(Boxes::default());
    }

    fn frame(kernel: &Kernel, _: &Scrolled, view: ViewId) -> GeometryAnswer {
        let arena = kernel.arena();
        let id = arena
            .key_of(view)
            .and_then(|key| arena.props(key.index).str(PropId::Id))
            .expect("mounted named frame");
        BOXES.with(|cell| {
            let mut boxes = cell.borrow_mut();
            boxes.reads.push(id.to_owned());
            match id {
                TARGET => GeometryAnswer {
                    x: 0.0,
                    y: 900.0,
                    width: 600.0,
                    height: 40.0,
                    ..Default::default()
                },
                "settings-content-column" => GeometryAnswer {
                    x: 0.0,
                    y: 100.0,
                    width: 600.0,
                    height: 2000.0,
                    ..Default::default()
                },
                "settings-content-scroll" => GeometryAnswer {
                    x: 0.0,
                    y: 60.0,
                    width: 600.0,
                    height: 600.0,
                    ..Default::default()
                },
                _ if !boxes.strict => GeometryAnswer::UNAVAILABLE,
                _ => panic!("unexpected frame in measured jump: {id}"),
            }
        })
    }

    fn other_read(kind: &str) {
        BOXES.with(|cell| {
            let mut boxes = cell.borrow_mut();
            boxes.reads.push(kind.to_owned());
            assert!(!boxes.strict, "unexpected geometry operation: {kind}");
        });
    }

    fn measure(_: &mut Kernel, _: &Scrolled, _: ViewId) -> GeometryAnswer {
        other_read("measure");
        GeometryAnswer::UNAVAILABLE
    }

    fn point(_: &Kernel, _: &Scrolled, _: f64, _: f64) -> Option<ViewId> {
        other_read("point");
        None
    }

    fn points(_: &Kernel, _: &Scrolled, _: f64, _: f64) -> Vec<ViewId> {
        other_read("points");
        Vec::new()
    }

    fn shown(_: &Kernel, _: &Scrolled, _: ViewId, _: bool) -> bool {
        other_read("shown");
        false
    }

    static GEOMETRY: GeometryLinks = GeometryLinks::new(frame, measure, point, points, shown);

    fn unique(runner: &Runner<SettingsData>, id: &str, test_id: bool) -> ViewId {
        let nodes = if test_id {
            runner.kernel().find_by_test_id(id)
        } else {
            runner.kernel().find_by_id(id)
        };
        assert_eq!(nodes.len(), 1, "one mounted {id}");
        runner.kernel().node_by_key(nodes[0]).unwrap().id
    }

    fn press(runner: &mut Runner<SettingsData>, test_id: &str) {
        let view = unique(runner, test_id, true);
        runner.dispatch(view, Event::Press).unwrap();
    }

    fn commands(runner: &mut Runner<SettingsData>) -> Vec<(String, Vec<Value>)> {
        runner
            .take_commands()
            .into_iter()
            .map(|command| (command.name, command.args))
            .collect()
    }

    fn visible_top(runner: &Runner<SettingsData>) -> f64 {
        let view = unique(runner, "settings-content-scroll", false);
        let node = runner.kernel().node(view).unwrap();
        let value = &node
            .props
            .iter()
            .find(|(prop, _)| *prop == PropId::ScrollTop)
            .expect("authored scrollTop binding")
            .1;
        value
            .as_float()
            .map(f64::from)
            .or_else(|| value.as_int().map(|n| n as f64))
            .expect("numeric scrollTop")
    }

    fn resource_field(runner: &Runner<SettingsData>, name: &str, field: &str) -> Value {
        record_field(
            runner.plan(),
            resource_type(runner.plan(), name),
            runner.resource(name).expect("activated resource"),
            field,
        )
    }

    #[derive(Debug, PartialEq)]
    struct Logical {
        open: bool,
        route: String,
        target: String,
        owner: String,
        raw_top: f64,
    }

    fn logical(runner: &Runner<SettingsData>) -> Logical {
        let plan = runner.plan();
        let read = |member: &str, old: &str| {
            if let Some(record) = runner.slot("settingsPageState") {
                let slot = plan
                    .slots
                    .iter()
                    .find(|row| plan.str(row.name) == "settingsPageState")
                    .unwrap();
                record_field(plan, slot.ty, record, member)
            } else {
                runner.slot(old).expect("original flat slot").clone()
            }
        };
        Logical {
            open: read("open", "settingsOpen").as_bool().unwrap(),
            route: read("route", "settingsRoute").as_str().unwrap().to_owned(),
            target: read("focusTarget", "settingsFocusTarget")
                .as_str()
                .unwrap()
                .to_owned(),
            owner: read("scrollPage", "settingsScrollPage")
                .as_str()
                .unwrap()
                .to_owned(),
            raw_top: read("scrollTop", "settingsScrollTop").as_number().unwrap(),
        }
    }

    fn expected_core(route: &str, target: &str, open: bool) -> Vec<Value> {
        vec![
            Value::str(""),
            Value::str(""),
            Value::str(""),
            Value::str(""),
            Value::str(route),
            Value::str(target),
            Value::Bool(open),
            Value::Number(0.0),
            Value::Number(1.0),
            Value::str(""),
            Value::str(""),
            Value::str("embedded"),
            Value::Bool(false),
            Value::str("light"),
        ]
    }

    fn advance(runner: &mut Runner<SettingsData>, time: f64) {
        let result = runner.advance_timed(time);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.now_ms, time);
    }

    #[derive(Debug, PartialEq)]
    struct Observed {
        setup_calls: Vec<(String, Vec<Value>)>,
        setup_commands: Vec<(String, Vec<Value>)>,
        setup_geometry: Vec<String>,
        before: Logical,
        landed: Logical,
        focused: Logical,
        same_page: Logical,
        other_page: Logical,
        forgotten: Logical,
        closed: Logical,
        reads: Vec<String>,
        due_times: Vec<f64>,
        calls: Vec<(String, Vec<Value>)>,
        parsed: Vec<(String, Vec<Value>)>,
        route_calls: Vec<(String, Vec<Value>)>,
        close_calls: Vec<(String, Vec<Value>)>,
    }

    fn run(bytes: &[u8]) -> Observed {
        BOXES.with(|cell| *cell.borrow_mut() = Boxes::default());
        let plan = Plan::decode(bytes).expect("complete compiled app plan");
        let data = SettingsData::new(&plan);
        let links = RunnerLinks {
            geometry: Some(&GEOMETRY),
            ..RunnerLinks::ALL
        };
        let mut runner = Runner::boot_with_delivery_linked(
            links,
            plan,
            data,
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
        runner.set_device_links(DeviceLinks::ALL);
        runner.set_live_clock(None);
        runner.data().ready = true;
        runner
            .data_ready()
            .unwrap()
            .expect("activated fixture readers");
        assert_eq!(
            resource_field(&runner, "data", "revision"),
            Value::Number(1.0)
        );
        assert_eq!(
            resource_field(&runner, "data", "connected"),
            Value::Bool(true)
        );
        assert_eq!(
            resource_field(&runner, "data", "sidebarOpen"),
            Value::Bool(true)
        );
        assert_eq!(
            resource_field(&runner, "welcome", "show"),
            Value::Bool(false)
        );
        assert_eq!(
            resource_field(&runner, "settings", "environmentId"),
            Value::str("fixture-environment"),
        );
        assert_eq!(
            resource_field(&runner, "settings", "available"),
            Value::Bool(true)
        );
        assert_eq!(
            runner.resource(CORE).unwrap(),
            &runner.data_ref().general_waiting
        );
        assert_eq!(
            runner.resource("settingsNavigation").unwrap(),
            &navigation(runner.plan())
        );
        assert!(runner.take_requests().is_empty());
        advance(&mut runner, 20.0);
        press(&mut runner, "connection-settings");
        assert!(logical(&runner).open);
        assert_eq!(logical(&runner).route, "general");
        assert_eq!(
            runner.slot("settingsEnvironmentId"),
            Some(&Value::str("fixture-environment")),
        );
        unique(&runner, "settings-content-scroll", false);
        press(&mut runner, "settings-appearance");
        advance(&mut runner, 22.0);
        assert_eq!(logical(&runner).route, "appearance");
        assert_eq!(visible_top(&runner), 0.0);
        assert_eq!(
            runner.resource("settingsNavigation").unwrap(),
            &navigation(runner.plan())
        );
        assert_eq!(
            runner.resource(CORE).unwrap(),
            &runner.data_ref().appearance
        );
        assert!(runner.take_requests().is_empty());

        press(&mut runner, "settings-fixture-jump");
        let pending = runner.take_requests();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].target, CORE);
        assert!(!pending[0].forced);
        let ticket = pending[0].ticket;
        assert_eq!(logical(&runner).route, "general");
        assert_eq!(logical(&runner).target, TARGET);
        assert_eq!(
            runner.derive("settingsJumpReady"),
            Some(&Value::Bool(false))
        );
        advance(&mut runner, 24.0);
        assert_eq!(visible_top(&runner), 0.0);
        assert_eq!(logical(&runner).owner, "");
        assert_eq!(logical(&runner).target, TARGET);
        let before = logical(&runner);
        let core_source = runner.data_ref().core_source.clone();
        assert_eq!(
            runner
                .data_ref()
                .calls
                .iter()
                .filter(|(source, _)| source == &core_source)
                .last()
                .unwrap()
                .1,
            expected_core("general", TARGET, true),
        );
        let setup_calls = std::mem::take(&mut runner.data().calls);
        let setup_commands = commands(&mut runner);
        let setup_geometry = BOXES.with(|cell| std::mem::take(&mut cell.borrow_mut().reads));

        runner
            .fulfill(
                ticket,
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: b"ready".to_vec(),
                }),
            )
            .unwrap();
        let armed_at = runner.now_ms();
        assert_eq!(armed_at, 24.0);
        assert_eq!(runner.derive("settingsJumpReady"), Some(&Value::Bool(true)));
        assert_eq!(
            runner.resource(CORE).unwrap(),
            &runner.data_ref().general_landed
        );
        for id in [TARGET, "settings-content-column", "settings-content-scroll"] {
            unique(&runner, id, false);
        }
        assert_eq!(visible_top(&runner), 0.0);
        let landed = logical(&runner);
        assert_eq!(landed.target, TARGET);
        assert_eq!(landed.raw_top, 0.0);
        assert_eq!(commands(&mut runner), vec![]);
        assert!(runner.take_requests().is_empty());
        assert!(runner.data_ref().calls.is_empty());
        BOXES.with(|cell| {
            let mut boxes = cell.borrow_mut();
            assert!(boxes.reads.is_empty());
            boxes.strict = true;
        });

        let early = runner.advance_timed(armed_at + 0.999);
        assert!(early.error.is_none(), "{:?}", early.error);
        assert!(early.receipts.is_empty());
        assert_eq!(early.now_ms, armed_at + 0.999);
        assert_eq!(commands(&mut runner), vec![]);
        assert!(runner.take_requests().is_empty());
        BOXES.with(|cell| assert!(cell.borrow().reads.is_empty()));

        let due = runner.advance_timed(armed_at + 1.0);
        assert!(due.error.is_none(), "{:?}", due.error);
        assert_eq!(due.now_ms, armed_at + 1.0);
        let due_times = due
            .receipts
            .iter()
            .map(|receipt| receipt.at_ms)
            .collect::<Vec<_>>();
        assert_eq!(due_times, vec![armed_at + 1.0]);
        let reads = BOXES.with(|cell| cell.borrow().reads.clone());
        assert_eq!(
            reads,
            vec![
                TARGET,
                "settings-content-column",
                TARGET,
                "settings-content-scroll",
            ]
        );
        assert_eq!(
            commands(&mut runner),
            vec![("focus".to_owned(), vec![Value::str(TARGET)]),]
        );
        assert!(runner.take_requests().is_empty());
        assert_eq!(
            runner.data_ref().calls,
            vec![(core_source.clone(), expected_core("general", "", true)),]
        );
        assert_eq!(
            runner.data_ref().parses,
            vec![(core_source.clone(), expected_core("general", TARGET, true)),]
        );
        let calls = std::mem::take(&mut runner.data().calls);
        let parsed = std::mem::take(&mut runner.data().parses);
        let focused = logical(&runner);
        assert_eq!(focused.target, "");
        assert_eq!(focused.owner, "general||||0");
        assert_eq!(focused.raw_top, 520.25);
        assert_eq!(visible_top(&runner), 520.25);
        assert_eq!(
            runner.derive("settingsJumpReady"),
            Some(&Value::Bool(false))
        );

        BOXES.with(|cell| {
            let mut boxes = cell.borrow_mut();
            boxes.reads.clear();
        });
        press(&mut runner, "settings-general");
        // This authored action checks the same-page page-left invariant;
        // focusSettingsTarget is never called manually in this test.
        runner.act("settingsPageForget", vec![]).unwrap();
        let same_page = logical(&runner);
        assert_eq!(same_page, focused);
        assert_eq!(visible_top(&runner), 520.25);
        assert_eq!(commands(&mut runner), vec![]);
        assert!(runner.take_requests().is_empty());
        assert!(runner.data_ref().calls.is_empty());

        press(&mut runner, "settings-appearance");
        let other_page = logical(&runner);
        assert_eq!(other_page.raw_top, 520.25);
        assert_eq!(other_page.owner, "general||||0");
        assert_eq!(visible_top(&runner), 0.0);
        let route_calls = std::mem::take(&mut runner.data().calls);
        assert_eq!(
            route_calls
                .iter()
                .filter(|(source, _)| source == &core_source)
                .cloned()
                .collect::<Vec<_>>(),
            vec![(core_source.clone(), expected_core("appearance", "", true))],
        );
        advance(&mut runner, 26.0);
        assert!(runner.data_ref().calls.is_empty());
        let forgotten = logical(&runner);
        assert_eq!(forgotten.owner, "");
        assert_eq!(forgotten.raw_top, 520.25);
        assert_eq!(visible_top(&runner), 0.0);
        press(&mut runner, "settings-go-to-threads");
        let close_calls = std::mem::take(&mut runner.data().calls);
        assert_eq!(
            close_calls
                .iter()
                .filter(|(source, _)| source == &core_source)
                .cloned()
                .collect::<Vec<_>>(),
            vec![(core_source.clone(), expected_core("appearance", "", false))],
        );
        advance(&mut runner, 27.0);
        assert!(runner.data_ref().calls.is_empty());
        let closed = logical(&runner);
        assert!(!closed.open);
        assert_eq!(closed.owner, "");
        assert!(runner
            .kernel()
            .find_by_id("settings-content-scroll")
            .is_empty());
        assert_eq!(commands(&mut runner), vec![]);
        assert!(runner.take_requests().is_empty());
        assert!(runner.data_ref().parses.is_empty());
        BOXES.with(|cell| assert!(cell.borrow().reads.is_empty()));
        // All route/close producer calls are compared, including sources
        // other than SettingsCore; no background-call exception is applied.
        Observed {
            setup_calls,
            setup_commands,
            setup_geometry,
            before,
            landed,
            focused,
            same_page,
            other_page,
            forgotten,
            closed,
            reads,
            due_times,
            calls,
            parsed,
            route_calls,
            close_calls,
        }
    }

    #[test]
    fn extraction_settings_jump_waits_for_its_page_and_runs_at_one_millisecond() {
        let mut prior = None;
        for (bytes, _) in crate::contract_keys_tests::extraction_plans() {
            let observed = run(bytes);
            if let Some(original) = prior {
                assert_eq!(original, observed);
            }
            prior = Some(observed);
        }
    }
}
