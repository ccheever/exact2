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

/// CSS `order` (feed F19): flex and grid items lay out in order-modified
/// document order, equal orders in document order; a block container
/// ignores it. A changed `order` moves its item at the next layout.
#[test]
fn order_lays_items_out_in_order_modified_document_order() {
    let items = |orders: [f64; 3]| -> Vec<(u32, u32, crate::browser_cases::Rows)> {
        (0..3)
            .map(|i| {
                (
                    2 + i as u32,
                    1,
                    vec![(Width, n(100.0)), (Height, n(10.0)), (Order, n(orders[i]))],
                )
            })
            .collect()
    };
    run(|case| {
        case(
            "a flex row: -1, then 0, then 1",
            vec![(Display, t("flex")), (Width, n(300.0))],
            items([1.0, -1.0, 0.0]),
            &[],
            &[],
            &[
                (3, [0.0, 0.0, 100.0, 10.0]),
                (4, [100.0, 0.0, 100.0, 10.0]),
                (2, [200.0, 0.0, 100.0, 10.0]),
            ],
        );
        case(
            "a grid's auto-placement, equal orders in document order",
            vec![
                (Display, t("grid")),
                (Width, n(300.0)),
                (GridTemplateColumns, t("100px 100px 100px")),
            ],
            items([2.0, 1.0, 1.0]),
            &[],
            &[],
            &[
                (3, [0.0, 0.0, 100.0, 10.0]),
                (4, [100.0, 0.0, 100.0, 10.0]),
                (2, [200.0, 0.0, 100.0, 10.0]),
            ],
        );
        case(
            "a block ignores order",
            vec![(Width, n(300.0))],
            items([1.0, -1.0, 0.0]),
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 100.0, 10.0]),
                (3, [0.0, 10.0, 100.0, 10.0]),
                (4, [0.0, 20.0, 100.0, 10.0]),
            ],
        );
    });
    let mut k = lay_out_with(
        props(&vec![(Display, t("flex")), (Width, n(300.0))]),
        items([0.0, 0.0, 0.0]),
        &[],
        &[],
    );
    let x = |k: &exact_kernel::Kernel, id| k.node(id).unwrap().frame.x;
    assert_eq!([x(&k, 2), x(&k, 3), x(&k, 4)], [0.0, 100.0, 200.0]);
    k.apply(
        1,
        2,
        &[exact_kernel::Op::SetStyle {
            id: 4,
            patch: Box::new(props(&vec![(Order, n(-1.0))])),
        }],
    )
    .unwrap();
    k.compute_layout(1, exact_kernel::Offer::definite(800.0, 600.0))
        .unwrap();
    assert_eq!([x(&k, 4), x(&k, 2), x(&k, 3)], [0.0, 100.0, 200.0]);
    // The container turns block: its children stack in document order.
    k.apply(
        2,
        3,
        &[exact_kernel::Op::SetStyle {
            id: 1,
            patch: Box::new(props(&vec![(Display, t("block"))])),
        }],
    )
    .unwrap();
    k.compute_layout(1, exact_kernel::Offer::definite(800.0, 600.0))
        .unwrap();
    let y = |id| k.node(id).unwrap().frame.y;
    assert_eq!([y(2), y(3), y(4)], [0.0, 10.0, 20.0]);
}

/// A flex container in a column that does not stretch it is sized at
/// fit-content, and its main size (height) is found at that width (CSS
/// Flexbox §9.2 3E). Measured under the available width alone, a wrapping
/// row took its items' summed bases, one line, and kept that line's height
/// after it was laid out at the narrower fit-content width: a Signal Clone
/// bubble's second line of text spilled out of it (2026-10-05). Chrome 154,
/// 2026-10-05, through the web host (Contract's own CSS).
#[test]
fn a_fit_content_flex_item_in_a_column_is_as_tall_as_its_wrapped_lines() {
    let column = |align: &str| {
        vec![
            (Display, t("flex")),
            (FlexDirection, t("column")),
            (AlignItems, t(align)),
            (Width, n(200.0)),
        ]
    };
    let boxed = |w: f64, h: f64| vec![(Width, n(w)), (Height, n(h))];
    run(|case| {
        case(
            "align-items: flex-start, a wrapping row of two 120s",
            column("flex-start"),
            vec![
                (2, 1, vec![(Display, t("flex")), (FlexWrap, t("wrap"))]),
                (3, 2, boxed(120.0, 10.0)),
                (4, 2, boxed(120.0, 10.0)),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 200.0, 20.0]),
                (3, [0.0, 0.0, 120.0, 10.0]),
                (4, [0.0, 10.0, 120.0, 10.0]),
            ],
        );
        case(
            "align-items: flex-start, a row holding a wrapping row of two 120s",
            column("flex-start"),
            vec![
                (2, 1, vec![(Display, t("flex"))]),
                (3, 2, vec![(Display, t("flex")), (FlexWrap, t("wrap"))]),
                (4, 3, boxed(120.0, 10.0)),
                (5, 3, boxed(120.0, 10.0)),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 200.0, 20.0]),
                (3, [0.0, 0.0, 200.0, 20.0]),
                (5, [0.0, 10.0, 120.0, 10.0]),
            ],
        );
        case(
            "a fit-content width under max-width: 0 keeps its padding (two 0-wide items, one line)",
            column("flex-start"),
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("flex")),
                        (FlexWrap, t("wrap")),
                        (MaxWidth, n(0.0)),
                        (PaddingLeft, n(10.0)),
                        (PaddingRight, n(10.0)),
                        (BoxSizing, t("border-box")),
                    ],
                ),
                (3, 2, boxed(0.0, 10.0)),
                (4, 2, boxed(0.0, 10.0)),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 20.0, 10.0]),
                (3, [10.0, 0.0, 0.0, 10.0]),
                (4, [10.0, 0.0, 0.0, 10.0]),
            ],
        );
        case(
            "intrinsic sizes inverted by a negative margin: fit-content is the min-content 100",
            {
                let mut rows = column("flex-start");
                rows[3] = (Width, n(80.0));
                rows
            },
            vec![
                (2, 1, vec![(Display, t("flex")), (FlexWrap, t("wrap"))]),
                (
                    3,
                    2,
                    vec![(Width, n(80.0)), (Height, n(10.0)), (FlexShrink, n(0.0))],
                ),
                (
                    4,
                    2,
                    vec![(Width, n(20.0)), (Height, n(10.0)), (FlexShrink, n(0.0))],
                ),
                (
                    5,
                    2,
                    vec![
                        (Width, n(10.0)),
                        (Height, n(10.0)),
                        (FlexShrink, n(0.0)),
                        (MarginLeft, n(-150.0)),
                    ],
                ),
                (
                    6,
                    2,
                    vec![(Width, n(100.0)), (Height, n(10.0)), (FlexShrink, n(0.0))],
                ),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 100.0, 10.0]),
                (5, [-50.0, 0.0, 10.0, 10.0]),
                (6, [-40.0, 0.0, 100.0, 10.0]),
            ],
        );
        case(
            "align-items: center, the same row",
            column("center"),
            vec![
                (2, 1, vec![(Display, t("flex")), (FlexWrap, t("wrap"))]),
                (3, 2, boxed(120.0, 10.0)),
                (4, 2, boxed(120.0, 10.0)),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 20.0]), (4, [0.0, 10.0, 120.0, 10.0])],
        );
        case(
            "the bubble's shape: a growing row and a footer, wrapping to the end",
            column("flex-start"),
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("flex")),
                        (FlexWrap, t("wrap")),
                        (ColumnGap, n(6.0)),
                        (JustifyContent, t("flex-end")),
                    ],
                ),
                (
                    3,
                    2,
                    vec![(Display, t("flex")), (FlexGrow, n(1.0)), (MinWidth, n(0.0))],
                ),
                (5, 3, boxed(150.0, 10.0)),
                (4, 2, boxed(80.0, 16.0)),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 200.0, 26.0]),
                (3, [0.0, 0.0, 200.0, 10.0]),
                (4, [120.0, 10.0, 80.0, 16.0]),
            ],
        );
    });
}

/// The same with text, against CSS's rule rather than a browser (the test
/// measurer's monospace is no browser font): a paragraph 290 wide in one
/// line wraps to two in the 200-wide fit-content row, and the row, and the
/// column it sits in, are two lines tall.
#[test]
fn a_fit_content_row_is_as_tall_as_its_wrapped_text() {
    for kid in ["flex", "block"] {
        for align in ["flex-start", "center", "stretch"] {
            let k = lay_out_with(
                props(&vec![
                    (Display, t("flex")),
                    (FlexDirection, t("column")),
                    (AlignItems, t(align)),
                    (Width, n(200.0)),
                ]),
                vec![(2, 1, vec![(Display, t(kid))]), (3, 2, vec![])],
                &[(3, "aaaa bbbb cccc dddd eeee ffff")],
                &[],
            );
            for id in [2, 3] {
                let f = k.node(id).unwrap().frame;
                assert_eq!(
                    [f.width, f.height],
                    [200.0, 36.0],
                    "{kid} in align-items {align}: #{id}"
                );
            }
        }
    }
}
