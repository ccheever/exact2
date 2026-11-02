use super::*;
use exact_runner::runner::ActionBindingRefusal;

#[derive(Clone, Copy)]
enum ActionContext {
    Parameter,
    ItemField,
    RowSlot,
    RootSlot,
}

fn rows(target: &str, body: &str) -> Value {
    Value::list(vec![Value::record(vec![
        Value::str("stable-id"),
        Value::str(body),
        Value::str(target),
    ])])
}

fn fixture(field: u16, context: ActionContext) -> Runner<Schedule> {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = b.primitive(TypeKind::String);
    let record = b.record(
        "BindingRow",
        &[("id", string), ("body", string), ("target", string)],
    );
    let list = b.list(record);
    let id = b.str("stable-id");
    let body = b.str("old body");
    let target = b.str("old-target");
    let mut initial = Asm::new();
    initial.str(id).str(body).str(target).record(record).list(1);
    let initial = b.code(initial);
    let items = b.slot("bindingRows", list, initial);
    let empty = b.constant(&Value::str(""));
    let selected = b.slot("selectedBinding", string, empty);
    let local_initial = b.constant(&Value::str("old-local"));
    let local = b.slot("bindingLocal", string, local_initial);
    let global = b.slot("bindingGlobal", string, local_initial);
    let mut set_global = Asm::new();
    set_global.load_param(0).store_slot(global);
    let set_global = b.code(set_global);
    b.action(
        "setBindingGlobal",
        &[("value", string)],
        &[global],
        set_global,
    );
    let mut replace = Asm::new();
    replace.load_param(0).store_slot(items);
    let replace = b.code(replace);
    b.action("replaceBindingRows", &[("rows", list)], &[items], replace);
    let mut body = Asm::new();
    match context {
        ActionContext::Parameter => {
            body.load_param(0);
        }
        ActionContext::ItemField => {
            body.load_item(0).field(2);
        }
        ActionContext::RowSlot => {
            body.load_slot(local);
        }
        ActionContext::RootSlot => {
            body.load_slot(global);
        }
    }
    body.store_slot(selected);
    let body = b.code(body);
    let reply = b.action("bindingReply", &[("id", string)], &[selected], body);
    let mut edit = Asm::new();
    edit.load_param(0).store_slot(local);
    let edit = b.code(edit);
    let edit = b.action("editBindingLocal", &[("value", string)], &[local], edit);
    let root = b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let mut subject = Asm::new();
    subject.load_slot(items);
    let subject = b.code(subject);
    let mut key = Asm::new();
    key.load_item(0).field(0);
    let key = b.code(key);
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    b.set_slot_owner(local, arms[0]);
    let mut argument = Asm::new();
    argument.load_item(0).field(field);
    let argument = b.code(argument);
    let test_id = b.constant(&Value::str("binding-button"));
    let button = b.node(
        NodeType::Pressable as u8,
        None,
        Some(arms[0]),
        0,
        &[prop("testId", test_id)],
        &[
            (EventKind::Press, reply, &[argument]),
            (EventKind::Swiperight, reply, &[argument]),
            (EventKind::Change, edit, &[]),
        ],
        None,
    );
    let mut label = Asm::new();
    label.load_item(0).field(1);
    let label = b.code(label);
    b.node(
        NodeType::Text as u8,
        Some(button),
        None,
        0,
        &[prop("text", label)],
        &[],
        None,
    );
    Runner::boot(
        b.finish().unwrap(),
        Schedule::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn key(r: &Runner<Schedule>) -> exact_kernel::NodeKey {
    let found = r.kernel().find_by_test_id("binding-button");
    assert_eq!(found.len(), 1);
    found[0]
}

fn replace(r: &mut Runner<Schedule>, target: &str, body: &str) {
    r.act("replaceBindingRows", vec![rows(target, body)])
        .unwrap();
}

#[test]
fn current_dispatch_re_evaluates_changed_curried_argument_on_same_key() {
    let mut r = fixture(2, ActionContext::Parameter);
    let before = key(&r);
    replace(&mut r, "new-target", "new body");
    assert_eq!(key(&r), before);
    let view = r.kernel().node_by_key(before).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
    assert_eq!(r.slot("selectedBinding"), Some(&Value::str("new-target")));
}

#[test]
fn retained_consumer_must_not_deliver_old_target_to_rebound_live_action() {
    let mut r = fixture(2, ActionContext::Parameter);
    let painted_key = key(&r);
    let binding = r
        .capture_action_binding(painted_key, EventKind::Press)
        .unwrap();
    replace(&mut r, "new-target", "new body");
    assert_eq!(key(&r), painted_key);
    let before = r.slot("selectedBinding").unwrap().clone();
    let clock = r.now_ms();
    let journal: Vec<_> = r.journal().map(str::to_owned).collect();
    assert_eq!(
        r.validate_action_binding(&binding, EventKind::Press),
        Err(ActionBindingRefusal::Stale)
    );
    let result = r.dispatch_bound(&binding, Event::Press);
    assert!(
        result.is_err(),
        "unqualified old painted binding dispatched live changed args"
    );
    assert_eq!(r.slot("selectedBinding"), Some(&before));
    assert_eq!(r.now_ms(), clock);
    assert_eq!(r.journal().collect::<Vec<_>>(), journal);
    assert!(r.take_commands().is_empty());
    assert!(r.take_requests().is_empty());
}

#[test]
fn unchanged_bound_id_survives_body_only_change_for_both_event_kinds() {
    for event in [Event::Press, Event::Swiperight] {
        let mut r = fixture(0, ActionContext::Parameter);
        let before = key(&r);
        let kind = if event == Event::Press {
            EventKind::Press
        } else {
            EventKind::Swiperight
        };
        let binding = r.capture_action_binding(before, kind).unwrap();
        replace(&mut r, "old-target", "new body only");
        assert_eq!(key(&r), before);
        r.validate_action_binding(&binding, kind).unwrap();
        r.dispatch_bound(&binding, event).unwrap();
        assert_eq!(r.slot("selectedBinding"), Some(&Value::str("stable-id")));
    }
}

#[test]
fn identical_args_do_not_certify_action_item_context() {
    let mut r = fixture(0, ActionContext::ItemField);
    let before = key(&r);
    assert!(matches!(
        r.capture_action_binding(before, EventKind::Press),
        Err(ActionBindingRefusal::Unsupported)
    ));
    replace(&mut r, "new-target", "new body");
    assert_eq!(key(&r), before);
    let view = r.kernel().node_by_key(before).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
    // Curry is still stable-id, but action code itself reads item.target.
    assert_eq!(r.slot("selectedBinding"), Some(&Value::str("new-target")));
}

#[test]
fn identical_args_do_not_certify_row_local_state_and_payload_is_separate() {
    let mut r = fixture(0, ActionContext::RowSlot);
    let before = key(&r);
    assert!(matches!(
        r.capture_action_binding(before, EventKind::Press),
        Err(ActionBindingRefusal::Unsupported)
    ));
    let view = r.kernel().node_by_key(before).unwrap().id;
    // Change has zero curried args; its payload is supplied by the event.
    r.dispatch(view, Event::Change("new-local".into())).unwrap();
    assert_eq!(key(&r), before);
    r.dispatch(view, Event::Swiperight).unwrap();
    assert_eq!(r.slot("selectedBinding"), Some(&Value::str("new-local")));
}

#[test]
fn identical_args_do_not_certify_root_state_read_by_action() {
    let mut r = fixture(0, ActionContext::RootSlot);
    let before = key(&r);
    assert!(matches!(
        r.capture_action_binding(before, EventKind::Press),
        Err(ActionBindingRefusal::Unsupported)
    ));
    r.act("setBindingGlobal", vec![Value::str("new-global")])
        .unwrap();
    assert_eq!(key(&r), before);
    let view = r.kernel().node_by_key(before).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
    assert_eq!(r.slot("selectedBinding"), Some(&Value::str("new-global")));
}

#[test]
fn foreign_runner_and_carrying_reload_refuse_identical_keys_and_plan() {
    let r = fixture(0, ActionContext::Parameter);
    let binding = r.capture_action_binding(key(&r), EventKind::Press).unwrap();
    let mut foreign = fixture(0, ActionContext::Parameter);
    assert_eq!(key(&r), key(&foreign));
    assert_eq!(r.plan().encode(), foreign.plan().encode());
    assert!(foreign.dispatch_bound(&binding, Event::Press).is_err());
    let mut reload = Runner::boot_carrying(
        r.plan().clone(),
        Schedule::default(),
        Kernel::with_monospace(),
        &r.carry(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(reload.dispatch_bound(&binding, Event::Press).is_err());
    assert_eq!(reload.slot("selectedBinding"), Some(&Value::str("")));
}

#[test]
fn deleted_remounted_and_wrong_event_bindings_cannot_deliver() {
    let mut r = fixture(0, ActionContext::Parameter);
    let old = key(&r);
    let binding = r.capture_action_binding(old, EventKind::Press).unwrap();
    assert!(r.dispatch_bound(&binding, Event::Swiperight).is_err());
    assert!(r
        .dispatch_bound(&binding, Event::Change("payload".into()))
        .is_err());
    r.act("replaceBindingRows", vec![Value::list(vec![])])
        .unwrap();
    assert!(r.dispatch_bound(&binding, Event::Press).is_err());
    replace(&mut r, "old-target", "old body");
    assert_ne!(key(&r), old);
    assert!(r.dispatch_bound(&binding, Event::Press).is_err());
    assert_eq!(r.slot("selectedBinding"), Some(&Value::str("")));
    let fresh = r.capture_action_binding(key(&r), EventKind::Press).unwrap();
    r.dispatch_bound(&fresh, Event::Press).unwrap();
    assert_eq!(r.slot("selectedBinding"), Some(&Value::str("stable-id")));
}

#[test]
fn long_curried_string_refuses_before_copy_but_body_is_not_retained() {
    let mut r = fixture(2, ActionContext::Parameter);
    replace(&mut r, &"x".repeat(1024), "body");
    let binding = r.capture_action_binding(key(&r), EventKind::Press).unwrap();
    replace(&mut r, &"x".repeat(1024), &"body".repeat(8192));
    r.dispatch_bound(&binding, Event::Press).unwrap();
    replace(&mut r, &"x".repeat(1025), "body");
    assert!(matches!(
        r.capture_action_binding(key(&r), EventKind::Press),
        Err(ActionBindingRefusal::Limit)
    ));
    assert!(r.dispatch_bound(&binding, Event::Press).is_err());
}
