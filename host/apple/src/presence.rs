//! Exit animation, layout transition and the presentation values the engine
//! changed (LLP 1063; LLP 1002 §3).
//!
//! A receipt's exits name the one view of each removed subtree that leaves
//! (the kernel's rule). The host tells the presenter `exit` before the
//! batch's destroys, withholds the destroys of that view and everything
//! under it, keeps its motion node, and plays its `-exact-exit-animation` from the
//! commit's clock; when the engine passes the end, one `destroy` of the
//! leaving view lets the presenter drop the lot. The leaving view is not in
//! the mirror, so no frame, style or children op reaches it: it keeps its
//! last laid-out frame while its old siblings take its place.
//!
//! A node with a `-exact-layout-transition` has its laid-out box in its parent
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
use exact_kernel::motion::{layout_presented, node_key, LayoutMotion};
use exact_kernel::CommitReceipt;

/// One view leaving with its exit.
#[derive(Debug)]
struct Leaving {
    view: ViewId,
    key: NodeKey,
    parent: ViewId,
    animations: exact_motion::Animations,
    /// The style it last showed: its paint moves over it.
    style: String,
    /// The view and everything under it the presenter knew.
    members: Vec<ViewId>,
    /// Whether the engine has its exit, and the clock time that ends, once
    /// known: an exit waiting for the first presented frame has none yet
    /// (LLP 1003.001 D3).
    played: bool,
    end: Option<f64>,
}

/// What the host keeps for exits and layout transitions.
#[derive(Debug, Default)]
pub(crate) struct Presence {
    leaving: Vec<Leaving>,
    /// Views whose destroy waits for a leaving view's end.
    held: IdSet<ViewId>,
    /// Nodes whose `Layout` the engine holds.
    layout: LayoutMotion,
    /// A resize lays out next: positions are taken, not animated.
    pub(super) snap: bool,
    /// The presented frame a motion tick is for (LLP 1003.001 D5): every
    /// present while it is set starts what waits for it.
    pub(super) frame: Option<f64>,
    /// The wall time a display frame's commits stop the engine's input clock
    /// at (LLP 1003.001 D5): its frame tasks run at the target.
    pub(super) input_cap: Option<f64>,
    /// Each virtualized list's data generation, and whether it runs along
    /// x, at the last layout.
    generations: Vec<(ViewId, u64, bool)>,
    /// The lists whose data this layout's commit did not change: their rows'
    /// moves are the list's own (`LayoutMotion::observe_unauthored`).
    settled: IdSet<ViewId>,
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
            // A flight inside it ends: the presenter puts the view back,
            // and it leaves with the rest (LLP 1013.000).
            for id in &members {
                self.end_flight_of(*id);
            }
            batch.exit(view);
            self.presence.leaving.push(Leaving {
                view,
                key: exit.key,
                parent,
                animations: exit.animations.clone(),
                style: self.mirror.get(&view).map_or("{}", |m| &m.style).to_owned(),
                members,
                played: false,
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
    /// A leaving node's animations are the engine's to sample from here on:
    /// the presenter has no mirror for it, so no Core Animation spec (LLP
    /// 1055 D7) follows it, and the ones it had come off.
    pub(super) fn play_exits(&mut self, batch: &mut Batch) {
        for leaving in &mut self.presence.leaving {
            if !leaving.played {
                let node = motion_node(leaving.key);
                self.engine.set_node_sampled(node, true);
                batch.animations(leaving.view, "[]");
                let played = self.engine.play_exit(node, &leaving.animations);
                debug_assert!(played.is_ok(), "the kernel validated the row");
                leaving.played = true;
                leaving.end = self.engine.exit_end(node, leaving.animations.0.len());
            }
        }
    }

    /// A leaving view's last style with its presented paint over it.
    pub(super) fn restyle_leaving(&self, view: ViewId, batch: &mut Batch) {
        if let Some(l) = self.presence.leaving.iter().find(|l| l.view == view) {
            let shown = self.shown_paint(motion_node(l.key));
            let env = self.runner.kernel().env();
            batch.style(view, &style::restyle_presented(&l.style, &env, &shown));
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
    pub(super) fn seed_layout(&mut self, receipt: &CommitReceipt, batch: &mut Batch) {
        let retired = self
            .presence
            .layout
            .seed(self.runner.kernel(), receipt, &mut self.engine);
        self.retire_layout(retired, batch);
    }

    /// Before a layout's observations: the lists whose keys are what they
    /// were at the last layout and none of whose rows changed size. Their
    /// rows moved for their window, a measurement or a scroll, whichever
    /// commit carried it (a report, a scroll event, a fill, a timer).
    pub(super) fn judge_list_moves(&mut self) {
        self.presence.settled.clear();
        let now = self.runner.collection_data();
        if self.presence.layout.is_empty() {
            self.presence.generations = now;
            return;
        }
        let before: IdMap<ViewId, u64> = self
            .presence
            .generations
            .iter()
            .map(|(v, g, _)| (*v, *g))
            .collect();
        let mut across: IdMap<ViewId, bool> = IdMap::default();
        for (view, generation, horizontal) in &now {
            across.insert(*view, *horizontal);
            if before.get(view) == Some(generation) {
                self.presence.settled.insert(*view);
            }
        }
        self.presence.generations = now;
        // What only an author moves: a row's wrapper changing size (its
        // content or margins) or moving across the list (its padding), or a
        // row's box moving or resizing in its wrapper. Either takes its list
        // out of the settled ones; a wrapper that only moved along the list
        // is the list's own doing. Places are compared to a hundredth of a
        // point: they are rebuilt from absolute coordinates.
        let kernel = self.runner.kernel();
        let wraps =
            |n: exact_kernel::NodeRef| n.props.str(exact_kernel::PropId::ListItemKey).is_some();
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        for key in &self.pending_layout {
            let Some(node) = kernel.node_by_key(*key) else {
                continue;
            };
            let Some(parent) = node.parent.and_then(|p| kernel.node(p)) else {
                continue;
            };
            let Some(was) = self.mirror.get(&node.id).and_then(|m| m.frame) else {
                continue;
            };
            let is = relative(node.frame, Some(parent.frame));
            let (list, authored) = if wraps(node) {
                let horizontal = node
                    .parent
                    .and_then(|l| across.get(&l).copied())
                    .unwrap_or(false);
                let cross = if horizontal {
                    (was.1, is.1)
                } else {
                    (was.0, is.0)
                };
                (
                    node.parent,
                    !near(was.2, is.2) || !near(was.3, is.3) || !near(cross.0, cross.1),
                )
            } else if wraps(parent) {
                let moved = !near(was.0, is.0)
                    || !near(was.1, is.1)
                    || !near(was.2, is.2)
                    || !near(was.3, is.3);
                (parent.parent, moved)
            } else {
                continue;
            };
            if authored {
                if let Some(list) = list {
                    self.presence.settled.remove(&list);
                }
            }
        }
    }

    /// Observe changed boxes after layout; shared policy also observes the
    /// row placed through a moving virtualized row wrapper.
    pub(super) fn observe_layout(&mut self, key: NodeKey, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        // A settled list's row: its wrapper, or the row's box in it.
        let settled = |id: Option<ViewId>| id.is_some_and(|id| self.presence.settled.contains(&id));
        let wraps =
            |n: exact_kernel::NodeRef| n.props.str(exact_kernel::PropId::ListItemKey).is_some();
        let unauthored = !self.presence.settled.is_empty()
            && kernel.node_by_key(key).is_some_and(|n| {
                (wraps(n) && settled(n.parent))
                    || n.parent
                        .and_then(|p| kernel.node(p))
                        .is_some_and(|p| wraps(p) && settled(p.parent))
            });
        let retired = if unauthored {
            self.presence
                .layout
                .observe_unauthored(kernel, key, &mut self.engine)
        } else {
            self.presence
                .layout
                .observe(kernel, key, &mut self.engine, false)
        };
        self.retire_layout(retired, batch);
    }

    /// A resize retires every move, even when its target box did not change.
    /// Send the resets in the resize's batch without waiting for a tick.
    pub(super) fn snap_layout(&mut self, batch: &mut Batch) {
        if std::mem::take(&mut self.presence.snap) {
            let retired =
                self.presence
                    .layout
                    .observe_all(self.runner.kernel(), &mut self.engine, true);
            self.retire_layout(retired, batch);
        }
    }

    fn retire_layout(&self, retired: Vec<NodeKey>, batch: &mut Batch) {
        for key in retired {
            if let Some(&view) = self.keys.get(&key) {
                batch.present4(view, "layout", [0.0, 0.0, 1.0, 1.0]);
            }
        }
    }

    /// Every presentation value the engine changed, as `present` ops, and the
    /// exits that ended. At boot, and for a view the batch creates, only
    /// values that are not the property's identity: the presenter starts
    /// every view at identity (a reused one is reset to it), and the four
    /// motion rows are never in the style dictionary. A list row's views were
    /// four identity ops each, about half of what a fill batch carried.
    pub(super) fn present(&mut self, batch: &mut Batch, boot: bool) {
        self.start_frame();
        let now = self.engine.sample_time();
        for l in self
            .presence
            .leaving
            .iter_mut()
            .filter(|l| l.played && l.end.is_none())
        {
            l.end = self
                .engine
                .exit_end(motion_node(l.key), l.animations.0.len());
        }
        let (ended, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.presence.leaving)
            .into_iter()
            .partition(|l| l.end.is_some_and(|end| end <= now));
        self.presence.leaving = kept;
        for leaving in ended {
            self.end_exit(leaving, batch);
        }
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        let mut colored: Vec<(ViewId, bool)> = Vec::new();
        let frame = self.engine.frame();

        for p in frame {
            if p.property == Property::Height || self.present_flight(&p, batch) {
                continue;
            }
            let key = node_key(p.node);
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
                // @ref LLP 1055.000 D6 — an `svg`'s scene shows its colours;
                // a box's are its style, re-sent with the presented values.
                if !self.svg.touch(self.runner.kernel(), view) {
                    let inherits = p.property == Property::Color;
                    if let Some((_, changed)) = colored.iter_mut().find(|(id, _)| *id == view) {
                        *changed |= inherits;
                    } else {
                        colored.push((view, inherits));
                    }
                }
                continue;
            }
            // A layout box is presented as its offset from the laid-out
            // origin and its scale of the laid-out size; identity is
            // `0 0 1 1`. The other three are one or two numbers.
            let values = match p.property {
                Property::Layout => layout_presented(&self.engine, p.node, p.value),
                Property::Translate => [p.value.x, p.value.y, p.value.z, p.value.w],
                _ => [p.value.x, 0.0, 0.0, 0.0],
            };
            let identity = match p.property.identity() {
                Some(identity) => identity == p.value,
                None => values == [0.0, 0.0, 1.0, 1.0],
            };
            if (boot && identity) || (identity && batch.creates(view)) {
                continue;
            }
            if self.inline_runs.contains_key(&view) || self.svg.presented(view) {
                continue;
            }
            if self.native_protected_id(view) && !self.native_current() {
                continue;
            }
            if p.property == Property::Layout {
                batch.present4(view, "layout", values);
            } else if p.property == Property::Translate && (values[2] != 0.0 || values[3] != 0.0) {
                // Its percentages ride as `w` and `h`; the presenter
                // resolves them against the box (chess diary #4).
                batch.present4(view, "translate", values);
            } else {
                batch.present(view, p.property.name(), values[0], values[1]);
            }
        }
        for (view, inherits) in colored {
            self.present_colors(view, batch, inherits);
        }
        self.svg.emit(self.runner.kernel(), &self.engine, batch);
        self.land_flights(batch);
    }
}
