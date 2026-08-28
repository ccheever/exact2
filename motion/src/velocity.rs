//! A pointer-velocity estimate for hosts whose platform does not supply one.
//!
//! @ref LLP 1002 §2 (gestures: the platform recognizes; the engine follows)
//!
//! UIKit hands a pan its velocity; the DOM does not. A web host feeds pointer
//! samples here and reads a velocity at release to seed the spring. Recency-
//! weighted least squares over a short window: robust to a jittery last
//! sample, responsive to a flick.

use crate::property::Value;

/// How far back samples count, in seconds.
pub const WINDOW: f64 = 0.1;

const CAPACITY: usize = 16;

/// Recent `(time, value)` samples and the slope through them.
#[derive(Debug, Clone, Default)]
pub struct VelocityTracker {
    samples: Vec<(f64, Value)>,
}

impl VelocityTracker {
    /// Empty.
    pub fn new() -> VelocityTracker {
        VelocityTracker::default()
    }

    /// Forget every sample.
    pub fn reset(&mut self) {
        self.samples.clear();
    }

    /// Record a sample. Non-finite or out-of-order samples are ignored.
    pub fn push(&mut self, time: f64, value: Value) {
        if !time.is_finite() || !value.is_finite() {
            return;
        }
        if self.samples.last().is_some_and(|(t, _)| time < *t) {
            return;
        }
        if self.samples.len() == CAPACITY {
            self.samples.remove(0);
        }
        self.samples.push((time, value));
    }

    /// Velocity at `now` in value units per second; zero with fewer than two
    /// samples inside the window.
    pub fn estimate(&self, now: f64) -> Value {
        let recent: Vec<&(f64, Value)> = self
            .samples
            .iter()
            .filter(|(t, _)| now - *t <= WINDOW)
            .collect();
        if recent.len() < 2 {
            return Value::ZERO;
        }
        // Weighted least-squares slope; weight decays linearly with age.
        let (mut sw, mut st, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
        for (t, v) in &recent {
            let w = 1.0 - (now - *t) / WINDOW;
            sw += w;
            st += w * t;
            sx += w * v.x;
            sy += w * v.y;
        }
        let (mt, mx, my) = (st / sw, sx / sw, sy / sw);
        let (mut stt, mut stx, mut sty) = (0.0, 0.0, 0.0);
        for (t, v) in &recent {
            let w = 1.0 - (now - *t) / WINDOW;
            let dt = t - mt;
            stt += w * dt * dt;
            stx += w * dt * (v.x - mx);
            sty += w * dt * (v.y - my);
        }
        if stt <= 0.0 {
            return Value::ZERO;
        }
        Value::new(stx / stt, sty / stt)
    }
}
