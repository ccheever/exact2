//! LLP 1075.003 §9.10: a fixed-size box in a `header` (a title's avatar)
//! carries its authored points as `headerBoxSize`, since a header the bar
//! replaces is laid out as `display: none`; a box outside a header does not.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn a_fixed_box_in_a_header_carries_its_authored_size() {
    let src = "component App\n  view\n    column\n      header\n        row\n          column width=40 height=40 background-color=\"#dcfce7\" testId=\"avatar\"\n            text \"MC\"\n          text \"Maya\" aria-level=2\n      column width=40 height=40 background-color=\"#dcfce7\" testId=\"elsewhere\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    assert_eq!(
        first.matches("\"headerBoxSize\":\"40x40\"").count(),
        1,
        "{first}"
    );
    let k = host.runner().kernel();
    let avatar = k.node_by_key(k.find_by_test_id("avatar")[0]).unwrap().id;
    assert!(
        first.contains(&format!("\"id\":{avatar},")),
        "the avatar is created"
    );
}
