//! Dropping across lists (LLP 1094 D5–D9) on Linux: a grip in a list with a
//! `reorderGroup` lifts its row as a ghost (`paint/lift.rs`) that follows
//! the contact; the ghost's centre picks the grouped list whose scrollport
//! holds it and the gap there (the runner's `preview_reorder_into`, in
//! process); the drop goes to that target and may hold until the move
//! shows; the ghost then springs onto the row wherever it is, or goes with
//! it. Keys move the gap a row or a list (D9). The session is the runner's;
//! this owns only the contact, the ghost and the scrolling.
use super::arrange_geometry::{finite, intersect};
use super::*;
use crate::paint::Ghost;
use exact_kernel::NodeKey;
use exact_motion::spring::SpringConfig;
use exact_runner::{ReorderBinding, ReorderEnding, ReorderPhase, ReorderStep, ReorderToken};

/// The edge band an autoscroll starts in, and its top speed (points, per
/// second): the in-list Arrange's.
const BAND: f32 = 24.;
const SPEED: f32 = 300.;
/// The lifted look, the same on every host (D6).
const LIFTED: f32 = 1.03;
/// The ghost's return: the preview's `-exact-spring(300,30,1)`.
const SPRING: SpringConfig = SpringConfig {
    stiffness: 300.,
    damping: 30.,
    mass: 1.,
};

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Active,
    /// Dropped or cancelled: waiting for the runner's last phase.
    Dropped,
    /// Springing onto the row (or onto nothing: gone) since `started`.
    Landing {
        from: (f32, f32),
        started: f64,
    },
}

pub(super) struct State {
    binding: ReorderBinding,
    pub(super) token: ReorderToken,
    group: String,
    pin: ViewId,
    pin_owned: bool,
    /// A pointer session draws a ghost; a key session does not.
    ghost: bool,
    /// The contact's offset into the ghost.
    grab: (f32, f32),
    /// The ghost's top left now, and its size.
    at: (f32, f32),
    size: (f32, f32),
    phase: Phase,
    edge_clock: f64,
}

impl<D: DataSource> Presenter<D> {
    /// The grip's list's non-empty `reorderGroup`.
    pub(super) fn group_of(&self, binding: &ReorderBinding) -> Option<String> {
        let list = self.host.kernel().node_by_key(binding.list)?;
        list.props
            .str(PropId::ReorderGroup)
            .filter(|g| !g.is_empty())
            .map(str::to_owned)
    }

    /// The pin a grouped session keeps (its grip, its source list) until it
    /// finishes, as `arrange_pin` keeps the in-list one.
    pub(super) fn group_pin(&self) -> Option<(ViewId, ViewId)> {
        let s = self.group.as_ref()?;
        if !s.pin_owned {
            return None;
        }
        self.host.runner().reorder_frame(s.token)?;
        Some((s.pin, self.host.kernel().node_by_key(s.binding.list)?.id))
    }

    pub(super) fn group_pin_transfer(&mut self, view: Option<ViewId>) {
        if let Some(s) = self.group.as_mut() {
            if view != Some(s.pin) {
                s.pin_owned = false;
            }
        }
    }

    /// Whether a contact's grouped session still follows it.
    pub(super) fn group_live(&self) -> bool {
        self.group.as_ref().is_some_and(|s| {
            s.phase == Phase::Active
                && s.ghost
                && self
                    .host
                    .runner()
                    .reorder_frame(s.token)
                    .is_some_and(|f| f.phase == ReorderPhase::Active)
        })
    }

    /// Lift: the session, with a ghost for a contact (D6). The ghost starts
    /// on the row, the contact's offset into it taken at the press.
    fn group_lift(
        &mut self,
        binding: ReorderBinding,
        origin: (f32, f32),
        ghost: bool,
        now: f64,
    ) -> Result<bool, String> {
        // A landing ghost ends at once (LLP 1102 §3.18); no new lift while a
        // session holds, cancels or settles (D8).
        if self
            .group
            .as_ref()
            .is_some_and(|s| matches!(s.phase, Phase::Landing { .. }))
        {
            self.finish_group();
        }
        if self.group.is_some() || self.arrange.is_some() {
            // Said, as the web says it (LLP 1102 §3.17): a drive's reply reads like a success otherwise.
            if self.group.is_some() {
                self.host.log("reorder: a drag refused: the last drop is held until its move shows (LLP 1094 D8); a person waits for the card to land; a drive waits with `clock settle` before the next drag".to_string());
            }
            return Ok(false);
        }
        let Some(group) = self.group_of(&binding) else {
            return Ok(false);
        };
        let Some(pin) = self.host.kernel().node_by_key(binding.handle).map(|n| n.id) else {
            return Ok(false);
        };
        self.set_collection_interaction(Some(pin));
        if let Some(e) = self.refine_collections() {
            return Err(e);
        }
        let (Some(_), Some(g), Some(base)) = (
            self.arrange_mapping(binding),
            self.host.runner().reorder_geometry(binding.list),
            self.arrange_base(binding.wrapper),
        ) else {
            return Ok(false);
        };
        if !self.reorder_facts_current(&g) {
            return Ok(false);
        }
        let Some(token) = self.host.group_begin(binding, g, ghost, now)? else {
            return Ok(false);
        };
        let frame = self
            .host
            .kernel()
            .node_by_key(binding.wrapper)
            .unwrap()
            .frame;
        let at = (base.x as f32, base.y as f32);
        self.group = Some(State {
            binding,
            token,
            group,
            pin,
            pin_owned: true,
            ghost,
            grab: (origin.0 - at.0, origin.1 - at.1),
            at,
            size: (frame.width, frame.height),
            phase: Phase::Active,
            edge_clock: now,
        });
        self.group_brush();
        self.dirty = true;
        Ok(true)
    }

    /// A contact past the slop, in any direction, lifts (D6).
    pub(super) fn begin_group(
        &mut self,
        binding: ReorderBinding,
        origin: (f32, f32),
        now: f64,
    ) -> Result<bool, String> {
        self.group_lift(binding, origin, true, now)
    }

    /// The ghost follows the contact; its centre samples the lists.
    pub(super) fn move_group(&mut self, point: (f32, f32), now: f64) -> Result<bool, String> {
        let Some(s) = self.group.as_mut().filter(|s| s.phase == Phase::Active) else {
            return Ok(false);
        };
        s.at = (point.0 - s.grab.0, point.1 - s.grab.1);
        s.edge_clock = now;
        self.group_brush();
        self.dirty = true;
        self.group_sample()
    }

    /// The grouped list whose port holds the ghost's centre, the gap at
    /// the centre's content y there (D5, D7); outside every port nothing,
    /// so the last certified gap stands.
    fn group_sample(&mut self) -> Result<bool, String> {
        let Some(s) = self.group.as_ref() else {
            return Ok(false);
        };
        let (token, centre) = (s.token, centre(s));
        let Some((target, port)) = self.group_lists(&s.group).into_iter().find_map(|list| {
            let (port, clip, _) = self.list_port(list)?;
            contains(clip, centre).then_some((list, port))
        }) else {
            return Ok(false);
        };
        let Some(g) = self.host.runner().reorder_geometry(target) else {
            return Ok(false);
        };
        if !self.reorder_facts_current(&g) {
            return Ok(false);
        }
        let y = (centre.1 - port.1) as f64 + g.scroll_top;
        let accepted = self.host.group_into(token, target, g, y)?;
        self.dirty = true;
        Ok(accepted)
    }

    /// The mounted lists sharing `group`.
    fn group_lists(&self, group: &str) -> Vec<NodeKey> {
        let kernel = self.host.kernel();
        self.host
            .collections()
            .iter()
            .filter_map(|c| kernel.node(c.view))
            .filter(|n| n.props.str(PropId::ReorderGroup) == Some(group))
            .map(|n| n.key)
            .collect()
    }

    /// The contact ended: a drop on the session's target, or a cancel. The
    /// rest follows the runner's phase (`observe_group`).
    pub(super) fn end_group(&mut self, drop: bool, now: f64) -> Result<bool, String> {
        let Some(s) = self.group.as_ref().filter(|s| s.phase == Phase::Active) else {
            return Ok(false);
        };
        let (token, ghost) = (s.token, s.ghost);
        let mut accepted = false;
        let mut error = None;
        if drop && (self.group_live() || !ghost) {
            let target = self
                .host
                .runner()
                .reorder_frame(token)
                .and_then(|f| f.target);
            if let Some(g) = target.and_then(|t| self.host.runner().reorder_geometry(t)) {
                match self.host.group_drop(token, g) {
                    Ok(v) => accepted = v,
                    Err(e) => error = Some(e),
                }
            }
        }
        if !accepted {
            error = error.or(self.host.arrange_cancel(token).err());
        }
        if let Some(s) = self.group.as_mut() {
            s.phase = Phase::Dropped;
        }
        error = error.or(self.after_commit());
        self.observe_group(now);
        error.map_or(Ok(accepted), Err)
    }

    /// After a commit or a frame: a held drop waits; its end (or a cancel)
    /// starts the ghost's return, or, with no ghost, finishes at once (D8).
    pub(super) fn observe_group(&mut self, now: f64) {
        let Some(s) = self.group.as_ref() else {
            return;
        };
        let frame = self.host.runner().reorder_frame(s.token);
        let over = frame
            .as_ref()
            .is_none_or(|f| matches!(f.phase, ReorderPhase::Settling | ReorderPhase::Cancelling));
        if !over || !matches!(s.phase, Phase::Active | Phase::Dropped) {
            return;
        }
        let gone = frame
            .as_ref()
            .is_none_or(|f| f.ending == Some(ReorderEnding::Gone) || f.row.is_none());
        if !s.ghost || gone {
            // Nothing to spring onto: the ghost goes with the row.
            self.finish_group();
            return;
        }
        let from = s.at;
        self.group.as_mut().unwrap().phase = Phase::Landing { from, started: now };
        self.dirty = true;
    }

    /// Retire the session: the runner's descriptor and the pin; the row
    /// shows again. A key session's focus follows the row that landed.
    fn finish_group(&mut self) {
        let Some(s) = self.group.take() else {
            return;
        };
        let frame = self.host.runner().reorder_frame(s.token);
        let landed = frame
            .as_ref()
            .filter(|f| f.ending == Some(ReorderEnding::Landed))
            .and_then(|f| f.row);
        self.brush.lift.ghost = None;
        if let Err(e) = self.host.arrange_finish(s.token) {
            self.host.log(e);
        }
        if s.pin_owned && self.collection_interaction() == Some(s.pin) {
            self.set_collection_interaction(None);
        }
        if !s.ghost {
            let grip = landed.and_then(|row| self.grip_in(row)).or_else(|| {
                self.host
                    .kernel()
                    .node_by_key(s.binding.handle)
                    .map(|n| n.id)
            });
            if let Some(e) = grip.and_then(|g| self.set_focus(Some(g), self.host.now())) {
                self.host.log(e);
            }
        }
        if let Some(e) = self.after_commit() {
            self.host.log(e);
        }
        self.queue_collections();
        self.dirty = true;
    }

    /// The grip (`reorderFor`) inside a row's wrapper.
    fn grip_in(&self, wrapper: NodeKey) -> Option<ViewId> {
        let kernel = self.host.kernel();
        let mut stack = vec![kernel.node_by_key(wrapper)?.id];
        while let Some(id) = stack.pop() {
            let node = kernel.node(id)?;
            if node.props.str(PropId::ReorderFor).is_some() {
                return Some(id);
            }
            stack.extend(node.children().into_iter().rev());
        }
        None
    }

    /// The ghost the painter draws: the row wherever it is now.
    fn group_brush(&mut self) {
        let Some(s) = self.group.as_ref().filter(|s| s.ghost) else {
            self.brush.lift.ghost = None;
            return;
        };
        let wrapper = match s.phase {
            Phase::Landing { .. } => self
                .host
                .runner()
                .reorder_frame(s.token)
                .and_then(|f| f.row)
                .unwrap_or(s.binding.wrapper),
            _ => s.binding.wrapper,
        };
        let reduced = self.host.runner().viewport().preferences.reduced_motion;
        let (scale, shadow) = match s.phase {
            Phase::Landing { from, .. } => {
                let travel = ((from.0 - s.at.0).hypot(from.1 - s.at.1)).max(1.);
                let left = self
                    .group_target()
                    .map_or(0., |to| (to.0 - s.at.0).hypot(to.1 - s.at.1) / travel);
                (1. + (LIFTED - 1.) * left.min(1.), left.min(1.))
            }
            _ => (LIFTED, 1.),
        };
        self.brush.lift.ghost = Some(Ghost {
            wrapper,
            at: s.at,
            scale: if reduced { 1. } else { scale },
            opacity: 1.,
            shadow,
        });
    }

    /// Where the ghost lands: the row's box where layout put it now.
    fn group_target(&self) -> Option<(f32, f32)> {
        let s = self.group.as_ref()?;
        let row = self.host.runner().reorder_frame(s.token)?.row?;
        let base = self.arrange_base(row)?;
        finite(base).then_some((base.x as f32, base.y as f32))
    }

    /// A frame: the ghost's return, or the edges' scrolling.
    pub(super) fn tick_group(&mut self, now: f64) {
        self.land_group(now);
        if self
            .group
            .as_ref()
            .is_some_and(|s| s.phase == Phase::Active)
        {
            self.group_edges(now);
        }
    }

    /// After a commit or a frame: the runner's phase, then the ghost's
    /// return to `now`; a clock that jumped past its rest finishes it.
    pub(super) fn land_group(&mut self, now: f64) {
        self.observe_group(now);
        let Some(s) = self.group.as_ref() else {
            return;
        };
        match s.phase {
            Phase::Landing { from, started } => {
                let Some(to) = self.group_target() else {
                    self.finish_group();
                    return;
                };
                let t = ((now - started) / 1000.).max(0.);
                let x = SPRING.sample((from.0 - to.0) as f64, 0., t);
                let y = SPRING.sample((from.1 - to.1) as f64, 0., t);
                let s = self.group.as_mut().unwrap();
                s.at = (to.0 + x.displacement as f32, to.1 + y.displacement as f32);
                if (x.at_rest() && y.at_rest()) || t >= exact_motion::spring::MAX_DURATION {
                    self.finish_group();
                } else {
                    self.group_brush();
                    self.dirty = true;
                }
            }
            Phase::Active | Phase::Dropped => {}
        }
    }

    /// When the ghost's return comes to rest, on the session clock: what
    /// the agent's `clock settle` waits for besides the motion engine.
    pub(crate) fn group_settles_at(&self) -> Option<f64> {
        let Phase::Landing { from, started } = self.group.as_ref()?.phase else {
            return None;
        };
        let to = self.group_target()?;
        let d = ((from.0 - to.0) as f64, (from.1 - to.1) as f64);
        let rest = (0..)
            .map(|n| n as f64 / exact_motion::spring::SAMPLE_RATE)
            .take_while(|t| *t < exact_motion::spring::MAX_DURATION)
            .find(|t| SPRING.sample(d.0, 0., *t).at_rest() && SPRING.sample(d.1, 0., *t).at_rest())
            .unwrap_or(exact_motion::spring::MAX_DURATION);
        Some(started + rest * 1000.)
    }

    /// Whether the ghost needs frames: it returns, or an edge scrolls.
    pub(super) fn group_needs_frame(&self) -> bool {
        self.group.as_ref().is_some_and(|s| match s.phase {
            Phase::Landing { .. } => true,
            Phase::Active => self.group_edge().is_some(),
            Phase::Dropped => false,
        })
    }

    /// Geometric autoscroll (D7): the target list's port, then each scroll
    /// ancestor of it whose box holds the ghost's centre, or the contact once
    /// the centre has gone past its edge (a tall ghost held by its top leaves
    /// the scroller before the contact reaches the band; a centre past an
    /// edge is in that edge's band); the innermost one that can still move
    /// toward an edge band, on its own axis, scrolls.
    fn group_edge(&self) -> Option<(ViewId, bool, f32)> {
        let s = self
            .group
            .as_ref()
            .filter(|s| s.phase == Phase::Active && s.ghost)?;
        let c = centre(s);
        let contact = (s.at.0 + s.grab.0, s.at.1 + s.grab.1);
        let target = self
            .host
            .runner()
            .reorder_frame(s.token)
            .and_then(|f| f.target)?;
        let kernel = self.host.kernel();
        let limits = self.collection_scroll_limits();
        let mut at = kernel.node_by_key(target).map(|n| n.id);
        while let Some(id) = at {
            let node = kernel.node(id)?;
            at = node.parent;
            let (ox, oy) = crate::paint::effective_overflow(&node);
            if ox == Overflow::Visible && oy == Overflow::Visible {
                continue;
            }
            let base = self.arrange_base(node.key)?;
            let rect = (
                base.x as f32,
                base.y as f32,
                node.frame.width,
                node.frame.height,
            );
            let shown = intersect(rect, (0., 0., self.viewport.0, self.viewport.1));
            if !contains(shown, c) && !contains(shown, contact) {
                continue;
            }
            let bounds = self.brush.scroll_bounds(
                kernel,
                self.host.content_region(),
                &node,
                limits.get(&id).copied(),
            );
            let off = self.scroll_of(id);
            let scrolls = |o: Overflow| matches!(o, Overflow::Scroll | Overflow::Auto);
            for (vertical, axis_scrolls, p, start, len, off, max) in [
                (
                    true,
                    scrolls(oy),
                    c.1,
                    shown.1,
                    shown.3,
                    off.1,
                    bounds.max.1,
                ),
                (
                    false,
                    scrolls(ox),
                    c.0,
                    shown.0,
                    shown.2,
                    off.0,
                    bounds.max.0,
                ),
            ] {
                let speed = if p < start + BAND {
                    -SPEED * ((start + BAND - p) / BAND).min(1.)
                } else if p > start + len - BAND {
                    SPEED * ((p - (start + len - BAND)) / BAND).min(1.)
                } else {
                    0.
                };
                if axis_scrolls && ((speed < 0. && off > 0.) || (speed > 0. && off < max)) {
                    return Some((id, vertical, speed));
                }
            }
        }
        None
    }

    fn group_edges(&mut self, now: f64) {
        let Some((id, vertical, speed)) = self.group_edge() else {
            return;
        };
        let s = self.group.as_mut().unwrap();
        let dt = ((now - s.edge_clock) / 1000.).clamp(0., 0.05) as f32;
        s.edge_clock = now;
        let Some(node) = self.host.kernel().node(id) else {
            return;
        };
        let list = self.collection_scroll_limits().contains_key(&id);
        let bounds = self.brush.scroll_bounds(
            self.host.kernel(),
            self.host.content_region(),
            &node,
            self.collection_scroll_limits().get(&id).copied(),
        );
        let old = self.scroll_of(id);
        let next = if vertical {
            (old.0, (old.1 + speed * dt).clamp(0., bounds.max.1))
        } else {
            ((old.0 + speed * dt).clamp(0., bounds.max.0), old.1)
        };
        if next == old {
            return;
        }
        self.scroll.insert(id, next);
        self.publish_scroll();
        self.dirty = true;
        if list {
            self.collection_scrolled_by_arrange(id);
        } else {
            self.collection_scrolled(id);
        }
        // After a scroll the gap is certified again where the ghost is.
        if let Err(e) = self.group_sample() {
            self.host.log(e);
        }
    }

    /// D9's keys on a focused grip of a grouped list that has no `press`,
    /// `key`, `pan` or `pointerdown` of its own: Space lifts (no ghost),
    /// the arrows move the gap, Space or Enter drops, Escape cancels. A
    /// hold takes no keys. Whether the key was the session's.
    pub(super) fn group_key(&mut self, id: ViewId, name: &str, now: f64) -> bool {
        // Escape cancels a drag with a ghost too, whatever has the focus (D8).
        if name == "Escape"
            && self
                .group
                .as_ref()
                .is_some_and(|s| s.ghost && s.phase == Phase::Active)
        {
            if let Some(e) = self.end_group(false, now).err().or(self.after_commit()) {
                self.host.log(e);
            }
            self.dirty = true;
            return true;
        }
        if let Some(s) = self.group.as_ref().filter(|s| !s.ghost) {
            if s.phase != Phase::Active {
                return true;
            }
            let token = s.token;
            let step = match name {
                "ArrowUp" => Some(ReorderStep::Earlier),
                "ArrowDown" => Some(ReorderStep::Later),
                "ArrowLeft" => Some(ReorderStep::PreviousList),
                "ArrowRight" => Some(ReorderStep::NextList),
                _ => None,
            };
            let result = match (step, name) {
                (Some(step), _) => self.host.group_step(token, step).map(|_| true),
                (None, " " | "Enter") => self.end_group(true, now),
                (None, "Escape") => self.end_group(false, now),
                _ => return false,
            };
            if let Some(e) = result.err().or(self.after_commit()) {
                self.host.log(e);
            }
            self.dirty = true;
            return true;
        }
        // A landing ghost yields to Space's lift, which ends it (LLP 1102 §3.18).
        let landing = self
            .group
            .as_ref()
            .is_some_and(|s| matches!(s.phase, Phase::Landing { .. }));
        if name != " " || (self.group.is_some() && !landing) {
            return false;
        }
        let Some(binding) = self.group_grip(id) else {
            return false;
        };
        match self.group_lift(binding, (0., 0.), false, now) {
            Ok(lifted) => lifted,
            Err(e) => {
                self.host.log(e);
                true
            }
        }
    }

    /// A grip D9's keys work on: its list grouped, and no `press`, `key`,
    /// `pan` or `pointerdown` of its own. It takes the focus.
    pub(crate) fn group_grip(&self, id: ViewId) -> Option<ReorderBinding> {
        let node = self.host.kernel().node(id)?;
        node.props.str(PropId::ReorderFor)?;
        let handlers = self.host.runner().handlers_of(id);
        if handlers.iter().any(|k| {
            matches!(
                k,
                EventKind::Press | EventKind::Key | EventKind::Pan | EventKind::Pointerdown
            )
        }) {
            return None;
        }
        let binding = self.host.runner().reorder_binding(node.key)?;
        self.group_of(&binding).map(|_| binding)
    }
}

fn centre(s: &State) -> (f32, f32) {
    (s.at.0 + s.size.0 / 2., s.at.1 + s.size.1 / 2.)
}

fn contains(r: Rect4, p: (f32, f32)) -> bool {
    p.0 >= r.0 && p.0 < r.0 + r.2 && p.1 >= r.1 && p.1 < r.1 + r.3
}
