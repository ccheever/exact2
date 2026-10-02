//! LLP 1017 P4c: per-instance state, proven on the runner — a row's state
//! follows its key, a use's own state is its own, and neither leaks.

use exact_kernel::{Kernel, PropValue};
use exact_plan::builder::PlanBuilder;
use exact_plan::{SlotsId, TypeKind, Value};
use exact_runner::{DataError, DataSource, Event, Runner, RunnerError};
use std::path::Path;

#[derive(Default)]
struct Stations;

impl DataSource for Stations {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let station = |id: &str, name: &str| Value::record(vec![Value::str(id), Value::str(name)]);
        match source {
            "stations" => {
                let asc = args.first().and_then(Value::as_str) == Some("asc");
                let mut rows = vec![station("mv", "Mountain View"), station("pa", "Palo Alto")];
                if !asc {
                    rows.reverse();
                }
                Ok(Value::list(rows))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn derived_row_state_in_action_props_uses_resolved_types_and_child_spans() {
    #[derive(Default)]
    struct Profile {
        saved: Vec<Value>,
    }
    impl DataSource for Profile {
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            Ok(match source {
                "app" => Value::record(vec![
                    Value::Bool(false),
                    Value::record(vec![Value::str("1"), Value::str("Alice")]),
                ]),
                "command" => {
                    self.saved.push(args[1].clone());
                    Value::record(vec![Value::Bool(true)])
                }
                _ => return Err(DataError::UnknownSource(source.into())),
            })
        }
    }
    let source = corpus("instance-args.contract");
    for source in [
        source.clone(),
        source
            .replace("      each e in stack(nav) key=e.id\n        column navigationKey=`${e.id}`\n          Screen(entry=e, data=data, save=save)", "      provide data = data\n        provide save = save\n          each e in stack(nav) key=e.id\n            column navigationKey=`${e.id}`\n              Screen(entry=e)")
            .replace("    data: App", "  inject\n    data: App"),
    ] {
        let plan = contract::compile(&source).unwrap();
        let mut r = Runner::boot(
            exact_plan::Plan::decode(&plan.encode()).unwrap(),
            Profile::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let view = |r: &Runner<Profile>, id: &str| {
            r.kernel()
                .node_by_key(r.kernel().find_by_test_id(id)[0])
                .unwrap()
                .id
        };
        r.dispatch(view(&r, "save-profile"), Event::Press).unwrap();
        r.dispatch(
            view(&r, "profile-name"),
            Event::Input("Edited Alice".into()),
        )
        .unwrap();
        r.dispatch(view(&r, "save-profile"), Event::Press).unwrap();
        assert_eq!(r.data_ref().saved, [Value::str("Alice"), Value::str("Edited Alice")]);
    }
    let invalid = source.replace("editingProfile = false", "editingProfile = 0");
    let error = contract::compile(&invalid).unwrap_err();
    let line = invalid
        .lines()
        .position(|line| line.contains("derive profileName"))
        .unwrap()
        + 1;
    assert_eq!(error.id, "type-condition");
    assert_eq!(error.span.line as usize, line);
    // A derive starts the type fixpoint as `?`, so a ternary over one used to be
    // refused outright while the same expression written inline compiled. Naming the
    // type in the message defers the round, the way a binary operand already did.
    let ternary_over_derive = r#"component T
  state count = 0
  derive ready = count > 0
  derive shown = ready ? "open" : "closed"
  view
    main
      text shown testId="shown"
"#;
    contract::compile(ternary_over_derive).unwrap();
    let not_a_bool = ternary_over_derive.replace("ready = count > 0", "ready = count");
    let error = contract::compile(&not_a_bool).unwrap_err();
    assert_eq!(error.id, "type-condition");
    assert!(
        error.message.contains("given `number`"),
        "{}",
        error.message
    );
}

fn text_of(r: &Runner<Stations>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .iter()
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
            _ => None,
        })
        .unwrap()
}

fn view_of(r: &Runner<Stations>, id: &str) -> u32 {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn a_use_owns_its_state_and_a_row_owns_its_own_which_follows_its_key() {
    let plan = contract::compile(&corpus("instance.contract")).unwrap();
    let plan = contract::bake(plan, Stations).unwrap();
    let mut r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // Two uses of Counter: two states, two derives, two actions.
    assert_eq!(text_of(&r, "count-text-a"), "a 0 0");
    let a = view_of(&r, "count-a");
    r.dispatch(a, Event::Press).unwrap();
    r.dispatch(a, Event::Press).unwrap();
    assert_eq!(text_of(&r, "count-text-a"), "a 2 4");
    assert_eq!(text_of(&r, "count-text-b"), "b 0 0");
    // A singleton's state is a root slot by its lifted name; carried by name.
    assert_eq!(r.slot("n#1"), Some(&Value::Number(2.0)));
    assert!(r.carry().slots.iter().any(|(n, _)| n == "n#1"));
    // Rows: `label` initialized from the row item, then hover one while the
    // other is untouched.
    let mv = view_of(&r, "station-mv");
    r.dispatch(mv, Event::Hover(true)).unwrap();
    assert_eq!(text_of(&r, "name-mv"), "Mountain View !");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto");
    // A row slot is the row's: not a root slot, not carried.
    assert_eq!(r.slot("hot#3"), None);
    assert!(!r.carry().slots.iter().any(|(n, _)| n == "hot#3"));
    // Reorder: the state followed the key.
    r.act("flip", vec![]).unwrap();
    assert_eq!(r.slot("order"), Some(&Value::str("desc")));
    assert_eq!(text_of(&r, "name-mv"), "Mountain View !");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto");
    let pa = view_of(&r, "station-pa");
    r.dispatch(pa, Event::Hover(true)).unwrap();
    r.dispatch(mv, Event::Hover(false)).unwrap();
    assert_eq!(text_of(&r, "name-mv"), "Mountain View");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto !");
    // A row action run with no row has nothing to write: a typed refusal,
    // and the kernel untouched.
    assert!(r.act("setHot#3", vec![Value::Bool(true)]).is_err());
    assert_eq!(text_of(&r, "name-mv"), "Mountain View");
}

#[test]
fn a_child_may_not_own_a_resource() {
    let src = "shape S\n  id: string\ncomponent A\n  view\n    Row()\ncomponent Row\n  resource s = s() as shape S\n  view\n    text s.id\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-child-resource");
}

#[test]
fn child_derives_resolve_in_either_order_without_capturing_the_parent() {
    let src = "component App\n  state a = 100\n  view\n    column\n      Forward()\n      Ordered()\ncomponent Forward\n  state n = 2\n  derive b = a * 2\n  derive a = n + 1\n  view\n    text `${a} ${b}` testId=\"forward\"\ncomponent Ordered\n  state n = 2\n  derive a = n + 1\n  derive b = a * 2\n  view\n    text `${a} ${b}` testId=\"ordered\"\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "forward"), "3 6");
    assert_eq!(text_of(&r, "ordered"), "3 6");

    let cycle = "component App\n  view\n    Cyclic()\ncomponent Cyclic\n  derive a = b\n  derive b = a\n  view\n    text a\n";
    assert_eq!(
        contract::compile(cycle).unwrap_err().id,
        "type-derive-cycle"
    );
}

#[test]
fn a_row_initializer_must_conform_before_the_row_is_published() {
    let plan = contract::compile(&corpus("instance.contract")).unwrap();
    let slot = plan
        .slots
        .iter()
        .position(|slot| {
            slot.owner.is_some() && plan.types[slot.ty.0 as usize].kind == TypeKind::Bool
        })
        .unwrap();
    let name = plan.str(plan.slots[slot].name).to_string();
    let mut b = PlanBuilder::from_plan(plan);
    let wrong = b.constant(&Value::str("not a bool"));
    b.set_slot_init(SlotsId(slot as u32), wrong);
    let malformed = b.finish().unwrap();

    let error = Runner::boot(
        malformed,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .err()
    .unwrap();
    assert!(matches!(error, RunnerError::SlotType { slot } if slot == name));
}

#[test]
fn an_action_parameter_shadows_a_same_named_child_prop() {
    let src = "component App\n  view\n    Capture(value=\"prop\")\ncomponent Capture\n  props\n    value: string\n  state seen = \"\"\n  action capture(value: string)\n    seen = value\n  view\n    column\n      input value=seen change=capture testId=\"capture-input\"\n      text seen testId=\"capture-result\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let input = view_of(&r, "capture-input");
    r.dispatch(input, Event::Change("payload".into())).unwrap();
    assert_eq!(text_of(&r, "capture-result"), "payload");
}

#[test]
fn nested_row_actions_use_lexical_items_even_when_a_root_name_collides() {
    let src = "shape Station\n  id: string\n  name: string\ncomponent App\n  state item = \"root collision\"\n  state visible = true\n  resource stations = stations(\"asc\") as shape list<Station>\n  view\n    column\n      each outer in stations key=outer.id\n        each item in stations key=item.id\n          when visible\n            ScopedRow(outer=outer, item=item)\ncomponent ScopedRow\n  props\n    outer: Station\n    item: Station\n  state selected = item.name\n  state result = \"\"\n  action choose\n    result = `${outer.name}/${item.name}`\n  view\n    column\n      text selected testId=`selected-${outer.id}-${item.id}`\n      button \"choose\" press=choose testId=`choose-${outer.id}-${item.id}`\n      text result testId=`result-${outer.id}-${item.id}`\n";
    let plan = contract::compile(src).unwrap();
    let plan = contract::bake(plan, Stations).unwrap();
    let mut r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "selected-mv-pa"), "Palo Alto");
    assert_listener_lookup(&r);
    let choose = view_of(&r, "choose-mv-pa");
    r.dispatch(choose, Event::Press).unwrap();
    assert_eq!(text_of(&r, "result-mv-pa"), "Mountain View/Palo Alto");
    assert_eq!(r.slot("item"), Some(&Value::str("root collision")));
}

fn assert_listener_lookup<D: DataSource>(runner: &Runner<D>) {
    let listeners = runner.handlers();
    let mut pending = runner.roots();
    while let Some(view) = pending.pop() {
        assert_eq!(
            runner.handlers_of(view),
            listeners.get(&view).cloned().unwrap_or_default()
        );
        pending.extend(runner.kernel().node(view).unwrap().children());
    }
    assert!(runner.handlers_of(u32::MAX).is_empty());
}

#[test]
fn numeric_keys_keep_identity_and_listener_catalog_follows_topology() {
    struct Keys;
    impl DataSource for Keys {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            let keys = if args == [Value::Bool(true)] {
                vec![2.0, -0.0, 1.0]
            } else {
                vec![0.0, 1.0, 2.0]
            };
            Ok(Value::list(keys.into_iter().map(Value::Number).collect()))
        }
    }
    let source = r#"component App
  state reverse = false
  state shown = true
  resource keys = keys(reverse) as shape list<number>
  action flip
    reverse = !reverse
  action toggle
    shown = !shown
  view
    column testId="root"
      when shown
        each item in keys key=item
          button press=flip testId=`key-${item}`
            text `${item}`
      else
        button "empty" press=toggle testId="empty"
"#;
    let plan = contract::compile(source).unwrap();
    let mut runner = Runner::boot(
        plan,
        Keys,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let before = runner.kernel().find_by_test_id("key-0")[0];
    let receipt = runner.act("flip", vec![]).unwrap();
    assert!(receipt.created.is_empty() && receipt.destroyed.is_empty());
    assert_eq!(runner.kernel().find_by_test_id("key-0")[0], before);
    let listeners = runner.handlers();
    assert_eq!(listeners.len(), 3);
    assert_listener_lookup(&runner);
    let root = runner.roots()[0];
    let old_order = runner.kernel().node(root).unwrap().children();
    let receipt = runner.act("toggle", vec![]).unwrap();
    assert_eq!(receipt.destroyed.len(), 6);
    assert_eq!(receipt.created.len(), 2);
    let new_listeners = runner.handlers();
    assert_eq!(new_listeners.len(), 1);
    assert_listener_lookup(&runner);
    assert!(listeners
        .keys()
        .all(|id| runner.handlers_of(*id).is_empty()));
    assert!(listeners.keys().all(|id| !new_listeners.contains_key(id)));
    assert!(old_order
        .iter()
        .all(|id| runner.kernel().node(*id).is_none()));
    assert_eq!(runner.kernel().node(root).unwrap().children().len(), 1);
    runner.act("toggle", vec![]).unwrap();
    assert_eq!(runner.handlers().len(), 3);
    assert_listener_lookup(&runner);
    assert_ne!(runner.kernel().find_by_test_id("key-0")[0], before);
}

#[test]
fn targeted_tree_is_the_same_live_subtree_and_keeps_first_preorder_matching() {
    let source = r#"component App
  state shown = true
  action toggle
    shown = !shown
  view
    column testId="root"
      when shown
        column testId="branch"
          button "first" press=toggle testId="repeated"
          column
            text "nested" testId="nested"
          button "second" press=toggle testId="repeated"
      else
        text "gone" testId="replacement"
      text "other sibling" testId="other"
"#;
    let mut runner = Runner::boot(
        contract::compile(source).unwrap(),
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let ask = |runner: &Runner<Stations>, target: serde_json::Value| -> serde_json::Value {
        serde_json::from_str(&exact_runner::agent::handle(
            runner,
            &serde_json::json!({"op":"tree", "target":target}).to_string(),
        ))
        .unwrap()
    };
    let full: serde_json::Value =
        serde_json::from_str(&exact_runner::agent::tree(&runner)).unwrap();
    let nodes = full["nodes"].as_array().unwrap();
    for (start, node) in nodes.iter().enumerate() {
        let depth = node["depth"].as_u64().unwrap();
        let end = start
            + 1
            + nodes[start + 1..]
                .iter()
                .take_while(|child| child["depth"].as_u64().unwrap() > depth)
                .count();
        let scoped = ask(&runner, node["id"].clone());
        assert_eq!(scoped["nodes"].as_array().unwrap(), &nodes[start..end]);
        assert_eq!(scoped["roots"], serde_json::json!([node["id"]]));
        for tag in ["epoch", "incarnation", "clock"] {
            assert_eq!(scoped[tag], full[tag]);
        }
        for shallow in [false, true] {
            let request = serde_json::json!({"op":"tree", "target":node["id"], "shallow":shallow});
            let reply: serde_json::Value =
                serde_json::from_str(&exact_runner::agent::handle(&runner, &request.to_string()))
                    .unwrap();
            let mut expected = scoped.clone();
            if shallow {
                expected["nodes"] = serde_json::json!([node]);
            }
            assert_eq!(reply, expected);
        }
    }
    let first = nodes
        .iter()
        .find(|n| n["props"]["testId"] == "repeated")
        .unwrap();
    assert_eq!(
        ask(&runner, "repeated".into())["nodes"],
        ask(&runner, first["id"].clone())["nodes"]
    );
    let shallow: serde_json::Value = serde_json::from_str(&exact_runner::agent::handle(
        &runner,
        r#"{"op":"tree","target":"repeated","shallow":true}"#,
    ))
    .unwrap();
    assert_eq!(shallow["nodes"], serde_json::json!([first]));
    assert!(
        exact_runner::agent::handle(&runner, r#"{"op":"tree","shallow":true}"#)
            .contains("shallow tree needs a target")
    );
    for bad in [
        serde_json::json!(1),
        serde_json::json!("true"),
        serde_json::Value::Null,
    ] {
        let request = serde_json::json!({"op":"tree", "target":"root", "shallow":bad});
        assert!(exact_runner::agent::handle(&runner, &request.to_string())
            .contains("tree shallow must be a boolean"));
    }
    let branch = ask(&runner, "branch".into());
    assert!(branch["nodes"].as_array().unwrap().len() > 1);
    assert!(branch["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["props"]["testId"] != "other"));
    for bad in [
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!(4294967296u64),
        serde_json::json!(true),
        serde_json::Value::Null,
        serde_json::json!([]),
    ] {
        assert_eq!(
            ask(&runner, bad)["error"],
            "tree target must be a view id or testId"
        );
    }
    runner.act("toggle", vec![]).unwrap();
    for target in ["branch".into(), first["id"].clone()] {
        let request = serde_json::json!({"op":"tree", "target":target, "shallow":true});
        assert!(
            exact_runner::agent::handle(&runner, &request.to_string()).contains("no view matches")
        );
    }
    assert!(ask(&runner, "branch".into())["error"]
        .as_str()
        .unwrap()
        .contains("no view matches"));
    assert!(ask(&runner, first["id"].clone())["error"]
        .as_str()
        .unwrap()
        .contains("no view matches"));
    assert_eq!(
        ask(&runner, "replacement".into())["nodes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn targeted_tree_uses_current_attachment_and_root_order() {
    use exact_kernel::{NodeType, Op, PropId};
    let mut runner = Runner::boot(
        contract::compile("component App\n  view\n    column testId=\"root\"\n      column testId=\"branch\"\n        text \"leaf\" testId=\"leaf\"\n").unwrap(),
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    ).unwrap();
    let ask = |runner: &Runner<Stations>, target: serde_json::Value| -> serde_json::Value {
        let response: serde_json::Value = serde_json::from_str(&exact_runner::agent::handle(
            runner,
            &serde_json::json!({"op":"tree", "target":target}).to_string(),
        ))
        .unwrap();
        let shallow: serde_json::Value = serde_json::from_str(&exact_runner::agent::handle(
            runner,
            &serde_json::json!({"op":"tree", "target":target, "shallow":true}).to_string(),
        ))
        .unwrap();
        let mut expected = response.clone();
        if let Some(nodes) = expected
            .get_mut("nodes")
            .and_then(serde_json::Value::as_array_mut)
        {
            nodes.truncate(1);
        }
        assert_eq!(shallow, expected);
        response
    };
    let root = runner.roots()[0];
    let branch = ask(&runner, "branch".into())["roots"][0].as_u64().unwrap() as u32;
    let leaf = ask(&runner, "leaf".into())["roots"][0].as_u64().unwrap() as u32;
    runner
        .kernel_mut()
        .apply(
            0,
            100,
            &[
                Op::CreateView {
                    id: 9000,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 9000,
                    prop: PropId::TestId,
                    value: "leaf".into(),
                },
            ],
        )
        .unwrap();
    assert_eq!(
        ask(&runner, "leaf".into())["roots"],
        serde_json::json!([leaf])
    );
    let matches = |runner: &Runner<Stations>| {
        assert_eq!(
            runner.kernel().find_first_by_test_id("leaf"),
            runner.kernel().find_by_test_id("leaf").first().copied()
        );
        runner
            .kernel()
            .find_by_test_id("leaf")
            .into_iter()
            .map(|key| runner.kernel().node_by_key(key).unwrap().id)
            .collect::<Vec<_>>()
    };
    assert_eq!(matches(&runner), vec![leaf, 9000]);
    assert!(ask(&runner, 9000.into())["error"].is_string());
    runner
        .kernel_mut()
        .apply(
            0,
            101,
            &[Op::SetChildren {
                id: root,
                children: vec![],
            }],
        )
        .unwrap();
    // A live selector can refer to an unattached node or an unattached subtree.
    assert_eq!(matches(&runner), vec![leaf, 9000]);
    for target in [
        serde_json::json!(leaf),
        serde_json::json!(branch),
        "leaf".into(),
        "branch".into(),
    ] {
        assert!(ask(&runner, target)["error"].is_string());
    }
    assert_eq!(
        ask(&runner, "root".into())["nodes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    runner
        .kernel_mut()
        .apply(
            0,
            102,
            &[Op::AttachRoot { id: 9000 }, Op::AttachRoot { id: branch }],
        )
        .unwrap();
    // The later allocation was attached first: root order wins over slot order.
    assert_eq!(
        ask(&runner, "leaf".into())["roots"],
        serde_json::json!([9000])
    );
    assert_eq!(matches(&runner), vec![9000, leaf]);
    assert_eq!(ask(&runner, 9000.into())["nodes"][0]["depth"], 0);
    let reattached = ask(&runner, leaf.into());
    assert_eq!(reattached["nodes"][0]["depth"], 1);
    assert_eq!(reattached["nodes"][0]["parent"], branch);
    runner
        .kernel_mut()
        .apply(0, 103, &[Op::DestroyView { id: 9000 }])
        .unwrap();
    assert_eq!(
        ask(&runner, "leaf".into())["roots"],
        serde_json::json!([leaf])
    );
}

#[test]
fn child_derives_keep_each_actions_captures_and_parameter_shadowing() {
    let src = r#"
component App
  state a = 100
  view
    column
      Child(a=2)
      Child(a=a)
component Child
  props
    a: number
  state n = 0
  derive b = c * 2
  derive c = a + n
  action add(a: number)
    n = b + a
  action step
    n = b
  view
    column
      text `${b}` testId=`value-${a}`
      button "Add" press=add(7) testId=`add-${a}`
      button "Step" press=step testId=`step-${a}`
"#;
    let mut r = Runner::boot(
        contract::compile(src).unwrap(),
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text_of(&r, "value-2"), "4");
    assert_eq!(text_of(&r, "value-100"), "200");
    let add = view_of(&r, "add-2");
    r.dispatch(add, Event::Press).unwrap();
    assert_eq!(text_of(&r, "value-2"), "26");
    assert_eq!(text_of(&r, "value-100"), "200");
    let step = view_of(&r, "step-2");
    r.dispatch(step, Event::Press).unwrap();
    assert_eq!(text_of(&r, "value-2"), "56");
    let other = view_of(&r, "step-100");
    r.dispatch(other, Event::Press).unwrap();
    assert_eq!(text_of(&r, "value-100"), "600");
    assert_eq!(text_of(&r, "value-2"), "56");
}
