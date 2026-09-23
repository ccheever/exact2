//! LLP 1017 P5: `fn` in Contract, expanded inline, proven on the runner.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn a_fn_is_a_pure_expression_over_its_parameters_expanded_where_called() {
    let plan = contract::compile(&corpus("fn.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.derive("price"), Some(&Value::str("$12")));
    assert_eq!(r.derive("label"), Some(&Value::str("yes")));
    r.act("flip", vec![]).unwrap();
    assert_eq!(r.derive("label"), Some(&Value::str("no")));
    r.act("bump", vec![]).unwrap();
    assert_eq!(r.slot("cents"), Some(&Value::Number(5000.0)));
    assert_eq!(r.derive("price"), Some(&Value::str("$50")));
    let k = r.kernel();
    let key = k.find_by_test_id("quad")[0];
    let text = k
        .node_by_key(key)
        .unwrap()
        .props
        .iter()
        .find_map(|(id, v)| match v {
            PropValue::Str(s) if id.name() == "text" => Some(s.clone()),
            _ => None,
        });
    assert_eq!(text.as_deref(), Some("12"));
}

#[test]
fn a_fn_sees_only_its_parameters_and_is_typed_like_a_roster_call() {
    let src = "fn f(n: number): number = n + cents\ncomponent A\n  state cents = 1\n  view\n    text `${f(1)}`\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-unknown-name", "{e}");
    let src = "fn f(n: number): number = n\ncomponent A\n  view\n    text `${f(\"a\")}`\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-argument", "{e}");
    let src = "fn f(n: number): number = n\ncomponent A\n  view\n    text `${f(1, 2)}`\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-arity", "{e}");
}

#[test]
fn scoped_actions_and_action_props_keep_their_names_when_the_roster_grows() {
    // @ref LLP 1038 D3 — `open` already names actions in the shipped readers.
    let plan = contract::compile(
        r#"component App
  state selected = ""
  action open(value: string) writes selected
    selected = value
  view
    column
      button "Direct" testId="direct" press=open("direct")
      Row(open=open)
component Row
  props
    open: action
  view
    button "Prop" testId="prop" press=open("prop")
"#,
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    for id in ["direct", "prop"] {
        let key = r.kernel().find_by_test_id(id)[0];
        let view = r.kernel().node_by_key(key).unwrap().id;
        r.dispatch(view, exact_runner::Event::Press).unwrap();
        assert_eq!(r.slot("selected"), Some(&Value::str(id)));
    }
}
