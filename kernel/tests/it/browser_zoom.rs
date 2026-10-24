//! CSS `zoom` on an `iframe` (LLP 1020.002 D6), against Chrome 155 headless
//! (2026-10-21): a frame's own absolute lengths grow by the zoom, and a
//! frame sized by its container keeps its size. The kernel zooms the box;
//! the host zooms the guest's page.
use crate::browser_cases::{lay_out_as, props};
use crate::support::reader::number as n;
use exact_kernel::NodeType;
use exact_kernel::StyleId::*;
use exact_kernel::StyleValue;

fn frame(kind: NodeType, rows: Vec<(exact_kernel::StyleId, StyleValue<'static>)>) -> [f32; 2] {
    let root = props(&vec![(Width, n(400.0))]);
    let k = lay_out_as(root, vec![(2, 1, rows)], &[], &[], kind);
    let f = k.node(2).unwrap().frame;
    [f.width, f.height]
}

#[test]
fn zoom_grows_an_iframes_own_lengths_and_not_a_percentage() {
    // `zoom: 1.5; width: 400px`: the box is 600 px (Chrome).
    let [w, _] = frame(NodeType::WebView, vec![(Zoom, n(1.5)), (Width, n(400.0))]);
    assert_eq!(w, 600.0);
    // `zoom: 1.5; width: 100%` of 400 px: the box stays 400 px (Chrome).
    let [w, _] = frame(
        NodeType::WebView,
        vec![(Zoom, n(1.5)), (Width, StyleValue::Percent(100.0))],
    );
    assert_eq!(w, 400.0);
    // The default object size is in CSS pixels, zoomed with the rest.
    assert_eq!(
        frame(NodeType::WebView, vec![(Zoom, n(2.0))]),
        [600.0, 300.0]
    );
    // Padding is the element's own length too.
    let [w, _] = frame(
        NodeType::WebView,
        vec![(Zoom, n(2.0)), (Width, n(100.0)), (PaddingLeft, n(10.0))],
    );
    assert_eq!(w, 220.0);
    // Admitted on an iframe alone: anything else ignores the row.
    let [w, _] = frame(NodeType::View, vec![(Zoom, n(1.5)), (Width, n(100.0))]);
    assert_eq!(w, 100.0);
}
