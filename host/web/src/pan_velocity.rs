//! A `pan`'s release velocity on the web (LLP 1057 §10.6 phase 2): the DOM
//! measures none, so the page feeds the contact's pointer samples here and
//! asks for `exact_motion::VelocityTracker`'s estimate when it ends — the one
//! estimator of LLP 1057.001 §3, never a second one in JavaScript.
//!
//! Reached only through motion's export (`exact_motion`, ops 13 and 14), so
//! an artifact links it only when its plan uses motion, which a plan with a
//! `panrelease` handler does (LLP 1047 stage 3).

use exact_motion::{Value, VelocityTracker};

/// One contact at a time, as the page's pan has (LLP 1035.003 D1).
pub struct PanVelocity {
    view: u32,
    tracker: Option<VelocityTracker>,
}

impl PanVelocity {
    /// No contact.
    pub const fn new() -> PanVelocity {
        PanVelocity {
            view: 0,
            tracker: None,
        }
    }

    /// Op 13: a pointer sample `(x, y)` in viewport CSS pixels at `ms`, the
    /// event's own timestamp (real time even under the agent's clock). `first`
    /// begins the contact on `view`, forgetting any other.
    pub fn sample(&mut self, view: u32, first: bool, x: f64, y: f64, ms: f64) -> bool {
        if first || self.view != view {
            self.view = view;
            self.tracker = Some(VelocityTracker::new());
        }
        let Some(tracker) = self.tracker.as_mut() else {
            return false;
        };
        tracker.push(ms / 1000.0, Value::new(x, y));
        true
    }

    /// Op 14: the contact's release velocity at `ms` in CSS pixels per
    /// second, then forget it; zero for another view or too few samples.
    pub fn release(&mut self, view: u32, ms: f64) -> (f64, f64) {
        let tracker = self.tracker.take().filter(|_| self.view == view);
        let v = tracker.map_or(Value::ZERO, |t| t.estimate(ms / 1000.0));
        if v.x.is_finite() && v.y.is_finite() {
            (v.x, v.y)
        } else {
            (0.0, 0.0)
        }
    }
}

impl Default for PanVelocity {
    fn default() -> Self {
        PanVelocity::new()
    }
}

#[cfg(test)]
mod tests {
    use super::PanVelocity;

    #[test]
    fn a_steady_flick_releases_at_its_speed_and_is_forgotten() {
        let mut p = PanVelocity::new();
        for i in 0..6 {
            p.sample(
                7,
                i == 0,
                100.0 + 20.0 * i as f64,
                50.0,
                1000.0 + 16.0 * i as f64,
            );
        }
        let (vx, vy) = p.release(7, 1080.0);
        assert!((vx - 1250.0).abs() < 1.0, "{vx}");
        assert!(vy.abs() < 1e-6);
        assert_eq!(p.release(7, 1080.0), (0.0, 0.0), "one release per contact");
    }

    #[test]
    fn a_held_contact_or_another_view_releases_at_rest() {
        let mut p = PanVelocity::new();
        p.sample(7, true, 0.0, 0.0, 0.0);
        p.sample(7, false, 40.0, 0.0, 16.0);
        // Held still for longer than the tracker's window before lifting.
        assert_eq!(p.release(7, 400.0), (0.0, 0.0));
        p.sample(7, true, 0.0, 0.0, 0.0);
        p.sample(7, false, 40.0, 0.0, 16.0);
        assert_eq!(p.release(8, 20.0), (0.0, 0.0));
        // A sample for another view begins a new contact there.
        p.sample(7, true, 0.0, 0.0, 0.0);
        p.sample(9, false, 0.0, 0.0, 10.0);
        p.sample(9, false, 0.0, 30.0, 20.0);
        let (_, vy) = p.release(9, 20.0);
        assert!(vy > 1000.0, "{vy}");
    }
}
