//! One native primary contact; property-specific policy stays in its helper.
use super::*;
use exact_kernel::{NodeKey, TransformDragBinding};
use exact_motion::{HoldEnd, HoldStart, Value, VelocityTracker};

#[derive(Clone, Copy)]
pub(super) enum Candidate {
    Arrange(exact_runner::ReorderBinding),
    Swipe(NodeKey),
    Pan(NodeKey),
    Transform(TransformDragBinding),
    Height { handle: NodeKey, target: NodeKey },
}
impl Candidate {
    fn node(&self) -> NodeKey {
        match *self {
            Candidate::Arrange(b) => b.handle,
            Candidate::Transform(b) => b.handle,
            Candidate::Height { handle, .. } => handle,
            Candidate::Swipe(key) | Candidate::Pan(key) => key,
        }
    }
    /// The candidate's own axis rule, at the slop (LLP 1057.001 §1).
    fn accepts(&self, dx: f64, dy: f64) -> bool {
        match self {
            Candidate::Arrange(_) | Candidate::Height { .. } => dy.abs() > dx.abs(),
            Candidate::Swipe(_) => dx.abs() > dy.abs(),
            Candidate::Transform(_) | Candidate::Pan(_) => true,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum HeldKind {
    Arrange(exact_runner::ReorderToken),
    Transform {
        binding: TransformDragBinding,
        pair: exact_motion::TransformHold,
        revision: u64,
    },
    Swipe {
        companions: [Option<HoldStart>; 2],
    },
    Height {
        handle: NodeKey,
        target: NodeKey,
    },
}
pub(super) struct Hold {
    pub(super) primary: HoldStart,
    pub(super) velocity: VelocityTracker,
    pub(super) kind: HeldKind,
}
pub(super) struct Contact {
    hit: NodeKey,
    candidate: Option<Candidate>,
    /// The candidates after `candidate`, in precedence order, until the slop.
    rest: Vec<Candidate>,
    origin: (f32, f32),
    position: (f32, f32),
    last_ms: f64,
    pub(super) hold: Option<Hold>,
    panning: bool,
    /// Every sample since down, for a pan's release velocity (LLP 1057 §10.6).
    pan_velocity: VelocityTracker,
    retained: Option<super::retained_action::RetainedContact>,
    press: Option<(NodeKey, PaintedBox)>,
    /// The canvas that owns this contact and sees its down, moves and up.
    canvas: Option<ViewId>,
}
impl<D: DataSource> Presenter<D> {
    pub(super) fn input_live(&self, key: NodeKey) -> bool {
        let Some(node) = self.host.kernel().node_by_key(key) else {
            return false;
        };
        if self.host.route_visibility(node.id).1 || self.brush.region_blocks_action(node.id) {
            return false;
        }
        let mut at = Some(node.id);
        while let Some(id) = at {
            let Some(node) = self.host.kernel().node(id) else {
                return false;
            };
            if node.props.bool(PropId::Disabled) == Some(true) {
                return false;
            }
            at = node.parent;
        }
        true
    }
    fn contact_live(&self, contact: &Contact) -> bool {
        if let Some(canvas) = contact.canvas {
            return self.input_live(contact.hit)
                && self
                    .host
                    .kernel()
                    .node_by_key(contact.hit)
                    .is_some_and(|n| self.input_surface(n.id) == Some(canvas));
        }
        if let Some(retained) = &contact.retained {
            return self.retained_contact_live(retained)
                && contact
                    .hold
                    .as_ref()
                    .is_none_or(|h| self.host.has_hold(h.primary.token));
        }
        match &contact.hold {
            Some(held) => {
                self.host.has_hold(held.primary.token)
                    && match held.kind {
                        HeldKind::Arrange(token) => self.arrange_live(token),
                        HeldKind::Transform {
                            binding,
                            pair,
                            revision,
                        } => {
                            self.host.transform_hold_live(pair, binding)
                                && self.transform_revision(binding) == Some(revision)
                        }
                        HeldKind::Height { handle, target } => {
                            self.input_live(handle)
                                && self.host.height_drag_target(handle) == Some(target)
                        }
                        HeldKind::Swipe { .. } => {
                            matches!(contact.candidate, Some(Candidate::Swipe(key)) if self.input_live(key))
                        }
                    }
            }
            None => {
                self.input_live(contact.hit)
                    && contact.press.is_none_or(|(key, _)| {
                        self.input_live(key)
                            && self.host.kernel().node_by_key(key).is_some_and(|n| {
                                self.host
                                    .runner()
                                    .handlers_of(n.id)
                                    .contains(&EventKind::Press)
                            })
                    })
                    && match contact.candidate {
                        Some(Candidate::Pan(key)) => {
                            self.input_live(key)
                                && self.host.kernel().node_by_key(key).is_some_and(|n| {
                                    self.host
                                        .runner()
                                        .handlers_of(n.id)
                                        .contains(&EventKind::Pan)
                                })
                        }
                        _ => true,
                    }
            }
        }
    }
    fn pointer_sample(&self, x: f32, y: f32, now_ms: f64) -> Result<(), String> {
        if !x.is_finite()
            || !y.is_finite()
            || !now_ms.is_finite()
            || now_ms < self.host.now()
            || self.contact.as_ref().is_some_and(|c| now_ms < c.last_ms)
        {
            Err("invalid pointer sample or clock".into())
        } else {
            Ok(())
        }
    }
    /// Retire after commits even when the target itself was untouched.
    pub(super) fn retire_pointer(&mut self) {
        if self.contact.as_ref().is_some_and(|c| !self.contact_live(c)) {
            if self.contact.as_ref().is_some_and(|c| c.retained.is_some()) {
                // Cancel only our token, at the already accepted Host time. An
                // invalid future sample must not strand a hold or advance time.
                let contact = self.contact.take().unwrap();
                if let Some(held) = contact.hold {
                    if let Err(error) = self.end_contact(&held, HoldEnd::Cancel, self.host.now()) {
                        self.host.log(error);
                    }
                }
                self.set_collection_interaction(None);
                return;
            }
            if let Err(error) = self.pointer_cancel(self.pointer_now()) {
                self.host.log(error);
            }
        }
    }
    /// Only after a successful, matching, current-origin picture installation.
    /// Same-A repaints retain their contact; foreign/duplicate ACKs never enter.
    #[cfg(any(target_os = "linux", test))]
    pub(super) fn retire_acknowledged_pointer(&mut self) {
        let stale = self
            .contact
            .as_ref()
            .and_then(|c| c.retained.as_ref())
            .is_some_and(|c| !self.retained_contact_picture_current(c));
        if stale {
            let contact = self.contact.take().unwrap();
            if let Some(held) = contact.hold {
                match self.host.snap_retained_motion(held.primary.token) {
                    Ok(changed) => self.dirty |= changed,
                    Err(error) => self.host.log(error),
                }
            }
            self.set_collection_interaction(None);
        }
        self.retire_retained_motion_picture();
    }
    pub(super) fn pointer_now(&self) -> f64 {
        self.contact
            .as_ref()
            .map_or(self.host.now(), |c| self.host.now().max(c.last_ms))
    }
    pub(crate) fn contact_position(&self) -> Option<(f32, f32)> {
        self.contact.as_ref().map(|c| c.position)
    }

    /// Every candidate of a contact on `hit`: innermost first, then reorder >
    /// transform > height > pan > swipe on one node (LLP 1057.001 §1).
    fn candidates(&self, hit: NodeKey) -> Vec<Candidate> {
        let ranked = [
            self.arrange_candidate(hit),
            self.transform_candidate(hit),
            self.height_candidate(hit),
            self.pan_candidate(hit),
            self.swipe_candidate(hit).map(Candidate::Swipe),
        ];
        let mut found: Vec<_> = ranked
            .into_iter()
            .enumerate()
            .filter_map(|(rank, c)| Some((self.depth(hit, c?.node())?, rank, c?)))
            .collect();
        found.sort_by_key(|&(depth, rank, _)| (depth, rank));
        found.into_iter().map(|(_, _, c)| c).collect()
    }
    fn depth(&self, hit: NodeKey, candidate: NodeKey) -> Option<usize> {
        let kernel = self.host.kernel();
        let mut at = kernel.node_by_key(hit);
        let mut steps = 0;
        while let Some(node) = at {
            if node.key == candidate {
                return Some(steps);
            }
            at = node.parent.and_then(|id| kernel.node(id));
            steps += 1;
        }
        None
    }
    // @ref LLP 1043.000 §3 D8 — ordinary commits move layout, not a motion hold.
    // Nearest explicit pan handler owns one contact; editor/press boundaries stop it.
    fn pan_candidate(&self, hit: NodeKey) -> Option<Candidate> {
        let mut at = self.host.kernel().node_by_key(hit).map(|n| n.id);
        while let Some(id) = at {
            let n = self.host.kernel().node(id)?;
            let handlers = self.host.runner().handlers_of(id);
            if self.input_live(n.key) && handlers.contains(&EventKind::Pan) {
                return Some(Candidate::Pan(n.key));
            }
            if n.node_type == NodeType::TextInput || handlers.contains(&EventKind::Press) {
                return None;
            }
            at = n.parent;
        }
        None
    }
    /// Primary down shared by evdev, VNC, and explicitly labeled agent synthesis.
    pub fn pointer_down(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.pointer_sample(x, y, now_ms)?;
        if self.host.content_region().is_some() {
            if let Some(view) = self
                .hit(x, y)
                .filter(|id| self.brush.region_blocks_action(*id))
            {
                let Some(hit) = self.host.kernel().node(view).map(|n| n.key) else {
                    return Ok(false);
                };
                let Some(retained) = self.retained_down(hit) else {
                    self.retire_pointer();
                    return Ok(false);
                };
                self.pointer_cancel(now_ms)?;
                // Cancellation may run layout/effects; it must not rebind A.
                if !self.retained_contact_live(&retained) {
                    return Ok(false);
                }
                let candidate = retained.swipe.as_ref().map(|t| Candidate::Swipe(t.key));
                self.contact = Some(Contact {
                    hit,
                    candidate,
                    rest: Vec::new(),
                    origin: (x, y),
                    position: (x, y),
                    last_ms: now_ms,
                    hold: None,
                    retained: Some(retained),
                    press: None,
                    canvas: None,
                    panning: false,
                    pan_velocity: VelocityTracker::new(),
                });
                self.set_collection_interaction(Some(view));
                return Ok(true);
            }
        }
        self.pointer_cancel(now_ms)?;
        let Some(hit) = self
            .hit(x, y)
            .and_then(|id| self.host.kernel().node(id).map(|n| n.key))
        else {
            return Ok(false);
        };
        if !self.input_live(hit) {
            return Ok(false);
        }
        let mut rest = self.candidates(hit);
        let arrange = rest
            .iter()
            .find(|c| matches!(c, Candidate::Arrange(_)))
            .copied();
        self.prepare_arrange_down(arrange)?;
        let Some(view) = self.host.kernel().node_by_key(hit).map(|n| n.id) else {
            return Ok(false);
        };
        // A canvas holds the contact itself: it sees the down now and every
        // move until the up, so a drag (mouse look) reaches its world.
        let canvas = if rest.is_empty() {
            self.canvas_contact_target(view)
        } else {
            None
        };
        if let Some(canvas) = canvas.filter(|_| self.canvas_contact_down(view, x, y, now_ms)) {
            self.contact = Some(Contact {
                hit,
                candidate: None,
                rest,
                origin: (x, y),
                position: (x, y),
                last_ms: now_ms,
                hold: None,
                panning: false,
                pan_velocity: VelocityTracker::new(),
                retained: None,
                press: None,
                canvas: Some(canvas),
            });
            return Ok(true);
        }
        let press = self.handler_target(view, EventKind::Press).and_then(|id| {
            let node = self.host.kernel().node(id)?;
            if node.style.press_scale == 1. {
                return None;
            }
            let (key, origin) = (node.key, node.style.transform_origin);
            self.box_of(id).map(|b| (key, b.unpressed(origin)))
        });
        if let Some((key, _)) = press {
            self.host.press_feedback(key, true, now_ms);
            self.dirty = true;
        }
        let candidate = (!rest.is_empty()).then(|| rest.remove(0));
        let mut pan_velocity = VelocityTracker::new();
        pan_velocity.push(now_ms / 1000., Value::new(x as f64, y as f64));
        self.contact = Some(Contact {
            hit,
            candidate,
            rest,
            origin: (x, y),
            position: (x, y),
            last_ms: now_ms,
            hold: None,
            panning: false,
            pan_velocity,
            retained: None,
            press,
            canvas: None,
        });
        self.set_collection_interaction(Some(view));
        Ok(true)
    }
    /// Recognize one dominant axis. Recognition has zero displacement at catch.
    pub fn pointer_move(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.retire_pointer();
        if let Some(error) = self.hover_at(Some((x, y)), now_ms) {
            return Err(error);
        }
        if self.contact.is_none() {
            return Ok(false);
        }
        self.pointer_sample(x, y, now_ms)?;
        let mut contact = self.contact.take().unwrap();
        contact.last_ms = now_ms;
        if let Some(canvas) = contact.canvas {
            let moved = (x, y) != contact.position;
            contact.position = (x, y);
            self.contact = Some(contact);
            return Ok(!moved || self.canvas_pointer(canvas, "move", 1, x, y, now_ms));
        }
        contact
            .pan_velocity
            .push(now_ms / 1000., Value::new(x as f64, y as f64));
        if let Some((key, box_)) = contact.press {
            self.host.press_feedback(key, box_.contains(x, y), now_ms);
            self.dirty = true;
        }
        let (dx, dy) = (
            x as f64 - contact.origin.0 as f64,
            y as f64 - contact.origin.1 as f64,
        );
        if contact.hold.is_none()
            && !contact.panning
            && dx.abs().max(dy.abs()) > exact_motion::gesture::SLOP
        {
            // The first candidate whose axis rule accepts begins; the ones
            // before it fall away (LLP 1057.001 §1).
            let rest = std::mem::take(&mut contact.rest);
            if let Some(next) = contact
                .candidate
                .into_iter()
                .chain(rest)
                .find(|c| c.accepts(dx, dy))
            {
                contact.candidate = Some(next);
            }
        }
        if let Some(Candidate::Pan(key)) = contact.candidate {
            let from = if contact.panning {
                contact.position
            } else {
                contact.origin
            };
            let dx = x as f64 - from.0 as f64;
            let dy = y as f64 - from.1 as f64;
            contact.position = (x, y);
            if contact.panning || dx.abs().max(dy.abs()) > exact_motion::gesture::SLOP {
                contact.panning = true;
                if let Some((key, _)) = contact.press.take() {
                    self.host.press_feedback(key, false, now_ms);
                }
                let view = self.host.kernel().node_by_key(key).unwrap().id;
                let error = if dx != 0. || dy != 0. {
                    self.host
                        .dispatch_at(view, Event::Pan(dx, dy), now_ms)
                        .or(self.after_commit())
                } else {
                    None
                };
                self.dirty = true;
                if self.contact_live(&contact) {
                    self.contact = Some(contact);
                } else {
                    self.set_collection_interaction(None);
                }
                return error.map_or(Ok(true), Err);
            }
            self.contact = Some(contact);
            return Ok(false);
        }
        contact.position = (x, y);
        if contact.hold.is_none() {
            let dx = x as f64 - contact.origin.0 as f64;
            let dy = y as f64 - contact.origin.1 as f64;
            if dx.abs().max(dy.abs()) <= exact_motion::gesture::SLOP {
                self.contact = Some(contact);
                return Ok(false);
            }
            if contact.candidate.is_some() {
                if let Some((key, _)) = contact.press.take() {
                    self.host.press_feedback(key, false, now_ms);
                }
            }
            let begin = match contact.candidate {
                Some(Candidate::Arrange(binding)) if dy.abs() > dx.abs() => {
                    self.begin_arrange(binding, now_ms)
                }
                Some(Candidate::Transform(binding)) => self.begin_transform_drag(binding, now_ms),
                Some(Candidate::Height { handle, target }) if dy.abs() > dx.abs() => {
                    self.begin_height_drag(handle, target, now_ms)
                }
                Some(Candidate::Swipe(key))
                    if dx.abs() > dy.abs()
                        && (contact.retained.is_some() || self.input_live(key)) =>
                {
                    if dx < 0. {
                        self.tick(now_ms);
                        if contact
                            .retained
                            .as_ref()
                            .is_some_and(|r| !self.retained_contact_live(r))
                        {
                            self.set_collection_interaction(None);
                            return Ok(false);
                        }
                        let view = self.host.kernel().node_by_key(key).unwrap().id;
                        if self.host.presented(view).translate.0 <= 0. {
                            self.set_collection_interaction(None);
                            return Ok(false);
                        }
                    }
                    self.begin_swipe(key, now_ms)
                }
                _ => {
                    if contact.press.is_some() {
                        self.contact = Some(contact);
                    } else {
                        self.set_collection_interaction(None);
                    }
                    return Ok(false);
                }
            };
            match begin {
                Ok(Some(held)) => {
                    if let Some(target) = contact.retained.as_ref().and_then(|c| c.swipe.as_ref()) {
                        if !self.begin_retained_motion(target, held.primary.token) {
                            self.end_contact(&held, HoldEnd::Cancel, self.host.now())?;
                            self.set_collection_interaction(None);
                            return Ok(false);
                        }
                    }
                    contact.hold = Some(held);
                    contact.origin = (x, y);
                }
                Ok(None) => {
                    self.set_collection_interaction(None);
                    return Ok(false);
                }
                Err(error) => {
                    self.set_collection_interaction(None);
                    return Err(error);
                }
            }
        }
        let held = contact.hold.as_mut().unwrap();
        let result = match held.kind {
            HeldKind::Arrange(_) => {
                self.move_arrange(held, y as f64 - contact.origin.1 as f64, (x, y), now_ms)
            }
            HeldKind::Transform { .. } => self.move_transform_drag(
                held,
                Value {
                    x: x as f64 - contact.origin.0 as f64,
                    y: y as f64 - contact.origin.1 as f64,
                    ..Value::ZERO
                },
                now_ms,
            ),
            HeldKind::Height { .. } => {
                self.move_height_drag(held, contact.origin.1 as f64 - y as f64, now_ms)
            }
            HeldKind::Swipe { .. } => {
                self.move_swipe(held, x as f64 - contact.origin.0 as f64, now_ms)
            }
        };
        if self.contact_live(&contact) {
            self.contact = Some(contact);
        } else {
            let _ = self.end_contact(contact.hold.as_ref().unwrap(), HoldEnd::Cancel, now_ms);
            if !matches!(contact.hold.as_ref().unwrap().kind, HeldKind::Arrange(_)) {
                self.set_collection_interaction(None);
            }
        }
        result
    }
    /// Accepted final sample, typed action while held, end once, pin released last.
    pub fn pointer_up(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.retire_pointer();
        if self.contact.is_some() {
            self.pointer_sample(x, y, now_ms)?;
        }
        let accepted = match self.pointer_move(x, y, now_ms) {
            Ok(accepted) => accepted,
            Err(error) => {
                let _ = self.pointer_cancel(now_ms);
                return Err(error);
            }
        };
        let Some(contact) = self.contact.take() else {
            return Ok(false);
        };
        if let Some(canvas) = contact.canvas {
            return Ok(self.canvas_pointer(canvas, "up", 0, x, y, now_ms));
        }
        if let Some((key, _)) = contact.press {
            self.host.press_feedback(key, false, now_ms);
            self.dirty = true;
        }
        let arranged = contact
            .hold
            .as_ref()
            .is_some_and(|h| matches!(h.kind, HeldKind::Arrange(_)));
        let result = if let Some(held) = contact.hold {
            if !accepted {
                self.end_contact(&held, HoldEnd::Cancel, now_ms)
                    .map(|_| false)
            } else {
                match held.kind {
                    HeldKind::Arrange(_) => self.end_arrange(&held, true, now_ms),
                    HeldKind::Height { .. } => self.finish_height_drag(&held, now_ms),
                    HeldKind::Transform { .. } => self.finish_transform_drag(&held, now_ms),
                    HeldKind::Swipe { .. } => {
                        let current = self
                            .host
                            .engine()
                            .value(held.primary.token.node(), held.primary.token.property())
                            .unwrap_or(Value::ZERO)
                            .x;
                        let mut error = None;
                        if current >= exact_motion::gesture::SWIPE_KNEE
                            && self.host.has_hold(held.primary.token)
                        {
                            if let Some(retained) = &contact.retained {
                                let delivered = match &retained.swipe {
                                    Some(target) => self.retained_swipe_dispatch(
                                        target,
                                        held.primary.token,
                                        now_ms,
                                    ),
                                    None => Ok(false),
                                };
                                match delivered {
                                    Ok(false) => {
                                        let ended =
                                            self.end_swipe(&held, HoldEnd::Cancel, self.host.now());
                                        self.set_collection_interaction(None);
                                        return ended.map(|_| false);
                                    }
                                    Err(e) => error = Some(e),
                                    Ok(true) => {}
                                }
                            } else {
                                error = self.host.dispatch_held(held.primary.token, now_ms).err();
                                error = error.or(self.after_commit());
                            }
                        }
                        let velocity = held.velocity.estimate(now_ms / 1000.);
                        let ended = self.end_swipe(&held, HoldEnd::Release { velocity }, now_ms);
                        error.map_or(ended.map(|_| true), Err)
                    }
                }
            }
        } else if contact.panning {
            // The last delta went out above; then the release, once.
            let velocity = contact.pan_velocity.estimate(now_ms / 1000.);
            match contact.candidate {
                Some(Candidate::Pan(key)) => self
                    .release_pan(key, (velocity.x, velocity.y), now_ms)
                    .map_or(Ok(true), Err),
                _ => Ok(true),
            }
        } else if let Some((key, box_)) = contact.press {
            if box_.contains(x, y) && self.input_live(key) {
                if let Some(node) = self.host.kernel().node_by_key(key) {
                    self.dispatch_press(node.id, now_ms, true);
                }
            }
            Ok(false)
        } else {
            let at = self
                .hit(x, y)
                .and_then(|id| self.host.kernel().node(id).map(|n| n.key));
            if at == Some(contact.hit) {
                if let Some(retained) = &contact.retained {
                    if let Some(target) = &retained.press {
                        self.retained_press_target(target, now_ms);
                    }
                } else {
                    self.press_at(x, y, now_ms);
                }
            }
            Ok(false)
        };
        if !arranged {
            self.set_collection_interaction(None);
        }
        result
    }
    fn end_contact(&mut self, held: &Hold, end: HoldEnd, now_ms: f64) -> Result<(), String> {
        match held.kind {
            HeldKind::Arrange(_) => self.end_arrange(held, false, now_ms).map(|_| ()),
            HeldKind::Transform { pair, .. } => self.end_transform_drag(pair, None, now_ms),
            HeldKind::Height { .. } => self.height_end(held.primary.token, end, now_ms).map(|_| ()),
            HeldKind::Swipe { .. } => self.end_swipe(held, end, now_ms),
        }
    }
    /// Escape, wheel takeover, disconnection, or invalidated binding: no
    /// release event, except a pan that began, which releases at rest.
    pub fn pointer_cancel(&mut self, now_ms: f64) -> Result<(), String> {
        if self.contact.is_none() {
            return Ok(());
        }
        self.pointer_sample(0., 0., now_ms)?;
        let contact = self.contact.take().unwrap();
        if let Some(canvas) = contact.canvas {
            let (x, y) = contact.position;
            self.canvas_pointer(canvas, "cancel", 0, x, y, now_ms);
        }
        if let Some((key, _)) = contact.press {
            self.host.press_feedback(key, false, now_ms);
            self.dirty = true;
        }
        let mut result = contact.hold.as_ref().map_or(Ok(()), |held| {
            self.end_contact(held, HoldEnd::Cancel, now_ms)
        });
        if let (true, Some(Candidate::Pan(key))) = (contact.panning, contact.candidate) {
            if let Some(error) = self.release_pan(key, (0., 0.), now_ms) {
                result = result.and(Err(error));
            }
        }
        if !contact
            .hold
            .as_ref()
            .is_some_and(|h| matches!(h.kind, HeldKind::Arrange(_)))
        {
            self.set_collection_interaction(None);
        }
        result
    }
}

#[cfg(test)]
#[path = "precedence_tests.rs"]
mod precedence_tests;
