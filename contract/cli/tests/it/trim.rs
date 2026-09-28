//! `trim(text)` (LLP 1054.000.005): a send button disabled on an empty
//! draft stays disabled on a draft of spaces.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SRC: &str = r#"component App
  state draft = ""
  action edit(value: string) writes draft
    draft = value
  view
    column
      input value=draft change=edit testId="draft"
      button "Send" disabled=(trim(draft) == "") testId="send"
"#;

#[test]
fn a_draft_of_spaces_leaves_send_disabled() {
    let plan = contract::compile(SRC).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let disabled = |r: &Runner<NoData>| {
        let k = r.kernel();
        let key = k.find_by_test_id("send")[0];
        k.node_by_key(key).unwrap().props.bool(PropId::Disabled) == Some(true)
    };
    assert!(disabled(&r));
    r.act("edit", vec![Value::str("   ")]).unwrap();
    assert!(disabled(&r), "three spaces");
    r.act("edit", vec![Value::str("\u{a0}\n\u{3000}")]).unwrap();
    assert!(disabled(&r), "JavaScript's whitespace, not only ASCII");
    r.act("edit", vec![Value::str("  hi ")]).unwrap();
    assert!(!disabled(&r));
}

#[test]
fn trim_takes_a_string() {
    let src = SRC.replace("trim(draft)", "trim(3)");
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "type-argument");
}
