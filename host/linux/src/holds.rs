//! Native tokens never cross a numeric bridge or enter a historical handle map.
use super::*;
use exact_motion::{HoldEnd, HoldStart, HoldToken, Value};

impl<D: DataSource> Host<D> {
    /// A distinct acknowledged picture cannot inherit an old contact's hold.
    /// Only its exact held/returning Translate token may snap to the current target;
    /// this never ends a replacement hold/curve or advances either clock.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn snap_retained_motion(&mut self, token: HoldToken) -> Result<bool, String> {
        if token.property() != Property::Translate
            || !(self.has_hold(token) || self.engine.owns_return(token))
        {
            return Ok(false);
        }
        let Some(value) = self.engine.target(token.node(), Property::Translate) else {
            return Ok(false);
        };
        self.engine
            .remove_property(token.node(), Property::Translate);
        self.engine
            .observe(exact_motion::Change {
                node: token.node(),
                property: Property::Translate,
                value,
                velocity: None,
            })
            .map_err(|e| format!("retained hold snap: {e:?}"))?;
        self.present();
        Ok(true)
    }
    pub(crate) fn dispatch_retained_held(
        &mut self,
        token: HoldToken,
        key: NodeKey,
        binding: &exact_runner::runner::ActionBinding,
        now_ms: f64,
    ) -> Result<bool, String> {
        if token.property() != Property::Translate
            || token.node() != motion_node(key)
            || !self.has_hold(token)
        {
            return Ok(false);
        }
        self.dispatch_retained(key, binding, exact_plan::EventKind::Swiperight, now_ms)
    }
    /// Sample at recognition, in milliseconds. Unknown/inert nodes do not adopt slots.
    pub fn hold_begin(
        &mut self,
        view: ViewId,
        property: Property,
        now_ms: f64,
    ) -> Result<Option<HoldStart>, String> {
        let Some(node) = self.kernel().node(view) else {
            return Ok(None);
        };
        if self.route_visibility(view).1 {
            return Ok(None);
        }
        let presented = if property == Property::Height {
            if self.height_owner != Some(node.key) || self.validate_height_owner(node.key).is_err()
            {
                return Ok(None);
            }
            // An unadopted slot is a no-op before validating presentation.
            if self
                .engine
                .value(motion_node(node.key), Property::Height)
                .is_none()
            {
                return Ok(None);
            }
            let px = node.frame.height as f64;
            if !px.is_finite() || px < 0. || px > f32::MAX as f64 {
                return Err("height position must be finite nonnegative layout pixels".into());
            }
            Some(Value::scalar(px))
        } else {
            None
        };
        let start = self
            .engine
            .begin_hold(motion_node(node.key), property, now_ms / 1000., presented)
            .map_err(|e| format!("hold begin: {e:?}"))?;
        if let Some(start) = start {
            self.now_ms = now_ms;
            if let Err(error) = self.present_hold() {
                // The caller never receives this token on error. Consume it
                // without another layout attempt; publication remains intact.
                let _ = self
                    .engine
                    .end_hold(start.token, now_ms / 1000., HoldEnd::Cancel);
                return Err(error);
            }
        }
        Ok(start)
    }

    /// The token belongs to this engine and the current node generation.
    /// Serials are process-unique; replacing the runtime cannot revive a token.
    pub fn has_hold(&self, token: HoldToken) -> bool {
        self.engine.has_hold(token)
    }

    /// Absolute presentation, never an authored target. Stale tokens are inert.
    pub fn hold_update(
        &mut self,
        token: HoldToken,
        value: Value,
        now_ms: f64,
    ) -> Result<bool, String> {
        if !self.has_hold(token) {
            return Ok(false);
        }
        if token.property() == Property::Height
            && (!value.x.is_finite() || value.x < 0. || value.x > f32::MAX as f64 || value.y != 0.)
        {
            return Err("height position must be finite nonnegative layout pixels".into());
        }
        let accepted = self
            .engine
            .update_hold(token, now_ms / 1000., value)
            .map_err(|e| format!("hold update: {e:?}"))?;
        if accepted {
            self.now_ms = now_ms;
            self.present_hold()?;
        }
        Ok(accepted)
    }

    /// Return to the latest target/declaration after the final sample and action.
    pub fn hold_end(
        &mut self,
        token: HoldToken,
        end: HoldEnd,
        now_ms: f64,
    ) -> Result<bool, String> {
        let accepted = self
            .engine
            .end_hold(token, now_ms / 1000., end)
            .map_err(|e| format!("hold end: {e:?}"))?;
        if accepted {
            self.now_ms = now_ms;
            self.present_hold()?;
        }
        Ok(accepted)
    }

    /// Dispatch the authored swipe only while this live translate token owns it.
    /// Destruction during that action makes the following release harmless.
    pub fn dispatch_held(&mut self, token: HoldToken, now_ms: f64) -> Result<bool, String> {
        if token.property() != Property::Translate || !self.has_hold(token) {
            return Ok(false);
        }
        let key = NodeKey {
            index: token.node() as u32,
            generation: (token.node() >> 32) as u32,
        };
        let Some(node) = self.kernel().node_by_key(key) else {
            return Ok(false);
        };
        let view = node.id;
        if self.route_visibility(view).1
            || !self
                .runner
                .handlers_of(view)
                .contains(&exact_plan::EventKind::Swiperight)
        {
            return Ok(false);
        }
        let mut at = Some(view);
        while let Some(id) = at {
            let Some(node) = self.kernel().node(id) else {
                return Ok(false);
            };
            if node.props.bool(exact_kernel::PropId::Disabled) == Some(true) {
                return Ok(false);
            }
            at = node.parent;
        }
        if !now_ms.is_finite() || now_ms < self.now_ms {
            return Err("invalid held action clock".into());
        }
        match self.dispatch_at(view, Event::Swiperight, now_ms) {
            Some(error) => Err(error),
            None => Ok(true),
        }
    }
}
