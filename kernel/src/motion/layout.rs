//! Native layout observation (LLP 1063). Hosts choose when to lay out;
//! the same bookkeeping feeds every native motion engine.

use super::motion_node;
use crate::id::IdSet;
use crate::{CommitReceipt, Kernel, NodeKey, PropId};
use exact_motion::{Change, Engine, Property, Value};

/// Nodes whose declared layout transitions the native engine observes.
#[derive(Debug, Default)]
pub struct LayoutMotion {
    tracked: IdSet<NodeKey>,
    /// The tracked boxes laid out in a block of their own when last seen.
    blocked: IdSet<NodeKey>,
}

impl LayoutMotion {
    /// Whether no node declares a layout transition.
    pub fn is_empty(&self) -> bool {
        self.tracked.is_empty()
    }

    /// Remember the tree's declared layout transitions at boot. Their first
    /// post-layout observation takes the value without motion.
    pub fn adopt(&mut self, kernel: &Kernel, keys: impl IntoIterator<Item = NodeKey>) {
        self.tracked
            .extend(keys.into_iter().filter(|key| declares(kernel, *key)));
    }

    /// Before a commit's layout, seed a newly declared transition with its
    /// old box. A newly created node has no old box. Returned keys no longer
    /// declare the row and need their presentation reset to identity.
    pub fn seed(
        &mut self,
        kernel: &Kernel,
        receipt: &CommitReceipt,
        engine: &mut Engine,
    ) -> Vec<NodeKey> {
        // A renewed node is a new one (LLP 1078): no old box to move from.
        for key in receipt.destroyed.iter().chain(&receipt.renewed) {
            self.tracked.remove(key);
            self.blocked.remove(key);
        }
        let mut retired = Vec::new();
        for key in receipt.created.iter().chain(&receipt.touched) {
            if declares(kernel, *key) {
                if self.tracked.insert(*key)
                    && !receipt.created.contains(key)
                    && !receipt.renewed.contains(key)
                {
                    if let Some(value) = kernel.layout_box(*key) {
                        observe_box(engine, *key, value);
                    }
                    // In the space that old box was in: crossing is from there.
                    let blocked = kernel
                        .node_by_key(*key)
                        .is_some_and(|n| kernel.host_block(n.id).is_some());
                    if blocked {
                        self.blocked.insert(*key);
                    }
                }
            } else if self.tracked.remove(key) {
                self.blocked.remove(key);
                engine.remove_property(motion_node(*key), Property::Layout);
                retired.push(*key);
            }
        }
        retired
    }

    /// Observe one laid-out box, including the row placed through a
    /// virtualized row wrapper. Hidden boxes lose their value and reappear as
    /// first seen; a resize snaps. Returned keys need identity presented.
    pub fn observe(
        &mut self,
        kernel: &Kernel,
        key: NodeKey,
        engine: &mut Engine,
        snap: bool,
    ) -> Vec<NodeKey> {
        self.observe_as(kernel, key, engine, snap, false)
    }

    /// Observe a box a list moved for no author: its window or a
    /// measurement placed rows before it (LLP 1010). An idle box takes its
    /// place with no transition, as UIKit's self-sizing does; the list keeps
    /// what shows still by its scroll offset. One already moving still
    /// moves.
    pub fn observe_unauthored(
        &mut self,
        kernel: &Kernel,
        key: NodeKey,
        engine: &mut Engine,
    ) -> Vec<NodeKey> {
        self.observe_as(kernel, key, engine, false, true)
    }

    fn observe_as(
        &mut self,
        kernel: &Kernel,
        key: NodeKey,
        engine: &mut Engine,
        snap: bool,
        unauthored: bool,
    ) -> Vec<NodeKey> {
        let mut keys = vec![key];
        if let Some(node) = kernel.node_by_key(key) {
            if node.props.str(PropId::ListItemKey).is_some() {
                keys.extend(
                    node.children()
                        .into_iter()
                        .filter_map(|id| kernel.node(id).map(|n| n.key)),
                );
            }
        }
        let mut retired = Vec::new();
        for key in keys {
            // A box that never declared a layout transition has no layout
            // target to move or retire: most of a list row's boxes.
            if !self.tracked.contains(&key) && !declares(kernel, key) {
                continue;
            }
            let node = motion_node(key);
            let Some(value) = kernel.layout_box(key) else {
                if !declares(kernel, key) {
                    self.tracked.remove(&key);
                }
                if engine.target(node, Property::Layout).is_some() {
                    engine.remove_property(node, Property::Layout);
                    retired.push(key);
                }
                continue;
            };
            self.tracked.insert(key);
            // A box moving into its block or out of it (a popover set or
            // cleared: stage 14 B) changes spaces, not place: it snaps.
            let blocked = kernel
                .node_by_key(key)
                .is_some_and(|n| kernel.host_block(n.id).is_some());
            let crossed = if blocked {
                self.blocked.insert(key)
            } else {
                self.blocked.remove(&key)
            };
            if (snap || crossed) && engine.remove_property(node, Property::Layout) {
                retired.push(key);
            }
            if unauthored {
                let observed = engine.observe_settled(Change {
                    node,
                    property: Property::Layout,
                    value,
                    velocity: None,
                });
                debug_assert!(observed.is_ok(), "layout is finite");
            } else {
                observe_box(engine, key, value);
            }
        }
        retired
    }

    /// Observe all declared boxes after a layout when the host has no
    /// separate list of changed frames.
    pub fn observe_all(
        &mut self,
        kernel: &Kernel,
        engine: &mut Engine,
        snap: bool,
    ) -> Vec<NodeKey> {
        let keys: Vec<_> = self.tracked.iter().copied().collect();
        keys.into_iter()
            .flat_map(|key| self.observe(kernel, key, engine, snap))
            .collect()
    }
}

fn declares(kernel: &Kernel, key: NodeKey) -> bool {
    kernel.node_by_key(key).is_some_and(|n| {
        n.style
            .rare
            .layout_transition
            .matching(Property::Layout)
            .is_some()
    })
}

fn observe_box(engine: &mut Engine, key: NodeKey, value: Value) {
    let observed = engine.observe(Change {
        node: motion_node(key),
        property: Property::Layout,
        value,
        velocity: None,
    });
    debug_assert!(observed.is_ok(), "layout is finite");
}

/// A presented box as `[dx, dy, dw, dh]`: its offset from the laid-out
/// origin, and how far its surface's size, never below zero (a spring can
/// overshoot it), stands from the laid-out size. Identity is all zeros.
/// Content retains its laid-out geometry. A difference rather than a ratio,
/// so a box laid out at zero size still shows the size it closes from.
pub fn layout_presented(engine: &Engine, node: u64, shown: Value) -> [f64; 4] {
    let at = engine.target(node, Property::Layout).unwrap_or(shown);
    [
        shown.x - at.x,
        shown.y - at.y,
        shown.z.max(0.0) - at.z,
        shown.w.max(0.0) - at.w,
    ]
}
