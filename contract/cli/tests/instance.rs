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
                let asc = matches!(args.first(), Some(Value::Str(s)) if s.as_ref() == "asc");
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
    let mut r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
    // Two uses of Counter: two states, two derives, two actions.
    assert_eq!(text_of(&r, "count-text-a"), "a 0 0");
    let a = view_of(&r, "count-a");
    r.dispatch(a, Event::Press).unwrap();
    r.dispatch(a, Event::Press).unwrap();
    assert_eq!(text_of(&r, "count-text-a"), "a 2 4");
    assert_eq!(text_of(&r, "count-text-b"), "b 0 0");
    // A singleton's state is a root slot by its lifted name; carried by name.
    assert_eq!(r.slot("n__1"), Some(&Value::Number(2.0)));
    assert!(r.carry().slots.iter().any(|(n, _)| n == "n__1"));
    // Rows: `label` initialized from the row item, then hover one while the
    // other is untouched.
    let mv = view_of(&r, "station-mv");
    r.dispatch(mv, Event::Hover(true)).unwrap();
    assert_eq!(text_of(&r, "name-mv"), "Mountain View !");
    assert_eq!(text_of(&r, "name-pa"), "Palo Alto");
    // A row slot is the row's: not a root slot, not carried.
    assert_eq!(r.slot("hot__3"), None);
    assert!(!r.carry().slots.iter().any(|(n, _)| n == "hot__3"));
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
    assert!(r.act("setHot__3", vec![Value::Bool(true)]).is_err());
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
    let r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
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

    let error = Runner::boot(malformed, Stations, Kernel::with_monospace())
        .err()
        .unwrap();
    assert!(matches!(error, RunnerError::SlotType { slot } if slot == name));
}

#[test]
fn an_action_parameter_shadows_a_same_named_child_prop() {
    let src = "component App\n  view\n    Capture(value=\"prop\")\ncomponent Capture\n  props\n    value: string\n  state seen = \"\"\n  action capture(value: string) writes seen\n    seen = value\n  view\n    column\n      input value=seen change=capture testId=\"capture-input\"\n      text seen testId=\"capture-result\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
    let input = view_of(&r, "capture-input");
    r.dispatch(input, Event::Change("payload".into())).unwrap();
    assert_eq!(text_of(&r, "capture-result"), "payload");
}

#[test]
fn nested_row_actions_use_lexical_items_even_when_a_root_name_collides() {
    let src = "shape Station\n  id: string\n  name: string\ncomponent App\n  state item = \"root collision\"\n  state visible = true\n  resource stations = stations(\"asc\") as shape list<Station>\n  view\n    column\n      each outer in stations key=outer.id\n        each item in stations key=item.id\n          when visible\n            ScopedRow(outer=outer, item=item)\ncomponent ScopedRow\n  props\n    outer: Station\n    item: Station\n  state selected = item.name\n  state result = \"\"\n  action choose writes result\n    result = `${outer.name}/${item.name}`\n  view\n    column\n      text selected testId=`selected-${outer.id}-${item.id}`\n      button \"choose\" press=choose testId=`choose-${outer.id}-${item.id}`\n      text result testId=`result-${outer.id}-${item.id}`\n";
    let plan = contract::compile(src).unwrap();
    let plan = contract::bake(plan, Stations).unwrap();
    let mut r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
    assert_eq!(text_of(&r, "selected-mv-pa"), "Palo Alto");
    let choose = view_of(&r, "choose-mv-pa");
    r.dispatch(choose, Event::Press).unwrap();
    assert_eq!(text_of(&r, "result-mv-pa"), "Mountain View/Palo Alto");
    assert_eq!(r.slot("item"), Some(&Value::str("root collision")));
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
  action flip writes reverse
    reverse = !reverse
  action toggle writes shown
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
    let mut runner = Runner::boot(plan, Keys, Kernel::with_monospace()).unwrap();
    let before = runner.kernel().find_by_test_id("key-0")[0];
    let receipt = runner.act("flip", vec![]).unwrap();
    assert!(receipt.created.is_empty() && receipt.destroyed.is_empty());
    assert_eq!(runner.kernel().find_by_test_id("key-0")[0], before);
    let listeners = runner.handlers();
    assert_eq!(listeners.len(), 3);
    for (view, events) in &listeners {
        assert_eq!(*events, runner.handlers_of(*view));
    }
    let root = runner.roots()[0];
    let old_order = runner.kernel().node(root).unwrap().children();
    let receipt = runner.act("toggle", vec![]).unwrap();
    assert_eq!(receipt.destroyed.len(), 6);
    assert_eq!(receipt.created.len(), 2);
    let new_listeners = runner.handlers();
    assert_eq!(new_listeners.len(), 1);
    assert!(listeners.keys().all(|id| !new_listeners.contains_key(id)));
    assert!(old_order
        .iter()
        .all(|id| runner.kernel().node(*id).is_none()));
    assert_eq!(runner.kernel().node(root).unwrap().children().len(), 1);
    runner.act("toggle", vec![]).unwrap();
    assert_eq!(runner.handlers().len(), 3);
    assert_ne!(runner.kernel().find_by_test_id("key-0")[0], before);
}
