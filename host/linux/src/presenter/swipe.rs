//! One primary-button contact. The native carrier recognizes; motion owns values.
use super::*;
use exact_kernel::{NodeKey, TouchAction};
use exact_motion::{HoldEnd, HoldStart, Property, Value, VelocityTracker};

pub(super) struct Contact {
    hit: NodeKey,
    candidate: Option<NodeKey>,
    origin: (f32, f32),
    last_ms: f64,
    pub(super) hold: Option<SwipeHold>,
}
pub(super) struct SwipeHold {
    pub(super) primary: HoldStart,
    companions: [Option<HoldStart>; 2],
    pub(super) velocity: VelocityTracker,
}

// Match native Apple resistance, inverting the caught presentation first so
// zero displacement is exactly the origin even beyond the 64-point knee.
fn displacement(base: f64, delta: f64) -> f64 {
    if delta == 0. {
        return base;
    }
    let origin = if base.abs() <= 64. {
        base
    } else {
        base.signum() * (64. + (base.abs() - 64.) / 0.2)
    };
    let at = origin + delta;
    if at.abs() <= 64. {
        at
    } else {
        at.signum() * (64. + (at.abs() - 64.) * 0.2)
    }
}
fn indicator(base: f64, caught: f64, progress: f64) -> f64 {
    if progress == caught {
        base
    } else if progress < caught {
        base * progress / caught
    } else {
        base + (1. - base) * (progress - caught) / (1. - caught)
    }
}

impl<D: DataSource> Presenter<D> {
    fn input_live(&self, key: NodeKey) -> bool {
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

    fn swipe_candidate(&self, hit: NodeKey) -> Option<NodeKey> {
        if !self.input_live(hit) {
            return None;
        }
        let mut at = self.host.kernel().node_by_key(hit).map(|n| n.id);
        while let Some(id) = at {
            let node = self.host.kernel().node(id)?;
            let handlers = self.host.runner().handlers_of(id);
            if handlers.contains(&EventKind::Swiperight) {
                return matches!(
                    node.style.touch_action,
                    TouchAction::None
                        | TouchAction::PanY
                        | TouchAction::PanLeftPanY
                        | TouchAction::PanRightPanY
                )
                .then_some(node.key);
            }
            // Child buttons and editors retain their own input, not an ancestor swipe.
            if handlers.contains(&EventKind::Press) || node.node_type == NodeType::TextInput {
                return None;
            }
            at = node.parent;
        }
        None
    }

    fn contact_live(&self, contact: &Contact) -> bool {
        match &contact.hold {
            Some(hold) => {
                contact.candidate.is_some_and(|key| self.input_live(key))
                    && self.host.has_hold(hold.primary.token)
            }
            None => self.input_live(contact.hit),
        }
    }

    fn pointer_sample(&self, x: f32, y: f32, now_ms: f64) -> Result<(), String> {
        if !x.is_finite()
            || !y.is_finite()
            || !now_ms.is_finite()
            || now_ms < self.host.now()
            || self
                .contact
                .as_ref()
                .is_some_and(|contact| now_ms < contact.last_ms)
        {
            Err("invalid pointer sample or clock".into())
        } else {
            Ok(())
        }
    }

    /// Retire immediately after commits, including navigation without destruction.
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

    /// Primary-button down, shared by the real display's evdev and VNC paths.
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
        let candidate = self.swipe_candidate(hit);
        let view = self.host.kernel().node_by_key(hit).unwrap().id;
        self.contact = Some(Contact {
            hit,
            candidate,
            origin: (x, y),
            last_ms: now_ms,
            hold: None,
        });
        self.set_collection_interaction(Some(view));
        Ok(true)
    }

    /// Recognize horizontal intent, including catching a right-displaced return
    /// with leftward motion. Vertical motion/wheels retain priority.
    /// The recognition sample has zero displacement from the caught presentation.
    pub fn pointer_move(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.retire_pointer();
        if self.contact.is_none() {
            return Ok(false);
        }
        self.pointer_sample(x, y, now_ms)?;
        let mut contact = self.contact.take().unwrap();
        contact.last_ms = now_ms;
        if contact.hold.is_none() {
            let dx = x - contact.origin.0;
            let dy = y - contact.origin.1;
            if dx.abs().max(dy.abs()) <= 4. {
                self.contact = Some(contact);
                return Ok(false);
            }
            if dy.abs() >= dx.abs() || contact.candidate.is_none() {
                self.set_collection_interaction(None);
                return Ok(false);
            }
            let key = contact.candidate.unwrap();
            if !self.input_live(key) {
                self.set_collection_interaction(None);
                return Ok(false);
            }
            if dx < 0. {
                // Sample the existing curve at recognition, not pointer-down.
                self.host.tick(now_ms);
                self.dirty = true;
                let view = self.host.kernel().node_by_key(key).unwrap().id;
                if self.host.presented(view).translate.0 <= 0. {
                    self.set_collection_interaction(None);
                    return Ok(false);
                }
            }
            match self.begin_swipe(key, now_ms) {
                Ok(Some(hold)) => {
                    contact.hold = Some(hold);
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
        let result = self.move_swipe(
            contact.hold.as_mut().unwrap(),
            (x - contact.origin.0) as f64,
            now_ms,
        );
        self.contact = Some(contact);
        result
    }

    fn begin_swipe(&mut self, key: NodeKey, now_ms: f64) -> Result<Option<SwipeHold>, String> {
        let node = self.host.kernel().node_by_key(key).unwrap();
        let view = node.id;
        let companion = node.children().into_iter().find(|id| {
            self.host
                .kernel()
                .node(*id)
                .is_some_and(|n| n.props.bool(PropId::SwipeIndicator) == Some(true))
        });
        let Some(primary) = self
            .host
            .hold_begin(view, Property::Translate, now_ms)
            .map_err(|e| format!("hold: {e:?}"))?
        else {
            return Ok(None);
        };
        let mut held = SwipeHold {
            primary,
            companions: [None, None],
            velocity: VelocityTracker::new(),
        };
        if let Some(companion) = companion {
            for (i, property) in [Property::Opacity, Property::Scale].into_iter().enumerate() {
                match self.host.hold_begin(companion, property, now_ms) {
                    Ok(start) => held.companions[i] = start,
                    Err(error) => {
                        let _ = self.end_swipe(&held, HoldEnd::Cancel, now_ms);
                        return Err(format!("hold: {error:?}"));
                    }
                }
            }
        }
        held.velocity.push(now_ms / 1000., primary.value);
        self.set_collection_interaction(Some(view));
        self.dirty = true;
        Ok(Some(held))
    }

    fn move_swipe(
        &mut self,
        held: &mut SwipeHold,
        delta: f64,
        now_ms: f64,
    ) -> Result<bool, String> {
        let value = Value::new(
            displacement(held.primary.value.x, delta),
            held.primary.value.y,
        );
        if !self
            .host
            .hold_update(held.primary.token, value, now_ms)
            .map_err(|e| format!("hold: {e:?}"))?
        {
            return Ok(false);
        }
        // Tracking presentation after resistance produces displayed units/sec.
        held.velocity.push(now_ms / 1000., value);
        let progress = (value.x / 64.).clamp(0., 1.);
        let caught = (held.primary.value.x / 64.).clamp(0., 1.);
        for companion in held.companions.iter().flatten() {
            let value = Value::scalar(indicator(companion.value.x, caught, progress));
            self.host
                .hold_update(companion.token, value, now_ms)
                .map_err(|e| format!("hold: {e:?}"))?;
        }
        self.dirty = true;
        Ok(true)
    }

    /// Last sample, authored action while live and held, then release; pin clears last.
    pub fn pointer_up(&mut self, x: f32, y: f32, now_ms: f64) -> Result<bool, String> {
        self.pointer_move(x, y, now_ms)?;
        let Some(contact) = self.contact.take() else {
            return Ok(false);
        };
        let result = if let Some(held) = contact.hold {
            let current = displacement(held.primary.value.x, (x - contact.origin.0) as f64);
            let mut action_error = None;
            if current >= 64. && self.host.has_hold(held.primary.token) {
                action_error = self.host.dispatch_held(held.primary.token, now_ms).err();
                let after = self.after_commit();
                action_error = action_error.or(after);
            }
            let velocity = held.velocity.estimate(now_ms / 1000.);
            let ended = self.end_swipe(&held, HoldEnd::Release { velocity }, now_ms);
            action_error.map_or(ended.map(|_| true), Err)
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

    fn end_swipe(&mut self, held: &SwipeHold, end: HoldEnd, now_ms: f64) -> Result<(), String> {
        let mut error = self.host.hold_end(held.primary.token, end, now_ms).err();
        for companion in held.companions.iter().flatten() {
            let ended = self.host.hold_end(companion.token, HoldEnd::Cancel, now_ms);
            error = error.or(ended.err());
        }
        self.dirty = true;
        error.map_or(Ok(()), |e| Err(format!("hold: {e:?}")))
    }

    /// Escape, scroll takeover, carrier cancellation or invalidated ownership.
    pub fn pointer_cancel(&mut self, now_ms: f64) -> Result<(), String> {
        if self.contact.is_none() {
            return Ok(());
        }
        self.pointer_sample(0., 0., now_ms)?;
        let contact = self.contact.take().unwrap();
        let result = contact
            .hold
            .as_ref()
            .map_or(Ok(()), |held| self.end_swipe(held, HoldEnd::Cancel, now_ms));
        self.set_collection_interaction(None);
        result
    }
}
