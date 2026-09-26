//! `aspect-ratio` (LLP 1053 G1) against literal Chrome.
//!
//! Chrome 154, 2026-09-25, by the method of `browser_cases.rs`: each case is
//! plain HTML/CSS in a `display: flow-root` box 400px wide (the kernel root,
//! id 1), descendants `box-sizing: content-box` with `font: 16px/18px
//! monospace`; an image is `<img style="display:block">` with an SVG source
//! whose natural size is 100×50, or no source while it has none. Text cases
//! use one word per line, so the measurer's advance does not matter.
use crate::browser_cases::{lay_out_with, mismatches, props, Rows};
use crate::support::reader::{number as n, text as t};
use exact_kernel::StyleId::*;

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

/// One definite dimension gives the other; two definite ones ignore the
/// ratio; min/max clamp each used size, and transfer through the ratio only
/// into an automatic dimension (CSS Box Sizing 4 §5.1–5.2).
#[test]
fn a_non_replaced_box_sizes_through_its_ratio() {
    run(|case| {
        case(
            "width + ratio",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "width + ratio 16 / 9",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("16 / 9"))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 112.5])],
        );
        case(
            "width + ratio 16/9",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("16/9"))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 112.5])],
        );
        case(
            "auto width stretches, ratio gives height",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 200.0])],
        );
        case(
            "height + ratio gives width",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Height, n(100.0)), (AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "both definite ignore the ratio",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![(Width, n(200.0)), (Height, n(50.0)), (AspectRatio, n(2.0))],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 50.0])],
        );
        case(
            "max-height clamps the ratio height",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(200.0)),
                    (AspectRatio, n(1.0)),
                    (MaxHeight, n(50.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 50.0])],
        );
        case(
            "min-height raises the ratio height",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(200.0)),
                    (AspectRatio, n(2.0)),
                    (MinHeight, n(150.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 150.0])],
        );
        case(
            "max-width: ratio from the used width",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(300.0)),
                    (MaxWidth, n(200.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "min-width: ratio from the used width",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(100.0)),
                    (MinWidth, n(200.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "max-height on a stretched block",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(AspectRatio, n(1.0)), (MaxHeight, n(100.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 100.0, 100.0])],
        );
        case(
            "max-width transfers to an auto width from height",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Height, n(100.0)),
                    (AspectRatio, n(4.0)),
                    (MaxWidth, n(200.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "min-height transfers to width (height auto, width auto)",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(AspectRatio, n(2.0)), (MinHeight, n(300.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 600.0, 300.0])],
        );
    });
}

/// A text leaf with a ratio is not replaced: its automatic minimum height is
/// its content (§5.2), so taller text grows the box unless `min-height: 0`
/// or a scroll container turns that off; a max still caps it. Taffy's leaf
/// patch 5 had applied replaced-element constraints to any leaf with a ratio.
#[test]
fn a_non_replaced_leaf_is_not_a_replaced_element() {
    run(|case| {
        case(
            "text taller than the ratio grows",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(20.0)), (AspectRatio, n(1.0))])],
            &[(2, "a b c d e")],
            &[],
            &[(2, [0.0, 0.0, 20.0, 90.0])],
        );
        case(
            "text taller, min-height 0 keeps the ratio",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![(Width, n(20.0)), (AspectRatio, n(1.0)), (MinHeight, n(0.0))],
            )],
            &[(2, "a b c d e")],
            &[],
            &[(2, [0.0, 0.0, 20.0, 20.0])],
        );
        case(
            "text taller, overflow hidden keeps the ratio",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(20.0)),
                    (AspectRatio, n(1.0)),
                    (OverflowX, t("hidden")),
                    (OverflowY, t("hidden")),
                ],
            )],
            &[(2, "a b c d e")],
            &[],
            &[(2, [0.0, 0.0, 20.0, 20.0])],
        );
        case(
            "text taller, max-height caps growth",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(20.0)),
                    (AspectRatio, n(1.0)),
                    (MaxHeight, n(40.0)),
                ],
            )],
            &[(2, "a b c d e")],
            &[],
            &[(2, [0.0, 0.0, 20.0, 40.0])],
        );
        case(
            "short text keeps the ratio",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, n(2.0))])],
            &[(2, "Hi")],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "text leaf with height gives width",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Height, n(36.0)), (AspectRatio, n(5.0))])],
            &[(2, "a b")],
            &[],
            &[(2, [0.0, 0.0, 180.0, 36.0])],
        );
        case(
            "flex row: grow item with a text leaf",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![(2, 1, vec![(FlexGrow, n(1.0)), (AspectRatio, n(4.0))])],
            &[(2, "Hi")],
            &[],
            &[(2, [0.0, 0.0, 400.0, 100.0])],
        );
    });
}

/// A box with children: the ratio's height is a floor its content can pass.
#[test]
fn a_container_grows_past_its_ratio_for_content() {
    run(|case| {
        case(
            "container with a child keeps the ratio",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Width, n(200.0)), (AspectRatio, n(2.0))]),
                (3, 2, vec![(Height, n(20.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0]), (3, [0.0, 0.0, 200.0, 20.0])],
        );
        case(
            "container with a taller child grows",
            vec![(Width, n(400.0))],
            vec![
                (2, 1, vec![(Width, n(200.0)), (AspectRatio, n(2.0))]),
                (3, 2, vec![(Height, n(150.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 150.0]), (3, [0.0, 0.0, 200.0, 150.0])],
        );
        case(
            "container border-box auto 2/1 with child",
            vec![(Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (BoxSizing, t("border-box")),
                        (Width, n(200.0)),
                        (PaddingTop, n(10.0)),
                        (PaddingRight, n(10.0)),
                        (PaddingBottom, n(10.0)),
                        (PaddingLeft, n(10.0)),
                        (AspectRatio, t("auto 2/1")),
                    ],
                ),
                (3, 2, vec![(Height, n(20.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 200.0, 110.0]),
                (3, [10.0, 10.0, 180.0, 20.0]),
            ],
        );
        case(
            "flex container with ratio",
            vec![(Width, n(400.0))],
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("flex")),
                        (Width, n(200.0)),
                        (AspectRatio, n(2.0)),
                    ],
                ),
                (3, 2, vec![(Width, n(20.0))]),
            ],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0]), (3, [0.0, 0.0, 20.0, 100.0])],
        );
    });
}

/// `<ratio>` sizes the box `box-sizing` names; `auto <ratio>` always the
/// content box. A degenerate ratio behaves as `auto`.
#[test]
fn the_ratio_box_follows_box_sizing_unless_auto() {
    run(|case| {
        case(
            "content-box padding: ratio of the content box",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(200.0)),
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 220.0, 120.0])],
        );
        case(
            "border-box padding: ratio of the border box",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (BoxSizing, t("border-box")),
                    (Width, n(200.0)),
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "content-box border",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(200.0)),
                    (BorderWidthTop, n(5.0)),
                    (BorderStyleTop, t("solid")),
                    (BorderWidthRight, n(5.0)),
                    (BorderStyleRight, t("solid")),
                    (BorderWidthBottom, n(5.0)),
                    (BorderStyleBottom, t("solid")),
                    (BorderWidthLeft, n(5.0)),
                    (BorderStyleLeft, t("solid")),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 210.0, 110.0])],
        );
        case(
            "border-box border",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (BoxSizing, t("border-box")),
                    (Width, n(200.0)),
                    (BorderWidthTop, n(5.0)),
                    (BorderStyleTop, t("solid")),
                    (BorderWidthRight, n(5.0)),
                    (BorderStyleRight, t("solid")),
                    (BorderWidthBottom, n(5.0)),
                    (BorderStyleBottom, t("solid")),
                    (BorderWidthLeft, n(5.0)),
                    (BorderStyleLeft, t("solid")),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "border-box, auto 2/1: ratio of the content box",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (BoxSizing, t("border-box")),
                    (Width, n(200.0)),
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, t("auto 2/1")),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 110.0])],
        );
        case(
            "content-box, auto 2/1",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(200.0)),
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, t("auto 2/1")),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 220.0, 120.0])],
        );
        case(
            "degenerate 0 behaves as auto",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, n(0.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 0.0])],
        );
        case(
            "degenerate 1/0 behaves as auto",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("1/0"))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 0.0])],
        );
        case(
            "stretched block, content-box padding",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 210.0])],
        );
        case(
            "stretched block, border-box padding",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (BoxSizing, t("border-box")),
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 200.0])],
        );
        case(
            "stretched block, border-box auto 2/1",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (BoxSizing, t("border-box")),
                    (PaddingTop, n(10.0)),
                    (PaddingRight, n(10.0)),
                    (PaddingBottom, n(10.0)),
                    (PaddingLeft, n(10.0)),
                    (AspectRatio, t("auto 2/1")),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 210.0])],
        );
    });
}

/// Flex row and column items: a ratio yields an automatic cross size, and
/// `stretch` gives way to it unless the container's cross size is definite.
#[test]
fn flex_items_size_through_their_ratio() {
    run(|case| {
        case(
            "flex row: stretch yields to the ratio",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(100.0)), (AspectRatio, n(1.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 100.0, 100.0])],
        );
        case(
            "flex row definite height: item keeps the ratio",
            vec![(Display, t("flex")), (Width, n(400.0)), (Height, n(200.0))],
            vec![(2, 1, vec![(Width, n(100.0)), (AspectRatio, n(1.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 100.0, 200.0])],
        );
        case(
            "flex row: grown width gives the height",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![(2, 1, vec![(FlexGrow, n(1.0)), (AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 200.0])],
        );
        case(
            "flex row: flex-basis gives the height",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (FlexBasis, n(100.0)),
                    (FlexShrink, n(0.0)),
                    (AspectRatio, n(2.0)),
                ],
            )],
            &[],
            &[],
            &[(2, [0.0, 0.0, 100.0, 50.0])],
        );
        case(
            "flex column: height gives the width",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (Width, n(400.0)),
                (AlignItems, t("flex-start")),
            ],
            vec![(2, 1, vec![(Height, n(100.0)), (AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "flex column: stretched width gives the height",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (Width, n(400.0)),
            ],
            vec![(2, 1, vec![(AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 200.0])],
        );
        case(
            "flex column: max-height clamps",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (Width, n(400.0)),
            ],
            vec![(2, 1, vec![(AspectRatio, n(2.0)), (MaxHeight, n(100.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 100.0])],
        );
        case(
            "flex row: two growing items",
            vec![(Display, t("flex")), (Width, n(400.0))],
            vec![
                (2, 1, vec![(FlexGrow, n(1.0)), (AspectRatio, n(2.0))]),
                (3, 1, vec![(FlexGrow, n(1.0)), (Height, n(10.0))]),
            ],
            &[],
            &[],
            &[
                (2, [0.0, 0.0, 200.0, 100.0]),
                (3, [200.0, 0.0, 200.0, 10.0]),
            ],
        );
        case(
            "flex row: ratio item with width",
            vec![
                (Display, t("flex")),
                (Width, n(400.0)),
                (AlignItems, t("flex-start")),
            ],
            vec![(2, 1, vec![(Width, n(100.0)), (AspectRatio, n(2.0))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 100.0, 50.0])],
        );
        case(
            "flex column: auto 4/3 box",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (Width, n(400.0)),
            ],
            vec![(2, 1, vec![(AspectRatio, t("auto 4/3"))])],
            &[],
            &[],
            &[(2, [0.0, 0.0, 400.0, 300.0])],
        );
    });
}

/// `auto <ratio>`: an image's natural ratio once it is known, the ratio
/// before; a plain `<ratio>` overrides the natural one; `auto` or a
/// degenerate ratio keeps it. One given dimension is clamped on its own, as
/// is the other derived from it; CSS 2.1 §10.4's table is for neither given.
#[test]
fn an_image_prefers_its_natural_ratio_under_auto() {
    run(|case| {
        case(
            "image loaded, no ratio",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0))])],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "image loaded, auto 4/3: natural wins",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("auto 4/3"))])],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "image unloaded, auto 4/3: the ratio",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("auto 4/3"))])],
            &[],
            &[(2, None)],
            &[(2, [0.0, 0.0, 200.0, 150.0])],
        );
        case(
            "image loaded, 4/3 overrides",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("4/3"))])],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 150.0])],
        );
        case(
            "image loaded, auto",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("auto"))])],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "image loaded, degenerate 0",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, n(0.0))])],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "image unloaded, no ratio",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0))])],
            &[],
            &[(2, None)],
            &[(2, [0.0, 0.0, 200.0, 0.0])],
        );
        case(
            "image loaded, 4/3 auto (either order)",
            vec![(Width, n(400.0))],
            vec![(2, 1, vec![(Width, n(200.0)), (AspectRatio, t("4/3 auto"))])],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 100.0])],
        );
        case(
            "image loaded, auto 4/3, max-height",
            vec![(Width, n(400.0))],
            vec![(
                2,
                1,
                vec![
                    (Width, n(200.0)),
                    (AspectRatio, t("auto 4/3")),
                    (MaxHeight, n(50.0)),
                ],
            )],
            &[],
            &[(2, Some((100.0, 50.0)))],
            &[(2, [0.0, 0.0, 200.0, 50.0])],
        );
        case(
            "image in flex column, auto 4/3 unloaded",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (Width, n(400.0)),
            ],
            vec![(2, 1, vec![(AspectRatio, t("auto 4/3"))])],
            &[],
            &[(2, None)],
            &[(2, [0.0, 0.0, 400.0, 300.0])],
        );
        case(
            "img height only",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (AlignItems, t("flex-start")),
                (Width, n(390.0)),
            ],
            vec![(2, 1, vec![(Height, n(30.0))])],
            &[],
            &[(2, Some((320.0, 120.0)))],
            &[(2, [0.0, 0.0, 80.0, 30.0])],
        );
        case(
            "img max-width",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (AlignItems, t("flex-start")),
                (Width, n(390.0)),
            ],
            vec![(2, 1, vec![(MaxWidth, n(100.0))])],
            &[],
            &[(2, Some((320.0, 120.0)))],
            &[(2, [0.0, 0.0, 100.0, 37.5])],
        );
        case(
            "img min-width",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (AlignItems, t("flex-start")),
                (Width, n(390.0)),
            ],
            vec![(2, 1, vec![(MinWidth, n(400.0))])],
            &[],
            &[(2, Some((320.0, 120.0)))],
            &[(2, [0.0, 0.0, 400.0, 150.0])],
        );
        case(
            "img width then max-height",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (AlignItems, t("flex-start")),
                (Width, n(390.0)),
            ],
            vec![(2, 1, vec![(Width, n(96.0)), (MaxHeight, n(20.0))])],
            &[],
            &[(2, Some((320.0, 120.0)))],
            &[(2, [0.0, 0.0, 96.0, 20.0])],
        );
        case(
            "img stretched column",
            vec![
                (Display, t("flex")),
                (FlexDirection, t("column")),
                (Width, n(390.0)),
            ],
            vec![(2, 1, vec![])],
            &[],
            &[(2, Some((320.0, 120.0)))],
            &[(2, [0.0, 0.0, 390.0, 146.25])],
        );
    });
}
