//! LLP 1115.000 D10 on the web host: a control's spacing and size are the
//! plan's, the same on every host, so the page is told nothing a native host
//! is not.

use exact_runner::{DataError, DataSource, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot(src: &str) -> (Host<NoData>, String) {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

/// The `cssText` the first batch creates the node whose testId is `id` with.
fn css_of(host: &Host<NoData>, batch: &str, id: &str) -> String {
    let k = host.runner().kernel();
    let view = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
    let at = batch
        .find(&format!("{{\"op\":\"create\",\"id\":{view},"))
        .unwrap_or_else(|| panic!("no create for `{id}` in {batch}"));
    let op = &batch[at..];
    let css = &op[op.find("\"css\":\"").expect("its css") + 7..];
    css[..css.find('"').expect("the css ends")].to_owned()
}

/// D10.1: a checkbox, a radio and a range reach the page with no margin of
/// their own (lowering wrote Chrome's UA margins inline until 2026-10-11),
/// and one the author writes is the control's.
#[test]
fn a_checkbox_a_radio_and_a_range_carry_no_margin() {
    let (host, batch) = boot(
        r#"component App
  view
    row
      input type="checkbox" testId="check"
      input type="checkbox" switch testId="switch"
      input type="radio" testId="radio"
      input type="range" testId="range"
      input type="checkbox" margin-left=6 testId="spaced"
"#,
    );
    for id in ["check", "switch", "radio", "range"] {
        let css = css_of(&host, &batch, id);
        assert!(!css.contains("margin"), "{id}: {css}");
    }
    let spaced = css_of(&host, &batch, "spaced");
    assert!(spaced.contains("margin-left:6px"), "{spaced}");
}
