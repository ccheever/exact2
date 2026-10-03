//! A `text` is a flex item, as CSS makes every child of a flex container
//! (the tennis game's `text has no attribute flex-grow`), and can be hidden
//! from assistive technology (`aria-hidden`, its decorative glyph).

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(body: &str) -> Runner<NoData> {
    let src =
        format!("component App\n  view\n    column testId=\"root\" width=300 height=400\n{body}");
    let plan = contract::bake(
        contract::compile(&src).unwrap_or_else(|e| panic!("{e}")),
        NoData,
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(300.0, 400.0))
        .unwrap();
    r
}

/// (x, y, width, height) of each test id, in the order asked.
fn frames(r: &Runner<NoData>, ids: &[&str]) -> Vec<(f32, f32, f32, f32)> {
    ids.iter()
        .map(|id| {
            let f = r
                .kernel()
                .node_by_key(r.kernel().find_by_test_id(id)[0])
                .unwrap()
                .frame;
            (f.x, f.y, f.width, f.height)
        })
        .collect()
}

#[test]
fn a_text_takes_the_flex_item_properties() {
    // flex-grow: the row's free space goes to the growing text.
    let r = boot("      row testId=\"row\" width=300 height=100 align-items=\"flex-start\"\n        text \"a\" flex-grow=1 testId=\"grow\"\n        text \"b\" testId=\"fixed\"\n");
    let [grow, fixed] = frames(&r, &["grow", "fixed"])[..] else {
        unreachable!()
    };
    assert_eq!(grow.2 + fixed.2, 300.0, "{grow:?} {fixed:?}");
    assert_eq!(fixed.0, grow.2, "{grow:?} {fixed:?}");

    // flex-basis, flex-shrink: a 200-wide basis shrinks in a 150 row unless
    // its shrink is 0.
    let r = boot("      row width=150 height=100 align-items=\"flex-start\"\n        text \"a\" flex-basis=200 testId=\"shrinks\"\n      row width=150 height=100 align-items=\"flex-start\"\n        text \"a\" flex-basis=200 flex-shrink=0 testId=\"keeps\"\n");
    let [shrinks, keeps] = frames(&r, &["shrinks", "keeps"])[..] else {
        unreachable!()
    };
    assert_eq!(shrinks.2, 150.0, "{shrinks:?}");
    assert_eq!(keeps.2, 200.0, "{keeps:?}");

    // align-self: one text stretches across a row that starts its items.
    let r = boot("      row width=300 height=100 align-items=\"flex-start\"\n        text \"a\" align-self=\"stretch\" testId=\"stretched\"\n        text \"b\" testId=\"started\"\n");
    let [stretched, started] = frames(&r, &["stretched", "started"])[..] else {
        unreachable!()
    };
    assert_eq!(stretched.3, 100.0, "{stretched:?}");
    assert!(started.3 < 100.0, "{started:?}");
}

#[test]
fn a_text_can_be_hidden_from_assistive_technology() {
    let r = boot("      text \"•\" aria-hidden=true testId=\"glyph\"\n");
    let node = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("glyph")[0])
        .unwrap();
    assert_eq!(
        node.props.bool(PropId::AccessibilityElementsHidden),
        Some(true)
    );
}
