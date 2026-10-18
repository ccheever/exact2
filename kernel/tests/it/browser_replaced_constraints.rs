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
