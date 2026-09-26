//! `flex-grow` (LLP 1053 G3) and CSS `direction` against literal Chrome.
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

/// `direction: rtl` (inherited) runs a flex row from the right, reverses
/// `row-reverse` to the left, puts `flex-start` at the right edge and a
/// block child against the right; physical margins and padding keep their
/// sides. An `ltr` descendant restores the order. (The case root stays
/// `ltr`, as the browser's body is: a root that is itself `rtl` sits against
/// the right of the viewport, which CSS also does.)
#[test]
fn direction_rtl_orders_flex_rows_and_places_blocks() {
    run(|case| {
        case(
            "rtl flex row runs right to left",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Display, t("flex")), (Direction, t("rtl"))]),
                (3, 2, vec![(Width, n(50.0)), (Height, n(10.0))]),
                (4, 2, vec![(Width, n(60.0)), (Height, n(10.0))]),
                (5, 2, vec![(Width, n(70.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 400.0, 10.0]),
                (3, [350.0, 0.0, 50.0, 10.0]),
                (4, [290.0, 0.0, 60.0, 10.0]),
                (5, [220.0, 0.0, 70.0, 10.0]),
            ],
        );
        case(
            "rtl row-reverse runs left to right",
            vec![(Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("flex")),
                        (Direction, t("rtl")),
                        (FlexDirection, t("row-reverse")),
                    ],
                ),
                (3, 2, vec![(Width, n(50.0)), (Height, n(10.0))]),
                (4, 2, vec![(Width, n(60.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 400.0, 10.0]),
                (3, [0.0, 0.0, 50.0, 10.0]),
                (4, [50.0, 0.0, 60.0, 10.0]),
            ],
        );
        case(
            "rtl justify-content: flex-start is the right edge",
            vec![(Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("flex")),
                        (Direction, t("rtl")),
                        (JustifyContent, t("flex-start")),
                        (ColumnGap, n(10.0)),
                    ],
                ),
                (3, 2, vec![(Width, n(50.0)), (Height, n(10.0))]),
                (4, 2, vec![(Width, n(60.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 400.0, 10.0]),
                (3, [350.0, 0.0, 50.0, 10.0]),
                (4, [280.0, 0.0, 60.0, 10.0]),
            ],
        );
        case(
            "rtl inherits into a nested flex row",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Direction, t("rtl"))]),
                (3, 2, vec![(Display, t("flex"))]),
                (4, 3, vec![(Width, n(50.0)), (Height, n(10.0))]),
                (5, 3, vec![(Width, n(60.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 400.0, 10.0]),
                (3, [0.0, 0.0, 400.0, 10.0]),
                (4, [350.0, 0.0, 50.0, 10.0]),
                (5, [290.0, 0.0, 60.0, 10.0]),
            ],
        );
        case(
            "ltr inside rtl restores the order",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Direction, t("rtl"))]),
                (3, 2, vec![(Display, t("flex")), (Direction, t("ltr"))]),
                (4, 3, vec![(Width, n(50.0)), (Height, n(10.0))]),
                (5, 3, vec![(Width, n(60.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 400.0, 10.0]),
                (3, [0.0, 0.0, 400.0, 10.0]),
                (4, [0.0, 0.0, 50.0, 10.0]),
                (5, [50.0, 0.0, 60.0, 10.0]),
            ],
        );
        case(
            "rtl block child sits at the right",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Direction, t("rtl"))]),
                (3, 2, vec![(Width, n(100.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 10.0]), (3, [300.0, 0.0, 100.0, 10.0])],
        );
        case(
            "rtl margin-left and margin-right keep their sides",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Display, t("flex")), (Direction, t("rtl"))]),
                (
                    3,
                    2,
                    vec![
                        (Width, n(50.0)),
                        (Height, n(10.0)),
                        (MarginLeft, n(5.0)),
                        (MarginRight, n(20.0)),
                    ],
                ),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 10.0]), (3, [330.0, 0.0, 50.0, 10.0])],
        );
        case(
            "rtl padding-left and padding-right keep their sides",
            vec![(Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("flex")),
                        (Direction, t("rtl")),
                        (PaddingLeft, n(7.0)),
                        (PaddingRight, n(30.0)),
                    ],
                ),
                (3, 2, vec![(Width, n(50.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 10.0]), (3, [320.0, 0.0, 50.0, 10.0])],
        );
    });
}
