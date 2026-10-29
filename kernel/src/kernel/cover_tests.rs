//! What a scroller's platform inset re-derives (theme2's Q2 on the bottom
//! edge's chain, `cover::scroller_insets_bottom`): the nearest box the bars
//! cover, however deep the scroller, and nothing further up.
use super::*;
use crate::layout::LayoutTree;
use crate::{HostCover, StyleValue};
use std::cell::RefCell;
use std::rc::Rc;

/// The engine, recording the slots whose style is derived again.
struct Restyles(Box<dyn LayoutMirror>, Rc<RefCell<Vec<u32>>>);
impl LayoutMirror for Restyles {
    fn new_leaf(&mut self, arena: &NodeArena, slot: u32, measured: bool) -> Option<taffy::NodeId> {
        self.0.new_leaf(arena, slot, measured)
    }
    fn new_leaf_unstyled(&mut self, slot: u32, measured: bool) -> Option<taffy::NodeId> {
        self.0.new_leaf_unstyled(slot, measured)
    }
    fn restyle(&mut self, arena: &NodeArena, slot: u32, node: taffy::NodeId) {
        self.1.borrow_mut().push(slot);
        self.0.restyle(arena, slot, node)
    }
    fn sync_children(&mut self, arena: &NodeArena, parent: u32, node: taffy::NodeId) -> usize {
        self.0.sync_children(arena, parent, node)
    }
    fn remove(&mut self, node: taffy::NodeId) {
        self.0.remove(node)
    }
    fn mark_dirty(&mut self, node: taffy::NodeId) {
        self.0.mark_dirty(node)
    }
    fn attached(&self, node: taffy::NodeId) -> bool {
        self.0.attached(node)
    }
    fn tree(&mut self) -> Option<&mut LayoutTree> {
        self.0.tree()
    }
    fn tree_ref(&self) -> Option<&LayoutTree> {
        self.0.tree_ref()
    }
}

fn style(id: ViewId, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

#[test]
fn an_inset_three_boxes_down_restyles_its_route_and_nothing_above() {
    let mut k = Kernel::with_monospace();
    let column = |id| {
        style(
            id,
            &[
                (StyleId::Display, StyleValue::Text("flex".into())),
                (StyleId::FlexDirection, StyleValue::Text("column".into())),
                (StyleId::FlexGrow, StyleValue::Number(1.0)),
                (StyleId::MinHeight, StyleValue::Number(0.0)),
            ],
        )
    };
    let mut ops = vec![];
    // 1 the root; 2 a sheet the bars cover too; 3 its route; 4, 5 boxes
    // down the route; 6 the scroller at their foot.
    for id in 1..=6 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
        ops.push(column(id));
    }
    ops.push(style(1, &[(StyleId::Height, StyleValue::Number(844.0))]));
    ops.push(style(
        6,
        &[(StyleId::OverflowY, StyleValue::Text("scroll".into()))],
    ));
    for (parent, child) in [(1, 2), (2, 3), (3, 4), (4, 5), (5, 6)] {
        ops.push(Op::SetChildren {
            id: parent,
            children: vec![child],
        });
    }
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.set_host_cover(2, Some(HostCover::Edges([0.0, 0.0, 34.0, 0.0])))
        .unwrap();
    k.set_host_cover(3, Some(HostCover::Edges([0.0, 0.0, 83.0, 0.0])))
        .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let engine = k.layout.take().unwrap();
    k.layout = Some(Box::new(Restyles(engine, seen.clone())));
    let slot = |k: &Kernel, id| k.arena.slot_of(id).unwrap();
    let (sheet, route, scroller) = (slot(&k, 2), slot(&k, 3), slot(&k, 6));
    k.set_host_cover(6, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    let restyled = seen.borrow().clone();
    assert!(
        restyled.contains(&scroller) && restyled.contains(&route),
        "{restyled:?}"
    );
    assert!(
        !restyled.contains(&sheet),
        "nothing above the route: {restyled:?}"
    );
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    let f = k.node(6).unwrap().frame;
    assert_eq!(
        f.y + f.height,
        844.0 - 34.0,
        "the route pads nothing; the sheet still does"
    );
    // The sheet derived again for a change of its own: the route's scroller
    // is the route's, not the sheet's (Astra's and Grok's round 1).
    k.apply(
        1,
        2,
        &[style(2, &[(StyleId::PaddingLeft, StyleValue::Number(1.0))])],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    let f = k.node(6).unwrap().frame;
    assert_eq!(f.y + f.height, 844.0 - 34.0, "the sheet keeps its padding");
}

/// The covers said in every order (round 2: a covered box that comes or
/// goes decides where the chain stops), then the route's bar cleared: its
/// scroller is then the sheet's.
#[test]
fn the_covers_in_any_order_leave_the_sheet_its_padding() {
    let sheet = Some(HostCover::Edges([0.0, 0.0, 34.0, 0.0]));
    let route = Some(HostCover::Edges([0.0, 0.0, 83.0, 0.0]));
    let inset = Some(HostCover::Inset([0.0, 83.0]));
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let mut k = Kernel::with_monospace();
        let mut ops = vec![];
        for id in 1..=6 {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::View,
            });
            ops.push(style(
                id,
                &[
                    (StyleId::Display, StyleValue::Text("flex".into())),
                    (StyleId::FlexDirection, StyleValue::Text("column".into())),
                    (StyleId::FlexGrow, StyleValue::Number(1.0)),
                    (StyleId::MinHeight, StyleValue::Number(0.0)),
                ],
            ));
        }
        ops.push(style(1, &[(StyleId::Height, StyleValue::Number(844.0))]));
        ops.push(style(
            6,
            &[(StyleId::OverflowY, StyleValue::Text("scroll".into()))],
        ));
        for (parent, child) in [(1, 2), (2, 3), (3, 4), (4, 5), (5, 6)] {
            ops.push(Op::SetChildren {
                id: parent,
                children: vec![child],
            });
        }
        ops.push(Op::AttachRoot { id: 1 });
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
        for i in order {
            let (id, cover) = [(2, sheet), (3, route), (6, inset)][i];
            k.set_host_cover(id, cover).unwrap();
        }
        k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
        let f = k.node(6).unwrap().frame;
        assert_eq!(f.y + f.height, 844.0 - 34.0, "{order:?}");
        k.set_host_cover(3, None).unwrap();
        k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
        let f = k.node(6).unwrap().frame;
        assert_eq!(
            f.y + f.height,
            844.0,
            "the route uncovered, the sheet's scroller: {order:?}"
        );
    }
}

/// HTML's UA sheet puts an open popover in the top layer (`position:
/// fixed`): uncovered for a host to lift, it takes no room in its parent's
/// flow, so a footer before it stays at the column's foot (Harmony's room
/// composer, a popover written after it, jumped up by the popover's height).
#[test]
fn an_open_popover_takes_no_room_in_its_parents_flow() {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    // 1 a column route; 2 a list that grows; 3 the footer; 4 a popover written after it.
    for id in 1..=4 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.push(style(
        1,
        &[
            (StyleId::Display, StyleValue::Text("flex".into())),
            (StyleId::FlexDirection, StyleValue::Text("column".into())),
            (StyleId::Height, StyleValue::Number(800.0)),
        ],
    ));
    ops.push(style(
        2,
        &[
            (StyleId::FlexGrow, StyleValue::Number(1.0)),
            (StyleId::MinHeight, StyleValue::Number(0.0)),
        ],
    ));
    ops.push(style(3, &[(StyleId::Height, StyleValue::Number(50.0))]));
    ops.push(style(4, &[(StyleId::Height, StyleValue::Number(300.0))]));
    ops.push(Op::SetProp {
        id: 4,
        prop: crate::PropId::Popover,
        value: crate::props::PropValue::Str("auto".into()),
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2, 3, 4],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(390.0, 800.0)).unwrap();
    assert_eq!(
        k.node(3).unwrap().frame.y,
        750.0,
        "the footer at the column's foot"
    );
    assert_eq!(
        k.node(2).unwrap().frame.height,
        750.0,
        "the list keeps its room"
    );
    assert_eq!(
        k.node(4).unwrap().frame.height,
        300.0,
        "the popover keeps its own size"
    );
    // The prop cleared, the box is in flow again; set again, out of it.
    k.apply(
        1,
        2,
        &[Op::ClearProp {
            id: 4,
            prop: crate::PropId::Popover,
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 800.0)).unwrap();
    assert_eq!(
        k.node(3).unwrap().frame.y,
        450.0,
        "no popover: the box takes its room"
    );
    k.apply(
        2,
        3,
        &[Op::SetProp {
            id: 4,
            prop: crate::PropId::Popover,
            value: crate::props::PropValue::Str("manual".into()),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 800.0)).unwrap();
    assert_eq!(
        k.node(3).unwrap().frame.y,
        750.0,
        "a popover again: out of flow"
    );
}

/// A popover written after a route's bottom-edge scroller leaves the
/// scroller the edge, open or closed (Astra's quick review): the route
/// pads nothing of its bottom bar, and setting or clearing the prop on the
/// box after it derives the route again.
#[test]
fn a_popover_after_the_scroller_leaves_it_the_bottom_edge() {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    // 1 the root; 2 the route the bars cover; 3 its scroller; 4 a box after it.
    for id in 1..=4 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    let column = [
        (StyleId::Display, StyleValue::Text("flex".into())),
        (StyleId::FlexDirection, StyleValue::Text("column".into())),
    ];
    ops.push(style(
        1,
        &[
            column[0].clone(),
            column[1].clone(),
            (StyleId::Height, StyleValue::Number(844.0)),
        ],
    ));
    ops.push(style(
        2,
        &[
            column[0].clone(),
            column[1].clone(),
            (StyleId::FlexGrow, StyleValue::Number(1.0)),
        ],
    ));
    ops.push(style(
        3,
        &[
            (StyleId::FlexGrow, StyleValue::Number(1.0)),
            (StyleId::MinHeight, StyleValue::Number(0.0)),
            (StyleId::OverflowY, StyleValue::Text("scroll".into())),
        ],
    ));
    ops.push(style(4, &[(StyleId::Height, StyleValue::Number(100.0))]));
    ops.push(Op::SetProp {
        id: 4,
        prop: crate::PropId::Popover,
        value: crate::props::PropValue::Str("auto".into()),
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2],
    });
    ops.push(Op::SetChildren {
        id: 2,
        children: vec![3, 4],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.set_host_cover(2, Some(HostCover::Edges([0.0, 0.0, 83.0, 0.0])))
        .unwrap();
    k.set_host_cover(3, Some(HostCover::Inset([0.0, 83.0])))
        .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    let bottom = |k: &Kernel| {
        let f = k.node(3).unwrap().frame;
        f.y + f.height
    };
    assert_eq!(
        bottom(&k),
        844.0,
        "open, the popover leaves the scroller the edge"
    );
    k.apply(
        1,
        2,
        &[Op::ClearProp {
            id: 4,
            prop: crate::PropId::Popover,
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    assert_eq!(
        bottom(&k),
        844.0 - 83.0 - 100.0,
        "a box in flow after it: the route pads its bar again"
    );
    k.apply(
        2,
        3,
        &[Op::SetProp {
            id: 4,
            prop: crate::PropId::Popover,
            value: crate::props::PropValue::Str("auto".into()),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(390.0, 844.0)).unwrap();
    assert_eq!(
        bottom(&k),
        844.0,
        "a popover again: the edge is the scroller's"
    );
}

/// A popover is laid out in the viewport, a block of its own (stage 14 B):
/// its parent's cover neither offsets nor trims it, before or after the
/// cover changes. A host places it.
#[test]
fn a_popover_is_out_of_its_parents_cover() {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    for id in 1..=2 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.push(style(1, &[(StyleId::Height, StyleValue::Number(800.0))]));
    ops.push(style(
        2,
        &[
            (StyleId::Top, StyleValue::Number(0.0)),
            (StyleId::Height, StyleValue::Number(40.0)),
        ],
    ));
    ops.push(Op::SetProp {
        id: 2,
        prop: crate::PropId::Popover,
        value: crate::props::PropValue::Str("auto".into()),
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    for top in [50.0, 80.0] {
        k.set_host_cover(1, Some(HostCover::Edges([top, 0.0, 0.0, 0.0])))
            .unwrap();
        k.compute_layout(1, Offer::definite(390.0, 800.0)).unwrap();
        let frame = k.node(2).unwrap().frame;
        assert_eq!(
            (frame.y, frame.height),
            (0.0, 40.0),
            "under a {top}-point bar"
        );
        assert_eq!(k.host_block_size(2), Some((0.0, 40.0)));
    }
}

#[test]
fn measured_scroller_gutters_change_only_the_available_content_space() {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            style(
                1,
                &[
                    (StyleId::Width, StyleValue::Number(300.0)),
                    (StyleId::Height, StyleValue::Number(200.0)),
                    (StyleId::OverflowX, StyleValue::Text("auto".into())),
                    (StyleId::OverflowY, StyleValue::Text("auto".into())),
                ],
            ),
            style(
                2,
                &[
                    (StyleId::Width, StyleValue::Text("100%".into())),
                    (StyleId::Height, StyleValue::Text("100%".into())),
                ],
            ),
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    for gutter in [[15.0, 0.0], [0.0, 15.0], [15.0, 15.0], [0.0, 0.0]] {
        k.set_host_cover(1, Some(HostCover::Gutter(gutter)))
            .unwrap();
        k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
        assert_eq!(k.node(1).unwrap().frame.width, 300.0);
        let child = k.node(2).unwrap().frame;
        assert_eq!(
            (child.width, child.height),
            (300.0 - gutter[0], 200.0 - gutter[1])
        );
        assert!(!k.node(1).unwrap().style.mask.has(StyleId::PaddingRight));
    }
    assert!(k
        .set_host_cover(1, Some(HostCover::Gutter([f32::NAN, 0.0])))
        .is_err());
}

#[test]
fn absolute_auto_width_cover_stays_inside_the_containing_padding_box() {
    for display in ["block", "flex"] {
        for sizing in ["content-box", "border-box"] {
            let mut k = Kernel::with_monospace();
            let text = |s: &'static str| StyleValue::Text(s.into());
            k.apply(
                0,
                1,
                &[
                    Op::CreateView {
                        id: 1,
                        node_type: NodeType::View,
                    },
                    Op::CreateView {
                        id: 2,
                        node_type: NodeType::View,
                    },
                    Op::CreateView {
                        id: 3,
                        node_type: NodeType::View,
                    },
                    style(
                        1,
                        &[
                            (StyleId::Display, text(display)),
                            (StyleId::Height, StyleValue::Number(800.)),
                        ],
                    ),
                    style(
                        2,
                        &[
                            (StyleId::Display, text("flex")),
                            (StyleId::FlexDirection, text("column")),
                            (StyleId::PositionType, text("absolute")),
                            (StyleId::BoxSizing, text(sizing)),
                            (StyleId::Left, StyleValue::Number(0.)),
                            (StyleId::Right, StyleValue::Number(0.)),
                            (StyleId::Top, StyleValue::Number(0.)),
                            (StyleId::Bottom, StyleValue::Number(0.)),
                        ],
                    ),
                    Op::SetChildren {
                        id: 1,
                        children: vec![2],
                    },
                    Op::SetChildren {
                        id: 2,
                        children: vec![3],
                    },
                    Op::AttachRoot { id: 1 },
                ],
            )
            .unwrap();
            for width in [1200., 1400., 1000.] {
                for _ in 0..4 {
                    k.set_host_cover(2, Some(HostCover::Edges([0., width - 320., 0., 0.])))
                        .unwrap();
                    k.compute_layout(1, Offer::definite(width, 800.)).unwrap();
                    assert_eq!(k.node(2).unwrap().frame.width, width, "{display} {sizing}");
                    assert_eq!(k.node(3).unwrap().frame.width, 320.);
                }
            }
        }
    }
}
