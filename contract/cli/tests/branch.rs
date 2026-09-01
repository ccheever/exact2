//! LLP 1017 P2: `if`/`else` and `match` as statements in an action, proven
//! on the runner (LLP 1004 D6: a construct exists at both ends).

use exact_kernel::Kernel;
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
fn an_action_branches_on_a_key_and_matches_an_option() {
    let plan = contract::compile(&corpus("branch.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(plan, NoData, Kernel::with_monospace()).unwrap();
    r.act("typed", vec![Value::str("a")]).unwrap();
    r.act("typed", vec![Value::str("b")]).unwrap();
    assert_eq!(r.slot("query"), Some(&Value::str("ab")));
    assert_eq!(r.slot("submitted"), Some(&Value::Number(0.0)));
    r.act("typed", vec![Value::str("Enter")]).unwrap();
    assert_eq!(r.slot("query"), Some(&Value::str("")));
    assert_eq!(r.slot("submitted"), Some(&Value::Number(1.0)));
    r.act("show", vec![]).unwrap();
    assert_eq!(r.slot("seen"), Some(&Value::str("nothing")));
    r.act("pick", vec![Value::str("mv")]).unwrap();
    r.act("show", vec![]).unwrap();
    assert_eq!(r.slot("seen"), Some(&Value::str("mv")));
}

#[test]
fn a_branch_may_nest_and_an_omitted_else_is_fine() {
    let src = "component A\n  state n = 0\n  state s = \"\"\n  action go(k) writes n, s\n    if k == \"a\"\n      if n > 0\n        s = \"again\"\n      else\n        s = \"first\"\n      n = n + 1\n  view\n    column testId=\"root\"\n      input value=s change=go testId=\"i\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(plan, NoData, Kernel::with_monospace()).unwrap();
    r.act("go", vec![Value::str("x")]).unwrap();
    assert_eq!(r.slot("n"), Some(&Value::Number(0.0)));
    r.act("go", vec![Value::str("a")]).unwrap();
    assert_eq!(r.slot("s"), Some(&Value::str("first")));
    r.act("go", vec![Value::str("a")]).unwrap();
    assert_eq!(r.slot("s"), Some(&Value::str("again")));
    assert_eq!(r.slot("n"), Some(&Value::Number(2.0)));
}
