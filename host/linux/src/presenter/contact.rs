//! One native primary contact; property-specific policy stays in its helper.
use super::*;
use exact_kernel::{NodeKey, TransformDragBinding};
use exact_motion::{HoldEnd, HoldStart, Value, VelocityTracker};

#[derive(Clone, Copy)]
pub(super) enum Candidate {
    Swipe(NodeKey),
    Transform(TransformDragBinding),
    Height { handle: NodeKey, target: NodeKey },
}
#[derive(Clone, Copy)]
pub(super) enum HeldKind {
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
    origin: (f32, f32),
    position: (f32, f32),
    last_ms: f64,
    pub(super) hold: Option<Hold>,
}
impl<D: DataSource> Presenter<D> {
    pub(super) fn input_live(&self, key: NodeKey) -> bool {
        let Some(node) = self.host.kernel().node_by_key(key) else {
            return false;
        };
        if self.host.route_visibility(node.id).1 {
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
        match &contact.hold {
            Some(held) => {
                self.host.has_hold(held.primary.token)
                    && match held.kind {
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
            None => self.input_live(contact.hit),
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
            if let Err(error) = self.pointer_cancel(self.pointer_now()) {
                self.host.log(error);
            }
        }
    }
    pub(super) fn pointer_now(&self) -> f64 {
        self.contact
            .as_ref()
            .map_or(self.host.now(), |c| self.host.now().max(c.last_ms))
    }
    pub(crate) fn contact_position(&self) -> Option<(f32, f32)> {
        self.contact.as_ref().map(|c| c.position)
    }

    /// Primary down shared by evdev, VNC, and explicitly labeled agent synthesis.
    pub fn pointer_down(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.pointer_sample(x, y, now_ms)?;
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
        let candidate = self
            .transform_candidate(hit)
            .or_else(|| self.height_candidate(hit))
            .or_else(|| self.swipe_candidate(hit).map(Candidate::Swipe));
        let view = self.host.kernel().node_by_key(hit).unwrap().id;
        self.contact = Some(Contact {
            hit,
            candidate,
            origin: (x, y),
            position: (x, y),
            last_ms: now_ms,
            hold: None,
        });
        self.set_collection_interaction(Some(view));
        Ok(true)
    }
    /// Recognize one dominant axis. Recognition has zero displacement at catch.
    pub fn pointer_move(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.retire_pointer();
        if self.contact.is_none() {
            return Ok(false);
        }
        self.pointer_sample(x, y, now_ms)?;
        let mut contact = self.contact.take().unwrap();
        contact.last_ms = now_ms;
        contact.position = (x, y);
        if contact.hold.is_none() {
            let dx = x as f64 - contact.origin.0 as f64;
            let dy = y as f64 - contact.origin.1 as f64;
            if dx.abs().max(dy.abs()) <= 4. {
                self.contact = Some(contact);
                return Ok(false);
            }
            let begin = match contact.candidate {
                Some(Candidate::Transform(binding)) => self.begin_transform_drag(binding, now_ms),
                Some(Candidate::Height { handle, target }) if dy.abs() > dx.abs() => {
                    self.begin_height_drag(handle, target, now_ms)
                }
                Some(Candidate::Swipe(key)) if dx.abs() > dy.abs() && self.input_live(key) => {
                    if dx < 0. {
                        self.tick(now_ms);
                        let view = self.host.kernel().node_by_key(key).unwrap().id;
                        if self.host.presented(view).translate.0 <= 0. {
                            self.set_collection_interaction(None);
                            return Ok(false);
                        }
                    }
                    self.begin_swipe(key, now_ms)
                }
                _ => {
                    self.set_collection_interaction(None);
                    return Ok(false);
                }
            };
            match begin {
                Ok(Some(held)) => {
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
            HeldKind::Transform { .. } => self.move_transform_drag(
                held,
                Value {
                    x: x as f64 - contact.origin.0 as f64,
                    y: y as f64 - contact.origin.1 as f64,
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
            self.set_collection_interaction(None);
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
        let result = if let Some(held) = contact.hold {
            if !accepted {
                self.end_contact(&held, HoldEnd::Cancel, now_ms)
                    .map(|_| false)
            } else {
                match held.kind {
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
                        if current >= 64. && self.host.has_hold(held.primary.token) {
                            error = self.host.dispatch_held(held.primary.token, now_ms).err();
                            error = error.or(self.after_commit());
                        }
                        let velocity = held.velocity.estimate(now_ms / 1000.);
                        let ended = self.end_swipe(&held, HoldEnd::Release { velocity }, now_ms);
                        error.map_or(ended.map(|_| true), Err)
                    }
                }
            }
        } else {
            let at = self
                .hit(x, y)
                .and_then(|id| self.host.kernel().node(id).map(|n| n.key));
            if at == Some(contact.hit) {
                self.press_at(x, y, now_ms);
            }
            Ok(false)
        };
        self.set_collection_interaction(None);
        result
    }
    fn end_contact(&mut self, held: &Hold, end: HoldEnd, now_ms: f64) -> Result<(), String> {
        match held.kind {
            HeldKind::Transform { pair, .. } => self.end_transform_drag(pair, None, now_ms),
            HeldKind::Height { .. } => self.height_end(held.primary.token, end, now_ms).map(|_| ()),
            HeldKind::Swipe { .. } => self.end_swipe(held, end, now_ms),
        }
    }
    /// Escape, wheel takeover, disconnection, or invalidated binding: no release event.
    pub fn pointer_cancel(&mut self, now_ms: f64) -> Result<(), String> {
        if self.contact.is_none() {
            return Ok(());
        }
        self.pointer_sample(0., 0., now_ms)?;
        let contact = self.contact.take().unwrap();
        let result = contact.hold.as_ref().map_or(Ok(()), |held| {
            self.end_contact(held, HoldEnd::Cancel, now_ms)
        });
        self.set_collection_interaction(None);
        result
    }
}
