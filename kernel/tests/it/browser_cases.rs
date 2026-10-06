//! Literal-Chrome layout cases from the 2026-09-22 kernel review.
//!
//! Chrome 153, 2026-09-23: each case is plain HTML/CSS (not Exact lowering),
//! measured with `getBoundingClientRect` relative to the case's outer box,
//! whose descendants are `box-sizing: content-box` and inherit
//! `font: 16px/18px`. The outer box is the kernel root, id 1.
use crate::support::reader::{kernel, number as n, text as t};
use exact_kernel::StyleId::*;
use exact_kernel::{
    GridTrack, GridTracks, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue,
};
use std::collections::BTreeMap;

pub(crate) type Rows = Vec<(StyleId, StyleValue)>;

pub(crate) fn props(rows: &Rows) -> StyleProps {
    let mut p = StyleProps::default();
    for (row, value) in rows {
        p.set_dynamic(*row, value).unwrap();
    }
    p
}

/// CSS declarations as kernel rows: px and bare numbers are numbers, the
/// rest (percentages, ratios, keywords) text.
pub(crate) fn css_rows(css: &str) -> Rows {
    let mut out = Vec::new();
    for decl in css.split(';').filter(|d| !d.is_empty()) {
        let (name, value) = decl.split_once(':').unwrap();
        let px = value.strip_suffix("px").unwrap_or(value).parse::<f64>();
        let percent = value.strip_suffix('%').map(str::parse::<f64>);
        // Keywords, and `line-height` in pixels (a bare number multiplies).
        let text = matches!(
            name,
            "aspect-ratio" | "column-fill" | "break-before" | "break-after" | "break-inside"
        );
        let v = match (px, percent, value) {
            (Ok(_), _, _) if name == "line-height" => t(value),
            (Ok(x), _, _) => n(x),
            (_, _, "auto") if !text => StyleValue::Auto,
            (_, Some(Ok(x)), _) if name != "width" && name != "height" => StyleValue::Percent(x),
            _ => t(value),
        };
        let ids: &[StyleId] = match name {
            "width" => &[Width],
            "height" => &[Height],
            "min-width" => &[MinWidth],
            "max-width" => &[MaxWidth],
            "min-height" => &[MinHeight],
            "max-height" => &[MaxHeight],
            "aspect-ratio" => &[AspectRatio],
            "display" => &[Display],
            "box-sizing" => &[BoxSizing],
            "appearance" => &[Appearance],
            "direction" => &[Direction],
            "flex-direction" => &[FlexDirection],
            "flex-wrap" => &[FlexWrap],
            "align-items" => &[AlignItems],
            "align-content" => &[AlignContent],
            "justify-items" => &[JustifyItems],
            "justify-content" => &[JustifyContent],
            "align-self" => &[AlignSelf],
            "flex-grow" => &[FlexGrow],
            "flex-shrink" => &[FlexShrink],
            "overflow" => &[OverflowX, OverflowY],
            "padding" => &[PaddingTop, PaddingRight, PaddingBottom, PaddingLeft],
            "padding-left" => &[PaddingLeft],
            "padding-top" => &[PaddingTop],
            "margin-left" => &[MarginLeft],
            "margin-right" => &[MarginRight],
            "margin-top" => &[MarginTop],
            "margin-bottom" => &[MarginBottom],
            "border-width" => &[
                BorderWidthTop,
                BorderWidthRight,
                BorderWidthBottom,
                BorderWidthLeft,
            ],
            "border-style" => &[
                BorderStyleTop,
                BorderStyleRight,
                BorderStyleBottom,
                BorderStyleLeft,
            ],
            "padding-right" => &[PaddingRight],
            "padding-bottom" => &[PaddingBottom],
            "opacity" => &[Opacity],
            "z-index" => &[ZIndex],
            // What makes a containing block in a browser and is not a row these
            // cases give the kernel (LLP 1074 T1: the compiler lowers `position:
            // relative` onto such a box).
            "translate" | "scale" | "rotate" | "transform" | "filter" | "backdrop-filter"
            | "isolation" => &[],
            "position" => &[PositionType],
            // LLP 1093: multi-column layout and the text its cases flow.
            "column-count" => &[ColumnCount],
            "column-width" => &[ColumnWidth],
            "column-gap" => &[ColumnGap],
            "column-fill" => &[ColumnFill],
            "widows" => &[Widows],
            "orphans" => &[Orphans],
            "break-before" => &[BreakBefore],
            "break-after" => &[BreakAfter],
            "break-inside" => &[BreakInside],
            "gap" => &[RowGap, ColumnGap],
            "white-space" => &[WhiteSpace],
            "line-height" => &[LineHeight],
            "font-size" => &[FontSize],
            "background-color" => &[BackgroundColor],
            "left" => &[Left],
            "right" => &[Right],
            "top" => &[Top],
            "bottom" => &[Bottom],
            _ => panic!("no row for {name}"),
        };
        out.extend(ids.iter().map(|id| (*id, v.clone())));
    }
    out
}

/// Lays out one case: the outer box and its descendants, each
/// `(id, parent, rows)`; a node given text is a text node.
fn lay_out(root: StyleProps, nodes: Vec<(u32, u32, Rows)>, texts: &[(u32, &str)]) -> Kernel {
    lay_out_with(root, nodes, texts, &[])
}

/// [`lay_out`], where a node in `images` is an `Image` whose natural size
/// is known (`Some`) or not yet (`None`, an `<img>` before it loads).
pub(crate) fn lay_out_with(
    root: StyleProps,
    nodes: Vec<(u32, u32, Rows)>,
    texts: &[(u32, &str)],
    images: &[(u32, Option<(f32, f32)>)],
) -> Kernel {
    lay_out_as(root, nodes, texts, images, NodeType::View)
}

/// [`lay_out_with`], where a node neither text nor an image is a `kind`.
pub(crate) fn lay_out_as(
    root: StyleProps,
    nodes: Vec<(u32, u32, Rows)>,
    texts: &[(u32, &str)],
    images: &[(u32, Option<(f32, f32)>)],
    kind: NodeType,
) -> Kernel {
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
    ];
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (id, parent, rows) in &nodes {
        let text = texts.iter().find(|(t, _)| t == id);
        ops.push(Op::CreateView {
            id: *id,
            node_type: if text.is_some() {
                NodeType::Text
            } else if images.iter().any(|(i, _)| i == id) {
                NodeType::Image
            } else {
                kind
            },
        });
        let mut rows = rows.clone();
        rows.extend([(FontSize, n(16.0)), (LineHeight, t("18px"))]);
        ops.push(Op::SetStyle {
            id: *id,
            patch: Box::new(props(&rows)),
        });
        if let Some((_, text)) = text {
            ops.push(Op::SetProp {
                id: *id,
                prop: PropId::Text,
                value: (*text).into(),
            });
        }
        children.entry(*parent).or_default().push(*id);
    }
    for (id, list) in children {
        ops.push(Op::SetChildren { id, children: list });
    }
    ops.push(Op::AttachRoot { id: 1 });
    let mut k = kernel();
    k.apply(0, 1, &ops).unwrap();
    for (id, size) in images {
        k.set_intrinsic_size(*id, *size).unwrap();
    }
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    k
}

/// Every mismatch of `[x, y, width, height]`, named by case and node.
pub(crate) fn mismatches(case: &str, k: &Kernel, expected: &[(u32, [f32; 4])]) -> Vec<String> {
    let mut out = Vec::new();
    for (id, want) in expected {
        let f = k.node(*id).unwrap().frame;
        let got = [f.x, f.y, f.width, f.height];
        if got
            .iter()
            .zip(want)
            .any(|(g, w)| !g.is_finite() || !w.is_finite() || (g - w).abs() > 0.01)
        {
            out.push(format!("{case} #{id}: kernel {got:?}, Chrome {want:?}"));
        }
    }
    out
}

fn empty(parent: u32, ids: &[u32]) -> Vec<(u32, u32, Rows)> {
    ids.iter().map(|id| (*id, parent, vec![])).collect()
}

/// A 400px-wide `display: grid` with these rows and track lists.
fn grid(rows: Rows, columns: &[GridTrack], track_rows: &[GridTrack]) -> StyleProps {
    let mut p = props(&[vec![(Display, t("grid")), (Width, n(400.0))], rows].concat());
    if !columns.is_empty() {
        p.rare.grid_template_columns = GridTracks::from_tracks(columns.to_vec());
        p.mask.set(GridTemplateColumns);
    }
    if !track_rows.is_empty() {
        p.rare.grid_template_rows = GridTracks::from_tracks(track_rows.to_vec());
        p.mask.set(GridTemplateRows);
    }
    p
}

/// A 400px-wide `display: flex` with these rows.
fn flex(rows: Rows) -> StyleProps {
    props(&[vec![(Display, t("flex")), (Width, n(400.0))], rows].concat())
}

/// CSS Box Alignment §5.1: the initial `normal` behaves as `stretch` in a
/// grid (auto tracks share the free space, items fill their areas) and as
/// `flex-start`/`stretch` in a flex container. The schema could not say
/// `normal`, so its `flex-start` default left grid auto tracks at content
/// size: `auto auto` columns were 0 wide and a bare grid's implicit column
/// was as wide as its text.
#[test]
fn normal_alignment_matches_chrome_in_grid_and_flex() {
    use GridTrack::{Auto, Fr, Points};
    let mut failures = Vec::new();
    let mut case = |name: &str, root, nodes, texts: &[(u32, &str)], want: &[_]| {
        failures.extend(mismatches(name, &lay_out(root, nodes, texts), want));
    };
    let halves = [(2, [0.0, 0.0, 200.0, 0.0]), (3, [200.0, 0.0, 200.0, 0.0])];
    case(
        "auto auto",
        grid(vec![], &[Auto, Auto], &[]),
        empty(1, &[2, 3]),
        &[],
        &halves,
    );
    case(
        "auto auto, justify-content: normal",
        grid(vec![(JustifyContent, t("normal"))], &[Auto, Auto], &[]),
        empty(1, &[2, 3]),
        &[],
        &halves,
    );
    case(
        "bare grid",
        grid(vec![], &[], &[]),
        empty(1, &[2]),
        &[(2, "Hello")],
        &[(2, [0.0, 0.0, 400.0, 18.0])],
    );
    case(
        "auto auto, justify-content: start",
        grid(vec![(JustifyContent, t("flex-start"))], &[Auto, Auto], &[]),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0; 4]), (3, [0.0; 4])],
    );
    case(
        "auto rows",
        grid(vec![(Height, n(300.0))], &[], &[Auto, Auto]),
        empty(1, &[2, 3]),
        &[],
        &[
            (2, [0.0, 0.0, 400.0, 150.0]),
            (3, [0.0, 150.0, 400.0, 150.0]),
        ],
    );
    case(
        "auto rows, align-content: start",
        grid(
            vec![(Height, n(200.0)), (AlignContent, t("flex-start"))],
            &[],
            &[Auto, Auto],
        ),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0, 0.0, 400.0, 0.0]), (3, [0.0, 0.0, 400.0, 0.0])],
    );
    case(
        "fixed tracks do not stretch",
        grid(vec![], &[Points(100.0), Points(100.0)], &[]),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0, 0.0, 100.0, 0.0]), (3, [100.0, 0.0, 100.0, 0.0])],
    );
    case(
        "a sized item starts in its stretched column",
        grid(vec![], &[], &[]),
        vec![(2, 1, vec![(Width, n(100.0)), (Height, n(10.0))])],
        &[],
        &[(2, [0.0, 0.0, 100.0, 10.0])],
    );
    case(
        "fr takes the free space before auto stretches",
        grid(vec![], &[Auto, Fr(1.0)], &[]),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0; 4]), (3, [0.0, 0.0, 400.0, 0.0])],
    );
    case(
        "grid items: normal",
        grid(
            vec![
                (Height, n(200.0)),
                (JustifyItems, t("normal")),
                (AlignItems, t("normal")),
            ],
            &[],
            &[],
        ),
        empty(1, &[2]),
        &[],
        &[(2, [0.0, 0.0, 400.0, 200.0])],
    );
    case(
        "flex: justify-content normal",
        flex(vec![(JustifyContent, t("normal"))]),
        vec![(2, 1, vec![(Width, n(100.0)), (Height, n(10.0))])],
        &[],
        &[(2, [0.0, 0.0, 100.0, 10.0])],
    );
    case(
        "flex: align-content normal",
        flex(vec![
            (Height, n(100.0)),
            (FlexWrap, t("wrap")),
            (AlignContent, t("normal")),
        ]),
        vec![
            (2, 1, vec![(Width, n(300.0)), (Height, n(10.0))]),
            (3, 1, vec![(Width, n(300.0)), (Height, n(10.0))]),
        ],
        &[],
        &[(2, [0.0, 0.0, 300.0, 10.0]), (3, [0.0, 50.0, 300.0, 10.0])],
    );
    case(
        "flex: align-items normal",
        flex(vec![(Height, n(100.0)), (AlignItems, t("normal"))]),
        vec![(2, 1, vec![(Width, n(100.0))])],
        &[],
        &[(2, [0.0, 0.0, 100.0, 100.0])],
    );
    // In a `display: flow-root` case box: `normal` lets the child's top
    // margin collapse through its block parent; any other value makes the
    // parent an independent formatting context that keeps it.
    let nested = |align: &str| {
        vec![
            (2, 1, vec![(Width, n(400.0)), (AlignContent, t(align))]),
            (3, 2, vec![(MarginTop, n(20.0)), (Height, n(10.0))]),
        ]
    };
    case(
        "block: align-content normal",
        props(&vec![]),
        nested("normal"),
        &[],
        &[(2, [0.0, 20.0, 400.0, 10.0]), (3, [0.0, 20.0, 400.0, 10.0])],
    );
    case(
        "block: align-content start",
        props(&vec![]),
        nested("flex-start"),
        &[],
        &[(2, [0.0, 0.0, 400.0, 30.0]), (3, [0.0, 20.0, 400.0, 10.0])],
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn physical_justify_items_uses_inherited_direction() {
    for (value, x) in [("left", 0.0), ("right", 80.0)] {
        let k = lay_out(
            props(&vec![(Width, n(200.0)), (Direction, t("rtl"))]),
            vec![
                (
                    2,
                    1,
                    vec![
                        (Display, t("grid")),
                        (Width, n(100.0)),
                        (JustifyItems, t(value)),
                    ],
                ),
                (3, 2, vec![(Width, n(20.0)), (Height, n(10.0))]),
            ],
            &[],
        );
        let grid = k.node(2).unwrap().frame;
        let item = k.node(3).unwrap().frame;
        assert_eq!(
            [item.x - grid.x, item.y - grid.y, item.width, item.height],
            [x, 0.0, 20.0, 10.0],
            "inherited rtl justify-items: {value}"
        );
    }
}

/// CSS Box Model §4 (CSS 2.1 §8.4): a percentage padding or border width
/// refers to the containing block's width on every side. Taffy's block
/// algorithm resolved vertical sides against the parent's height when it
/// sized a child (vendor/taffy patch 10), so `padding-top: 5%` under an
/// auto-height parent was zero there, and the `padding-top: 56.25%` embed
/// idiom collapsed to its `height: 0`.
#[test]
fn percentage_padding_resolves_against_the_containing_block_width() {
    let pct = StyleValue::Percent;
    let mut failures = Vec::new();
    let mut case = |name: &str, root: Rows, nodes, want: &[_]| {
        failures.extend(mismatches(name, &lay_out(props(&root), nodes, &[]), want));
    };
    let parent = |height: Option<f32>| {
        let mut rows = vec![(Width, n(400.0))];
        rows.extend(height.map(|h| (Height, n(h as f64))));
        rows
    };
    case(
        "padding-top: 5% under an auto height",
        parent(None),
        vec![(2, 1, vec![(Height, n(50.0)), (PaddingTop, pct(5.0))])],
        &[(2, [0.0, 0.0, 400.0, 70.0])],
    );
    // The embed idiom: a zero-height box whose padding is the frame, and an
    // absolutely positioned child that fills it.
    let embed = || {
        vec![
            (
                2,
                1,
                vec![
                    (PositionType, t("relative")),
                    (Height, n(0.0)),
                    (PaddingTop, pct(56.25)),
                ],
            ),
            (
                3,
                2,
                vec![
                    (PositionType, t("absolute")),
                    (Top, n(0.0)),
                    (Left, n(0.0)),
                    (Width, pct(100.0)),
                    (Height, pct(100.0)),
                ],
            ),
        ]
    };
    let filled = [(2, [0.0, 0.0, 400.0, 225.0]), (3, [0.0, 0.0, 400.0, 225.0])];
    case("the 56.25% embed", parent(None), embed(), &filled);
    case(
        "the 56.25% embed in a definite height",
        parent(Some(300.0)),
        embed(),
        &filled,
    );
    case(
        "an auto height holds its percentage padding before what follows",
        parent(Some(300.0)),
        vec![
            (2, 1, vec![(PaddingTop, pct(10.0))]),
            (3, 2, vec![(Height, n(20.0))]),
            (4, 1, vec![(Height, n(10.0))]),
        ],
        &[
            (2, [0.0, 0.0, 400.0, 60.0]),
            (3, [0.0, 40.0, 400.0, 20.0]),
            (4, [0.0, 60.0, 400.0, 10.0]),
        ],
    );
    case(
        "a definite parent height is not the basis",
        parent(Some(300.0)),
        vec![(
            2,
            1,
            vec![
                (Height, n(10.0)),
                (PaddingTop, pct(5.0)),
                (PaddingBottom, pct(10.0)),
            ],
        )],
        &[(2, [0.0, 0.0, 400.0, 70.0])],
    );
    case(
        "content sits inside both percentage sides",
        parent(Some(300.0)),
        vec![
            (
                2,
                1,
                vec![
                    (Height, n(10.0)),
                    (PaddingLeft, pct(10.0)),
                    (PaddingTop, pct(10.0)),
                ],
            ),
            (3, 2, vec![(Height, n(5.0))]),
        ],
        &[(2, [0.0, 0.0, 400.0, 50.0]), (3, [40.0, 40.0, 360.0, 5.0])],
    );
    case(
        "border-box keeps its height",
        parent(None),
        vec![(
            2,
            1,
            vec![
                (BoxSizing, t("border-box")),
                (Height, n(50.0)),
                (PaddingTop, pct(5.0)),
            ],
        )],
        &[(2, [0.0, 0.0, 400.0, 50.0])],
    );
    case(
        "a percentage width beside a percentage padding",
        parent(None),
        vec![(2, 1, vec![(Width, pct(50.0)), (PaddingTop, pct(25.0))])],
        &[(2, [0.0, 0.0, 200.0, 100.0])],
    );
    // The flex algorithm's stretched cross size had the same basis error in
    // the content-box adjustment of its `max-height` clamp.
    case(
        "a stretched flex item's max-height clamp",
        [parent(Some(300.0)), vec![(Display, t("flex"))]].concat(),
        vec![(2, 1, vec![(MaxHeight, n(50.0)), (PaddingTop, pct(10.0))])],
        &[(2, [0.0, 0.0, 0.0, 90.0])],
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn calc_of_a_percentage_and_a_length_resolves_against_the_containing_block() {
    // CSS Values 4 §10: each term against the percentage's own basis. The
    // numbers are that arithmetic (2026-09-24), not a Chrome capture.
    // <div style="width:400px;height:100px">
    //   <div style="width:calc(100% - 89px);height:20px"></div>
    //   <div style="width:calc(25% + 10px);height:calc(50% - 5px);margin-left:calc(10% - 4px)"></div>
    //   <div style="padding-left:calc(50% - 100px);height:10px"></div>
    // </div>
    let root = props(&vec![(Width, n(400.0)), (Height, n(100.0))]);
    let k = lay_out(
        root,
        vec![
            (
                2,
                1,
                vec![(Width, t("calc(100% - 89px)")), (Height, n(20.0))],
            ),
            (
                3,
                1,
                vec![
                    (Width, t("calc(25% + 10px)")),
                    (Height, t("calc(50% - 5px)")),
                    (MarginLeft, t("calc(10% - 4px)")),
                ],
            ),
            (
                4,
                1,
                vec![(PaddingLeft, t("calc(50% - 100px)")), (Height, n(10.0))],
            ),
        ],
        &[],
    );
    let bad = mismatches(
        "calc",
        &k,
        &[
            (2, [0.0, 0.0, 311.0, 20.0]),
            (3, [36.0, 20.0, 110.0, 45.0]),
            (4, [0.0, 65.0, 400.0, 10.0]),
        ],
    );
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Lays out a 400px-wide block holding `nodes` (`(id, parent, rows)`),
/// where `button` is a `Pressable` (a `<button>` on the web) and every other
/// node a box; `text` names the one text node, if any.
fn lay_out_button(button: u32, nodes: Vec<(u32, u32, Rows)>, text: Option<(u32, &str)>) -> Kernel {
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&vec![(Width, n(400.0))])),
        },
    ];
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (id, parent, rows) in nodes {
        let node_type = match text {
            Some((t, _)) if t == id => NodeType::Text,
            _ if id == button => NodeType::Pressable,
            _ => NodeType::View,
        };
        ops.push(Op::CreateView { id, node_type });
        let mut rows = rows;
        rows.extend([(FontSize, n(16.0)), (LineHeight, t("18px"))]);
        ops.push(Op::SetStyle {
            id,
            patch: Box::new(props(&rows)),
        });
        children.entry(parent).or_default().push(id);
    }
    if let Some((id, text)) = text {
        ops.push(Op::SetProp {
            id,
            prop: PropId::Text,
            value: text.into(),
        });
    }
    for (id, list) in children {
        ops.push(Op::SetChildren { id, children: list });
    }
    ops.push(Op::AttachRoot { id: 1 });
    let mut k = kernel();
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    k
}

/// A `button` is Chrome's `<button>` with `index.html`'s reset (`all:
/// unset; display: block`; Charlie, 2026-10-04, reversing 2026-09-23's flex
/// column): its automatic width shrinks to fit, and a block button's content
/// sits in an anonymous flow-root box centred, safely, in its height, which
/// no `align-content` moves. A flex or grid button is an ordinary container.
/// Chrome 154, 2026-10-04, `<span style="display:block">` children,
/// `getBoundingClientRect` relative to the outer box.
#[test]
fn a_button_lays_out_its_content_as_chrome_does() {
    use crate::browser_cases::css_rows as css;
    let mut failures = Vec::new();
    let mut case = |name: &str, nodes: &[(u32, u32, &str)], want: &[(u32, [f32; 4])]| {
        let nodes = nodes.iter().map(|(id, p, c)| (*id, *p, css(c))).collect();
        failures.extend(mismatches(name, &lay_out_button(2, nodes, None), want));
    };
    let block = |rows: &'static str| [(2, 1, rows), (3, 2, "height:20px")];
    case(
        "a 100px button's 20px child is centred",
        &block("height:100px"),
        &[(2, [0.0, 0.0, 0.0, 100.0]), (3, [0.0, 40.0, 0.0, 20.0])],
    );
    case(
        "two children are centred as one group",
        &[
            (2, 1, "height:100px"),
            (3, 2, "height:20px"),
            (4, 2, "height:20px;width:30px"),
        ],
        &[(3, [0.0, 30.0, 30.0, 20.0]), (4, [0.0, 50.0, 30.0, 20.0])],
    );
    case(
        "overflowing content starts at the top",
        &[(2, 1, "height:100px"), (3, 2, "height:150px")],
        &[(3, [0.0, 0.0, 0.0, 150.0])],
    );
    case(
        "a child's margin stays inside the centred box",
        &[
            (2, 1, "height:100px"),
            (3, 2, "height:20px;margin-top:10px"),
        ],
        &[(3, [0.0, 45.0, 0.0, 20.0])],
    );
    case(
        "align-content does not move it",
        &block("height:100px;align-content:flex-start"),
        &[(3, [0.0, 40.0, 0.0, 20.0])],
    );
    case(
        "padding",
        &block("height:100px;padding:10px"),
        &[(2, [0.0, 0.0, 20.0, 120.0]), (3, [10.0, 50.0, 0.0, 20.0])],
    );
    case(
        "min-height",
        &[(2, 1, "min-height:60px"), (3, 2, "height:20px;width:30px")],
        &[(2, [0.0, 0.0, 30.0, 60.0]), (3, [0.0, 20.0, 30.0, 20.0])],
    );
    case(
        "border-box with padding and border",
        &[
            (
                2,
                1,
                "box-sizing:border-box;height:60px;padding-top:5px;padding-bottom:5px;padding-left:10px;padding-right:10px;border-width:2px;border-style:solid",
            ),
            (3, 2, "height:20px;width:30px"),
        ],
        &[(2, [0.0, 0.0, 54.0, 60.0]), (3, [12.0, 20.0, 30.0, 20.0])],
    );
    case(
        "a nested block is centred whole",
        &[
            (2, 1, "height:100px;width:200px"),
            (3, 2, "padding:5px"),
            (4, 3, "height:20px;width:30px"),
        ],
        &[(3, [0.0, 35.0, 200.0, 30.0]), (4, [5.0, 40.0, 30.0, 20.0])],
    );
    case(
        "a flex button is a row that starts its items",
        &[
            (2, 1, "display:flex;height:100px"),
            (3, 2, "height:20px;width:30px"),
            (4, 2, "height:10px;width:30px"),
        ],
        &[
            (2, [0.0, 0.0, 60.0, 100.0]),
            (3, [0.0, 0.0, 30.0, 20.0]),
            (4, [30.0, 0.0, 30.0, 10.0]),
        ],
    );
    case(
        "a flex column button centres by justify-content",
        &block("display:flex;flex-direction:column;justify-content:center;height:100px"),
        &[(3, [0.0, 40.0, 0.0, 20.0])],
    );
    case(
        "a grid button starts its items",
        &block("display:grid;height:100px"),
        &[(3, [0.0, 0.0, 0.0, 20.0])],
    );
    case(
        "stretched in a flex column",
        &[
            (5, 1, "display:flex;flex-direction:column;height:100px"),
            (2, 5, "height:60px"),
            (3, 2, "height:20px;width:30px"),
        ],
        &[(2, [0.0, 0.0, 400.0, 60.0]), (3, [0.0, 20.0, 30.0, 20.0])],
    );
    case(
        "stretched in a grid",
        &[
            (5, 1, "display:grid;height:60px"),
            (2, 5, ""),
            (3, 2, "height:20px;width:30px"),
        ],
        &[(2, [0.0, 0.0, 400.0, 60.0]), (3, [0.0, 20.0, 30.0, 20.0])],
    );
    case(
        "an absolute child's static position follows the centred content",
        &[
            (2, 1, "position:relative;height:100px"),
            (3, 2, "height:20px;width:30px"),
            (4, 2, "position:absolute;height:10px;width:10px"),
        ],
        &[(3, [0.0, 40.0, 30.0, 20.0]), (4, [0.0, 60.0, 10.0, 10.0])],
    );
    case(
        "a lone absolute child's static position is the centre",
        &[
            (2, 1, "position:relative;height:100px;width:50px"),
            (4, 2, "position:absolute;height:10px;width:10px"),
        ],
        &[(4, [0.0, 50.0, 10.0, 10.0])],
    );
    let k = lay_out_button(
        2,
        vec![(2, 1, css("height:100px;width:200px")), (3, 2, vec![])],
        Some((3, "Hello")),
    );
    failures.extend(mismatches(
        "a text label",
        &k,
        &[(3, [0.0, 41.0, 200.0, 18.0])],
    ));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
