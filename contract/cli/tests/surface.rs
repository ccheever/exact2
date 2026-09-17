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
