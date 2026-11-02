//! A textarea the person resizes by its CSS `resize` grip: the border box
//! the host reports becomes the `width` and `height` rows Chrome writes
//! (`Runner::resize_box`), bounded by `min-*` and `max-*` at layout.

use crate::host::{view, NoData};
use exact_apple::host::Restyle;
use exact_apple::Host;
use exact_kernel::{MonospaceMeasurer, RowValue, StyleId};

const SRC: &str = r#"component Form
  view
    column width=400 height=800 align-items="flex-start"
      textarea testId="tall" resize="vertical" width=200 height=60 padding=4 border="1px solid" min-height=40 max-height=200
      textarea testId="grown" resize="block" field-sizing="content" width=200 padding=4 border="1px solid" min-height=40 max-height=200
      textarea testId="both" resize="both" box-sizing="border-box" width=200 height=60 padding=4 border="1px solid"
      textarea testId="wide" resize="horizontal" width=200 height=60 padding=4 border="1px solid"
      textarea testId="fixed" width=200 height=60 padding=4 border="1px solid"
"#;

fn boot() -> Host<NoData> {
    let plan = contract::compile(SRC).unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        800.0,
    )
    .unwrap();
    host
}

/// The person drags `id`'s grip to a border box of `size`; its laid-out
/// border box after.
fn drag(host: &mut Host<NoData>, id: &str, size: [Option<f32>; 2]) -> (f32, f32) {
    let view = view(host, id);
    let batch = host.set_claims(&[], &[], &[], &[], &[], &[(view, Restyle::Size(size))]);
    let batch: serde_json::Value = serde_json::from_str(&batch).unwrap();
    assert!(batch["error"].is_null(), "{id}: {batch}");
    let frame = host.runner().kernel().node(view).unwrap().frame;
    (frame.width, frame.height)
}

fn row(host: &Host<NoData>, id: &str, row: StyleId) -> String {
    let node = view(host, id);
    match host.runner().kernel().node(node).unwrap().computed(row) {
        RowValue::Dimension(d) => format!("{d:?}"),
        other => format!("{other:?}"),
    }
}

#[test]
fn a_dragged_grip_writes_the_size_in_the_box_sizing_chrome_writes() {
    let mut host = boot();
    // A content box: the 10 px of padding and border come off, as Chrome
    // writes `height: 110px` for a 120 px border box.
    assert_eq!(drag(&mut host, "tall", [None, Some(120.0)]), (210.0, 120.0));
    assert!(
        row(&host, "tall", StyleId::Height).contains("110"),
        "{}",
        row(&host, "tall", StyleId::Height)
    );
    // Past `max-height` the row holds what was dragged; layout bounds it.
    assert_eq!(drag(&mut host, "tall", [None, Some(400.0)]), (210.0, 210.0));
    // Below its first size, down to `min-height`: Chrome sets no other floor.
    assert_eq!(drag(&mut host, "tall", [None, Some(20.0)]), (210.0, 50.0));
    // `vertical` leaves the width alone.
    assert_eq!(
        drag(&mut host, "tall", [Some(300.0), Some(100.0)]),
        (210.0, 100.0)
    );
}

#[test]
fn a_dragged_height_replaces_the_content_height() {
    let mut host = boot();
    let before = host
        .runner()
        .kernel()
        .node(view(&host, "grown"))
        .unwrap()
        .frame;
    assert_eq!(
        before.height, 50.0,
        "an empty content-sized textarea is its min-height"
    );
    assert_eq!(
        drag(&mut host, "grown", [None, Some(150.0)]),
        (210.0, 150.0)
    );
}

#[test]
fn each_value_drags_its_own_axes() {
    let mut host = boot();
    // A border box is written as it is.
    assert_eq!(
        drag(&mut host, "both", [Some(260.0), Some(90.0)]),
        (260.0, 90.0)
    );
    assert!(row(&host, "both", StyleId::Width).contains("260"));
    assert_eq!(
        drag(&mut host, "wide", [Some(260.0), Some(90.0)]),
        (260.0, 70.0)
    );
    // No grip, no change.
    assert_eq!(
        drag(&mut host, "fixed", [Some(260.0), Some(90.0)]),
        (210.0, 70.0)
    );
}
