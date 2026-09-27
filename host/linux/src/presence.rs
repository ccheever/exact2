//! Layout transition on Linux, and its refusal of exit animation (LLP 1063).
//!
//! A node with a `layout-transition` has its laid-out origin in its parent
//! observed as `Property::Layout` after every layout; the engine's transition
//! rules apply. What it shows minus the laid-out origin is an offset the
//! painter adds to the node's translation, so a moving parent carries its
//! children and nothing is laid out per frame.
//!
//! Exit animation is refused here: this painter reads the live kernel tree
//! every frame, and a destroyed node is not in it, so there is nothing to
//! keep painting. A removed node leaves at once and the journal says so.

use super::*;
use exact_kernel::id::IdSet;

/// What the host keeps for layout transitions.
#[derive(Debug, Default)]
pub(super) struct Presence {
    /// Nodes declaring a layout transition.
    pub(super) tracked: IdSet<NodeKey>,
    /// The offset each view is painted at, from where layout put it.
    pub(super) offsets: BTreeMap<ViewId, (f32, f32)>,
    /// Whether the refusal of exit animation is in the journal.
    refused: bool,
    /// A resize lays out next: positions are taken, not animated.
    pub(super) snap: bool,
}

impl<D: DataSource> Host<D> {
    /// Which nodes declare a layout transition, after a commit; and the
    /// refusal, the first time a node leaves with an exit.
    pub(super) fn track_presence(&mut self, receipts: &[Timed]) {
        for t in receipts {
            if !t.receipt.exits.is_empty() && !self.presence.refused {
                self.presence.refused = true;
                self.log("exit-animation: refused on Linux (LLP 1063): the painter reads the live tree, so a removed node leaves at once");
            }
            for key in t.receipt.created.iter().chain(&t.receipt.touched) {
                let declared = self.runner.kernel().node_by_key(*key).is_some_and(|n| {
                    n.style
                        .layout_transition
                        .matching(Property::Layout)
                        .is_some()
                });
                if declared {
                    self.presence.tracked.insert(*key);
                } else if self.presence.tracked.remove(key) {
                    self.engine
                        .remove_property(motion_node(*key), Property::Layout);
                    if let Some(view) = self.keys.get(key) {
                        self.presence.offsets.remove(view);
                    }
                }
            }
        }
    }

    /// Observe every tracked node's origin in its parent, after a layout.
    pub(super) fn observe_layout(&mut self) {
        let kernel = self.runner.kernel();
        let mut gone = Vec::new();
        let mut seen = Vec::new();
        for key in &self.presence.tracked {
            let Some(node) = kernel.node_by_key(*key) else {
                gone.push(*key);
                continue;
            };
            let parent = node.parent.and_then(|p| kernel.node(p)).map(|p| p.frame);
            let (px, py) = parent.map_or((0.0, 0.0), |p| (p.x, p.y));
            seen.push((*key, node.frame.x - px, node.frame.y - py));
        }
        for key in gone {
            self.presence.tracked.remove(&key);
        }
        let snap = std::mem::take(&mut self.presence.snap);
        for (key, x, y) in seen {
            if snap {
                self.engine
                    .remove_property(motion_node(key), Property::Layout);
            }
            let observed = self.engine.observe(Change {
                node: motion_node(key),
                property: Property::Layout,
                value: exact_motion::Value::new(x as f64, y as f64),
                velocity: None,
            });
            debug_assert!(observed.is_ok(), "layout is finite");
        }
    }

    /// The painted translation of `view`: the engine's `translate` and the
    /// layout offset, when `property` changed either.
    pub(super) fn present_translate(
        &mut self,
        view: ViewId,
        node: u64,
        property: Property,
    ) -> (f32, f32) {
        if property == Property::Layout {
            let shown = self.engine.value(node, Property::Layout);
            let at = self.engine.target(node, Property::Layout);
            if let (Some(shown), Some(at)) = (shown, at) {
                let offset = ((shown.x - at.x) as f32, (shown.y - at.y) as f32);
                if offset == (0.0, 0.0) {
                    self.presence.offsets.remove(&view);
                } else {
                    self.presence.offsets.insert(view, offset);
                }
            }
        }
        let authored = self
            .engine
            .value(node, Property::Translate)
            .map_or((0.0, 0.0), |v| (v.x as f32, v.y as f32));
        let offset = self
            .presence
            .offsets
            .get(&view)
            .copied()
            .unwrap_or((0.0, 0.0));
        (authored.0 + offset.0, authored.1 + offset.1)
    }
}
