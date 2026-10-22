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
}
