use exact_kernel::{Kernel, NodeType};
use exact_plan::{asm::Asm, builder::PlanBuilder, Plan, TypeKind, Value};
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("surface fact reached app data: {source}")
    }
    fn ready(&self) -> bool {
        false
    }
}
fn plan() -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let boolean = b.primitive(TypeKind::Bool);
    let string = b.primitive(TypeKind::String);
    let nested = b.record("Nested", &[("count", number)]);
    let list = b.list(nested);
    let option = b.option(string);
    let hud = b.record(
        "Hud",
        &[
            ("beacons", number),
            ("done", boolean),
            ("text", string),
            ("nested", nested),
            ("items", list),
            ("maybe", option),
        ],
    );
    for (resource, name) in [("hud", "world"), ("other", "other"), ("same", "world")] {
        let name = b.str(name);
        let mut asm = Asm::new();
        asm.str(name);
        let code = b.code(asm);
        b.resource(resource, "exactSurface", &[code], hud, None);
    }
    b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.finish().unwrap()
}
fn runner() -> Runner<NoData> {
    Runner::boot(
        plan(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn defaults() -> Value {
    Value::record(vec![
        Value::Number(0.0),
        Value::Bool(false),
        Value::str(""),
        Value::record(vec![Value::Number(0.0)]),
        Value::list(vec![]),
        Value::NONE,
    ])
}
#[test]
fn defaults_partial_extra_keys_clear_and_only_named_readers_recommit() {
    let mut r = runner();
    assert_eq!(r.resource("hud"), Some(&defaults()));
    assert!(r
        .set_surface_record("unread", Some(r#"{"x":1}"#))
        .unwrap()
        .is_none());
    let record = r#"{"beacons":3,"done":true,"text":"ready","nested":{"count":2},"items":[{}, {"count":4}],"maybe":"yes","extra":{"whatever":[null,false]}}"#;
    assert!(r
        .set_surface_record("world", Some(record))
        .unwrap()
        .is_some());
    assert_eq!(r.resource("hud"), r.resource("same"));
    assert_eq!(r.resource("other"), Some(&defaults()));
    assert!(agent::logs(&r, 0).contains("surface world (2 asked again)"));
    assert_eq!(
        r.resource("hud"),
        Some(&Value::record(vec![
            Value::Number(3.),
            Value::Bool(true),
            Value::str("ready"),
            Value::record(vec![Value::Number(2.)]),
            Value::list(vec![
                Value::record(vec![Value::Number(0.)]),
                Value::record(vec![Value::Number(4.)])
            ]),
            Value::some(Value::str("yes"))
        ]))
    );
    assert!(r
        .set_surface_record("world", Some(record))
        .unwrap()
        .is_none());
    r.set_surface_record("world", Some(r#"{"beacons":1,"nested":{}}"#))
        .unwrap();
    let Value::Record(fields) = r.resource("hud").unwrap() else {
        panic!()
    };
    assert_eq!(fields[0], Value::Number(1.));
    assert_eq!(fields[3], Value::record(vec![Value::Number(0.)]));
    r.set_surface_record("world", None).unwrap();
    assert_eq!(r.resource("hud"), Some(&defaults()));
    assert!(r.take_store_writes().is_empty());
    assert!(r.carry().store.is_empty());
}
#[test]
fn wrong_kind_names_resource_and_nested_field_and_recovers() {
    let mut r = runner();
    for (json, field) in [
        (r#"{"beacons":"one"}"#, "beacons"),
        (r#"{"nested":{"count":false}}"#, "nested.count"),
        (r#"{"items":[{"count":null}]}"#, "items[0].count"),
        (r#"{"maybe":2}"#, "maybe"),
    ] {
        let err = format!(
            "{:?}",
            r.set_surface_record("world", Some(json)).unwrap_err()
        );
        assert!(err.contains("hud") && err.contains(field), "{err}");
        assert_eq!(r.resource("hud"), Some(&defaults()));
    }
    r.set_surface_record("world", Some(r#"{"beacons":2}"#))
        .unwrap();
}
#[test]
fn json_unicode_numbers_duplicates_and_null_option() {
    let mut r = runner();
    r.set_surface_record("world",Some(r#"{"beacons":"ignored duplicate","beacons":1e3,"text":"雪\uD83D\uDE80\u0000\b\f\n\r\t\/\\\"","maybe":null}"#)).unwrap();
    let Value::Record(v) = r.resource("hud").unwrap() else {
        panic!()
    };
    assert_eq!(v[0], Value::Number(1000.));
    assert_eq!(v[2], Value::str("雪🚀\0\u{8}\u{c}\n\r\t/\\\""));
    assert_eq!(v[5], Value::NONE);
    r.set_surface_record("world", Some(r#"{"beacons":-0}"#))
        .unwrap();
    let Value::Record(v) = r.resource("hud").unwrap() else {
        panic!()
    };
    assert!(v[0].as_number().unwrap().is_sign_negative());
}
#[test]
fn malformed_input_and_caps_refuse_by_resource_name() {
    let mut r = runner();
    for json in [
        "",
        "null",
        "[]",
        "{}x",
        "{",
        "{\"text\":\"\n\"}",
        r#"{"x":true,}"#,
        r#"{"x":[1,]}"#,
        r#"{"x":01}"#,
        r#"{"x":1.}"#,
        r#"{"x":.1}"#,
        r#"{"x":+1}"#,
        r#"{"x":1e}"#,
        r#"{"x":NaN}"#,
        r#"{"x":Infinity}"#,
        r#"{"x":1e999}"#,
        r#"{"text":"\uD800"}"#,
        r#"{"text":"\uDC00"}"#,
        r#"{"text":"\uD800\u0000"}"#,
        r#"{"text":"\x00"}"#,
    ] {
        let error = format!(
            "{:?}",
            r.set_surface_record("world", Some(json)).unwrap_err()
        );
        assert!(error.contains("hud"), "{json}: {error}");
    }
    let sized = format!("{{\"extra\":\"{}\"}}", "x".repeat(65524));
    assert_eq!(sized.len(), 65536);
    r.set_surface_record("world", Some(&sized)).unwrap();
    let error = format!(
        "{:?}",
        r.set_surface_record("world", Some(&(sized + " ")))
            .unwrap_err()
    );
    assert!(error.contains("hud") && error.contains("64 KiB"));
    let nested = |n: usize| format!("{{\"extra\":{}0{}}}", "[".repeat(n), "]".repeat(n));
    r.set_surface_record("world", Some(&nested(31))).unwrap();
    let error = format!(
        "{:?}",
        r.set_surface_record("world", Some(&nested(32)))
            .unwrap_err()
    );
    assert!(error.contains("hud") && error.contains("depth"));
}
#[test]
fn reload_does_not_carry_surface_records() {
    let mut r = runner();
    r.set_surface_record("world", Some(r#"{"beacons":2}"#))
        .unwrap();
    let next = Runner::boot_carrying(
        plan(),
        NoData,
        Kernel::with_monospace(),
        &r.carry(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(next.resource("hud"), Some(&defaults()));
}
