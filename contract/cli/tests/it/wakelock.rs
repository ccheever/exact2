//! LLP 1125 D11: `wakelock` lowers to a prop on any element, a literal other
//! than its two words is refused naming both, and a bound one follows state.

use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn it_lowers_on_any_element() {
    let src = "component App\n  state on = true\n  view\n    column testId=\"box\" wakelock=\"screen\"\n      text \"a\" testId=\"text\" wakelock=(on ? \"screen\" : \"none\")\n      image \"symbol:star\" testId=\"image\" wakelock=\"none\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let prop = |id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .str(PropId::Wakelock)
            .map(str::to_string)
    };
    assert_eq!(prop("box").as_deref(), Some("screen"));
    assert_eq!(prop("text").as_deref(), Some("screen"));
    assert_eq!(prop("image").as_deref(), Some("none"));
}

#[test]
fn a_literal_outside_the_two_words_is_refused() {
    for value in ["system", "Screen", "on", ""] {
        let src = format!("component App\n  view\n    column wakelock=\"{value}\"\n");
        let error = contract::compile(&src).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value")
                && error.contains("\"screen\"")
                && error.contains("\"none\""),
            "wakelock={value}: {error}"
        );
    }
    // Another platform's spelling names this one.
    let error = contract::compile("component App\n  view\n    column keepScreenOn=true\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("wakelock"), "{error}");
}

#[test]
fn test_steps_read_the_holder_record() {
    let src = "test \"lock\"\n  expect wakelock held\n  expect wakelock released\n";
    let tests = contract::tests(src).unwrap();
    let json = contract::tests_json(&tests);
    for want in [
        r#"{"op":"expect-wakelock","held":true,"line":2}"#,
        r#"{"op":"expect-wakelock","held":false,"line":3}"#,
    ] {
        assert!(json.contains(want), "{want}\n{json}");
    }
    for bad in ["on", "screen", "Held", "", "\"held\""] {
        let error =
            contract::tests(&format!("test \"lock\"\n  expect wakelock {bad}\n")).unwrap_err();
        assert_eq!(error.id, "syntax-expected-step");
        assert!(
            error.message.contains("held") && error.message.contains("released"),
            "{bad}: {error}"
        );
    }
    assert!(contract::tests("test \"lock\"\n  expect wakelock held by \"box\"\n").is_err());
}
