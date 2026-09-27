//! Exit animation, layout transition and the presentation values the engine
//! changed (LLP 1063; LLP 1002 §3).
//!
//! A receipt's exits name the one view of each removed subtree that leaves
//! (the kernel's rule). The host tells the presenter `exit` before the
//! batch's destroys, withholds the destroys of that view and everything
//! under it, keeps its motion node, and plays its `exit-animation` from the
//! commit's clock; when the engine passes the end, one `destroy` of the
//! leaving view lets the presenter drop the lot. The leaving view is not in
//! the mirror, so no frame, style or children op reaches it: it keeps its
//! last laid-out frame while its old siblings take its place.
//!
//! A node with a `layout-transition` has its laid-out box in its parent
//! (`Kernel::layout_box`) observed as `Property::Layout` after every layout
//! that moved or resized it. The engine's transition rules apply (first seen
//! takes the value, an interrupt starts from where it is); the presenter gets
//! what the engine shows against the laid-out box as a `layout` offset and
//! scale, applied outermost from the box's top-left corner as a web FLIP
//! is, so a frame is never re-laid out per tick and a moving parent carries
//! its children. A resize takes new boxes at once. A node that gains the row
//! is seeded with its box before the commit's layout, so its first move
//! after gaining it animates, as a CSS transition gained in the same style
//! change runs.

use super::*;
use exact_kernel::CommitReceipt;

/// One view leaving with its exit.
#[derive(Debug)]
struct Leaving {
    view: ViewId,
    key: NodeKey,
    parent: ViewId,
    animations: exact_motion::Animations,
    /// The view and everything under it the presenter knew.
    members: Vec<ViewId>,
    /// The clock time its exit ends, once the engine has heard it.
    end: Option<f64>,
}

/// What the host keeps for exits and layout transitions.
#[derive(Debug, Default)]
pub(crate) struct Presence {
    leaving: Vec<Leaving>,
    /// Views whose destroy waits for a leaving view's end.
    held: IdSet<ViewId>,
    /// Nodes whose `Layout` the engine holds.
    layout: IdSet<NodeKey>,
    /// A resize lays out next: positions are taken, not animated.
    pub(super) snap: bool,
}

impl<D: DataSource> Host<D> {
    /// A receipt's exits, before its destroys reach the batch: each leaving
    /// view and everything under it stay with the presenter. A view this
    /// batch created was never presented, so it simply goes.
    pub(super) fn begin_exits(&mut self, receipt: &CommitReceipt, batch: &mut Batch) {
        for exit in &receipt.exits {
            let (Some(&view), Some(&parent)) =
                (self.keys.get(&exit.key), self.keys.get(&exit.parent))
            else {
                continue;
            };
            if batch.creates(view) || self.native_selected_id(view) {
                continue;
            }
            let mut members = Vec::new();
            let mut stack = vec![view];
            while let Some(id) = stack.pop() {
                members.push(id);
                self.presence.held.insert(id);
                if let Some(m) = self.mirror.get(&id) {
                    stack.extend(&m.children);
                }
            }
            batch.exit(view);
            self.presence.leaving.push(Leaving {
                view,
                key: exit.key,
                parent,
                animations: exit.animations.clone(),
                members,
                end: None,
            });
        }
    }

    /// Whether a destroyed view's `destroy` waits for an exit. A destroyed
    /// view that a leaving view left from ends that exit now: nested exits
    /// play only on the outermost.
    pub(super) fn exit_holds(&mut self, id: ViewId, batch: &mut Batch) -> bool {
        let (ended, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.presence.leaving)
            .into_iter()
            .partition(|l| l.parent == id);
        self.presence.leaving = kept;
        for leaving in ended {
            self.end_exit(leaving, batch);
        }
        self.presence.held.contains(&id)
    }

    /// Spare the leaving views' motion nodes from a commit's removals.
    pub(super) fn spare_exits(&self, sync: &mut MotionSync) {
        if !self.presence.leaving.is_empty() {
            sync.removed.retain(|n| {
                !self
                    .presence
                    .leaving
                    .iter()
                    .any(|l| motion_node(l.key) == *n)
            });
        }
    }

    /// Start every exit the engine has not heard, at the engine's clock.
    pub(super) fn play_exits(&mut self) {
        for leaving in &mut self.presence.leaving {
            if leaving.end.is_none() {
                let end = self
                    .engine
                    .play_exit(motion_node(leaving.key), leaving.animations.clone());
                debug_assert!(end.is_ok(), "the kernel validated the row");
                leaving.end = Some(end.unwrap_or(0.0));
            }
        }
    }

    fn end_exit(&mut self, leaving: Leaving, batch: &mut Batch) {
        for id in &leaving.members {
            self.presence.held.remove(id);
        }
        self.engine.remove(motion_node(leaving.key));
        batch.destroy(leaving.view);
    }

    /// Before a commit's layout: each node the commit gave the row starts
    /// from the box it had, which layout has not replaced yet. A node the
    /// commit created has none; its first layout is first seen.
    pub(super) fn seed_layout(&mut self, receipt: &CommitReceipt) {
        for key in &receipt.touched {
            if receipt.created.contains(key) || self.presence.layout.contains(key) {
                continue;
            }
            if let Some(value) = self.runner.kernel().layout_box(*key) {
                self.presence.layout.insert(*key);
                self.observe_box(*key, value);
            }
        }
    }

    /// Observe a laid-out node's box when it declares a layout transition,
    /// and retire it when it no longer does. A windowed row's wrapper moving
    /// moves the row placed through it.
    pub(super) fn observe_layout(&mut self, key: NodeKey) {
        let kernel = self.runner.kernel();
        let Some(node) = kernel.node_by_key(key) else {
            return;
        };
        let mut keys = vec![key];
        if node.props.str(PropId::ListItemKey).is_some() {
            keys.extend(
                node.children()
                    .into_iter()
                    .filter_map(|c| kernel.node(c).map(|n| n.key)),
            );
        }
        for key in keys {
            let node = motion_node(key);
            let Some(value) = self.runner.kernel().layout_box(key) else {
                if self.presence.layout.remove(&key) {
                    self.engine.remove_property(node, Property::Layout);
                }
                continue;
            };
            if self.presence.snap || self.presence.layout.insert(key) {
                self.engine.remove_property(node, Property::Layout);
            }
            self.observe_box(key, value);
        }
    }

    fn observe_box(&mut self, key: NodeKey, value: exact_motion::Value) {
        let observed = self.engine.observe(Change {
            node: motion_node(key),
            property: Property::Layout,
            value,
            velocity: None,
        });
        debug_assert!(observed.is_ok(), "layout is finite");
    }

    /// Every presentation value the engine changed, as `present` ops, and the
    /// exits that ended. At boot, and for a view the batch creates, only
    /// values that are not the property's identity: the presenter starts
    /// every view at identity (a reused one is reset to it), and the four
    /// motion rows are never in the style dictionary. A list row's views were
    /// four identity ops each, about half of what a fill batch carried.
    pub(super) fn present(&mut self, batch: &mut Batch, boot: bool) {
        let now = self.engine.now();
        let (ended, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.presence.leaving)
            .into_iter()
            .partition(|l| l.end.is_some_and(|end| end <= now));
        self.presence.leaving = kept;
        for leaving in ended {
            self.end_exit(leaving, batch);
        }
        self.presence
            .layout
            .retain(|key| self.keys.contains_key(key));
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        for p in self.engine.frame() {
            if p.property == Property::Height {
                continue;
            }
            let key = NodeKey {
                index: p.node as u32,
                generation: (p.node >> 32) as u32,
            };
            let Some(view) = self.keys.get(&key).copied().or_else(|| {
                self.presence
                    .leaving
                    .iter()
                    .find(|l| l.key == key)
                    .map(|l| l.view)
            }) else {
                continue;
            };
            if Property::PAINT.contains(&p.property) {
                self.present_paint(p, view, batch);
                continue;
            }
            // A layout box is presented as its offset from the laid-out
            // origin and its scale of the laid-out size; identity is
            // `0 0 1 1`. The other three are one or two numbers.
            let values = match p.property {
                Property::Layout => {
                    let at = self
                        .engine
                        .target(p.node, Property::Layout)
                        .unwrap_or(p.value);
                    let scale = |shown: f64, laid: f64| if laid > 0.0 { shown / laid } else { 1.0 };
                    [
                        p.value.x - at.x,
                        p.value.y - at.y,
                        scale(p.value.z, at.z),
                        scale(p.value.w, at.w),
                    ]
                }
                Property::Translate => [p.value.x, p.value.y, 0.0, 0.0],
                _ => [p.value.x, 0.0, 0.0, 0.0],
            };
            let identity = match p.property.identity() {
                Some(identity) => identity == p.value,
                None => values == [0.0, 0.0, 1.0, 1.0],
            };
            if (boot && identity) || (identity && batch.creates(view)) {
                continue;
            }
            if self.inline_runs.contains_key(&view) {
                continue;
            }
            if self.native_protected_id(view) && !self.native_current() {
                continue;
            }
            if p.property == Property::Layout {
                batch.present4(view, "layout", values);
            } else {
                batch.present(view, p.property.name(), values[0], values[1]);
            }
        }
    }
}
