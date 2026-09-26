//! `flex-grow` (LLP 1053 G3) against literal Chrome.
//!
//! Chrome 154, 2026-09-25, by the method of `browser_cases.rs`. `flex: <n>`
//! is written as the three rows Contract lowers it to, in authored order, so
//! a later `flex-grow` row replaces the shorthand's, as the later
//! declaration wins in CSS.
use crate::browser_cases::{lay_out_with, mismatches, props, Rows};
use crate::support::reader::{number as n, text as t};
use exact_kernel::StyleId::*;
use exact_kernel::StyleValue;

type Frames<'a> = &'a [(u32, [f32; 4])];
type Images<'a> = &'a [(u32, Option<(f32, f32)>)];

fn run(
    cases: impl FnOnce(
        &mut dyn FnMut(&str, Rows, Vec<(u32, u32, Rows)>, &[(u32, &str)], Images, Frames),
    ),
) {
    let mut failures = Vec::new();
    cases(&mut |name, root, nodes, texts, images, want| {
        let k = lay_out_with(props(&root), nodes, texts, images);
        failures.extend(mismatches(name, &k, want));
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `flex-grow: 1` leaves `flex-basis: auto`, so an item's content keeps its
/// width before the free space is shared; `flex: 1` is `1 1 0%`. Whichever
/// is declared later sets the grow factor.
#[test]
fn flex_grow_is_the_longhand_not_flex_1() {
    run(|case| {
        case(
            "flex-grow: 1 keeps flex-basis auto; flex: 1 is basis 0%",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![
                (2, 1, vec![(FlexGrow, n(1.0))]),
                (4, 2, vec![(Width, n(100.0)), (Height, n(10.0))]),
                (
                    3,
                    1,
                    vec![
                        (FlexGrow, n(1.0)),
                        (FlexShrink, n(1.0)),
                        (FlexBasis, StyleValue::Percent(0.0)),
                    ],
                ),
                (5, 3, vec![(Width, n(100.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 250.0, 10.0]), (3, [250.0, 0.0, 150.0, 10.0])],
        );
        case(
            "flex: 1 then flex-grow: 3 (the longhand wins)",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (FlexGrow, n(1.0)),
                        (FlexShrink, n(1.0)),
                        (FlexBasis, StyleValue::Percent(0.0)),
                        (FlexGrow, n(3.0)),
                    ],
                ),
                (
                    3,
                    1,
                    vec![
                        (FlexGrow, n(1.0)),
                        (FlexShrink, n(1.0)),
                        (FlexBasis, StyleValue::Percent(0.0)),
                    ],
                ),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 300.0, 0.0]), (3, [300.0, 0.0, 100.0, 0.0])],
        );
        case(
            "flex-grow: 2 then flex: 1 (the shorthand resets it)",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (FlexGrow, n(2.0)),
                        (FlexGrow, n(1.0)),
                        (FlexShrink, n(1.0)),
                        (FlexBasis, StyleValue::Percent(0.0)),
                    ],
                ),
                (
                    3,
                    1,
                    vec![
                        (FlexGrow, n(1.0)),
                        (FlexShrink, n(1.0)),
                        (FlexBasis, StyleValue::Percent(0.0)),
                    ],
                ),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 0.0]), (3, [200.0, 0.0, 200.0, 0.0])],
        );
        case(
            "flex-grow with an explicit flex-basis",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![
                (2, 1, vec![(FlexGrow, n(1.0)), (FlexBasis, n(50.0))]),
                (3, 1, vec![(Width, n(50.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 350.0, 0.0]), (3, [350.0, 0.0, 50.0, 0.0])],
        );
    });
}
