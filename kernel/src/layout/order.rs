//! CSS `order` (feed F19): a flex or grid container lays its items out in
//! order-modified document order — ascending `order`, ties in document order
//! (CSS Flexbox §5.4.1, CSS Grid §8.1). The kernel hands the layout engine a
//! container's children in that order; the arena keeps document order, which
//! painting and focus follow (LLP 1001 §2 declares the paint half).

use crate::arena::NodeArena;
use taffy::prelude::{NodeId, TaffyTree};

/// `parent`'s children's engine handles in the order its layout takes them;
/// `node` is `parent`'s own handle, whose style says whether it is a flex or
/// grid container; `handle` is a child slot's handle in that engine tree.
pub(super) fn laid_out<T>(
    arena: &NodeArena,
    parent: u32,
    taffy: &TaffyTree<T>,
    node: NodeId,
    handle: impl Fn(u32) -> Option<NodeId>,
) -> Vec<NodeId> {
    let mut items: Vec<(i32, NodeId)> = arena
        .children(parent)
        .iter()
        .filter_map(|&c| Some((arena.style(c).order, handle(c)?)))
        .collect();
    let orders = taffy
        .style(node)
        .is_ok_and(|s| matches!(s.display, taffy::Display::Flex | taffy::Display::Grid));
    if orders && items.iter().any(|(order, _)| *order != 0) {
        // A stable sort: equal `order`s keep document order.
        items.sort_by_key(|(order, _)| *order);
    }
    items.into_iter().map(|(_, n)| n).collect()
}
