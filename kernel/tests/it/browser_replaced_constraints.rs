//! Chrome 151.0.7922.34, 2026-10-17: two replaced elements, directly in a
//! 400px flex row or each inside an automatic block item. Columns are 200px
//! tall with `align-items:flex-start`. Frames are the flex container, block
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
