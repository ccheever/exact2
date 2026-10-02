//! Fixed photo-transform authoring: IDREF, four geometry and six release numbers.
use exact_kernel::{Kernel, PropId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected query: {name}")
    }
}

/// The handle with `event` bound to `handler`, and its partner (a transform
/// drag needs both, LLP 1057.003 C6) bound to a well-formed `partner`.
fn source(event: &str, params: &str, handler: &str, idref: &str) -> String {
    let (other, other_params) = partner(event);
    format!("component App\n  state value = 0\n  action receive({params})\n    value = 1\n  action partner({other_params})\n    value = 2\n  view\n    column overflow=\"hidden\"\n      column id=\"photo\" width=\"100%\" height=\"100%\" box-sizing=\"border-box\"\n        column testId=\"handle\" transformDragFor={idref} {event}={handler} {other}=partner\n")
}
fn partner(event: &str) -> (&'static str, &'static str) {
    if event == "transformgeometry" {
        ("transformrelease", RELEASE)
    } else {
        ("transformgeometry", GEOMETRY)
    }
}

const GEOMETRY: &str = "bw: number, bh: number, pw: number, ph: number";
const RELEASE: &str = "x: number, y: number, scale: number, vx: number, vy: number, vs: number";

#[test]
fn transform_handlers_compile_bake_roundtrip_and_export_old_and_new_ordinals() {
    for (event, params, ordinal) in [
        ("transformgeometry", GEOMETRY, 15),
        ("transformrelease", RELEASE, 16),
    ] {
        let plan = contract::compile(&source(event, params, "receive", "\"photo\"")).unwrap();
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
        let key = runner.kernel().find_by_test_id("handle")[0];
        let handle = runner.kernel().node_by_key(key).unwrap();
        let prop = PropId::from_name("transformDragFor").unwrap();
        assert_eq!(prop as u16, 76);
        assert_eq!(handle.props.str(prop), Some("photo"));
        let event_kind = EventKind::from_name(event).unwrap();
        assert_eq!(event_kind as u8, ordinal);
        assert!(runner.handlers_of(handle.id).contains(&event_kind));
        let tree: serde_json::Value = serde_json::from_str(&agent::tree(&runner)).unwrap();
        assert!(tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["props"]["transformDragFor"] == "photo"
                && n["handlers"]
                    .as_array()
                    .is_some_and(|h| h.contains(&serde_json::json!(event)))));
    }
    assert_eq!(EventKind::Heightrelease as u8, 14);
    assert_eq!(EventKind::Navigate as u8, 13);
    assert_eq!(PropId::HeightDragFor as u16, 75);
}

#[test]
fn exact_numeric_arity_and_string_idref_are_required() {
    for (event, params) in [
        ("transformgeometry", GEOMETRY),
        ("transformrelease", RELEASE),
    ] {
        for bad in [
            params.rsplit_once(',').unwrap().0.to_string(),
            format!("{params}, extra: number"),
        ] {
            let e = contract::compile(&source(event, &bad, "receive", "\"photo\""))
                .unwrap_err()
                .to_string();
            assert!(e.contains("handler-arity"), "{e}");
        }
        for ty in ["string", "bool"] {
            let bad = params.replacen("number", ty, 1);
            let e = contract::compile(&source(event, &bad, "receive", "\"photo\""))
                .unwrap_err()
                .to_string();
            assert!(e.contains("handler-type"), "{e}");
        }
        for bad in ["1", "false"] {
            let e = contract::compile(&source(event, params, "receive", bad))
                .unwrap_err()
                .to_string();
            assert!(e.contains("attr-type"), "{e}");
        }
        contract::compile(&source(
            event,
            &format!("origin: string, {params}"),
            "receive(\"photo\")",
            "\"photo\"",
        ))
        .unwrap();
    }
}

#[test]
fn one_transform_handler_without_the_other_is_refused() {
    for (event, params, missing) in [
        ("transformgeometry", GEOMETRY, "transformrelease"),
        ("transformrelease", RELEASE, "transformgeometry"),
    ] {
        let src = format!("component App\n  state value = 0\n  action receive({params})\n    value = 1\n  view\n    column\n      column id=\"photo\"\n        column transformDragFor=\"photo\" {event}=receive\n");
        let e = contract::compile(&src).unwrap_err().to_string();
        assert!(
            e.contains("lower-transform-drag-handlers") && e.contains(&format!("add `{missing}=`")),
            "{e}"
        );
    }
}

#[test]
fn numeric_types_are_rechecked_after_child_action_inlining() {
    for (event, params) in [
        ("transformgeometry", GEOMETRY),
        ("transformrelease", RELEASE),
    ] {
        for invalid in [false, true] {
            let params = if invalid {
                params.replacen("number", "string", 1)
            } else {
                params.into()
            };
            let (other, other_params) = partner(event);
            let src = format!("component App\n  state n = 0\n  action receive({params})\n    n = 1\n  action partner({other_params})\n    n = 2\n  view\n    column\n      Handle(callback=receive, other=partner)\ncomponent Handle\n  props\n    callback: action\n    other: action\n  view\n    column transformDragFor=\"photo\" {event}=callback {other}=other\n");
            let result = contract::compile(&src);
            if invalid {
                assert!(result.unwrap_err().to_string().contains("handler-type"));
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn pixel_translate_literals_and_dynamic_template_reach_kernel_rows() {
    let src = "component App\n  state x = 0\n  state y = 0\n  action move(a: number, b: number)\n    x = a\n    y = b\n  view\n    column\n      column testId=\"literal\" translate=\"-12.5px 3px\"\n      column testId=\"dynamic\" translate=`${x}px ${y}px`\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let literal = r.kernel().find_by_test_id("literal")[0];
    assert_eq!(
        r.kernel().node_by_key(literal).unwrap().style.translate,
        exact_kernel::Vec2 { x: -12.5, y: 3.0 }
    );
    let dynamic = r.kernel().find_by_test_id("dynamic")[0];
    r.act("move", vec![Value::Number(27.25), Value::Number(-40.0)])
        .unwrap();
    assert_eq!(
        r.kernel().node_by_key(dynamic).unwrap().style.translate,
        exact_kernel::Vec2 { x: 27.25, y: -40.0 }
    );
    // A value the row refuses is unset, as CSS does an invalid value at
    // computed-value time, and journaled; the runner goes on.
    r.act("move", vec![Value::Number(1e40), Value::Number(0.0)])
        .unwrap();
    assert!(!r.is_poisoned());
    assert_eq!(
        r.kernel().node_by_key(dynamic).unwrap().style.translate,
        exact_kernel::Vec2 { x: 0.0, y: 0.0 }
    );
    assert!(r.journal().any(|l| l.contains("invalid translate value")));
    r.act("move", vec![Value::Number(1.0), Value::Number(2.0)])
        .unwrap();
    assert_eq!(
        r.kernel().node_by_key(dynamic).unwrap().style.translate,
        exact_kernel::Vec2 { x: 1.0, y: 2.0 }
    );
    for bad in [
        "1 2",
        "20% 0",
        "calc(1px + 2px) 0",
        "1px 2px 3px",
        "NaNpx 0",
        "1e39px 0",
    ] {
        let src = format!("component App\n  view\n    column translate=\"{bad}\"\n");
        assert!(contract::compile(&src).is_err(), "{bad}");
    }
}
