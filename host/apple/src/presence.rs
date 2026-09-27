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
//! A node with a `layout-transition` has its laid-out origin in its parent
//! observed as `Property::Layout` after every layout that moved it. The
//! engine's transition rules apply (first seen takes the value, an interrupt
//! starts from where it is); the presenter gets the difference between what
//! the engine shows and the laid-out origin as a `layout` offset it adds to
//! the view's translation, so a frame is never re-laid out per tick and a
//! moving parent carries its children. A resize takes new positions at once.

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
                    .restart_animations(motion_node(leaving.key), leaving.animations.clone());
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

    /// Observe a laid-out node's origin in its parent, when it declares a
    /// layout transition; retire it when it no longer does.
    pub(super) fn observe_layout(&mut self, key: NodeKey, declared: bool, x: f32, y: f32) {
        let node = motion_node(key);
        if !declared {
            if self.presence.layout.remove(&key) {
                self.engine.remove_property(node, Property::Layout);
            }
            return;
        }
        if self.presence.snap || self.presence.layout.insert(key) {
            self.engine.remove_property(node, Property::Layout);
        }
        let observed = self.engine.observe(Change {
            node,
            property: Property::Layout,
            value: exact_motion::Value::new(x as f64, y as f64),
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
            let (x, y) = match p.property {
                // The offset from the laid-out origin; zero is its identity.
                Property::Layout => {
                    let at = self
                        .engine
                        .target(p.node, Property::Layout)
                        .unwrap_or(p.value);
                    (p.value.x - at.x, p.value.y - at.y)
                }
                Property::Translate => (p.value.x, p.value.y),
                _ => (p.value.x, 0.0),
            };
            let identity = match p.property.identity() {
                Some(identity) => identity == p.value,
                None => (x, y) == (0.0, 0.0),
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
            batch.present(view, p.property.name(), x, y);
        }
    }
}
