//! The `pan` contact's release velocity, where the platform measures none.
//!
//! @ref LLP 1057 §10.6 (`panrelease`), LLP 1057.001 §3 (one velocity estimator)
//!
//! UIKit hands its pan a velocity; AppKit's mouse and the iOS agent's
//! recognized contact do not. Those feed viewport-space pointer samples here
//! and read the engine's [`VelocityTracker`] at release — never a Swift
//! estimator. One tracker per runtime: a session holds one contact (LLP
//! 1035.003 D1), and a first sample starts it over.

use crate::abi::Bridge;
use exact_motion::{Value, VelocityTracker};
use exact_runner::DataSource;

/// The live contact's samples; `None` before the first contact.
#[derive(Debug, Default)]
pub struct PanVelocity {
    tracker: Option<VelocityTracker>,
}

impl PanVelocity {
    /// No contact yet.
    pub const fn new() -> PanVelocity {
        PanVelocity { tracker: None }
    }

    /// One pointer sample at `t` seconds (any monotonic origin): viewport
    /// CSS px. `first` forgets the previous contact before it. A non-finite
    /// sample is refused (`false`), and a first one still forgets.
    pub fn sample(&mut self, first: bool, x: f64, y: f64, t: f64) -> bool {
        let tracker = self.tracker.get_or_insert_with(VelocityTracker::new);
        if first {
            tracker.reset();
        }
        if !(x.is_finite() && y.is_finite() && t.is_finite()) {
            return false;
        }
        tracker.push(t, Value::new(x, y));
        true
    }

    /// Velocity along `axis` (0 x, 1 y) at `t` seconds, px per second: zero
    /// with fewer than two samples in the window, before any contact, for a
    /// non-finite `t` or another axis — a contact with no measure releases
    /// at rest.
    pub fn estimate(&self, axis: u32, t: f64) -> f64 {
        let Some(tracker) = self.tracker.as_ref().filter(|_| t.is_finite()) else {
            return 0.0;
        };
        let v = tracker.estimate(t);
        match axis {
            0 => v.x,
            1 => v.y,
            _ => 0.0,
        }
    }
}

impl<D: DataSource> Bridge<D> {
    /// This runtime's pan tracker (`exact_pan_sample`, `exact_pan_velocity`).
    pub fn pan_velocity(&mut self) -> &mut PanVelocity {
        &mut self.pan
    }
}

/// Export `exact_pan_sample` and `exact_pan_velocity` over `host!`'s
/// registry; invoked inside `host!`, after `EXACT_RUNTIMES`.
#[macro_export]
macro_rules! pan_velocity_exports {
    () => {
        /// One pointer sample (viewport CSS px, seconds) for the pan contact;
        /// nonzero `first` starts a contact. 1 taken, 0 refused.
        #[no_mangle]
        pub extern "C" fn exact_pan_sample(rt: u32, first: u32, x: f64, y: f64, t: f64) -> u32 {
            $crate::abi::with_runtime(
                &EXACT_RUNTIMES,
                rt,
                false,
                |b, _| u32::from(b.pan_velocity().sample(first != 0, x, y, t)),
                |_| 0,
            )
        }
        /// The contact's velocity along `axis` (0 x, 1 y) at `t` seconds, px/s.
        #[no_mangle]
        pub extern "C" fn exact_pan_velocity(rt: u32, axis: u32, t: f64) -> f64 {
            $crate::abi::with_runtime(
                &EXACT_RUNTIMES,
                rt,
                false,
                |b, _| b.pan_velocity().estimate(axis, t),
                |_| 0.0,
            )
        }
    };
}

#[cfg(test)]
mod tests {
    use super::PanVelocity;

    #[test]
    fn a_steady_drag_measures_its_speed_in_px_per_second() {
        let mut pan = PanVelocity::new();
        assert!(pan.sample(true, 100.0, 50.0, 10.0));
        for i in 1..=6 {
            let t = 10.0 + f64::from(i) * 0.016;
            assert!(pan.sample(
                false,
                100.0 + f64::from(i) * 32.0,
                50.0 - f64::from(i) * 8.0,
                t
            ));
        }
        let now = 10.0 + 6.0 * 0.016;
        assert!(
            (pan.estimate(0, now) - 2000.0).abs() < 1e-6,
            "{}",
            pan.estimate(0, now)
        );
        assert!(
            (pan.estimate(1, now) + 500.0).abs() < 1e-6,
            "{}",
            pan.estimate(1, now)
        );
        assert_eq!(pan.estimate(2, now), 0.0, "no third axis");
    }

    #[test]
    fn fewer_than_two_samples_or_no_contact_is_at_rest() {
        let mut pan = PanVelocity::new();
        assert_eq!(pan.estimate(0, 1.0), 0.0, "before any contact");
        assert!(pan.sample(true, 0.0, 0.0, 1.0));
        assert_eq!((pan.estimate(0, 1.0), pan.estimate(1, 1.0)), (0.0, 0.0));
        assert!(pan.sample(false, 40.0, 0.0, 1.016));
        assert!(pan.estimate(0, 1.016) > 0.0);
        // A pause longer than the window leaves nothing to measure.
        assert_eq!(pan.estimate(0, 2.0), 0.0);
    }

    #[test]
    fn a_first_sample_forgets_the_previous_contact() {
        let mut pan = PanVelocity::new();
        pan.sample(true, 0.0, 0.0, 1.0);
        pan.sample(false, 100.0, 0.0, 1.05);
        assert!(pan.estimate(0, 1.05) > 0.0);
        pan.sample(true, 500.0, 0.0, 1.06);
        assert_eq!(pan.estimate(0, 1.06), 0.0, "one sample of the new contact");
    }

    #[test]
    fn non_finite_samples_and_instants_are_refused() {
        let mut pan = PanVelocity::new();
        pan.sample(true, 0.0, 0.0, 1.0);
        pan.sample(false, 20.0, 0.0, 1.01);
        let before = pan.estimate(0, 1.01);
        assert!(!pan.sample(false, f64::NAN, 0.0, 1.012));
        assert!(!pan.sample(false, 0.0, f64::INFINITY, 1.012));
        assert!(!pan.sample(false, 30.0, 0.0, f64::NAN));
        assert_eq!(
            pan.estimate(0, 1.01),
            before,
            "refused samples change nothing"
        );
        assert_eq!(pan.estimate(0, f64::NAN), 0.0);
        assert_eq!(pan.estimate(0, f64::INFINITY), 0.0);
        // A refused first sample still ends the old contact.
        assert!(!pan.sample(true, f64::NAN, 0.0, 1.02));
        assert_eq!(pan.estimate(0, 1.02), 0.0);
    }
}
