use exact_js_value::{
    from_json, from_json_text, reply_from_json_slice, reply_from_json_text, Shape,
};
use exact_plan::Value;
use serde_json::Value as Json;

fn legacy(text: &str, shape: &Shape) -> Result<Value, String> {
    serde_json::from_str(text)
        .map_err(|e| e.to_string())
        .and_then(|json| from_json(&json, shape))
}

fn equal(actual: Result<Value, String>, expected: Result<Value, String>, context: &str) {
    // Value's ordinary PartialEq treats -0 as +0. The wire bytes don't.
    assert_eq!(
        actual.map(|v| v.to_bytes()),
        expected.map(|v| v.to_bytes()),
        "{context}"
    );
}

fn record() -> Shape {
    Shape::Record(vec![
        ("name".into(), Shape::String),
        ("count".into(), Shape::Number),
        ("enabled".into(), Shape::Option(Box::new(Shape::Bool))),
    ])
}

#[test]
fn direct_decode_matches_the_existing_shape_and_json_rules() {
    let shapes = [
        Shape::Unit,
        Shape::Bool,
        Shape::Number,
        Shape::String,
        Shape::Option(Box::new(Shape::Number)),
        Shape::Option(Box::new(Shape::Option(Box::new(Shape::String)))),
        Shape::List(Box::new(Shape::Option(Box::new(Shape::Number)))),
        Shape::Record(vec![]),
        record(),
        Shape::List(Box::new(record())),
        Shape::Record(vec![
            ("name".into(), Shape::String),
            ("name".into(), Shape::String),
        ]),
    ];
    let values = [
        "null",
        "true",
        "false",
        "0",
        "-0",
        "-0.0",
        "1.0",
        "1e-9999",
        "-1e-9999",
        "9007199254740993",
        "18446744073709551615",
        "18446744073709551616",
        "-9223372036854775808",
        "-9223372036854775809",
        "1.7976931348623157e308",
        "1e309",
        "1e9999",
        "NaN",
        "Infinity",
        "01",
        "1.",
        "+1",
        "--1",
        "1e",
        "",
        " ",
        "null true",
        "[",
        "[1,]",
        "{\"x\":}",
        "{\"x\":0,}",
        r#""plain""#,
        r#""\u0000é😀\u2028\u2029\"\\\n\t""#,
        r#""\ud83d\ude00""#,
        r#""\ud800""#,
        r#""\udfff""#,
        r#""\x00""#,
        "[]",
        "[1, null, -0, 1.25]",
        "[1, false, 1e9999]",
        "{}",
        r#"{"name":"one","count":2,"enabled":false}"#,
        r#"{"enabled":null,"count":-0,"name":"last"}"#,
        r#"{"name":2,"name":"last","count":2,"enabled":true}"#,
        r#"{"name":{"wrong":[0,false]},"name":"last","count":2,"enabled":true}"#,
        r#"{"name":"first","name":2,"count":2,"enabled":true}"#,
        r#"{"name":"first","count":2,"count":3,"enabled":true}"#,
        r#"{"name":"first","count":2,"enabled":true,"z":0,"a":1,"z":2}"#,
        r#"{"count":false,"z":0,"name":4,"enabled":true}"#,
        r#"{"name":"one","enabled":true,"z":0}"#,
        r#"{"name":"one","count":2,"enabled":null,"extra":1e9999}"#,
        r#"{"name":1e9999,"name":"last","count":2,"enabled":true}"#,
        r#"{"name":"one","extra":true}"#,
    ];
    for shape in &shapes {
        for text in values {
            equal(
                from_json_text(text, shape),
                legacy(text, shape),
                &format!("{shape:?}: {text}"),
            );
        }
    }
}

#[test]
fn nested_duplicate_errors_can_be_overwritten_and_error_order_is_declared_order() {
    let shape = Shape::Record(vec![
        ("first".into(), Shape::List(Box::new(record()))),
        ("second".into(), Shape::Number),
    ]);
    for text in [
        r#"{"first":[{"name":5}],"first":[],"second":1}"#,
        r#"{"first":[{"name":5}],"second":1}"#,
        r#"{"second":null,"first":[{"name":5}]}"#,
        r#"{"first":[],"second":1,"extra":false,"extra":{}}"#,
    ] {
        equal(from_json_text(text, &shape), legacy(text, &shape), text);
    }
}

#[test]
fn recursion_limit_also_applies_inside_discarded_values_and_envelopes() {
    for depth in [63, 64, 65, 125, 126, 127, 128, 129, 140] {
        let text = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        let mut shape = Shape::Number;
        for _ in 0..depth {
            shape = Shape::List(Box::new(shape));
        }
        for shape in [&shape, &Shape::Bool] {
            equal(from_json_text(&text, shape), legacy(&text, shape), &text);
            compare_reply(&format!(r#"{{"tag":0,"value":{text}}}"#), shape);
            compare_reply(&format!(r#"{{"tag":2,"extra":{text}}}"#), shape);
        }
    }
}

fn compare_reply(text: &str, shape: &Shape) {
    let expected = serde_json::from_str::<Json>(text);
    for actual in [
        reply_from_json_text(text, shape),
        reply_from_json_slice(text.as_bytes(), shape),
    ] {
        match (&expected, actual) {
            (Ok(json), Ok(reply)) => {
                equal(
                    reply.value,
                    from_json(json.get("value").unwrap_or(&Json::Null), shape),
                    text,
                );
                let mut fields = json.clone();
                if let Json::Object(fields) = &mut fields {
                    fields.remove("value");
                }
                assert_eq!(reply.fields, fields, "{text}");
            }
            (Err(expected), Err(actual)) => {
                assert_eq!(actual.to_string(), expected.to_string(), "{text}")
            }
            (expected, _) => panic!("envelope parse mismatch for {text}: expected {expected:?}"),
        }
    }
}

#[test]
fn envelopes_keep_metadata_errors_missing_values_and_last_duplicate_values() {
    for shape in [Shape::Unit, Shape::Number, record()] {
        for text in [
            "null",
            "0",
            "-0",
            "0.5",
            "true",
            r#""text""#,
            "[]",
            "[0]",
            "{}",
            r#"{"tag":0}"#,
            r#"{"tag":0,"value":null}"#,
            r#"{"tag":0,"value":false,"value":12}"#,
            r#"{"tag":0,"value":{},"value":{"name":"yes","count":1,"enabled":null}}"#,
            r#"{"tag":0,"value":12,"tag":2,"message":"no","writes":[["x","y"]]}"#,
            r#"{"tag":0,"value":12,"extra":1e9999}"#,
            r#"{"tag":0,"value":1e9999,"value":12}"#,
            r#"{"tag":0,"value":12} []"#,
        ] {
            compare_reply(text, &shape);
        }
    }
    assert!(reply_from_json_slice(b"{\"tag\":0,\"value\":\"\xff\"}", &Shape::String).is_err());
}
