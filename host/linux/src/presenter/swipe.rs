//! Swipe-specific presentation and completion policy.
use super::contact::{HeldKind, Hold};
use super::*;
use exact_kernel::{NodeKey, TouchAction};
use exact_motion::{HoldEnd, Property, Value, VelocityTracker};

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
    pub(super) fn swipe_candidate(&self, hit: NodeKey) -> Option<NodeKey> {
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

    pub(super) fn begin_swipe(
        &mut self,
        key: NodeKey,
        now_ms: f64,
    ) -> Result<Option<Hold>, String> {
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
        let mut held = Hold {
            primary,
            kind: HeldKind::Swipe {
                companions: [None, None],
            },
            velocity: VelocityTracker::new(),
        };
        if let Some(companion) = companion {
            for (i, property) in [Property::Opacity, Property::Scale].into_iter().enumerate() {
                match self.host.hold_begin(companion, property, now_ms) {
                    Ok(start) => {
                        let HeldKind::Swipe { companions } = &mut held.kind else {
                            unreachable!()
                        };
                        companions[i] = start;
                    }
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

    pub(super) fn move_swipe(
        &mut self,
        held: &mut Hold,
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
        let HeldKind::Swipe { companions } = &held.kind else {
            unreachable!()
        };
        for companion in companions.iter().flatten() {
            let value = Value::scalar(indicator(companion.value.x, caught, progress));
            self.host
                .hold_update(companion.token, value, now_ms)
                .map_err(|e| format!("hold: {e:?}"))?;
        }
        self.dirty = true;
        Ok(true)
    }

    pub(super) fn end_swipe(
        &mut self,
        held: &Hold,
        end: HoldEnd,
        now_ms: f64,
    ) -> Result<(), String> {
        let mut error = self.host.hold_end(held.primary.token, end, now_ms).err();
        let HeldKind::Swipe { companions } = &held.kind else {
            unreachable!()
        };
        for companion in companions.iter().flatten() {
            let ended = self.host.hold_end(companion.token, HoldEnd::Cancel, now_ms);
            error = error.or(ended.err());
        }
        self.dirty = true;
        error.map_or(Ok(()), |e| Err(format!("hold: {e:?}")))
    }
}
