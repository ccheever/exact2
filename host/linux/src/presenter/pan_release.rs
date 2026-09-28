//! A `pan` that began ends with one `panrelease` (LLP 1057 §10.6 phase 2).
//! evdev measures no velocity, so the contact's `VelocityTracker` does, over
//! every sample since the contact went down (LLP 1057.001 §3); a cancelled
//! contact (Escape, wheel takeover, disconnection, an invalidated node)
//! releases at rest.
use super::*;
use exact_kernel::NodeKey;

impl<D: DataSource> Presenter<D> {
    /// Deliver the release to the pan node if it is still live and asks for
    /// one; `None`, or the refusal or commit error.
    pub(super) fn release_pan(
        &mut self,
        key: NodeKey,
        (vx, vy): (f64, f64),
        now_ms: f64,
    ) -> Option<String> {
        if !self.input_live(key) {
            return None;
        }
        let view = self.host.kernel().node_by_key(key)?.id;
        if !self
            .host
            .runner()
            .handlers_of(view)
            .contains(&EventKind::Panrelease)
        {
            return None;
        }
        let (vx, vy) = if vx.is_finite() && vy.is_finite() {
            (vx, vy)
        } else {
            (0.0, 0.0)
        };
        self.dirty = true;
        self.host
            .dispatch_at(view, Event::PanRelease(vx, vy), now_ms)
            .or(self.after_commit())
    }
}

#[cfg(test)]
#[path = "pan_release_tests.rs"]
mod pan_release_tests;
