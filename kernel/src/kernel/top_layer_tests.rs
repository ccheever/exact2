//! HTML's top layer on kernel hosts (stage 14 B): a popover is laid out in
//! the viewport, a block the kernel implies (`Kernel::imply_blocks`), not in
//! its nearest positioned ancestor.
use super::*;
use crate::{BlockAxis, HostBlock, StyleValue};

const OFFER: Offer = Offer::definite(800.0, 600.0);

/// The rows as the author's (`written`), as a page's CSS is.
fn style(id: ViewId, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
        patch.written.set(*row);
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn t(s: &str) -> StyleValue<'_> {
    StyleValue::Text(s.into())
}
fn n(x: f64) -> StyleValue<'static> {
    StyleValue::Number(x)
}
fn popover(id: ViewId) -> Op {
    Op::SetProp {
        id,
        prop: PropId::Popover,
        value: "auto".into(),
    }
}

/// A root (1, a column) holding a 100-point positioned box (2), holding the
/// popover (3, with `rows`), holding a text (4) when `text` is given; and a
/// box after it in the root (5, 20 points).
fn page(rows: &[(StyleId, StyleValue)], text: Option<&str>) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    for id in 1..=5 {
        let node_type = if id == 4 {
            NodeType::Text
        } else {
            NodeType::View
        };
        ops.push(Op::CreateView { id, node_type });
    }
    ops.push(style(
        1,
        &[
            (StyleId::Display, t("flex")),
            (StyleId::FlexDirection, t("column")),
        ],
    ));
    ops.push(style(
        2,
        &[
            (StyleId::PositionType, t("relative")),
            (StyleId::Width, n(100.0)),
            (StyleId::Height, n(100.0)),
        ],
    ));
    ops.push(style(3, rows));
    ops.push(popover(3));
    ops.push(style(5, &[(StyleId::Height, n(20.0))]));
    if let Some(text) = text {
        ops.push(Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: text.into(),
        });
        ops.push(Op::SetChildren {
            id: 3,
            children: vec![4],
        });
    }
    ops.push(Op::SetChildren {
        id: 2,
        children: vec![3],
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2, 5],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    k
}

fn frame(k: &Kernel, id: ViewId) -> Frame {
    k.node(id).unwrap().frame
}

/// QUEUE's repro: `width: 50%` in a 100-point positioned box is half the
/// viewport, as on the web, not 50 points; a percentage height likewise.
#[test]
fn a_popovers_percentages_are_the_viewports() {
    let k = page(
        &[(StyleId::Width, t("50%")), (StyleId::Height, t("50%"))],
        None,
    );
    let f = frame(&k, 3);
    assert_eq!((f.x, f.y, f.width, f.height), (0.0, 0.0, 400.0, 300.0));
    let viewport = Some(HostBlock {
        width: BlockAxis::Fit(800.0),
        height: BlockAxis::Fit(600.0),
    });
    assert_eq!(k.host_block(3), viewport);
    assert_eq!(k.host_block_size(3), Some((400.0, 300.0)));
}

/// With no size a popover fits its content, as the UA sheet's
/// `fit-content`, capped at the viewport: no positioned ancestor trims it.
#[test]
fn a_popover_with_no_size_fits_its_content_under_the_viewport() {
    let k = page(&[], Some("Pins"));
    let f = frame(&k, 3);
    assert_eq!(
        f.width,
        frame(&k, 4).width,
        "fit-content, not the 100-point box"
    );
    assert!(f.width > 0.0 && f.height > 0.0);

    let long = "word ".repeat(400);
    let k = page(&[], Some(&long));
    let f = frame(&k, 3);
    assert_eq!(f.width, 800.0, "capped at the viewport's width");
    assert!(f.height > 20.0, "and wraps there");
}

/// Opening a popover moves nothing on the page: its parent's size and
/// scroll extent, and the box after it, are as without it.
#[test]
fn a_popover_takes_no_room_and_no_extent() {
    let big = [(StyleId::Width, n(2000.0)), (StyleId::Height, n(3000.0))];
    let mut k = page(&big, None);
    let open = (frame(&k, 2), k.node(2).unwrap().content, frame(&k, 5));
    k.apply(
        1,
        2,
        &[Op::ClearProp {
            id: 3,
            prop: PropId::Popover,
        }],
    )
    .unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(k.host_block(3), None, "out of the top layer: no block");
    let closed = k.node(2).unwrap().content;
    assert!(
        closed.0 >= 2000.0,
        "in its parent again, it adds its extent"
    );
    assert_eq!(open.0, frame(&k, 2));
    assert!(
        open.1 .0 <= 100.0 && open.1 .1 <= 100.0,
        "the popover adds none"
    );
    assert_eq!(open.2, frame(&k, 5));
}

/// The prop set and cleared moves the box into its block and out again; a
/// popover its author made sticky keeps its flow and has none.
#[test]
fn the_prop_and_sticky_move_a_box_in_and_out_of_its_block() {
    let width = [(StyleId::Width, t("50%"))];
    let mut k = page(&width, None);
    assert_eq!(frame(&k, 3).width, 400.0);
    k.apply(
        1,
        2,
        &[Op::ClearProp {
            id: 3,
            prop: PropId::Popover,
        }],
    )
    .unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!((k.host_block(3), frame(&k, 3).width), (None, 50.0));
    k.apply(2, 3, &[popover(3)]).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(frame(&k, 3).width, 400.0, "and back in");

    let mut sticky = StyleProps::default();
    sticky
        .set_dynamic(StyleId::PositionType, &t("sticky"))
        .unwrap();
    sticky.written = sticky.mask;
    k.apply(
        3,
        4,
        &[Op::SetStyle {
            id: 3,
            patch: Box::new(sticky),
        }],
    )
    .unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!((k.host_block(3), frame(&k, 3).width), (None, 50.0));
}

/// A host's block wins over the viewport, and taking it back leaves the
/// viewport, not the popover's parent.
#[test]
fn a_hosts_block_wins_and_none_falls_back_to_the_viewport() {
    let mut k = page(&[(StyleId::Width, t("50%"))], None);
    let screen = Some(HostBlock {
        width: BlockAxis::AtMost(300.0),
        height: BlockAxis::AtMost(500.0),
    });
    k.set_host_block(3, screen).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!((k.host_block(3), frame(&k, 3).width), (screen, 150.0));
    k.set_host_block(3, None).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(frame(&k, 3).width, 400.0);
    assert!(k.host_block(3).is_some());
}

/// The block is the viewport of each layout: a new offer resizes it.
#[test]
fn a_new_viewport_resizes_the_block() {
    let mut k = page(&[(StyleId::Width, t("50%"))], None);
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    assert_eq!(frame(&k, 3).width, 195.0);
    assert_eq!(
        k.host_block(3).map(|b| (b.width, b.height)),
        Some((BlockAxis::Fit(390.0), BlockAxis::Fit(844.0)))
    );
}

/// A popover destroyed takes its block with it, and with no viewport yet
/// (no definite offer) there is none: it stays in its positioned ancestor.
#[test]
fn no_viewport_no_block_and_destroy_drops_it() {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    for id in 1..=2 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.push(style(1, &[(StyleId::Width, n(100.0))]));
    ops.push(style(2, &[(StyleId::Width, t("50%"))]));
    ops.push(popover(2));
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
    assert_eq!((k.host_block(2), frame(&k, 2).width), (None, 50.0));
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(frame(&k, 2).width, 400.0);
    k.apply(1, 2, &[Op::DestroyView { id: 2 }]).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert!(!k.arena.blocks.any());
}

/// What-if heights read a popover's own layout: its block's subtree, laid
/// out under the offer given, at that offer's viewport.
#[test]
fn heights_are_measured_in_the_popovers_block_at_the_offers_viewport() {
    let long = "word ".repeat(60);
    let mut k = page(
        &[(StyleId::Width, t("50%")), (StyleId::Height, n(200.0))],
        Some(&long),
    );
    let key = k.node(3).unwrap().key;
    let (auto, _) = k.measure_auto_height(key).unwrap();
    assert!(auto.height < 200.0, "its content's height, {}", auto.height);
    // At a narrower viewport the text wraps more: the measure reads that
    // viewport, not the last layout's.
    let rows = [
        (StyleId::Width, t("50%")),
        (StyleId::BoxSizing, t("border-box")),
    ];
    let mut k = page(&rows, Some(&long));
    let key = k.node(3).unwrap().key;
    let wide = frame(&k, 3).height;
    let narrow = Offer::definite(400.0, 600.0);
    let measured = k.measure_height_targets(1, narrow, &[key]).unwrap()[0].px;
    k.compute_layout(1, narrow).unwrap();
    assert_eq!(measured, frame(&k, 3).height);
    assert!(measured > wide, "{measured} wraps more than {wide}");
}

/// A popover whose ancestors move out from under a box its host replaced
/// whole is styled again: no longer hidden with that box, it is laid out.
#[test]
fn a_popover_moved_out_from_under_a_replaced_box_is_laid_out() {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    for id in 1..=5 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.push(style(
        5,
        &[(StyleId::Width, t("50%")), (StyleId::Height, n(30.0))],
    ));
    ops.push(popover(5));
    ops.push(Op::SetChildren {
        id: 4,
        children: vec![5],
    });
    ops.push(Op::SetChildren {
        id: 2,
        children: vec![4],
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2, 3],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.set_host_cover(2, Some(crate::HostCover::Whole)).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(frame(&k, 5).width, 0.0, "hidden with the replaced box");
    let moved = [
        Op::SetChildren {
            id: 2,
            children: vec![],
        },
        Op::SetChildren {
            id: 3,
            children: vec![4],
        },
    ];
    k.apply(1, 2, &moved).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(frame(&k, 5).width, 400.0);
}

/// Where a popover sits in the viewport by its insets (`top_layer_origin`,
/// CSS 2 §10.3.7 and §10.6.4): two `auto` margins share even a negative
/// room down; across, the containing block's direction picks the inset
/// that wins when both are written.
#[test]
fn a_popovers_insets_place_it_by_the_css_rules() {
    let place = |root: &[(StyleId, StyleValue)], rows: &[(StyleId, StyleValue)], offer: Offer| {
        let mut k = Kernel::with_monospace();
        let mut ops = vec![];
        for id in 1..=2 {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::View,
            });
        }
        ops.push(style(1, root));
        ops.push(style(2, rows));
        ops.push(popover(2));
        ops.push(Op::SetChildren {
            id: 1,
            children: vec![2],
        });
        ops.push(Op::AttachRoot { id: 1 });
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, offer).unwrap();
        let (crate::AxisOffer::Definite(w), crate::AxisOffer::Definite(h)) =
            (offer.width, offer.height)
        else {
            unreachable!()
        };
        k.top_layer_origin(2, (w, h)).unwrap()
    };
    let tall = [
        (StyleId::Width, n(80.0)),
        (StyleId::Height, n(400.0)),
        (StyleId::Top, n(0.0)),
        (StyleId::Bottom, n(0.0)),
        (StyleId::MarginTop, StyleValue::Auto),
        (StyleId::MarginBottom, StyleValue::Auto),
    ];
    let (_, y) = place(&[], &tall, Offer::definite(800.0, 320.0));
    assert_eq!(y, -40.0, "down, a negative share is shared");
    let both = [
        (StyleId::Width, n(80.0)),
        (StyleId::Height, n(40.0)),
        (StyleId::Left, n(0.0)),
        (StyleId::Right, n(0.0)),
    ];
    let (x, _) = place(&[], &both, OFFER);
    assert_eq!(x, 0.0, "left to right, the left inset wins");
    let rtl = [(StyleId::Direction, t("rtl"))];
    let (x, _) = place(&rtl, &both, OFFER);
    assert_eq!(x, 720.0, "right to left, the right one");
    // Its right margin as laid out, though a negative left one makes its
    // margin box narrower than nothing (Astra's re-check).
    let pulled = [both.as_slice(), &[(StyleId::MarginLeft, n(-100.0))]].concat();
    let (x, _) = place(&rtl, &pulled, OFFER);
    assert_eq!(x, 720.0, "the right inset and margin, not a clamped width");
}
