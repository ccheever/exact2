use exact_kernel::Kernel;
use exact_runner::{DataError, DataSource, Runner, Value};
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
    fn ready(&self) -> bool {
        false
    }
}
const SOURCE: &str = include_str!("../../corpus/surface.contract");
#[test]
fn bake_defaults_are_the_first_frame_and_never_kept_or_typescript() {
    let p = contract::bake(contract::compile(SOURCE).unwrap(), NoData).unwrap();
    assert_eq!(
        Value::from_bytes(p.bytes(p.resources[0].initial)).unwrap(),
        Value::record(vec![Value::Number(0.)])
    );
    assert!(p.resources[0].reader);
    assert!(!contract::typescript(&p).unwrap().contains("exactSurface"));
    let mut r = Runner::boot(p, NoData, Kernel::with_monospace(), Default::default(), "/").unwrap();
    r.set_surface_record("world", Some(r#"{"beacons":1}"#))
        .unwrap();
    assert_eq!(
        r.resource("hud"),
        Some(&Value::record(vec![Value::Number(1.)]))
    );
    r.set_surface_record("status", Some(r#"{"ready":true}"#))
        .unwrap();
    assert_eq!(
        r.resource("status"),
        Some(&Value::record(vec![Value::Bool(true)]))
    );
    assert_eq!(
        r.resource("same"),
        Some(&Value::record(vec![Value::Bool(false)]))
    );
    r.set_surface_record("world", Some(r#"{"beacons":2,"ready":true}"#))
        .unwrap();
    assert_eq!(
        r.resource("same"),
        Some(&Value::record(vec![Value::Bool(true)]))
    );
    assert!(r.take_store_writes().is_empty());
}
#[test]
fn lint_refuses_arity_nonliteral_nonrecord_and_unknown_surface_by_name() {
    let cases = [
        SOURCE.replace("exactSurface(\"world\")", "exactSurface()"),
        SOURCE.replace(
            "exactSurface(\"world\")",
            "exactSurface(\"world\", \"other\")",
        ),
        SOURCE.replace("exactSurface(\"world\")", "exactSurface(12)"),
        SOURCE
            .replace(
                "  resource hud",
                "  state surfaceName = \"world\"\n  resource hud",
            )
            .replace("exactSurface(\"world\")", "exactSurface(surfaceName)"),
        SOURCE.replace("exactSurface(\"world\")", "exactSurface(\"missing\")"),
    ];
    let mut nonrecord = contract::compile(SOURCE).unwrap();
    let number = nonrecord.fields[0].ty;
    nonrecord.resources[0].ty = number;
    assert!(matches!(
        contract::bake(nonrecord, NoData),
        Err(contract::BakeError::Lint {
            id: "bake-surface-record",
            ..
        })
    ));
    for source in cases {
        let p = contract::compile(&source).unwrap();
        let error = contract::bake(p, NoData).unwrap_err();
        assert!(
            matches!(
                error,
                contract::BakeError::Lint {
                    id: "bake-surface-record",
                    ..
                }
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("hud"));
    }
}

#[test]
fn viewport_and_delivery_readers_choose_subsets_without_source_unification() {
    for (source, fields) in [
        ("exactViewport", ["width: number", "height: number"]),
        (
            "exactDelivery",
            ["stream: string", "compatibilityId: string"],
        ),
    ] {
        let text = format!("shape A\n  {}\nshape B\n  {}\ncomponent App\n  resource a = {source}() as shape A\n  resource b = {source}() as shape B\n  view\n    text \"facts\"\n", fields[0], fields[1]);
        let p = contract::compile(&text).unwrap();
        // Named once in the plan's source table (LLP 1030 D7), never unified.
        assert_eq!(p.sources.len(), 1);
        contract::bake(p, NoData).unwrap();
    }
}

#[test]
fn control_action_is_a_canvas_child_property() {
    let source = "component Controls\n  view\n    canvas\n      button action=\"jump\" testId=\"jump\"\n        text \"Jump\"\n";
    let plan = contract::compile(source).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(exact_runner::agent::tree(&r).contains(r#""action":"jump""#));
    let outside = source
        .replace("    canvas\n", "")
        .replace("      button", "    button")
        .replace("        text", "      text");
    assert!(contract::compile(&outside)
        .unwrap_err()
        .to_string()
        .contains("analyze-control-parent"));
    assert!(
        contract::compile(&source.replace("button action", "view action"))
            .unwrap_err()
            .to_string()
            .contains("analyze-control-parent")
    );
}
