//! Layout transition on Linux, and its refusal of exit animation (LLP 1063).
//!
//! A node with a `layout-transition` has its laid-out box in its parent
//! (`Kernel::layout_box`) observed as `Property::Layout` after every layout;
//! the engine's transition rules apply. What it shows against the laid-out
//! box is an offset and scale the painter applies outermost, from the box's
//! top-left corner (`Presented::layout`), so a moving parent carries its
//! children and nothing is laid out per frame. A node that gains the row is
//! seeded with the box it had before the commit's layout, so its first move
//! after gaining it animates.
//!
//! Exit animation is refused here: this painter reads the live kernel tree
//! every frame, and a destroyed node is not in it, so there is nothing to
//! keep painting. A removed node leaves at once and the journal says so.

use super::*;
use exact_kernel::id::IdSet;
use exact_motion::Change;

/// What the host keeps for layout transitions.
#[derive(Debug, Default)]
pub(super) struct Presence {
    /// Nodes declaring a layout transition.
    pub(super) tracked: IdSet<NodeKey>,
    /// Whether the refusal of exit animation is in the journal.
    refused: bool,
    /// A resize lays out next: positions are taken, not animated.
    pub(super) snap: bool,
}

impl<D: DataSource> Host<D> {
    /// Which nodes declare a layout transition, after a commit and before
    /// its layout; and the refusal, the first time a node leaves with an exit.
    pub(super) fn track_presence(&mut self, receipts: &[Timed]) {
        for t in receipts {
            if !t.receipt.exits.is_empty() && !self.presence.refused {
                self.presence.refused = true;
                self.log("exit-animation: refused on Linux (LLP 1063): the painter reads the live tree, so a removed node leaves at once");
            }
            for key in t.receipt.created.iter().chain(&t.receipt.touched) {
                let kernel = self.runner.kernel();
                let declares = kernel.node_by_key(*key).is_some_and(|n| {
                    n.style
                        .layout_transition
                        .matching(Property::Layout)
                        .is_some()
                });
                let node = motion_node(*key);
                if declares {
                    // Gained by a node already laid out: its box until now.
                    let was = kernel.layout_box(*key);
                    if self.presence.tracked.insert(*key) && !t.receipt.created.contains(key) {
                        if let Some(value) = was {
                            self.observe_box(node, value);
                        }
                    }
                } else if self.presence.tracked.remove(key) {
                    self.engine.remove_property(node, Property::Layout);
                    if let Some(p) = self.keys.get(key).and_then(|v| self.presented.get_mut(v)) {
                        p.layout = Presented::IDENTITY.layout;
                    }
                }
            }
        }
    }

    /// Observe every tracked node's box, after a layout.
    pub(super) fn observe_layout(&mut self) {
        let snap = std::mem::take(&mut self.presence.snap);
        let tracked: Vec<NodeKey> = self.presence.tracked.iter().copied().collect();
        for key in tracked {
            if self.runner.kernel().node_by_key(key).is_none() {
                self.presence.tracked.remove(&key);
                continue;
            }
            let Some(value) = self.runner.kernel().layout_box(key) else {
                // No box while it is not displayed: shown again, first seen.
                self.engine
                    .remove_property(motion_node(key), Property::Layout);
                continue;
            };
            if snap {
                self.engine
                    .remove_property(motion_node(key), Property::Layout);
            }
            self.observe_box(motion_node(key), value);
        }
    }

    fn observe_box(&mut self, node: u64, value: exact_motion::Value) {
        let observed = self.engine.observe(Change {
            node,
            property: Property::Layout,
            value,
            velocity: None,
        });
        debug_assert!(observed.is_ok(), "layout is finite");
    }

    /// A shown layout box as the painter applies it: its offset from the
    /// laid-out origin and scale of the laid-out size.
    pub(super) fn layout_presented(&self, node: u64, shown: exact_motion::Value) -> [f32; 4] {
        let at = self.engine.target(node, Property::Layout).unwrap_or(shown);
        let scale = |shown: f64, laid: f64| if laid > 0.0 { shown / laid } else { 1.0 };
        [
            (shown.x - at.x) as f32,
            (shown.y - at.y) as f32,
            scale(shown.z, at.z) as f32,
            scale(shown.w, at.w) as f32,
        ]
    }
}
