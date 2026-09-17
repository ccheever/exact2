//! Native tokens never cross a numeric bridge or enter a historical handle map.
use super::*;
use exact_motion::{EngineError, HoldEnd, HoldStart, HoldToken, Value};

impl<D: DataSource> Host<D> {
    /// Sample at recognition, in milliseconds. Unknown/inert nodes do not adopt slots.
    pub fn hold_begin(
        &mut self,
        view: ViewId,
        property: Property,
        now_ms: f64,
    ) -> Result<Option<HoldStart>, EngineError> {
        let Some(node) = self.kernel().node(view) else {
            return Ok(None);
        };
        if self.route_visibility(view).1 {
            return Ok(None);
        }
        let start =
            self.engine
                .begin_hold(motion_node(node.key), property, now_ms / 1000., None)?;
        if start.is_some() {
            self.now_ms = now_ms;
            self.present();
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
    ) -> Result<bool, EngineError> {
        let accepted = self.engine.update_hold(token, now_ms / 1000., value)?;
        if accepted {
            self.now_ms = now_ms;
            self.present();
        }
        Ok(accepted)
    }

    /// Return to the latest target/declaration after the final sample and action.
    pub fn hold_end(
        &mut self,
        token: HoldToken,
        end: HoldEnd,
        now_ms: f64,
    ) -> Result<bool, EngineError> {
        let accepted = self.engine.end_hold(token, now_ms / 1000., end)?;
        if accepted {
            self.now_ms = now_ms;
            self.present();
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
