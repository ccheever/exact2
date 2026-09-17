//! Generic presentation ownership. Recognition and displacement belong to Swift.
use super::*;
use exact_motion::{HoldEnd, Value};

impl<D: DataSource> Host<D> {
    pub(crate) fn hold_refusal(&self, error: &str) -> String {
        self.finish(Batch::new(), Some(error.into()))
    }

    /// Capture the engine's current presentation, retaining only live tokens.
    pub fn hold_begin(&mut self, view: ViewId, property: Property, now_ms: f64) -> String {
        let mut batch = Batch::new();
        let Some(node) = self.runner.kernel().node(view) else {
            return self.finish(batch, None);
        };
        let presented = if property == Property::Height {
            let Some(value) = self.height_catch(node.key) else {
                return self.finish(batch, None);
            };
            Some(value)
        } else {
            None
        };
        let result =
            self.engine
                .begin_hold(motion_node(node.key), property, now_ms / 1000., presented);
        let error = match result {
            Ok(Some(start)) => {
                self.now_ms = now_ms;
                self.holds.insert(start.token.serial(), start.token);
                batch.hold(start.token.serial(), start.value.x, start.value.y);
                self.height_layout_if_needed(&mut batch).err()
            }
            Ok(None) => None,
            Err(e) => Some(format!("hold: {e:?}")),
        };
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    /// Reject a foreign, replaced, removed or previous-incarnation token before action.
    pub fn has_hold(&self, handle: u64) -> bool {
        self.holds
            .get(&handle)
            .is_some_and(|t| self.engine.has_hold(*t))
    }

    /// Set a held presentation. Stale requests cannot move the host's clock.
    pub fn hold_update(&mut self, handle: u64, value: Value, now_ms: f64) -> String {
        let mut batch = Batch::new();
        let Some(token) = self
            .holds
            .get(&handle)
            .copied()
            .filter(|t| self.engine.has_hold(*t))
        else {
            return self.finish(batch, None);
        };
        if token.property() == Property::Height
            && (!value.x.is_finite() || value.x < 0. || value.x > f32::MAX as f64)
        {
            return self.hold_refusal("height hold outside finite nonnegative layout range");
        }
        let result = self.engine.update_hold(token, now_ms / 1000., value);
        let layout_error = if matches!(result, Ok(true)) {
            self.now_ms = now_ms;
            self.height_layout_if_needed(&mut batch).err()
        } else {
            None
        };
        self.present(&mut batch, false);
        self.finish(
            batch,
            result
                .err()
                .map(|e| format!("hold: {e:?}"))
                .or(layout_error),
        )
    }

    /// Consume ownership once, after the final sample and any authored action.
    pub fn hold_end(&mut self, handle: u64, end: HoldEnd, now_ms: f64) -> String {
        let mut batch = Batch::new();
        let Some(token) = self
            .holds
            .get(&handle)
            .copied()
            .filter(|t| self.engine.has_hold(*t))
        else {
            return self.finish(batch, None);
        };
        let result = self.engine.end_hold(token, now_ms / 1000., end);
        let layout_error = if matches!(result, Ok(true)) {
            self.now_ms = now_ms;
            self.holds.remove(&handle);
            self.height_layout_if_needed(&mut batch).err()
        } else {
            None
        };
        self.present(&mut batch, false);
        self.finish(
            batch,
            result
                .err()
                .map(|e| format!("hold: {e:?}"))
                .or(layout_error),
        )
    }
}
