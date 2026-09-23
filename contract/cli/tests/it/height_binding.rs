//! The sheet handle is an authored IDREF and a typed two-number release.

use exact_kernel::{Kernel, PropId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected data query: {name}")
    }
}

fn source(params: &str, handler: &str, prop: &str) -> String {
    format!(
        "component App\n  state sheetPx = 360\n  action snap({params}) writes sheetPx\n    sheetPx = 180\n  view\n    column id=\"sheet\" height=sheetPx box-sizing=\"border-box\"\n      column testId=\"handle\" heightDragFor={prop} heightrelease={handler}\n"
    )
}

#[test]
fn height_binding_compiles_bakes_roundtrips_and_exports_the_handler() {
    let plan = contract::compile(&source(
        "height: number, velocity: number",
        "snap",
        "\"sheet\"",
    ))
    .unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let handle = runner.kernel().find_by_test_id("handle")[0];
    let handle = runner.kernel().node_by_key(handle).unwrap();
    let prop = PropId::from_name("heightDragFor").expect("schema prop");
    assert_eq!(prop as u16, 75);
    assert_eq!(handle.props.str(prop), Some("sheet"));
    let event = EventKind::from_name("heightrelease").expect("plan event");
    assert_eq!(event as u8, 14);
    assert_eq!(runner.handlers_of(handle.id), vec![event]);
    let tree: serde_json::Value = serde_json::from_str(&agent::tree(&runner)).unwrap();
    assert!(tree["nodes"].as_array().unwrap().iter().any(|node| {
        node["props"]["heightDragFor"] == "sheet"
            && node["handlers"] == serde_json::json!(["heightrelease"])
    }));
}

#[test]
fn release_requires_two_numeric_trailing_parameters_and_string_idref() {
    for (params, handler, prop, error) in [
        ("height: number", "snap", "\"sheet\"", "handler-arity"),
        (
            "height: number, velocity: number, extra: number",
            "snap",
            "\"sheet\"",
            "handler-arity",
        ),
        (
            "height: string, velocity: number",
            "snap",
            "\"sheet\"",
            "handler-type",
        ),
        (
            "height: number, velocity: bool",
            "snap",
            "\"sheet\"",
            "handler-type",
        ),
        ("height: number, velocity: number", "snap", "3", "attr-type"),
        (
            "height: number, velocity: number",
            "snap",
            "false",
            "attr-type",
        ),
    ] {
        let err = contract::compile(&source(params, handler, prop))
            .unwrap_err()
            .to_string();
        assert!(err.contains(error), "{params}/{prop}: {err}");
    }
    contract::compile(&source(
        "origin: string, height: number, velocity: number",
        "snap(\"drag\")",
        "\"sheet\"",
    ))
    .unwrap();
}

#[test]
fn child_action_props_are_rechecked_after_inlining() {
    for ty in ["number", "string"] {
        let src = format!(
            "component App\n  state count = 0\n  action snap(height: {ty}, velocity: number) writes count\n    count = count + 1\n  view\n    column\n      Handle(release=snap)\ncomponent Handle\n  props\n    release: action\n  view\n    column heightDragFor=\"sheet\" heightrelease=release\n"
        );
        let result = contract::compile(&src);
        if ty == "number" {
            result.unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains("handler-type"));
        }
    }
}
