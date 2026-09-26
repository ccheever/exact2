//! LLP 1054 P1 — a flex item's negative margin shortens its container, as in
//! CSS, including when the item has a `min-height`: Taffy added the margin
//! before clamping by the inner min size, and the margin disappeared (the
//! Bluesky profile header measured 44 pt tall on iOS; vendor/taffy patch 14).
use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// The height of `col`, a content-sized column holding `body`.
fn col_height(body: &str) -> f32 {
    let src = format!("component App\n  view\n    column testId=\"root\" width=390 height=844\n      column testId=\"col\"\n{body}");
    let plan = contract::bake(contract::compile(&src).unwrap(), NoData).unwrap();
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
    k.compute_layout(root, Offer::definite(390.0, 844.0))
        .unwrap();
    k.node_by_key(k.find_by_test_id("col")[0])
        .unwrap()
        .frame
        .height
}

#[test]
fn a_negative_margin_shortens_the_column_with_or_without_a_min_height() {
    let (top, bottom) = ("        box height=100\n", "        box height=100\n");
    for row in [
        "        row height=90 margin-top=-44\n",
        "        row margin-top=-44\n          box width=90 height=90\n",
        "        row margin-top=-44 min-height=48\n          box width=90 height=90\n",
        "        row margin-top=-44 min-height=48 height=90\n",
        "        column margin-top=-44 max-height=200\n          box width=90 height=90\n",
    ] {
        assert_eq!(
            col_height(&format!("{top}{row}{bottom}")),
            246.0,
            "100 + (90 - 44) + 100 for\n{row}"
        );
    }
}

#[test]
fn the_profile_header_the_port_found() {
    let header = "        box width=\"100%\" aspect-ratio=3\n        row justify-content=\"space-between\" align-items=\"flex-end\" margin-top=-44 min-height=48\n          box width=90 height=90\n          row\n            box width=40 height=36\n        column padding-top=10\n          box height=100\n";
    assert_eq!(col_height(header), 130.0 + (90.0 - 44.0) + 110.0);
}
