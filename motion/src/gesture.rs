//! The recognition thresholds exact2 defines itself, once (LLP 1057.001 §3).
//! Linux reads them here; Apple through `exact_gesture_constant`, the web
//! through its motion `gesture` operation. Where the platform has its own
//! (UIKit's pan hysteresis, AppKit's double-click interval), the platform's wins.

/// A swipe's knee: displacement follows the finger up to here, and commits
/// `swiperight` at release when presented at or past it.
pub const SWIPE_KNEE: f64 = 64.0;
/// Past the knee, presentation moves this fraction of the finger.
pub const SWIPE_RESISTANCE: f64 = 0.2;
/// An authored swipe yields this much of the leading edge to the system's
/// back gesture (precedence rule 1).
pub const SWIPE_EDGE: f64 = 20.0;

/// In the order the hosts' `gesture` lookups index them.
pub const CONSTANTS: [f64; 3] = [SWIPE_KNEE, SWIPE_RESISTANCE, SWIPE_EDGE];

/// Presentation after `delta` of finger travel from a hold caught at `base`:
/// the caught value is inverted through the resistance first, so zero travel
/// is exactly `base` even beyond the knee.
pub fn swipe_displacement(base: f64, delta: f64) -> f64 {
    if delta == 0. {
        return base;
    }
    let origin = if base.abs() <= SWIPE_KNEE {
        base
    } else {
        base.signum() * (SWIPE_KNEE + (base.abs() - SWIPE_KNEE) / SWIPE_RESISTANCE)
    };
    let at = origin + delta;
    if at.abs() <= SWIPE_KNEE {
        at
    } else {
        at.signum() * (SWIPE_KNEE + (at.abs() - SWIPE_KNEE) * SWIPE_RESISTANCE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_knee_resists_and_a_catch_beyond_it_is_exact() {
        assert_eq!(swipe_displacement(0., 40.), 40.);
        assert_eq!(swipe_displacement(0., 164.), 64. + 100. * 0.2);
        assert_eq!(swipe_displacement(84., 0.), 84.);
        assert!((swipe_displacement(84., 10.) - 86.).abs() < 1e-12);
        assert_eq!(swipe_displacement(0., -30.), -30.);
    }
}
