//! What a native container covers of a box (LLP 1075.003 §3.5, §3.7): a
//! navigation bar over a route's top edge adds to its padding, the header
//! whose heading the bar shows takes no space, and clearing either lays the
//! route out as authored again.

use exact_kernel::{Env, HostCover, Kernel, NodeType, Offer, Op, StyleId, StyleProps, StyleValue};

fn style(rows: &[(StyleId, StyleValue)]) -> StyleProps {
    let mut s = StyleProps::default();
    for (id, value) in rows {
        s.set_dynamic(*id, value).unwrap();
    }
    s
}

/// A root, one route filling it (a flex column padded by the top inset as
/// an app under `viewport-fit=cover` pads it), a 52-point header and a list.
fn route() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let column = |extra: &[(StyleId, StyleValue)]| {
        let mut rows = vec![
            (StyleId::Display, StyleValue::Text("flex".into())),
            (StyleId::FlexDirection, StyleValue::Text("column".into())),
        ];
        rows.extend_from_slice(extra);
        style(&rows)
    };
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(style(&[(StyleId::Height, StyleValue::Percent(100.0))])),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(column(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Top, StyleValue::Number(0.0)),
                (StyleId::Bottom, StyleValue::Number(0.0)),
                (StyleId::Left, StyleValue::Number(0.0)),
                (StyleId::Right, StyleValue::Number(0.0)),
                (
                    StyleId::PaddingTop,
                    StyleValue::Text("env(safe-area-inset-top)".into()),
                ),
            ])),
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(52.0)),
                (StyleId::FlexShrink, StyleValue::Number(0.0)),
            ])),
        },
        Op::CreateView {
            id: 4,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 4,
            patch: Box::new(style(&[(StyleId::FlexGrow, StyleValue::Number(1.0))])),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn frame(kernel: &mut Kernel, id: u32) -> (f32, f32, f32, f32) {
    kernel
        .compute_layout(1, Offer::definite(390.0, 844.0))
        .unwrap();
    let f = kernel.node(id).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn a_bar_over_the_top_edge_adds_to_the_authored_padding() {
    let mut kernel = route();
    kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    assert_eq!(
        frame(&mut kernel, 3),
        (0.0, 62.0, 390.0, 52.0),
        "as authored"
    );
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 3),
        (0.0, 106.0, 390.0, 52.0),
        "the bar's 44 below the status bar's 62"
    );
    assert_eq!(frame(&mut kernel, 4), (0.0, 158.0, 390.0, 686.0));
}

#[test]
fn an_absolute_footer_in_a_covered_route_clears_the_bar_it_rests_on() {
    let mut kernel = route();
    // A footer pinned to the route's bottom edge, as a cart's Undo button.
    let footer = vec![
        Op::CreateView {
            id: 5,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 5,
            patch: Box::new(style(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Bottom, StyleValue::Number(0.0)),
                (StyleId::Left, StyleValue::Number(0.0)),
                (StyleId::Right, StyleValue::Number(0.0)),
                (StyleId::Height, StyleValue::Number(40.0)),
            ])),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 5],
        },
    ];
    kernel.apply(0, 1, &footer).unwrap();
    assert_eq!(
        frame(&mut kernel, 5),
        (0.0, 804.0, 390.0, 40.0),
        "as authored"
    );
    // The tab bar and home indicator, 83, cover the route's bottom: the
    // in-flow content pads past them and the footer rests above them.
    kernel
        .set_host_cover(2, Some(HostCover::Edges([0.0, 0.0, 83.0, 0.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 52.0, 390.0, 709.0),
        "in-flow content ends above the bar"
    );
    assert_eq!(
        frame(&mut kernel, 5),
        (0.0, 721.0, 390.0, 40.0),
        "the footer rests on the bar, not under it"
    );
    // The route's last in-flow child is a scroller the platform insets at
    // the bottom edge: the route pads nothing (the scroller reaches under
    // the bar), and the footer still clears it.
    kernel
        .set_host_cover(4, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 52.0, 390.0, 792.0),
        "the scroller reaches the bottom edge"
    );
    assert_eq!(frame(&mut kernel, 5), (0.0, 721.0, 390.0, 40.0));
    // A percentage inset is the author's to resolve; it stays as written.
    kernel
        .apply(
            0,
            1,
            &[Op::SetStyle {
                id: 5,
                patch: Box::new(style(&[(StyleId::Bottom, StyleValue::Percent(10.0))])),
            }],
        )
        .unwrap();
    kernel.set_host_cover(4, None).unwrap();
    let (_, y, _, _) = frame(&mut kernel, 5);
    assert!((y - (844.0 - 84.4 - 40.0)).abs() < 0.5, "{y}");
}

#[test]
fn a_header_the_bar_replaces_takes_no_space_until_the_cover_is_cleared() {
    let mut kernel = route();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(frame(&mut kernel, 3), (0.0, 0.0, 0.0, 0.0));
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 44.0, 390.0, 800.0),
        "the list starts under the bar"
    );
    kernel.set_host_cover(3, None).unwrap();
    kernel.set_host_cover(2, None).unwrap();
    assert_eq!(frame(&mut kernel, 4), (0.0, 52.0, 390.0, 792.0));
}

#[test]
fn a_cover_on_a_percent_padding_keeps_the_percent() {
    let mut kernel = route();
    let patch = style(&[(StyleId::PaddingTop, StyleValue::Percent(10.0))]);
    kernel
        .apply(
            1,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    // Padding percentages resolve against the containing block's width.
    assert_eq!(frame(&mut kernel, 3), (0.0, 83.0, 390.0, 52.0));
    // The bar is the host's sizing, not the route's CSS padding
    let route = kernel.node(2).unwrap().key;
    assert_eq!(kernel.resolved_padding(route), Some((0.0, 39.0, 0.0, 0.0)));
    assert_eq!(kernel.layout_padding(route), Some((0.0, 83.0, 0.0, 0.0)));
}

#[test]
fn a_negative_or_nonfinite_edge_is_refused() {
    let mut kernel = route();
    assert!(kernel
        .set_host_cover(2, Some(HostCover::Edges([-1.0, 0.0, 0.0, 0.0])))
        .is_err());
    assert!(kernel
        .set_host_cover(2, Some(HostCover::Edges([f32::NAN, 0.0, 0.0, 0.0])))
        .is_err());
    assert!(kernel.set_host_cover(99, Some(HostCover::Whole)).is_err());
}

/// The route as `route()` builds it, but with the status bar cleared by
/// the header (`padding-top: env(safe-area-inset-top)`), as a web page under
/// `viewport-fit=cover` usually pads it, rather than by the route.
fn header_padded() -> Kernel {
    let mut kernel = route();
    let route = style(&[(StyleId::PaddingTop, StyleValue::Number(0.0))]);
    let header = style(&[(
        StyleId::PaddingTop,
        StyleValue::Text("env(safe-area-inset-top)".into()),
    )]);
    let ops = [
        Op::SetStyle {
            id: 2,
            patch: Box::new(route),
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(header),
        },
    ];
    kernel.apply(1, 2, &ops).unwrap();
    kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    kernel
}

#[test]
fn a_replaced_header_that_cleared_the_status_bar_hands_that_inset_to_the_bar() {
    let mut kernel = header_padded();
    assert_eq!(frame(&mut kernel, 4).1, 114.0, "as authored: 62 + 52");
    // The host reports the bar's 44 past the status bar, as it does when
    // the route pads the inset; the list still starts under the bar.
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(
        frame(&mut kernel, 4).1,
        106.0,
        "the status bar's 62 and the bar's 44"
    );
    // The header's padding changing while it is replaced reaches the route.
    let unpadded = style(&[(StyleId::PaddingTop, StyleValue::Number(0.0))]);
    let op = Op::SetStyle {
        id: 3,
        patch: Box::new(unpadded),
    };
    kernel.apply(2, 3, &[op]).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 44.0, "nothing to hand over");
    let padded = style(&[(
        StyleId::PaddingTop,
        StyleValue::Text("env(safe-area-inset-top)".into()),
    )]);
    let op = Op::SetStyle {
        id: 3,
        patch: Box::new(padded),
    };
    kernel.apply(3, 4, &[op]).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 106.0);
    // A new inset (a rotation) reaches the route through the header's.
    kernel.set_env(Env::new(20.0, 0.0, 0.0, 0.0)).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 64.0);
    // The header given back: laid out as authored again.
    kernel.set_host_cover(3, None).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 20.0 + 44.0 + 52.0);
}

/// LLP 1001 §2 (2026-10-07): a header padded by a comparison that reads
/// the top inset cleared it too; one that reads only another inset did not.
/// The corner-adapted safe area's top is never less than the inset (LLP 1123
/// D10), so it clears it; the margins' top can lie above the safe area where
/// an app turns `insetsLayoutMarginsFromSafeArea` off, so it does not.
#[test]
fn a_replaced_header_padded_by_a_comparison_of_the_top_inset_hands_it_over() {
    let mut kernel = header_padded();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    for (padding, y) in [
        ("clamp(0px, env(safe-area-inset-top), 200px)", 106.0),
        ("max(12px, env(safe-area-inset-bottom))", 44.0),
        ("env(-exact-safe-area-corners-inset-top)", 106.0),
        ("max(12px, env(-exact-safe-area-corners-inset-top))", 106.0),
        ("env(-exact-layout-margins-inset-top)", 44.0),
    ] {
        let op = Op::SetStyle {
            id: 3,
            patch: Box::new(style(&[(
                StyleId::PaddingTop,
                StyleValue::Text(padding.into()),
            )])),
        };
        kernel.apply(2, 3, &[op]).unwrap();
        assert_eq!(frame(&mut kernel, 4).1, y, "{padding}");
    }
}

#[test]
fn a_scroller_under_the_bar_takes_no_inset_from_the_header() {
    let mut kernel = header_padded();
    // The bar covers no top edge (a scroller goes under it, inset by the
    // platform); only the sides are covered.
    kernel
        .set_host_cover(2, Some(HostCover::Edges([0.0, 8.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(
        frame(&mut kernel, 4).1,
        0.0,
        "the list starts at the top, under the bar"
    );
}

#[test]
fn a_route_whose_replaced_header_leaves_takes_back_its_inset() {
    let mut kernel = header_padded();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 106.0);
    // The header goes (an `if`), the bar's cover stays.
    let op = Op::SetChildren {
        id: 2,
        children: vec![4],
    };
    kernel.apply(2, 3, &[op]).unwrap();
    assert_eq!(
        frame(&mut kernel, 4).1,
        44.0,
        "no replaced header, no inset"
    );
    let op = Op::SetChildren {
        id: 2,
        children: vec![3, 4],
    };
    kernel.apply(3, 4, &[op]).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 106.0, "and back with it");
}

#[test]
fn a_replaced_header_destroyed_outright_leaves_no_inset() {
    let mut kernel = header_padded();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 106.0);
    kernel.apply(2, 3, &[Op::DestroyView { id: 3 }]).unwrap();
    assert_eq!(frame(&mut kernel, 4).1, 44.0);
}

/// The route's list (4) as a scroller padded by 16 holding a column (5)
/// that fills its height (`flex: 1` in its flex column) and, when `long`,
/// a 2000-point box after it: trivia-quiz's results and a filled-height
/// screen (LLP 1116.000 D1.2).
fn scroller(long: bool) -> Kernel {
    let mut kernel = route();
    let flex_column = [
        (StyleId::Display, StyleValue::Text("flex".into())),
        (StyleId::FlexDirection, StyleValue::Text("column".into())),
        (StyleId::OverflowY, StyleValue::Text("scroll".into())),
        (StyleId::MinHeight, StyleValue::Number(0.0)),
        (StyleId::PaddingTop, StyleValue::Number(16.0)),
        (StyleId::PaddingBottom, StyleValue::Number(16.0)),
    ];
    let mut ops = vec![
        Op::SetStyle {
            id: 4,
            patch: Box::new(style(&flex_column)),
        },
        Op::CreateView {
            id: 5,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 5,
            patch: Box::new(style(&[(StyleId::FlexGrow, StyleValue::Number(1.0))])),
        },
        Op::CreateView {
            id: 6,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 6,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(2000.0)),
                (StyleId::FlexShrink, StyleValue::Number(0.0)),
            ])),
        },
    ];
    ops.push(Op::SetChildren {
        id: 4,
        children: if long { vec![5, 6] } else { vec![5] },
    });
    kernel.apply(1, 2, &ops).unwrap();
    // The bar replaces the header and the list runs under it, as a route's
    // collapsing scroller does on iOS.
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    kernel
}

#[test]
fn a_scroller_the_platform_insets_lays_out_its_content_between_the_insets() {
    let mut kernel = scroller(false);
    assert_eq!(frame(&mut kernel, 4), (0.0, 0.0, 390.0, 844.0));
    assert_eq!(
        frame(&mut kernel, 5),
        (0.0, 16.0, 390.0, 812.0),
        "as authored"
    );
    // UIKit insets the scroller by the large title's 140 and the home
    // indicator's 34: the filled column ends where the indicator begins
    // once the title's inset moves it down, and keeps its start.
    kernel
        .set_host_cover(4, Some(HostCover::Inset([140.0, 34.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 0.0, 390.0, 844.0),
        "the scroller keeps its box"
    );
    assert_eq!(frame(&mut kernel, 5), (0.0, 16.0, 390.0, 638.0));
    let list = kernel.node(4).unwrap().key;
    assert_eq!(
        kernel.resolved_padding(list),
        Some((0.0, 16.0, 0.0, 16.0)),
        "CSS's"
    );
    assert_eq!(kernel.layout_padding(list), Some((0.0, 16.0, 0.0, 190.0)));
    assert_eq!(
        kernel.node(4).unwrap().content.1,
        670.0,
        "the extent the platform adds its insets to: 16 + 638 + 16"
    );
    kernel.set_host_cover(4, None).unwrap();
    assert_eq!(frame(&mut kernel, 5).3, 812.0, "cleared, as authored");
}

#[test]
fn scrolling_content_keeps_its_extent_under_an_inset() {
    let mut kernel = scroller(true);
    kernel
        .set_host_cover(4, Some(HostCover::Inset([140.0, 34.0])))
        .unwrap();
    assert_eq!(frame(&mut kernel, 6), (0.0, 16.0, 390.0, 2000.0));
    assert_eq!(kernel.node(4).unwrap().content.1, 2032.0, "16 + 2000 + 16");
    assert!(kernel
        .set_host_cover(4, Some(HostCover::Inset([f32::NAN, 0.0])))
        .is_err());
}

/// The route's list (4) as a column holding a 40-point header (5) and then
/// a scroller (6) filling the rest, with a footer (7) pinned to the route's
/// bottom edge: a sheet component's root column, the scroller two boxes
/// down from the route (keyboard-form's pinned tab).
fn nested() -> Kernel {
    let mut kernel = route();
    let grow = |extra: &[(StyleId, StyleValue)]| {
        let mut rows = vec![
            (StyleId::FlexGrow, StyleValue::Number(1.0)),
            (StyleId::MinHeight, StyleValue::Number(0.0)),
        ];
        rows.extend_from_slice(extra);
        style(&rows)
    };
    let view = |id| Op::CreateView {
        id,
        node_type: NodeType::View,
    };
    let ops = vec![
        Op::SetStyle {
            id: 4,
            patch: Box::new(grow(&[
                (StyleId::Display, StyleValue::Text("flex".into())),
                (StyleId::FlexDirection, StyleValue::Text("column".into())),
            ])),
        },
        view(5),
        Op::SetStyle {
            id: 5,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(40.0)),
                (StyleId::FlexShrink, StyleValue::Number(0.0)),
            ])),
        },
        view(6),
        Op::SetStyle {
            id: 6,
            patch: Box::new(grow(&[(
                StyleId::OverflowY,
                StyleValue::Text("scroll".into()),
            )])),
        },
        view(7),
        Op::SetStyle {
            id: 7,
            patch: Box::new(style(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Bottom, StyleValue::Number(0.0)),
                (StyleId::Left, StyleValue::Number(0.0)),
                (StyleId::Right, StyleValue::Number(0.0)),
                (StyleId::Height, StyleValue::Number(40.0)),
            ])),
        },
        Op::SetChildren {
            id: 4,
            children: vec![5, 6],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 7],
        },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    // The tab bar and home indicator, 83, over the route's bottom.
    kernel
        .set_host_cover(2, Some(HostCover::Edges([0.0, 0.0, 83.0, 0.0])))
        .unwrap();
    kernel
}

#[test]
fn a_scroller_two_boxes_down_reaches_under_the_bar_and_the_footer_still_rests_on_it() {
    let mut kernel = nested();
    assert_eq!(
        frame(&mut kernel, 6),
        (0.0, 92.0, 390.0, 669.0),
        "padded above the bar while the platform insets nothing"
    );
    assert_eq!(frame(&mut kernel, 7), (0.0, 721.0, 390.0, 40.0));
    // The platform insets the scroller at the bottom edge by the bar's 83:
    // the route pads nothing there, however deep the scroller sits.
    kernel
        .set_host_cover(6, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 52.0, 390.0, 792.0),
        "the column reaches the bottom edge"
    );
    assert_eq!(frame(&mut kernel, 6), (0.0, 92.0, 390.0, 752.0));
    assert_eq!(
        frame(&mut kernel, 7),
        (0.0, 721.0, 390.0, 40.0),
        "the footer still rests on the bar"
    );
    kernel.set_host_cover(6, None).unwrap();
    assert_eq!(
        frame(&mut kernel, 6),
        (0.0, 92.0, 390.0, 669.0),
        "the padding comes back with the cover"
    );
    // Inset at its top alone (a keyboard ended the route above the bars),
    // then at its bottom again as the keyboard goes: the route re-derived
    // each time, though the scroller was inset throughout.
    kernel
        .set_host_cover(6, Some(HostCover::Inset([116.0, 0.0])))
        .unwrap();
    assert_eq!(frame(&mut kernel, 6).3, 669.0);
    kernel
        .set_host_cover(6, Some(HostCover::Inset([116.0, 83.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 6),
        (0.0, 92.0, 390.0, 752.0),
        "under the bar again once it is inset at the bottom"
    );
}

/// theme2's Q1: a scroller the host marks off the kernel's chain counts for
/// nothing, so the route keeps its padding: an inset twice at worst, never
/// the scroller above the bar for good nor the footer under it.
#[test]
fn a_marked_scroller_off_the_chain_leaves_the_route_its_padding() {
    let mut kernel = nested();
    // A box after the scroller in its column: the chain ends there.
    let after = vec![
        Op::CreateView {
            id: 8,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 8,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(10.0)),
                (StyleId::FlexShrink, StyleValue::Number(0.0)),
            ])),
        },
        Op::SetChildren {
            id: 4,
            children: vec![5, 6, 8],
        },
    ];
    kernel.apply(1, 2, &after).unwrap();
    kernel
        .set_host_cover(6, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    assert_eq!(
        frame(&mut kernel, 4),
        (0.0, 52.0, 390.0, 709.0),
        "the route still pads above the bar"
    );
    assert_eq!(frame(&mut kernel, 7), (0.0, 721.0, 390.0, 40.0));
    // Taken out of the column, the scroller ends it again: the route,
    // re-derived for a change two boxes down, pads nothing.
    kernel
        .apply(
            2,
            3,
            &[Op::SetChildren {
                id: 4,
                children: vec![5, 6],
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, 4), (0.0, 52.0, 390.0, 792.0));
    // A scroller inside the edge scroller is past where the chain stops.
    kernel.set_host_cover(6, None).unwrap();
    let inner = vec![
        Op::CreateView {
            id: 9,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 9,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(100.0)),
                (StyleId::OverflowY, StyleValue::Text("scroll".into())),
            ])),
        },
        Op::SetChildren {
            id: 6,
            children: vec![9],
        },
    ];
    kernel.apply(3, 4, &inner).unwrap();
    kernel
        .set_host_cover(9, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    assert_eq!(frame(&mut kernel, 4), (0.0, 52.0, 390.0, 709.0));
}

/// Astra's and Grok's round-1 finding: a box's `position` or `display` moves
/// it onto the chain or off it with the scroller's inset unchanged, and the
/// route is derived again.
#[test]
fn a_box_moved_onto_the_chain_or_off_it_re_derives_the_route() {
    let mut kernel = nested();
    kernel
        .set_host_cover(6, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    assert_eq!(frame(&mut kernel, 4).3, 792.0);
    // The footer taken into the flow ends the route: it pads above the bar.
    let flow = |kernel: &mut Kernel, batch: u64, position: &str| {
        kernel
            .apply(
                1,
                batch,
                &[Op::SetStyle {
                    id: 7,
                    patch: Box::new(style(&[(
                        StyleId::PositionType,
                        StyleValue::Text(position.into()),
                    )])),
                }],
            )
            .unwrap();
    };
    flow(&mut kernel, 2, "relative");
    assert_eq!(
        frame(&mut kernel, 4).3,
        669.0,
        "the footer in the flow, the route padded"
    );
    flow(&mut kernel, 3, "absolute");
    assert_eq!(frame(&mut kernel, 4).3, 792.0);
    // The scroller hidden, its inset still said: the chain ends at the header.
    kernel
        .apply(
            1,
            4,
            &[Op::SetStyle {
                id: 6,
                patch: Box::new(style(&[(
                    StyleId::Display,
                    StyleValue::Text("none".into()),
                )])),
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, 4).3, 709.0, "the route padded again");
}

/// Round 2 (Astra, Grok): a footer attached, or moved away, after the
/// route's cover has settled is placed past it, or no longer; a box that
/// clips across (`overflow-x: hidden`, so its `visible` height scrolls as
/// the engine computes it) is a scroller the chain stops at; and the
/// padding the route reports is what layout pads.
#[test]
fn a_footer_attached_late_a_clipping_box_and_the_reported_padding() {
    let mut kernel = nested();
    kernel
        .set_host_cover(6, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    assert_eq!(frame(&mut kernel, 4).3, 792.0);
    let late = vec![
        Op::CreateView {
            id: 8,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 8,
            patch: Box::new(style(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Bottom, StyleValue::Number(0.0)),
                (StyleId::Left, StyleValue::Number(0.0)),
                (StyleId::Height, StyleValue::Number(20.0)),
                (StyleId::Width, StyleValue::Number(20.0)),
            ])),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 7, 8],
        },
    ];
    kernel.apply(1, 2, &late).unwrap();
    assert_eq!(
        frame(&mut kernel, 8).1,
        844.0 - 83.0 - 20.0,
        "attached late, on the bar"
    );
    // Moved into the header, which no bar covers: placed as written, at the
    // foot of its containing block (the positioned route), under the bar.
    let moved = [
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 7],
        },
        Op::SetChildren {
            id: 3,
            children: vec![8],
        },
    ];
    kernel.apply(1, 3, &moved).unwrap();
    assert_eq!(
        frame(&mut kernel, 8).1,
        844.0 - 20.0,
        "no longer past a cover"
    );
    // The route's own padding at its bottom, as layout pads it: the
    // authored 16, none of the bar's.
    kernel
        .apply(
            1,
            4,
            &[Op::SetStyle {
                id: 2,
                patch: Box::new(style(&[(StyleId::PaddingBottom, StyleValue::Number(16.0))])),
            }],
        )
        .unwrap();
    frame(&mut kernel, 4);
    let key = kernel.node(2).unwrap().key;
    assert_eq!(kernel.layout_padding(key).unwrap().3, 16.0);
    assert_eq!(kernel.resolved_padding(key).unwrap().3, 16.0);
    assert_eq!(kernel.cover_bottom(key), 0.0);
    // The column clipping across scrolls as the engine computes it: the
    // inset below it is past the chain's end, and the route pads again.
    kernel
        .apply(
            1,
            5,
            &[Op::SetStyle {
                id: 4,
                patch: Box::new(style(&[(
                    StyleId::OverflowX,
                    StyleValue::Text("hidden".into()),
                )])),
            }],
        )
        .unwrap();
    assert_eq!(frame(&mut kernel, 4).3, 844.0 - 83.0 - 16.0 - 52.0);
    assert_eq!(kernel.cover_bottom(key), 83.0);
}

/// Round 3 (Astra, Grok): an absolute child at `top: 0` of a route whose top
/// cover holds its replaced header's inset (`header_inset`) follows that
/// inset as the header is replaced, its padding changes, and the inset does.
#[test]
fn an_absolute_child_follows_the_inset_its_routes_replaced_header_hands_the_bar() {
    let mut kernel = header_padded();
    let badge = [
        Op::CreateView {
            id: 5,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 5,
            patch: Box::new(style(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Top, StyleValue::Number(0.0)),
                (StyleId::Left, StyleValue::Number(0.0)),
                (StyleId::Width, StyleValue::Number(10.0)),
                (StyleId::Height, StyleValue::Number(10.0)),
            ])),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 5],
        },
    ];
    kernel.apply(1, 3, &badge).unwrap();
    kernel
        .set_host_cover(2, Some(HostCover::Edges([44.0, 0.0, 0.0, 0.0])))
        .unwrap();
    assert_eq!(frame(&mut kernel, 5).1, 44.0, "the bar's 44");
    kernel.set_host_cover(3, Some(HostCover::Whole)).unwrap();
    assert_eq!(
        frame(&mut kernel, 5).1,
        106.0,
        "and the status bar's 62 the header handed it"
    );
    let padding = |kernel: &mut Kernel, batch, value| {
        let op = Op::SetStyle {
            id: 3,
            patch: Box::new(style(&[(StyleId::PaddingTop, value)])),
        };
        kernel.apply(1, batch, &[op]).unwrap();
    };
    padding(&mut kernel, 4, StyleValue::Number(0.0));
    assert_eq!(frame(&mut kernel, 5).1, 44.0, "nothing handed over");
    padding(
        &mut kernel,
        5,
        StyleValue::Text("env(safe-area-inset-top)".into()),
    );
    assert_eq!(frame(&mut kernel, 5).1, 106.0);
    kernel.set_env(Env::new(20.0, 0.0, 0.0, 0.0)).unwrap();
    assert_eq!(frame(&mut kernel, 5).1, 64.0, "a rotation's new inset");
    kernel.set_host_cover(3, None).unwrap();
    assert_eq!(frame(&mut kernel, 5).1, 44.0, "the header given back");
    // The route's cover cleared (a keyboard ending it above the bars).
    kernel.set_host_cover(2, None).unwrap();
    assert_eq!(frame(&mut kernel, 5).1, 0.0, "placed as written");
}

/// Round 3 (Astra): an inset at its top alone on a box down the chain stops
/// the walk there; said or cleared, in either order with the scroller's,
/// the route is derived again.
#[test]
fn a_top_only_inset_down_the_chain_said_or_cleared_re_derives_the_route() {
    for scroller_first in [true, false] {
        let mut kernel = nested();
        let top = Some(HostCover::Inset([20.0, 0.0]));
        let edge = Some(HostCover::Inset([0.0, 83.0]));
        if scroller_first {
            kernel.set_host_cover(6, edge).unwrap();
            kernel.set_host_cover(4, top).unwrap();
        } else {
            kernel.set_host_cover(4, top).unwrap();
            kernel.set_host_cover(6, edge).unwrap();
        }
        assert_eq!(
            frame(&mut kernel, 4).3,
            709.0,
            "the walk stops at the column: {scroller_first}"
        );
        kernel.set_host_cover(4, None).unwrap();
        assert_eq!(
            frame(&mut kernel, 4).3,
            792.0,
            "and reaches the scroller: {scroller_first}"
        );
    }
}

#[test]
fn a_host_block_does_not_inherit_its_parents_covered_edges() {
    use exact_kernel::{BlockAxis, HostBlock};
    let mut k = route();
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 5,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 5,
                patch: Box::new(style(&[
                    (StyleId::PositionType, StyleValue::Text("absolute".into())),
                    (StyleId::Top, StyleValue::Number(0.)),
                    (StyleId::Right, StyleValue::Number(0.)),
                    (StyleId::Bottom, StyleValue::Number(0.)),
                    (StyleId::Left, StyleValue::Number(0.)),
                ])),
            },
            Op::SetChildren {
                id: 2,
                children: vec![3, 4, 5],
            },
        ],
    )
    .unwrap();
    k.set_host_cover(2, Some(HostCover::Edges([20., 250., 0., 0.])))
        .unwrap();
    assert_eq!(frame(&mut k, 5), (0., 20., 140., 824.));
    k.set_host_block(
        5,
        Some(HostBlock {
            width: BlockAxis::Is(600.),
            height: BlockAxis::Is(500.),
        }),
    )
    .unwrap();
    assert_eq!(frame(&mut k, 5), (0., 0., 600., 500.));
    k.set_host_cover(2, Some(HostCover::Edges([30., 200., 0., 0.])))
        .unwrap();
    assert_eq!(frame(&mut k, 5), (0., 0., 600., 500.));
    k.set_host_block(5, None).unwrap();
    assert_eq!(frame(&mut k, 5), (0., 30., 190., 814.));
}
