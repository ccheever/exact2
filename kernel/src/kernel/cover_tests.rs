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
}
