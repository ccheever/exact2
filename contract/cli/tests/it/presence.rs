//! LLP 1063: `-exact-exit-animation` resolves its keyframes like `animation` and
//! reaches the receipt of the commit that removes its node; `-exact-layout-transition`
//! reaches its row.

use exact_kernel::{Kernel, StyleId};
use exact_motion::{Property, TimingFunction, Value};
use exact_plan::{Plan, Value as PlanValue};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[PlanValue]) -> Result<PlanValue, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn a_removed_row_leaves_with_its_exit_and_its_sibling_declares_a_layout_transition() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/presence.contract");
    let plan = contract::compile(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let first = k.node_by_key(k.find_by_test_id("first")[0]).unwrap();
    let (first_key, first_view) = (first.key, first.id);
    let second = k.node_by_key(k.find_by_test_id("second")[0]).unwrap();
    assert!(second.style.mask.has(StyleId::LayoutTransition));
    let layout = second
        .style
        .rare
        .layout_transition
        .matching(Property::Layout)
        .unwrap();
    assert!((layout.duration - 0.32).abs() < 1e-6);
    assert!(matches!(layout.timing, TimingFunction::Easing(_)));
    let toggle = k.node_by_key(k.find_by_test_id("toggle")[0]).unwrap().id;

    let receipt = r.dispatch(toggle, Event::Press).unwrap();
    assert!(receipt.destroyed.contains(&first_key));
    let [exit] = receipt.exits.as_slice() else {
        panic!(
            "{:?} {:?} {:?}",
            receipt.exits, receipt.destroyed, first_key
        )
    };
    assert_eq!(exit.key, first_key);
    let a = &exit.animations.0[0];
    assert_eq!(a.name, "leave");
    assert!((a.duration - 0.2).abs() < 1e-6);
    assert_eq!(
        a.keyframes.0.last().unwrap().values,
        [
            (Property::Opacity, Value::scalar(0.0)),
            (Property::Scale, Value::scalar(0.96)),
        ]
    );
    assert!(r.kernel().node(first_view).is_none());
}

#[test]
fn an_endless_exit_is_refused_at_compile_time() {
    let error = contract::compile(
        "keyframes k\n  to opacity=0\ncomponent App\n  view\n    text \"a\" -exact-exit-animation=\"k 1s infinite\"\n",
    )
    .unwrap_err();
    assert_eq!(error.id, "lower-exit-endless", "{error}");
    let error = contract::compile(
        "component App\n  view\n    text \"a\" -exact-exit-animation=\"gone 1s\"\n",
    )
    .unwrap_err();
    assert_eq!(error.id, "lower-animation-name");
    assert!(error.message.contains("no `keyframes gone`"), "{error}");
}
