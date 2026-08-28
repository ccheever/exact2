//! The spring: the one timing function CSS does not have.
//!
//! @ref LLP 1002 §2 (declared deviation: `spring()`)
//!
//! A damped harmonic oscillator in closed form, sampled at true elapsed time
//! from its anchor, so a sample depends on the clock and never on the frame
//! cadence that reached it. Its duration is not authored; it is derived from
//! when the spring comes to rest.
//!
//! On the web the same closed form is *lowered*, not evaluated per frame: the
//! evaluator samples it once into keyframes ([`keyframes`]) and the browser
//! plays them through the Web Animations API. One curve, two executors.

use crate::math;

/// A spring's physical parameters (WebKit's `spring()` proposal, minus the
/// initial velocity, which comes from the value's motion at start).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    /// Spring constant. Larger pulls harder toward the target.
    pub stiffness: f64,
    /// Damping coefficient. `2·sqrt(stiffness·mass)` is critical damping.
    pub damping: f64,
    /// Mass of the animated body. Larger responds more slowly.
    pub mass: f64,
}

impl Default for SpringConfig {
    fn default() -> Self {
        SpringConfig {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        }
    }
}

/// Why a spring configuration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringError {
    /// A parameter was infinite or NaN.
    NonFinite,
    /// Stiffness must be positive.
    NonPositiveStiffness,
    /// Damping must be nonnegative.
    NegativeDamping,
    /// Mass must be positive.
    NonPositiveMass,
}

/// Below this displacement and speed, the spring is at rest. Absolute, in the
/// property's own units — a thousandth of a point, degree, or opacity.
pub const REST_THRESHOLD: f64 = 1e-3;

/// Longest a spring may run before it is snapped to its target.
pub const MAX_DURATION: f64 = 10.0;

/// Samples per second when lowering to keyframes or scanning for rest.
pub const SAMPLE_RATE: f64 = 240.0;

/// One sample: the spring's state relative to its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringSample {
    /// Distance from the target.
    pub displacement: f64,
    /// Rate of change of displacement.
    pub velocity: f64,
}

impl SpringSample {
    /// Whether both displacement and speed are under [`REST_THRESHOLD`].
    pub fn at_rest(self) -> bool {
        self.displacement.abs() < REST_THRESHOLD && self.velocity.abs() < REST_THRESHOLD
    }
}

impl SpringConfig {
    /// Check the parameters. Validated once, at the boundary.
    pub fn validate(&self) -> Result<(), SpringError> {
        if !self.stiffness.is_finite() || !self.damping.is_finite() || !self.mass.is_finite() {
            return Err(SpringError::NonFinite);
        }
        if self.stiffness <= 0.0 {
            return Err(SpringError::NonPositiveStiffness);
        }
        if self.damping < 0.0 {
            return Err(SpringError::NegativeDamping);
        }
        if self.mass <= 0.0 {
            return Err(SpringError::NonPositiveMass);
        }
        Ok(())
    }

    /// Whether every parameter is finite.
    pub fn is_finite(&self) -> bool {
        self.stiffness.is_finite() && self.damping.is_finite() && self.mass.is_finite()
    }

    /// The state at `elapsed` seconds after being released at `displacement`
    /// from the target with `velocity`. Closed form in all three regimes.
    pub fn sample(&self, displacement: f64, velocity: f64, elapsed: f64) -> SpringSample {
        let alpha = self.damping / (2.0 * self.mass);
        let omega_squared = self.stiffness / self.mass;
        let discriminant = alpha * alpha - omega_squared;
        let epsilon = omega_squared * 1.0e-12;
        let decay = math::exp(-alpha * elapsed);
        let (x, v) = if discriminant < -epsilon {
            let omega_d = math::sqrt(-discriminant);
            let a = displacement;
            let b = (velocity + alpha * displacement) / omega_d;
            let phase = omega_d * elapsed;
            let cosine = math::cos(phase);
            let sine = math::sin(phase);
            (
                decay * (a * cosine + b * sine),
                decay * ((-alpha * a + b * omega_d) * cosine + (-alpha * b - a * omega_d) * sine),
            )
        } else if discriminant.abs() <= epsilon {
            let a = displacement;
            let b = velocity + alpha * displacement;
            (
                decay * (a + b * elapsed),
                decay * (b - alpha * (a + b * elapsed)),
            )
        } else {
            let root = math::sqrt(discriminant);
            let r1 = -alpha + root;
            let r2 = -alpha - root;
            let c1 = (velocity - r2 * displacement) / (r1 - r2);
            let c2 = displacement - c1;
            let e1 = math::exp(r1 * elapsed);
            let e2 = math::exp(r2 * elapsed);
            (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
        };
        SpringSample {
            displacement: x,
            velocity: v,
        }
    }

    /// Seconds until the spring released at `displacement` with `velocity`
    /// first samples at rest on the [`SAMPLE_RATE`] grid, capped at
    /// [`MAX_DURATION`]. Zero when it is already at rest.
    pub fn settle_time(&self, displacement: f64, velocity: f64) -> f64 {
        let mut n = 0u32;
        loop {
            let t = n as f64 / SAMPLE_RATE;
            if t >= MAX_DURATION {
                return MAX_DURATION;
            }
            if self.sample(displacement, velocity, t).at_rest() {
                return t;
            }
            n += 1;
        }
    }
}

/// One keyframe of a lowered spring: an offset in `[0, 1]` and the absolute
/// value there. Consecutive keyframes interpolate linearly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe {
    /// Position within the animation, `0` at start and `1` at the end.
    pub offset: f64,
    /// The property value at that offset.
    pub value: f64,
}

/// Lower a spring from `from` (moving at `velocity`) to `to` into keyframes
/// sampled at [`SAMPLE_RATE`]. Returns the duration in seconds and the
/// frames; the last frame is exactly `to` at offset `1`.
///
/// This is how the web plays a spring: the host hands the frames to
/// `Element.animate` with a `linear` easing, and the browser's compositor
/// interpolates the same curve the native evaluator would sample.
pub fn keyframes(config: &SpringConfig, from: f64, velocity: f64, to: f64) -> (f64, Vec<Keyframe>) {
    let displacement = from - to;
    let duration = config.settle_time(displacement, velocity);
    if duration <= 0.0 {
        return (
            0.0,
            vec![
                Keyframe {
                    offset: 0.0,
                    value: from,
                },
                Keyframe {
                    offset: 1.0,
                    value: to,
                },
            ],
        );
    }
    let count = (duration * SAMPLE_RATE).round() as usize;
    let mut frames = Vec::with_capacity(count + 1);
    for n in 0..count {
        let t = n as f64 / SAMPLE_RATE;
        frames.push(Keyframe {
            offset: t / duration,
            value: to + config.sample(displacement, velocity, t).displacement,
        });
    }
    frames.push(Keyframe {
        offset: 1.0,
        value: to,
    });
    (duration, frames)
}
