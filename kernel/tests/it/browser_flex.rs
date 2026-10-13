//! `flex-grow` (LLP 1053 G3) and CSS `direction` against literal Chrome.
//!
//! Chrome 154, 2026-09-25, by the method of `browser_cases.rs`. `flex: <n>`
//! is written as the three rows Contract lowers it to, in authored order, so
//! a later `flex-grow` row replaces the shorthand's, as the later
//! declaration wins in CSS.
use crate::browser_cases::{css_rows, lay_out_with, mismatches, props, Rows};
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

/// A flex column whose height is automatic is as tall as its items, each
/// clamped by its minimum and maximum (CSS Flexbox §9.2 step 4, §9.9.1),
/// whatever space it is offered (issue X86, vendored Taffy patch 33). A
/// leaf's flex base size is its whole content, and the column took it past
/// the leaf's `max-height`. In definite space it did whatever the leaf: C,
/// a row of definite height's unstretched item (the issue's case C), and D,
/// absolutely positioned. Under a max-content offer (R, a row of automatic
/// height's item, and H, a block's child) it did when the leaf scrolls
/// (`overflow: hidden`) or may not shrink (S, `flex-shrink: 0`). E, F and G
/// held before. W: a row's min-content width, where an unbroken word at
/// `max-width: 130px` made a fit-content row as wide as the word. Chrome
/// 155, 2026-10-12, plain HTML by the method of `browser_cases.rs`; `prose`
/// is the issue's 1,220 characters, and every size asserted is one a clamp
/// sets, not the font.
#[test]
fn a_column_is_as_tall_as_its_items_clamped_by_max_height() {
    let prose = "The quick brown fox jumps over the lazy dog. "
        .repeat(28)
        .chars()
        .take(1220)
        .collect::<String>();
    let prose = prose.trim_end();
    let word = "x".repeat(200);
    let row = "display:flex;align-items:flex-start;width:400px";
    let col = "display:flex;flex-direction:column;width:200px";
    // A row's column holding one text leaf styled `leaf`.
    let in_row = |leaf: &str| {
        vec![
            (2, 1, css_rows(row)),
            (3, 2, css_rows(col)),
            (4, 3, css_rows(leaf)),
        ]
    };
    // The same in a row 300 tall: the column is measured in definite space.
    let in_tall_row = |leaf: &str| {
        vec![
            (2, 1, css_rows(&format!("{row};height:300px"))),
            (3, 2, css_rows(col)),
            (4, 3, css_rows(leaf)),
        ]
    };
    // An absolutely positioned column in a 300-tall box.
    let absolute = |leaf: &str| {
        vec![
            (2, 1, css_rows("width:200px;height:300px;position:relative")),
            (
                3,
                2,
                css_rows(&format!("{col};position:absolute;top:0px;left:0px")),
            ),
            (4, 3, css_rows(leaf)),
        ]
    };
    // A column in a 300-tall block, offered a max-content height.
    let in_block = |leaf: &str| {
        vec![
            (2, 1, css_rows("width:200px;height:300px")),
            (3, 2, css_rows("display:flex;flex-direction:column")),
            (4, 3, css_rows(leaf)),
        ]
    };
    let clipped = "max-height:160px;overflow:hidden";
    let under_padding = "max-height:10px;padding:20px;box-sizing:border-box;overflow:hidden";
    // A name, the nodes, the text leaves, the images and Chrome's frames.
    type Case<'a> = (
        &'a str,
        Vec<(u32, u32, Rows)>,
        Vec<u32>,
        Images<'a>,
        Frames<'a>,
    );
    let cases: Vec<Case> = vec![
        (
            "R: a text leaf at max-height",
            in_row(clipped),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "R: without overflow",
            in_row("max-height:160px"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "R: min-height wins over max-height",
            in_row("min-height:200px;max-height:160px;overflow:hidden"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 200.0]), (4, [0.0, 0.0, 200.0, 200.0])],
        ),
        (
            "R: margins outside the clamp",
            in_row("max-height:160px;overflow:hidden;margin-top:10px;margin-bottom:4px"),
            vec![4],
            &[],
            &[
                (3, [0.0, 0.0, 200.0, 174.0]),
                (4, [0.0, 10.0, 200.0, 160.0]),
            ],
        ),
        (
            "R: a negative margin",
            in_row("max-height:160px;overflow:hidden;margin-top:-20px"),
            vec![4],
            &[],
            &[
                (3, [0.0, 0.0, 200.0, 140.0]),
                (4, [0.0, -20.0, 200.0, 160.0]),
            ],
        ),
        (
            "R: content-box padding",
            in_row("max-height:160px;overflow:hidden;padding:10px"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 180.0]), (4, [0.0, 0.0, 200.0, 180.0])],
        ),
        (
            "R: border-box padding",
            in_row("max-height:160px;overflow:hidden;padding:10px;box-sizing:border-box"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "R: a border-box max-height under its padding",
            in_row(under_padding),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 40.0]), (4, [0.0, 0.0, 200.0, 40.0])],
        ),
        (
            "R: a definite height",
            in_row("height:10px"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 10.0]), (4, [0.0, 0.0, 200.0, 10.0])],
        ),
        (
            "R: two leaves and a gap",
            vec![
                (2, 1, css_rows(row)),
                (3, 2, css_rows(&format!("{col};gap:6px"))),
                (4, 3, css_rows("max-height:50px;overflow:hidden")),
                (5, 3, css_rows("max-height:70px;overflow:hidden")),
            ],
            vec![4, 5],
            &[],
            &[
                (3, [0.0, 0.0, 200.0, 126.0]),
                (4, [0.0, 0.0, 200.0, 50.0]),
                (5, [0.0, 56.0, 200.0, 70.0]),
            ],
        ),
        (
            "R: an image, max-width through its ratio",
            in_row("max-width:100px"),
            vec![],
            &[(4, Some((200.0, 656.0)))],
            &[(3, [0.0, 0.0, 200.0, 328.0]), (4, [0.0, 0.0, 100.0, 328.0])],
        ),
        (
            "R: an image at max-height",
            in_row("max-height:130px"),
            vec![],
            &[(4, Some((200.0, 656.0)))],
            &[(3, [0.0, 0.0, 200.0, 130.0]), (4, [0.0, 0.0, 200.0, 130.0])],
        ),
        (
            "C: a row 300 tall, a text leaf at max-height",
            in_tall_row("max-height:160px"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "C: a row 300 tall, the leaf clipped",
            in_tall_row(clipped),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "C: a row 300 tall, a margin outside a max-height under its padding",
            in_tall_row(&format!("{under_padding};margin-top:10px")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 50.0]), (4, [0.0, 10.0, 200.0, 40.0])],
        ),
        (
            "D: an absolutely positioned column",
            absolute(clipped),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "D: a margin outside a max-height under its padding",
            absolute(&format!("{under_padding};margin-top:10px")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 50.0]), (4, [0.0, 10.0, 200.0, 40.0])],
        ),
        (
            "D: a negative margin past a max-height under its padding",
            absolute(&format!("{under_padding};margin-top:-20px")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 20.0]), (4, [0.0, -20.0, 200.0, 40.0])],
        ),
        (
            "S: flex-shrink: 0, the leaf clipped",
            in_row(&format!("flex-shrink:0;{clipped}")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "S: flex-shrink: 0 without overflow",
            in_row("flex-shrink:0;max-height:160px"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "S: flex-shrink: 0 in a block",
            in_block(&format!("flex-shrink:0;{clipped}")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "S: flex-shrink: 0 in a block without overflow",
            in_block("flex-shrink:0;max-height:160px"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "S: flex-shrink: 0, a height past max-height",
            in_row(&format!("flex-shrink:0;height:300px;{clipped}")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "S: flex-shrink: 0, min-height wins over max-height",
            in_row("flex-shrink:0;min-height:200px;max-height:160px;overflow:hidden"),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 200.0]), (4, [0.0, 0.0, 200.0, 200.0])],
        ),
        (
            "S: flex-shrink: 0, a margin outside the clamp",
            in_row(&format!("flex-shrink:0;{clipped};margin-top:10px")),
            vec![4],
            &[],
            &[
                (3, [0.0, 0.0, 200.0, 170.0]),
                (4, [0.0, 10.0, 200.0, 160.0]),
            ],
        ),
        (
            "S: flex-shrink: 0 and flex-grow: 1",
            in_row(&format!("flex-shrink:0;flex-grow:1;{clipped}")),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "E: a block",
            vec![(2, 1, css_rows("width:200px")), (3, 2, css_rows(clipped))],
            vec![3],
            &[],
            &[(2, [0.0, 0.0, 200.0, 160.0]), (3, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "F: a row (the cross axis)",
            vec![
                (2, 1, css_rows("display:flex;width:200px")),
                (3, 2, css_rows(clipped)),
            ],
            vec![3],
            &[],
            &[(2, [0.0, 0.0, 200.0, 160.0]), (3, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "G: a column holding a box",
            vec![
                (2, 1, css_rows(row)),
                (3, 2, css_rows(col)),
                (
                    4,
                    3,
                    css_rows(&format!("display:flex;flex-direction:column;{clipped}")),
                ),
                (5, 4, vec![]),
            ],
            vec![5],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "H: a column in a 300-tall block",
            in_block(clipped),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 160.0]), (4, [0.0, 0.0, 200.0, 160.0])],
        ),
        (
            "H: a border-box max-height under its padding",
            in_block(under_padding),
            vec![4],
            &[],
            &[(3, [0.0, 0.0, 200.0, 40.0]), (4, [0.0, 0.0, 200.0, 40.0])],
        ),
    ];
    let mut failures = Vec::new();
    for (name, nodes, text_ids, images, want) in cases {
        let texts: Vec<(u32, &str)> = text_ids.iter().map(|id| (*id, prose)).collect();
        let k = lay_out_with(props(&css_rows("width:800px")), nodes, &texts, images);
        failures.extend(mismatches(name, &k, want));
    }
    for (wrap, leaf) in [
        ("wrap", "max-width:130px;overflow:hidden"),
        ("nowrap", "max-width:130px;overflow:hidden"),
        ("wrap", "max-width:130px"),
        ("nowrap", "max-width:130px"),
        ("nowrap", "flex-shrink:0;max-width:130px;overflow:hidden"),
        ("nowrap", "flex-shrink:0;max-width:130px"),
    ] {
        let nodes = vec![
            (
                2,
                1,
                css_rows("display:flex;flex-direction:column;align-items:flex-start;width:400px"),
            ),
            (3, 2, css_rows(&format!("display:flex;flex-wrap:{wrap}"))),
            (4, 3, css_rows(leaf)),
        ];
        let k = lay_out_with(
            props(&css_rows("width:800px")),
            nodes,
            &[(4, word.as_str())],
            &[],
        );
        failures.extend(mismatches(
            &format!("W: flex-wrap: {wrap}, {leaf}"),
            &k,
            &[(3, [0.0, 0.0, 130.0, 18.0]), (4, [0.0, 0.0, 130.0, 18.0])],
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
