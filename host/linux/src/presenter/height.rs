//! Programmatic single-panel trial; physical vertical recognition is separate.
use super::*;
use exact_motion::{HoldEnd, HoldStart, HoldToken, Property, Value};

impl<D: DataSource> Presenter<D> {
    /// Register the sole numeric border-box panel without advancing time.
    pub fn set_height_owner(&mut self, owner: Option<ViewId>) -> Result<(), String> {
        let previous = self.host.height_owner();
        self.host.set_height_owner(owner)?;
        if self.host.height_owner() == previous {
            return Ok(());
        }
        self.height_feedback()
    }

    /// Catch the panel's actual constrained CSS height at recognition.
    pub fn height_begin(&mut self, now_ms: f64) -> Result<Option<HoldStart>, String> {
        let Some(view) = self
            .host
            .height_owner()
            .and_then(|key| self.host.kernel().node_by_key(key).map(|node| node.id))
        else {
            return Ok(None);
        };
        let start = self.host.hold_begin(view, Property::Height, now_ms)?;
        if let Some(start) = start {
            if let Err(error) = self.height_feedback() {
                let _ = self.host.hold_end(start.token, HoldEnd::Cancel, now_ms);
                return Err(error);
            }
        }
        Ok(start)
    }

    /// Absolute nonnegative CSS height in finite layout pixels.
    pub fn height_update(
        &mut self,
        token: HoldToken,
        px: f64,
        now_ms: f64,
    ) -> Result<bool, String> {
        if token.property() != Property::Height {
            return Ok(false);
        }
        let accepted = self.host.hold_update(token, Value::scalar(px), now_ms)?;
        if accepted {
            self.height_feedback()?;
        }
        Ok(accepted)
    }

    /// Release once toward the latest authored height and transition.
    pub fn height_end(
        &mut self,
        token: HoldToken,
        end: HoldEnd,
        now_ms: f64,
    ) -> Result<bool, String> {
        if token.property() != Property::Height {
            return Ok(false);
        }
        let accepted = self.host.hold_end(token, end, now_ms)?;
        if accepted {
            self.height_feedback()?;
        }
        Ok(accepted)
    }

    fn height_feedback(&mut self) -> Result<(), String> {
        self.dirty = true;
        self.clamp_scroll();
        self.queue_collections();
        self.refine_collections().map_or(Ok(()), Err)
    }
}
