//! Rehydrate or fork the layout engine tree from arena columns.

use super::*;
use std::collections::HashMap;

impl LayoutTree {
    /// The engine node holding `node`, if any.
    pub(crate) fn engine_parent(&self, node: NodeId) -> Option<NodeId> {
        self.taffy.parent(node)
    }

    /// Reconstruct the whole engine tree from the arena's columns, writing the
    /// new handles back. This is rehydration: columns plus rebuild.
    pub fn rebuild(arena: &mut NodeArena) -> LayoutTree {
        let mut tree = LayoutTree::new();
        arena.clear_taffy();
        let slots: Vec<u32> = arena
            .iter_live()
            .filter(|s| !arena.in_paragraph(*s))
            .collect();
        for slot in &slots {
            let node = tree.new_leaf(
                taffy_style(arena, *slot),
                *slot,
                arena.node_type(*slot).is_measured_leaf(),
            );
            arena.set_taffy(*slot, Some(node));
            if arena.has_alert_prefix(*slot) {
                tree.new_alert_prefix(*slot, arena.grouped_alert(*slot));
            }
        }
        for slot in &slots {
            // Inline runs and a select's options are never laid out.
            if matches!(arena.node_type(*slot), NodeType::Text | NodeType::Control) {
                continue;
            }
            if let Some(node) = arena.taffy(*slot) {
                let before = tree.alert_prefixes.get(slot).copied();
                let children = order::laid_out(arena, *slot, &tree.taffy, node, before, |child| {
                    arena.taffy(child)
                });
                tree.set_children(node, &children);
            }
        }
        tree
    }

    /// A separate engine tree of the subtrees at `roots`, for a trial that
    /// must leave the ordinary tree, its caches and its frames alone; its
    /// handles by slot. The arena's own handles are never written.
    pub(crate) fn of_subtrees(
        arena: &NodeArena,
        roots: &[u32],
    ) -> (LayoutTree, HashMap<u32, NodeId>) {
        let mut tree = LayoutTree::new();
        let slots: Vec<u32> = roots.iter().flat_map(|&root| arena.subtree(root)).collect();
        let nodes: HashMap<u32, NodeId> = slots
            .iter()
            .map(|&slot| {
                let node = tree.new_leaf(
                    taffy_style(arena, slot),
                    slot,
                    arena.node_type(slot).is_measured_leaf(),
                );
                (slot, node)
            })
            .collect();
        for &slot in &slots {
            if arena.has_alert_prefix(slot) {
                tree.new_alert_prefix(slot, arena.grouped_alert(slot));
            }
        }
        for slot in slots {
            if matches!(arena.node_type(slot), NodeType::Text | NodeType::Control) {
                continue;
            }
            let node = nodes[&slot];
            let before = tree.alert_prefixes.get(&slot).copied();
            let children = order::laid_out(arena, slot, &tree.taffy, node, before, |child| {
                nodes.get(&child).copied()
            });
            tree.set_children(node, &children);
        }
        (tree, nodes)
    }

    /// Give `node`, standing in for `parent`, those of `parent`'s children
    /// `nodes` holds, in the order its layout takes them.
    pub(crate) fn adopt(
        &mut self,
        arena: &NodeArena,
        parent: u32,
        node: NodeId,
        nodes: &HashMap<u32, NodeId>,
    ) {
        let before = if arena.has_alert_prefix(parent) {
            match self.alert_prefixes.get(&parent).copied() {
                Some(prefix) => Some(prefix),
                None => Some(self.new_alert_prefix(parent, arena.grouped_alert(parent))),
            }
        } else {
            None
        };
        let children = order::laid_out(arena, parent, &self.taffy, node, before, |child| {
            nodes.get(&child).copied()
        });
        self.set_children(node, &children);
    }
}
