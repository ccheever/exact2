//! Chrome 151.0.7922.34, 2026-10-17: two replaced elements, directly in a
//! 400px flex row or each inside an automatic block item. Columns are 200px
//! tall with `align-items:flex-start`; stretch rows are 100px tall and stretch
//! columns 500px tall. Frames are the flex container, block
//! items (when present), and leaves, relative to the flow-root (ids 2 onward).
//! Canvas and loaded images have a natural size of 300×150; video has no metadata.
use super::{lay_out_held, Frames};
use crate::browser_cases::mismatches;
use exact_kernel::NodeType;

#[test]
fn flexing_and_constraints_preserve_the_replaced_ratio_box() {
    let cases: &[(&str, &str, [Frames; 2])] = &[
        (
            "direct",
            "width:100%",
            [
                &[
                    [0.0, 0.0, 400.0, 150.0],
                    [0.0, 0.0, 300.0, 150.0],
                    [300.0, 0.0, 300.0, 150.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 150.0],
                    [0.0, 0.0, 300.0, 150.0],
                    [300.0, 0.0, 300.0, 150.0],
                ],
            ],
        ),
        (
            "direct",
            "width:100%;min-width:0",
            [
                &[
                    [0.0, 0.0, 400.0, 100.0],
                    [0.0, 0.0, 200.0, 100.0],
                    [200.0, 0.0, 200.0, 100.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 150.0],
                    [0.0, 0.0, 200.0, 150.0],
                    [200.0, 0.0, 200.0, 150.0],
                ],
            ],
        ),
        (
            "direct",
            "max-width:100%",
            [
                &[
                    [0.0, 0.0, 400.0, 150.0],
                    [0.0, 0.0, 300.0, 150.0],
                    [300.0, 0.0, 300.0, 150.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 150.0],
                    [0.0, 0.0, 300.0, 150.0],
                    [300.0, 0.0, 300.0, 150.0],
                ],
            ],
        ),
        (
            "row",
            "max-width:100%;padding:10px;box-sizing:border-box",
            [
                &[
                    [0.0, 0.0, 400.0, 110.0],
                    [0.0, 0.0, 200.0, 110.0],
                    [200.0, 0.0, 200.0, 110.0],
                    [0.0, 0.0, 200.0, 110.0],
                    [200.0, 0.0, 200.0, 110.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 170.0],
                    [0.0, 0.0, 200.0, 170.0],
                    [200.0, 0.0, 200.0, 170.0],
                    [0.0, 0.0, 200.0, 170.0],
                    [200.0, 0.0, 200.0, 170.0],
                ],
            ],
        ),
        (
            "row",
            "max-width:100%;padding:10px;box-sizing:content-box",
            [
                &[
                    [0.0, 0.0, 400.0, 120.0],
                    [0.0, 0.0, 200.0, 120.0],
                    [200.0, 0.0, 200.0, 120.0],
                    [0.0, 0.0, 220.0, 120.0],
                    [200.0, 0.0, 220.0, 120.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 170.0],
                    [0.0, 0.0, 200.0, 170.0],
                    [200.0, 0.0, 200.0, 170.0],
                    [0.0, 0.0, 220.0, 170.0],
                    [200.0, 0.0, 220.0, 170.0],
                ],
            ],
        ),
        (
            "column",
            "height:100%",
            [
                &[
                    [0.0, 0.0, 400.0, 200.0],
                    [0.0, 0.0, 300.0, 150.0],
                    [0.0, 150.0, 300.0, 150.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 200.0],
                    [0.0, 0.0, 300.0, 150.0],
                    [0.0, 150.0, 300.0, 150.0],
                ],
            ],
        ),
        (
            "column",
            "height:100%;min-height:0",
            [
                &[
                    [0.0, 0.0, 400.0, 200.0],
                    [0.0, 0.0, 200.0, 100.0],
                    [0.0, 100.0, 200.0, 100.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 200.0],
                    [0.0, 0.0, 300.0, 100.0],
                    [0.0, 100.0, 300.0, 100.0],
                ],
            ],
        ),
        (
            "direct",
            "width:100%;min-width:0;padding:10px;box-sizing:border-box",
            [
                &[
                    [0.0, 0.0, 400.0, 110.0],
                    [0.0, 0.0, 200.0, 110.0],
                    [200.0, 0.0, 200.0, 110.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 170.0],
                    [0.0, 0.0, 200.0, 170.0],
                    [200.0, 0.0, 200.0, 170.0],
                ],
            ],
        ),
        (
            "direct",
            "width:100%;min-width:0;padding:10px;box-sizing:border-box;aspect-ratio:2/1",
            [
                &[
                    [0.0, 0.0, 400.0, 100.0],
                    [0.0, 0.0, 200.0, 100.0],
                    [200.0, 0.0, 200.0, 100.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 100.0],
                    [0.0, 0.0, 200.0, 100.0],
                    [200.0, 0.0, 200.0, 100.0],
                ],
            ],
        ),
        (
            "direct",
            "width:100%;height:80px",
            [
                &[
                    [0.0, 0.0, 400.0, 80.0],
                    [0.0, 0.0, 200.0, 80.0],
                    [200.0, 0.0, 200.0, 80.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 80.0],
                    [0.0, 0.0, 300.0, 80.0],
                    [300.0, 0.0, 300.0, 80.0],
                ],
            ],
        ),
        (
            "direct",
            "width:100%;max-height:80px",
            [
                &[
                    [0.0, 0.0, 400.0, 80.0],
                    [0.0, 0.0, 200.0, 80.0],
                    [200.0, 0.0, 200.0, 80.0],
                ],
                &[
                    [0.0, 0.0, 400.0, 80.0],
                    [0.0, 0.0, 300.0, 80.0],
                    [300.0, 0.0, 300.0, 80.0],
                ],
            ],
        ),
    ];
    check(
        cases,
        &[
            ("canvas", NodeType::Canvas, 0),
            ("img", NodeType::Image, 0),
            ("video", NodeType::Video, 1),
        ],
    );
}

#[test]
fn authored_ratios_use_the_authored_box() {
    let cases: &[(&str, &str, [Frames; 1])] = &[
        (
            "row",
            "max-width:100%;padding:10px;box-sizing:border-box;aspect-ratio:2/1",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 200.0, 100.0],
                [200.0, 0.0, 200.0, 100.0],
                [0.0, 0.0, 200.0, 100.0],
                [200.0, 0.0, 200.0, 100.0],
            ]],
        ),
        (
            "row",
            "max-width:100%;padding:10px;box-sizing:content-box;aspect-ratio:2/1",
            [&[
                [0.0, 0.0, 400.0, 120.0],
                [0.0, 0.0, 200.0, 120.0],
                [200.0, 0.0, 200.0, 120.0],
                [0.0, 0.0, 220.0, 120.0],
                [200.0, 0.0, 220.0, 120.0],
            ]],
        ),
    ];
    check(
        cases,
        &[("canvas", NodeType::Canvas, 0), ("img", NodeType::Image, 0)],
    );
}

fn check<const N: usize>(cases: &[(&str, &str, [Frames; N])], tags: &[(&str, NodeType, usize)]) {
    let mut failures = Vec::new();
    for (layout, item, frames) in cases {
        for &(tag, kind, index) in tags {
            let k = lay_out_held(layout, item, kind, false);
            let want: Vec<_> = (2..).zip(frames[index].iter().copied()).collect();
            failures.extend(mismatches(&format!("{tag} {layout} {item}"), &k, &want));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

// Chrome 151.0.7922.34, 2026-10-17: percentage padding resolves against
// the 400px container even when the item's content minimum is measured.
#[test]
fn independent_content_minima_keep_percentage_padding() {
    let cases: &[(&str, &str, [Frames; 1])] = &[
        (
            "direct",
            "width:300px;padding:10%;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 230.0],
                [0.0, 0.0, 380.0, 230.0],
                [380.0, 0.0, 380.0, 230.0],
            ]],
        ),
        (
            "direct",
            "width:300px;padding:10%;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 190.0],
                [0.0, 0.0, 300.0, 190.0],
                [300.0, 0.0, 300.0, 190.0],
            ]],
        ),
        (
            "direct",
            "width:100%;padding:10%;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 230.0],
                [0.0, 0.0, 380.0, 230.0],
                [380.0, 0.0, 380.0, 230.0],
            ]],
        ),
        (
            "direct",
            "height:100px;padding:10%;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 180.0],
                [0.0, 0.0, 280.0, 180.0],
                [280.0, 0.0, 280.0, 180.0],
            ]],
        ),
        (
            "direct",
            "padding:10%;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 230.0],
                [0.0, 0.0, 380.0, 230.0],
                [380.0, 0.0, 380.0, 230.0],
            ]],
        ),
        (
            "column",
            "height:100%;padding:10%;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 200.0],
                [0.0, 0.0, 380.0, 230.0],
                [0.0, 230.0, 380.0, 230.0],
            ]],
        ),
    ];
    check(
        cases,
        &[("canvas", NodeType::Canvas, 0), ("img", NodeType::Image, 0)],
    );
}

// Chrome 151.0.7922.34, 2026-10-17: the shrink-to-fit absolute wrapper
// and its block leaf. SVGs have viewBox="0 0 100 100" and no natural size.
#[test]
fn zero_content_minima_transfer_through_the_ratio() {
    use exact_kernel::{Offer, Op, PropId};
    let cases: &[(NodeType, &str, Frames)] = &[
        (
            NodeType::Svg,
            "min-height:20px;padding:10px;box-sizing:content-box",
            &[
                [0.0, 0.0, 40.0, 40.0],
                [0.0, 0.0, 40.0, 40.0],
            ],
        ),
        (
            NodeType::Svg,
            "min-width:20px;padding:10px;box-sizing:content-box",
            &[
                [0.0, 0.0, 40.0, 40.0],
                [0.0, 0.0, 40.0, 40.0],
            ],
        ),
        (
            NodeType::Video,
            "min-height:20px;padding:10px;box-sizing:content-box;aspect-ratio:2/1",
            &[
                [0.0, 0.0, 60.0, 40.0],
                [0.0, 0.0, 60.0, 40.0],
            ],
        ),
        (
            NodeType::Svg,
            "min-height:40px;padding:10px;box-sizing:border-box",
            &[
                [0.0, 0.0, 40.0, 40.0],
                [0.0, 0.0, 40.0, 40.0],
            ],
        ),
        (
            NodeType::Svg,
            "min-height:20px;padding:10px;box-sizing:content-box;max-width:10px",
            &[
                [0.0, 0.0, 30.0, 40.0],
                [0.0, 0.0, 30.0, 40.0],
            ],
        ),
        (
            NodeType::Svg,
            "min-height:20px;padding:10px;border-width:2px;border-style:solid;box-sizing:content-box",
            &[
                [0.0, 0.0, 44.0, 44.0],
                [0.0, 0.0, 44.0, 44.0],
            ],
        ),
        (
            NodeType::Svg,
            "min-height:20px;padding:10px;box-sizing:content-box;aspect-ratio:2/1",
            &[
                [0.0, 0.0, 60.0, 40.0],
                [0.0, 0.0, 60.0, 40.0],
            ],
        ),
    ];
    let mut failures = Vec::new();
    for &(kind, item, frames) in cases {
        let mut k = lay_out_held("abs", item, kind, false);
        if kind == NodeType::Svg {
            k.apply(
                0,
                3,
                &[Op::SetProp {
                    id: 3,
                    prop: PropId::ViewBox,
                    value: "0 0 100 100".into(),
                }],
            )
            .unwrap();
            k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
        }
        let want: Vec<_> = (2..).zip(frames.iter().copied()).collect();
        failures.extend(mismatches(&format!("{kind:?} {item}"), &k, &want));
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

// Chrome 151.0.7922.34, 2026-10-18: padding and border stay in the box
// the ratio relates, including intrinsic floors and both flex axes.
#[test]
fn intrinsic_padding_floors_preserve_the_replaced_ratio() {
    let cases: &[(&str, &str, [Frames; 1])] = &[
        (
            "row",
            "max-width:100%;padding:10px;box-sizing:content-box;aspect-ratio:2/1",
            [&[
                [0.0, 0.0, 400.0, 20.0],
                [0.0, 0.0, 20.0, 20.0],
                [20.0, 0.0, 20.0, 20.0],
                [0.0, 0.0, 20.0, 20.0],
                [20.0, 0.0, 20.0, 20.0],
            ]],
        ),
        (
            "row",
            "max-width:100%;padding:10px;box-sizing:border-box;aspect-ratio:2/1",
            [&[
                [0.0, 0.0, 400.0, 20.0],
                [0.0, 0.0, 40.0, 20.0],
                [40.0, 0.0, 40.0, 20.0],
                [0.0, 0.0, 40.0, 20.0],
                [40.0, 0.0, 40.0, 20.0],
            ]],
        ),
        (
            "row",
            "max-width:100%;padding:5px;padding-left:15px;padding-right:15px;border-width:2px;border-style:solid;box-sizing:border-box;aspect-ratio:3/1",
            [&[
                [0.0, 0.0, 400.0, 14.0],
                [0.0, 0.0, 42.0, 14.0],
                [42.0, 0.0, 42.0, 14.0],
                [0.0, 0.0, 42.0, 14.0],
                [42.0, 0.0, 42.0, 14.0],
            ]],
        ),
    ];
    check(cases, &[("video", NodeType::Video, 0)]);
}
#[test]
fn flex_ratio_transfers_keep_padding_and_border() {
    let cases: &[(&str, &str, [Frames; 1])] = &[
        (
            "stretch-row",
            "padding:10px;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "direct",
            "max-height:50px;padding:10px;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 70.0],
                [0.0, 0.0, 120.0, 70.0],
                [120.0, 0.0, 120.0, 70.0],
            ]],
        ),
        (
            "stretch-column",
            "padding:10px;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 500.0],
                [0.0, 0.0, 400.0, 210.0],
                [0.0, 210.0, 400.0, 210.0],
            ]],
        ),
        (
            "column",
            "max-width:100px;padding:10px;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 200.0],
                [0.0, 0.0, 120.0, 70.0],
                [0.0, 70.0, 120.0, 70.0],
            ]],
        ),
        (
            "direct",
            "max-width:100px;padding:10px;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 70.0],
                [0.0, 0.0, 120.0, 70.0],
                [120.0, 0.0, 120.0, 70.0],
            ]],
        ),
        (
            "stretch-row",
            "padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "direct",
            "max-height:50px;padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 50.0],
                [0.0, 0.0, 80.0, 50.0],
                [80.0, 0.0, 80.0, 50.0],
            ]],
        ),
        (
            "stretch-column",
            "padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 500.0],
                [0.0, 0.0, 400.0, 210.0],
                [0.0, 210.0, 400.0, 210.0],
            ]],
        ),
        (
            "column",
            "max-width:100px;padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 200.0],
                [0.0, 0.0, 100.0, 60.0],
                [0.0, 60.0, 100.0, 60.0],
            ]],
        ),
        (
            "direct",
            "max-width:100px;padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 60.0],
                [0.0, 0.0, 100.0, 60.0],
                [100.0, 0.0, 100.0, 60.0],
            ]],
        ),
        (
            "stretch-row",
            "padding:5px;padding-left:15px;padding-right:15px;border-width:2px;border-style:solid;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 206.0, 100.0],
                [206.0, 0.0, 206.0, 100.0],
            ]],
        ),
        (
            "stretch-row",
            "padding:10px;box-sizing:border-box;aspect-ratio:2/1",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 200.0, 100.0],
                [200.0, 0.0, 200.0, 100.0],
            ]],
        ),
        (
            "stretch-row",
            "padding:10px;box-sizing:border-box;aspect-ratio:auto 3/1",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "direct",
            "max-height:50px;padding:5px;padding-left:15px;padding-right:15px;border-width:2px;border-style:solid;box-sizing:content-box",
            [&[
                [0.0, 0.0, 400.0, 64.0],
                [0.0, 0.0, 134.0, 64.0],
                [134.0, 0.0, 134.0, 64.0],
            ]],
        ),
        (
            "direct",
            "max-height:50px;padding:5px;padding-left:15px;padding-right:15px;border-width:2px;border-style:solid;box-sizing:border-box",
            [&[
                [0.0, 0.0, 400.0, 50.0],
                [0.0, 0.0, 106.0, 50.0],
                [106.0, 0.0, 106.0, 50.0],
            ]],
        ),
    ];
    check(cases, &[("canvas", NodeType::Canvas, 0)]);
}

// Chrome 151.0.7922.34, 2026-10-18: the border-box padding floor can
// exceed both a definite offer and the percentage default object width.
#[test]
fn unsized_ratio_padding_floors_survive_width_offers() {
    let cases: &[(&str, &str, [Frames; 1])] = &[
        (
            "narrow",
            "aspect-ratio:4/1;box-sizing:border-box;padding:10px",
            [&[[0.0, 0.0, 50.0, 20.0], [0.0, 0.0, 80.0, 20.0]]],
        ),
        (
            "abs",
            "width:100%;aspect-ratio:100/1;padding:10px;box-sizing:border-box",
            [&[[0.0, 0.0, 400.0, 20.0], [0.0, 0.0, 400.0, 20.0]]],
        ),
        (
            "wide-abs",
            "width:100%;aspect-ratio:100/1;padding:10px;box-sizing:border-box",
            [&[[0.0, 0.0, 2000.0, 20.0], [0.0, 0.0, 2000.0, 20.0]]],
        ),
    ];
    check(
        cases,
        &[
            ("video", NodeType::Video, 0),
            ("iframe", NodeType::WebView, 0),
            ("svg", NodeType::Svg, 0),
        ],
    );
}

// Chrome 151.0.7922.34, 2026-10-18: an absolute block holds the flex
// container; the canvases grow from their content flex bases.
#[test]
fn intrinsic_flex_ratio_transfers_keep_padding_and_border() {
    let cases: &[(&str, &str, [Frames; 1])] = &[
        (
            "intrinsic-row",
            "flex-grow:1;padding:10px;box-sizing:content-box",
            [&[
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "intrinsic-row",
            "flex-grow:1;padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "intrinsic-row",
            "padding:10px;box-sizing:border-box",
            [&[
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "intrinsic-row",
            "flex-grow:1;padding:10px;box-sizing:border-box;aspect-ratio:2/1",
            [&[
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 400.0, 100.0],
                [0.0, 0.0, 200.0, 100.0],
                [200.0, 0.0, 200.0, 100.0],
            ]],
        ),
        (
            "intrinsic-row",
            "flex-grow:1;padding:10px;box-sizing:border-box;aspect-ratio:auto 3/1",
            [&[
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 360.0, 100.0],
                [0.0, 0.0, 180.0, 100.0],
                [180.0, 0.0, 180.0, 100.0],
            ]],
        ),
        (
            "intrinsic-row",
            "flex-grow:1;padding:10px;padding-left:5px;padding-right:5px;border-width:2px;border-style:solid;box-sizing:content-box",
            [&[
                [0.0, 0.0, 332.0, 100.0],
                [0.0, 0.0, 332.0, 100.0],
                [0.0, 0.0, 166.0, 100.0],
                [166.0, 0.0, 166.0, 100.0],
            ]],
        ),
        (
            "intrinsic-column",
            "flex-grow:1;padding:2px;padding-left:15px;padding-right:15px;border-width:2px;border-style:solid;box-sizing:content-box",
            [&[
                [0.0, 0.0, 100.0, 82.0],
                [0.0, 0.0, 100.0, 82.0],
                [0.0, 0.0, 100.0, 41.0],
                [0.0, 41.0, 100.0, 41.0],
            ]],
        ),
    ];
    check(cases, &[("canvas", NodeType::Canvas, 0)]);
}

// Chrome 151.0.7922.34, 2026-10-18: an 800×600 positioned block holds
// an absolute flex row and two canvases. Only a definite height sizes the
// row's intrinsic width probe; a maximum alone leaves its width at 640px.
#[test]
fn absolute_flex_intrinsic_width_uses_the_clamped_definite_height() {
    use crate::browser_cases::{css_rows as rows, props};
    use exact_kernel::{Offer, Op};

    let natural: Frames = &[
        [0.0, 0.0, 360.0, 100.0],
        [0.0, 0.0, 180.0, 100.0],
        [180.0, 0.0, 180.0, 100.0],
    ];
    let border_box: Frames = &[
        [0.0, 0.0, 400.0, 100.0],
        [0.0, 0.0, 200.0, 100.0],
        [200.0, 0.0, 200.0, 100.0],
    ];
    let capped_auto: Frames = &[
        [0.0, 0.0, 640.0, 100.0],
        [0.0, 0.0, 320.0, 100.0],
        [320.0, 0.0, 320.0, 100.0],
    ];
    let mut cases = Vec::new();
    for height in [
        "height:100px",
        "top:0;bottom:500px",
        "height:200px;max-height:100px",
        "height:60px;min-height:100px",
        "height:60px;min-height:100px;max-height:80px",
        "top:0;bottom:400px;max-height:100px",
        "top:0;bottom:540px;min-height:100px",
        "height:50%;max-height:100px",
        "height:100px;top:0;bottom:0",
    ] {
        cases.extend([
            (height, "flex-grow:1;padding:10px", natural),
            (height, "flex-grow:0;padding:10px", natural),
            (
                height,
                "flex-grow:1;padding:10px;box-sizing:border-box;aspect-ratio:2/1",
                border_box,
            ),
        ]);
    }
    cases.extend([
        ("max-height:100px", "flex-grow:1;padding:10px", capped_auto),
        (
            "max-height:100px",
            "flex-grow:1;padding:10px;box-sizing:border-box;aspect-ratio:2/1",
            capped_auto,
        ),
    ]);
    let mut failures = Vec::new();
    for (height, item, frames) in cases {
        let mut k = lay_out_held("direct", item, NodeType::Canvas, false);
        k.apply(
            0,
            3,
            &[
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(props(&rows("position:relative;width:800px;height:600px"))),
                },
                Op::SetStyle {
                    id: 2,
                    patch: Box::new(props(&rows(&format!(
                        "position:absolute;display:flex;{height}"
                    )))),
                },
            ],
        )
        .unwrap();
        k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
        let want: Vec<_> = (2..).zip(frames.iter().copied()).collect();
        failures.extend(mismatches(&format!("{height}; {item}"), &k, &want));
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

// Chrome 151.0.7922.34, 2026-10-18: cross-axis auto margins and wrapping
// prevent a definite container height from sizing intrinsic contributions.
#[test]
fn intrinsic_flex_stretch_requires_a_single_line_and_non_auto_cross_margins() {
    let variants: &[(&str, &str, Frames)] = &[
        (
            "height:300px",
            "flex-grow:1;padding:10px",
            &[
                [0.0, 0.0, 1160.0, 300.0],
                [0.0, 0.0, 580.0, 300.0],
                [580.0, 0.0, 580.0, 300.0],
            ],
        ),
        (
            "height:300px",
            "flex-grow:1;padding:10px;margin-top:auto",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 130.0, 320.0, 170.0],
                [320.0, 130.0, 320.0, 170.0],
            ],
        ),
        (
            "height:300px",
            "flex-grow:1;padding:10px;margin-bottom:auto",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 0.0, 320.0, 170.0],
                [320.0, 0.0, 320.0, 170.0],
            ],
        ),
        (
            "height:300px",
            "flex-grow:1;padding:10px;margin-top:auto;margin-bottom:auto",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 65.0, 320.0, 170.0],
                [320.0, 65.0, 320.0, 170.0],
            ],
        ),
        (
            "height:300px;flex-wrap:wrap",
            "flex-grow:1;padding:10px",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 0.0, 320.0, 300.0],
                [320.0, 0.0, 320.0, 300.0],
            ],
        ),
        (
            "height:300px;flex-wrap:wrap-reverse",
            "flex-grow:1;padding:10px",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 0.0, 320.0, 300.0],
                [320.0, 0.0, 320.0, 300.0],
            ],
        ),
        (
            "height:300px",
            "flex-grow:1;padding:10px;align-self:flex-start",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 0.0, 320.0, 170.0],
                [320.0, 0.0, 320.0, 170.0],
            ],
        ),
        (
            "height:300px",
            "flex-grow:1;padding:10px;height:150px",
            &[
                [0.0, 0.0, 640.0, 300.0],
                [0.0, 0.0, 320.0, 170.0],
                [320.0, 0.0, 320.0, 170.0],
            ],
        ),
        (
            "height:300px",
            "flex-grow:1;padding:10px;margin-left:auto",
            &[
                [0.0, 0.0, 1160.0, 300.0],
                [0.0, 0.0, 580.0, 300.0],
                [580.0, 0.0, 580.0, 300.0],
            ],
        ),
    ];
    let cases: Vec<_> = ["absolute", "held"]
        .into_iter()
        .flat_map(|mode| {
            variants
                .iter()
                .map(move |&(row, item, frames)| (mode, row, item, frames))
        })
        .collect();
    check_intrinsic_flex_rows(&cases);
}

// Percentages and opposing insets resolve against the laid-out containing
// block even when its own height is automatic (Chrome 151.0.7922.34).
#[test]
fn absolute_flex_height_resolves_against_an_auto_height_containing_block() {
    let cases: &[(&str, &str, &str, Frames)] = &[
        (
            "auto-parent",
            "height:100%",
            "flex-grow:1;padding:10px",
            &[
                [0.0, 400.0, 1560.0, 400.0],
                [0.0, 400.0, 780.0, 400.0],
                [780.0, 400.0, 780.0, 400.0],
            ],
        ),
        (
            "auto-parent",
            "height:100%",
            "flex-grow:1;padding:10px;min-width:0",
            &[
                [0.0, 400.0, 1560.0, 400.0],
                [0.0, 400.0, 780.0, 400.0],
                [780.0, 400.0, 780.0, 400.0],
            ],
        ),
        (
            "auto-parent",
            "height:calc(100% - 100px)",
            "flex-grow:1;padding:10px",
            &[
                [0.0, 400.0, 1160.0, 300.0],
                [0.0, 400.0, 580.0, 300.0],
                [580.0, 400.0, 580.0, 300.0],
            ],
        ),
        (
            "auto-parent",
            "height:calc(100% - 100px)",
            "flex-grow:1;padding:10px;min-width:0",
            &[
                [0.0, 400.0, 1160.0, 300.0],
                [0.0, 400.0, 580.0, 300.0],
                [580.0, 400.0, 580.0, 300.0],
            ],
        ),
        (
            "auto-parent",
            "top:0;bottom:0",
            "flex-grow:1;padding:10px",
            &[
                [0.0, 0.0, 1560.0, 400.0],
                [0.0, 0.0, 780.0, 400.0],
                [780.0, 0.0, 780.0, 400.0],
            ],
        ),
        (
            "auto-parent",
            "top:0;bottom:0",
            "flex-grow:1;padding:10px;min-width:0",
            &[
                [0.0, 0.0, 1560.0, 400.0],
                [0.0, 0.0, 780.0, 400.0],
                [780.0, 0.0, 780.0, 400.0],
            ],
        ),
    ];
    check_intrinsic_flex_rows(cases);
}

fn check_intrinsic_flex_rows(cases: &[(&str, &str, &str, Frames)]) {
    use crate::browser_cases::{css_rows as rows, lay_out_as, props};
    use exact_kernel::{Offer, Op};
    let mut failures = Vec::new();
    for &(mode, container, item, frames) in cases {
        let root = props(&rows("position:relative;width:800px;height:600px"));
        let (parent, mut nodes) = match mode {
            "held" => (2, vec![(2, 1, rows("position:absolute"))]),
            "auto-parent" => (
                2,
                vec![
                    (2, 1, rows("position:relative;width:800px")),
                    (6, 2, rows("height:400px")),
                ],
            ),
            _ => (1, vec![]),
        };
        let position = if mode == "held" {
            ""
        } else {
            "position:absolute;"
        };
        nodes.push((
            3,
            parent,
            rows(&format!("{position}display:flex;{container}")),
        ));
        let mut k = lay_out_as(root, nodes, &[], &[], NodeType::View);
        let mut ops = Vec::new();
        for id in [4, 5] {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::Canvas,
            });
            ops.push(Op::SetStyle {
                id,
                patch: Box::new(props(&rows(item))),
            });
        }
        ops.push(Op::SetChildren {
            id: 3,
            children: vec![4, 5],
        });
        k.apply(0, 2, &ops).unwrap();
        k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
        let want: Vec<_> = (3..).zip(frames.iter().copied()).collect();
        failures.extend(mismatches(
            &format!("{mode}; {container}; {item}"),
            &k,
            &want,
        ));
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}
