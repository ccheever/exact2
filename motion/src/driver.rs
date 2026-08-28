//! Closed-form animation drivers and the main-owned driver table.
//!
//! @ref LLP 0099#animation-drivers
//! @ref LLP 0099#clock-ownership
//!
//! Every leaf driver is analytic: a spring, a timing curve, or a decay is
//! sampled at true elapsed time from its most recent anchor, so a sample
//! depends on the clock's time and never on the frame cadence that reached it.
//! That is what makes the evaluator refresh invariant and seekable — a test
//! advances the clock instead of waiting for a settle.
//!
//! Composed drivers (sequence, repeat) cannot be closed-form, because the
//! handoff between members depends on when the previous member settled. They
//! step a fixed quantum instead, which keeps them refresh invariant too, at a
//! bounded catch-up cost.

use crate::math;
use std::collections::BTreeMap;

/// Fixed step used only by composed drivers.
///
/// Analytic leaf drivers always sample at true elapsed time and never cap a
/// frame delta; only sequence and repeat, whose member handoff depends on when
/// the previous member settled, need a quantum.
pub const MOTION_COMPOSITION_QUANTUM_SECONDS: f64 = 1.0 / 240.0;
/// Bound numerical catch-up for composed drivers to two seconds. Any larger
/// gap is explicitly re-anchored after this many deterministic steps.
pub const MOTION_MAX_COMPOSITION_CATCH_UP_STEPS: usize = 480;

#[derive(Debug, Clone, Copy, PartialEq)]
/// One evaluated point on a driver's curve.
pub struct MotionSample {
    /// Value at this point on the curve.
    pub position: f64,
    /// Rate of change at this point, in value units per second.
    pub velocity: f64,
    /// Whether the driver has reached its rest condition and will not move again.
    pub settled: bool,
}

impl MotionSample {
    fn validate_finite(self, field: &'static str) -> Result<Self, MotionDriverError> {
        if self.position.is_finite() && self.velocity.is_finite() {
            Ok(self)
        } else {
            Err(MotionDriverError::NonFinite(field))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Why a driver stopped owning its value.
///
/// Exactly one terminal is delivered per driver. The reason is what a host
/// needs to distinguish a completed animation from an interrupted one.
pub enum DriverTerminalReason {
    /// The driver reached its rest thresholds.
    Settled,
    /// A new driver took the value.
    Replaced,
    /// The application wrote the value directly.
    ImperativeWrite,
    /// A gesture took ownership of the value.
    GestureWrite,
    /// The owner cancelled the driver.
    ExplicitCancel,
    /// The descriptor that started the driver left the graph.
    DescriptorDetached,
    /// Reduced-motion policy resolved the animation straight to its target.
    ReducedMotion,
    /// The driver table was reset.
    Reset,
    /// Arithmetic left the finite range; the last finite sample was published.
    NumericalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The terminal record published when a driver stops owning a value.
pub struct DriverTerminal {
    /// Identity of the driver that ended, unique within its table.
    pub driver_sequence: u64,
    /// The value the driver owned.
    pub value_handle: u64,
    /// Why it ended.
    pub reason: DriverTerminalReason,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// The result of advancing one driver by one frame.
pub struct DriverAdvance {
    /// The value and velocity to publish for this frame.
    pub sample: MotionSample,
    /// A leaf or composed driver reports settlement exactly once.
    pub terminal: Option<DriverTerminalReason>,
}

#[derive(Debug, Clone, PartialEq)]
/// Why a driver could not be built or advanced.
///
/// Every variant is a rejection at construction or advance time. A driver that
/// was accepted never silently publishes a non-finite value.
pub enum MotionDriverError {
    /// A named input or intermediate was infinite or NaN.
    NonFinite(&'static str),
    /// A named input that must be positive was zero or negative.
    NonPositive(&'static str),
    /// A decay deceleration outside the open interval `(0, 1)`.
    InvalidDeceleration,
    /// A sequence with no members.
    EmptySequence,
    /// A repeat count of zero, or negative other than `-1` for forever.
    InvalidRepeatCount,
    /// A sequence or repeat was used where only a leaf driver can be reversed.
    UnsupportedReverseComposition,
    /// Time was advanced backwards.
    NegativeElapsed,
    /// The table's `u64` driver sequence counter is exhausted.
    DriverSequenceExhausted,
}

fn require_finite(value: f64, field: &'static str) -> Result<f64, MotionDriverError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(MotionDriverError::NonFinite(field))
    }
}

fn require_positive(value: f64, field: &'static str) -> Result<f64, MotionDriverError> {
    require_finite(value, field)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(MotionDriverError::NonPositive(field))
    }
}

fn require_nonnegative(value: f64, field: &'static str) -> Result<f64, MotionDriverError> {
    require_finite(value, field)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(MotionDriverError::NonPositive(field))
    }
}

fn add_elapsed(current: f64, delta: f64) -> Result<f64, MotionDriverError> {
    require_elapsed(delta)?;
    require_finite(current + delta, "driver.accumulated_elapsed_seconds")
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// A damped harmonic oscillator's physical parameters and rest thresholds.
pub struct SpringConfig {
    /// Damping coefficient. Zero oscillates forever; larger values settle sooner.
    pub damping: f64,
    /// Spring constant. Larger values pull harder toward the target.
    pub stiffness: f64,
    /// Mass of the animated body. Larger values respond more slowly.
    pub mass: f64,
    /// Speed below which the spring may be considered at rest.
    pub rest_velocity_threshold: f64,
    /// Distance from the target below which the spring may be considered at rest.
    pub rest_displacement_threshold: f64,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            damping: 15.0,
            stiffness: 150.0,
            mass: 1.0,
            rest_velocity_threshold: 0.01,
            rest_displacement_threshold: 0.01,
        }
    }
}

impl SpringConfig {
    pub(crate) fn validate(self) -> Result<Self, MotionDriverError> {
        require_nonnegative(self.damping, "spring.damping")?;
        require_positive(self.stiffness, "spring.stiffness")?;
        require_positive(self.mass, "spring.mass")?;
        require_positive(
            self.rest_velocity_threshold,
            "spring.rest_velocity_threshold",
        )?;
        require_positive(
            self.rest_displacement_threshold,
            "spring.rest_displacement_threshold",
        )?;
        let alpha = require_finite(
            self.damping / (2.0 * self.mass),
            "spring.damping_mass_ratio",
        )?;
        let omega_squared =
            require_finite(self.stiffness / self.mass, "spring.stiffness_mass_ratio")?;
        require_finite(alpha * alpha, "spring.alpha_squared")?;
        require_finite(alpha * alpha - omega_squared, "spring.discriminant")?;
        Ok(self)
    }
}

/// Closed-form damped harmonic oscillator. `elapsed_seconds` is measured from
/// the most recent start or retarget anchor, so sampling is refresh invariant.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDriver {
    target: f64,
    config: SpringConfig,
    anchor_position: f64,
    anchor_velocity: f64,
    elapsed_seconds: f64,
    sample: MotionSample,
}

impl SpringDriver {
    /// Anchor a spring at `position` and `velocity`, aimed at `target`.
    ///
    /// The configuration is validated here, so a spring that exists can be sampled
    /// at any elapsed time without producing a non-finite value.
    pub fn new(
        position: f64,
        velocity: f64,
        target: f64,
        config: SpringConfig,
    ) -> Result<Self, MotionDriverError> {
        require_finite(position, "spring.position")?;
        require_finite(velocity, "spring.velocity")?;
        require_finite(target, "spring.target")?;
        let config = config.validate()?;
        spring_sample(position, velocity, target, config, 0.0)?;
        Ok(Self {
            target,
            config,
            anchor_position: position,
            anchor_velocity: velocity,
            elapsed_seconds: 0.0,
            sample: MotionSample {
                position,
                velocity,
                settled: false,
            },
        })
    }

    /// Aim the spring at a new target from its current position and velocity.
    ///
    /// Retargeting re-anchors elapsed time at zero, which is what keeps an
    /// interrupted animation continuous instead of snapping.
    pub fn retarget(&mut self, target: f64) -> Result<(), MotionDriverError> {
        require_finite(target, "spring.target")?;
        spring_sample(
            self.sample.position,
            self.sample.velocity,
            target,
            self.config,
            0.0,
        )?;
        self.target = target;
        self.anchor_position = self.sample.position;
        self.anchor_velocity = self.sample.velocity;
        self.elapsed_seconds = 0.0;
        self.sample.settled = false;
        Ok(())
    }

    /// Advance by `elapsed_seconds` and return the new sample.
    pub fn advance(&mut self, elapsed_seconds: f64) -> Result<MotionSample, MotionDriverError> {
        require_elapsed(elapsed_seconds)?;
        if self.sample.settled {
            return Ok(self.sample);
        }
        let next_elapsed = add_elapsed(self.elapsed_seconds, elapsed_seconds)?;
        let sample = spring_sample(
            self.anchor_position,
            self.anchor_velocity,
            self.target,
            self.config,
            next_elapsed,
        )?;
        self.elapsed_seconds = next_elapsed;
        self.sample = sample;
        Ok(self.sample)
    }

    /// The most recently computed sample.
    pub fn sample(&self) -> MotionSample {
        self.sample
    }
}

fn spring_sample(
    initial_position: f64,
    initial_velocity: f64,
    target: f64,
    config: SpringConfig,
    elapsed_seconds: f64,
) -> Result<MotionSample, MotionDriverError> {
    let displacement = require_finite(initial_position - target, "spring.displacement")?;
    let alpha = config.damping / (2.0 * config.mass);
    let omega_0 = math::sqrt(config.stiffness / config.mass);
    let discriminant = require_finite(alpha * alpha - omega_0 * omega_0, "spring.discriminant")?;
    let epsilon = require_finite(omega_0 * omega_0 * 1.0e-12, "spring.epsilon")?;
    let (relative_position, velocity) = if discriminant < -epsilon {
        let omega_d = math::sqrt(-discriminant);
        let a = displacement;
        let b = (initial_velocity + alpha * displacement) / omega_d;
        let phase = omega_d * elapsed_seconds;
        let decay = math::exp(-alpha * elapsed_seconds);
        let cosine = math::cos(phase);
        let sine = math::sin(phase);
        let position = decay * (a * cosine + b * sine);
        let velocity =
            decay * ((-alpha * a + b * omega_d) * cosine + (-alpha * b - a * omega_d) * sine);
        (position, velocity)
    } else if discriminant.abs() <= epsilon {
        let a = displacement;
        let b = initial_velocity + alpha * displacement;
        let decay = math::exp(-alpha * elapsed_seconds);
        let position = decay * (a + b * elapsed_seconds);
        let velocity = decay * (b - alpha * (a + b * elapsed_seconds));
        (position, velocity)
    } else {
        let root = math::sqrt(discriminant);
        let r1 = -alpha + root;
        let r2 = -alpha - root;
        let c1 = (initial_velocity - r2 * displacement) / (r1 - r2);
        let c2 = displacement - c1;
        let e1 = math::exp(r1 * elapsed_seconds);
        let e2 = math::exp(r2 * elapsed_seconds);
        (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
    };
    let mut sample = MotionSample {
        position: target + relative_position,
        velocity,
        settled: velocity.abs() < config.rest_velocity_threshold
            && relative_position.abs() < config.rest_displacement_threshold,
    };
    if sample.settled {
        sample.position = target;
        sample.velocity = 0.0;
    }
    sample.validate_finite("spring.sample")
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// The progress curve a timing driver follows.
pub enum MotionEasing {
    /// Progress advances at a constant rate.
    Linear,
    /// A cubic Bezier with the two control points CSS `cubic-bezier()` takes.
    /// The `x` coordinates are clamped to `[0, 1]` so the curve stays a function
    /// of progress.
    CubicBezier {
        /// First control point's progress coordinate, in `[0, 1]`.
        x1: f64,
        /// First control point's value coordinate.
        y1: f64,
        /// Second control point's progress coordinate, in `[0, 1]`.
        x2: f64,
        /// Second control point's value coordinate.
        y2: f64,
    },
}

impl MotionEasing {
    fn validate(self) -> Result<Self, MotionDriverError> {
        match self {
            Self::Linear => Ok(self),
            Self::CubicBezier { x1, y1, x2, y2 } => {
                require_finite(x1, "timing.easing.x1")?;
                require_finite(y1, "timing.easing.y1")?;
                require_finite(x2, "timing.easing.x2")?;
                require_finite(y2, "timing.easing.y2")?;
                if (0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2) {
                    Ok(self)
                } else {
                    Err(MotionDriverError::NonPositive("timing.easing.x"))
                }
            }
        }
    }

    fn value_and_slope(self, progress: f64) -> (f64, f64) {
        match self {
            Self::Linear => (progress, 1.0),
            Self::CubicBezier { x1, y1, x2, y2 } => {
                let mut low = 0.0;
                let mut high = 1.0;
                let mut parameter = progress;
                for _ in 0..12 {
                    let x = cubic_bezier(parameter, x1, x2);
                    if x < progress {
                        low = parameter;
                    } else {
                        high = parameter;
                    }
                    parameter = (low + high) * 0.5;
                }
                let value = cubic_bezier(parameter, y1, y2);
                let dx = cubic_bezier_derivative(parameter, x1, x2);
                let dy = cubic_bezier_derivative(parameter, y1, y2);
                let slope = if dx.abs() <= f64::EPSILON {
                    0.0
                } else {
                    dy / dx
                };
                (value, slope)
            }
        }
    }
}

fn cubic_bezier(parameter: f64, control1: f64, control2: f64) -> f64 {
    let inverse = 1.0 - parameter;
    3.0 * inverse * inverse * parameter * control1
        + 3.0 * inverse * parameter * parameter * control2
        + parameter * parameter * parameter
}

fn cubic_bezier_derivative(parameter: f64, control1: f64, control2: f64) -> f64 {
    let inverse = 1.0 - parameter;
    3.0 * inverse * inverse * control1
        + 6.0 * inverse * parameter * (control2 - control1)
        + 3.0 * parameter * parameter * (1.0 - control2)
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// A timing driver's duration and progress curve.
pub struct TimingConfig {
    /// Time from start to target, in seconds. Zero settles on the first advance.
    pub duration_seconds: f64,
    /// Curve mapping elapsed progress to eased progress.
    pub easing: MotionEasing,
}

impl TimingConfig {
    fn validate(self) -> Result<Self, MotionDriverError> {
        require_nonnegative(self.duration_seconds, "timing.duration_seconds")?;
        self.easing.validate()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// A duration-and-curve driver.
///
/// Closed form: the sample depends only on elapsed time, so the same duration
/// reached in a different number of frames produces the same value.
pub struct TimingDriver {
    initial_position: f64,
    target: f64,
    config: TimingConfig,
    elapsed_seconds: f64,
    sample: MotionSample,
}

impl TimingDriver {
    /// Start a timing driver at `position`, aimed at `target`.
    pub fn new(
        position: f64,
        target: f64,
        config: TimingConfig,
    ) -> Result<Self, MotionDriverError> {
        require_finite(position, "timing.position")?;
        require_finite(target, "timing.target")?;
        let config = config.validate()?;
        if config.duration_seconds > 0.0 {
            timing_sample(position, target, config, 0.0)?;
        }
        Ok(Self {
            initial_position: position,
            target,
            config,
            elapsed_seconds: 0.0,
            sample: MotionSample {
                position,
                velocity: 0.0,
                settled: false,
            },
        })
    }

    /// Advance by `elapsed_seconds` and return the new sample.
    pub fn advance(&mut self, elapsed_seconds: f64) -> Result<MotionSample, MotionDriverError> {
        require_elapsed(elapsed_seconds)?;
        if self.sample.settled {
            return Ok(self.sample);
        }
        if self.config.duration_seconds == 0.0 {
            self.sample = MotionSample {
                position: self.target,
                velocity: 0.0,
                settled: true,
            };
            return Ok(self.sample);
        }
        let next_elapsed = add_elapsed(self.elapsed_seconds, elapsed_seconds)?;
        let sample = timing_sample(
            self.initial_position,
            self.target,
            self.config,
            next_elapsed,
        )?;
        self.elapsed_seconds = next_elapsed;
        self.sample = sample;
        Ok(self.sample)
    }

    /// The most recently computed sample.
    pub fn sample(&self) -> MotionSample {
        self.sample
    }
}

fn timing_sample(
    initial_position: f64,
    target: f64,
    config: TimingConfig,
    elapsed_seconds: f64,
) -> Result<MotionSample, MotionDriverError> {
    let distance = require_finite(target - initial_position, "timing.distance")?;
    let progress = (elapsed_seconds / config.duration_seconds).clamp(0.0, 1.0);
    if progress >= 1.0 {
        return Ok(MotionSample {
            position: target,
            velocity: 0.0,
            settled: true,
        });
    }
    let (eased, slope) = config.easing.value_and_slope(progress);
    MotionSample {
        position: initial_position + distance * eased,
        velocity: distance * slope / config.duration_seconds,
        settled: false,
    }
    .validate_finite("timing.sample")
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// A decay driver's friction, bounds, and rest threshold.
pub struct DecayConfig {
    /// Per-millisecond velocity multiplier, strictly between zero and one.
    pub deceleration: f64,
    /// Optional inclusive `(minimum, maximum)` bound on the value.
    pub clamp: Option<(f64, f64)>,
    /// Whether crossing a clamp bound pulls back elastically instead of stopping.
    pub rubber_band_effect: bool,
    /// Fraction of the overshoot kept while rubber banding, in `(0, 1]`.
    pub rubber_band_factor: f64,
    /// Speed below which the decay may be considered at rest.
    pub rest_velocity_threshold: f64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            deceleration: 0.998,
            clamp: None,
            rubber_band_effect: false,
            rubber_band_factor: 0.55,
            rest_velocity_threshold: 0.01,
        }
    }
}

impl DecayConfig {
    fn validate(self) -> Result<Self, MotionDriverError> {
        require_finite(self.deceleration, "decay.deceleration")?;
        if !(0.0..1.0).contains(&self.deceleration) {
            return Err(MotionDriverError::InvalidDeceleration);
        }
        require_positive(
            self.rest_velocity_threshold,
            "decay.rest_velocity_threshold",
        )?;
        require_positive(self.rubber_band_factor, "decay.rubber_band_factor")?;
        if self.rubber_band_factor > 1.0 {
            return Err(MotionDriverError::NonPositive("decay.rubber_band_factor"));
        }
        if let Some((minimum, maximum)) = self.clamp {
            require_finite(minimum, "decay.clamp.minimum")?;
            require_finite(maximum, "decay.clamp.maximum")?;
            if minimum > maximum {
                return Err(MotionDriverError::NonPositive("decay.clamp.range"));
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// A velocity-and-friction driver, for flings and momentum scrolling.
///
/// Closed form: position and velocity are exponential in elapsed time, so a
/// long frame gap lands exactly where a run of short frames would have.
pub struct DecayDriver {
    initial_position: f64,
    initial_velocity: f64,
    config: DecayConfig,
    elapsed_seconds: f64,
    sample: MotionSample,
}

impl DecayDriver {
    /// Start a decay from `position` at `velocity`.
    pub fn new(
        position: f64,
        velocity: f64,
        config: DecayConfig,
    ) -> Result<Self, MotionDriverError> {
        require_finite(position, "decay.position")?;
        require_finite(velocity, "decay.velocity")?;
        let config = config.validate()?;
        validate_decay_range(position, velocity, config)?;
        Ok(Self {
            initial_position: position,
            initial_velocity: velocity,
            config,
            elapsed_seconds: 0.0,
            sample: MotionSample {
                position,
                velocity,
                settled: false,
            },
        })
    }

    /// Advance by `elapsed_seconds` and return the new sample.
    pub fn advance(&mut self, elapsed_seconds: f64) -> Result<MotionSample, MotionDriverError> {
        require_elapsed(elapsed_seconds)?;
        if self.sample.settled {
            return Ok(self.sample);
        }
        let next_elapsed = add_elapsed(self.elapsed_seconds, elapsed_seconds)?;
        let exponent = math::ln(self.config.deceleration) * 1000.0;
        let velocity_factor = math::exp(exponent * next_elapsed);
        let raw_velocity = self.initial_velocity * velocity_factor;
        let terminal_displacement = self.initial_velocity / exponent;
        let raw_position = self.initial_position - terminal_displacement * (1.0 - velocity_factor);
        let (position, velocity, clamped) = project_decay_sample(
            raw_position,
            raw_velocity,
            self.config.clamp,
            self.config.rubber_band_effect,
            self.config.rubber_band_factor,
        );
        let mut sample = MotionSample {
            position,
            velocity,
            settled: clamped || raw_velocity.abs() < self.config.rest_velocity_threshold,
        };
        if sample.settled {
            if let Some((minimum, maximum)) = self.config.clamp {
                sample.position = sample.position.clamp(minimum, maximum);
            }
            sample.velocity = 0.0;
        }
        let sample = sample.validate_finite("decay.sample")?;
        self.elapsed_seconds = next_elapsed;
        self.sample = sample;
        Ok(self.sample)
    }

    /// The most recently computed sample.
    pub fn sample(&self) -> MotionSample {
        self.sample
    }
}

fn validate_decay_range(
    initial_position: f64,
    initial_velocity: f64,
    config: DecayConfig,
) -> Result<(), MotionDriverError> {
    let exponent = require_finite(math::ln(config.deceleration) * 1000.0, "decay.exponent")?;
    let terminal_displacement =
        require_finite(initial_velocity / exponent, "decay.terminal_displacement")?;
    let asymptote = require_finite(initial_position - terminal_displacement, "decay.asymptote")?;
    if config.rubber_band_effect {
        for position in [initial_position, asymptote] {
            let (position, velocity, _) = project_decay_sample(
                position,
                initial_velocity,
                config.clamp,
                true,
                config.rubber_band_factor,
            );
            MotionSample {
                position,
                velocity,
                settled: false,
            }
            .validate_finite("decay.rubber_band_sample")?;
        }
    }
    Ok(())
}

fn project_decay_sample(
    position: f64,
    velocity: f64,
    clamp: Option<(f64, f64)>,
    rubber_band_effect: bool,
    rubber_band_factor: f64,
) -> (f64, f64, bool) {
    let Some((minimum, maximum)) = clamp else {
        return (position, velocity, false);
    };
    if position < minimum {
        if rubber_band_effect {
            (
                minimum + (position - minimum) * rubber_band_factor,
                velocity * rubber_band_factor,
                false,
            )
        } else {
            (minimum, 0.0, true)
        }
    } else if position > maximum {
        if rubber_band_effect {
            (
                maximum + (position - maximum) * rubber_band_factor,
                velocity * rubber_band_factor,
                false,
            )
        } else {
            (maximum, 0.0, true)
        }
    } else {
        (position, velocity, false)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// The authored description of a driver — plan data, not a running driver.
///
/// A compiler emits these; [`AnimationDriverSpec::instantiate`] turns one into
/// a running [`AnimationDriver`] anchored at a starting position and velocity.
pub enum AnimationDriverSpec {
    /// Settle at a target under spring physics.
    Spring {
        /// Value the spring is aimed at.
        target: f64,
        /// Physical parameters and rest thresholds.
        config: SpringConfig,
    },
    /// Move to a target over a fixed duration along a curve.
    Timing {
        /// Value reached when the duration elapses.
        target: f64,
        /// Duration and progress curve.
        config: TimingConfig,
    },
    /// Coast to a stop under friction.
    Decay {
        /// Starting velocity, or `None` to inherit the velocity at instantiation.
        velocity: Option<f64>,
        /// Friction, bounds, and rest threshold.
        config: DecayConfig,
    },
    /// Run each member in order, each starting where the previous one settled.
    Sequence(Vec<AnimationDriverSpec>),
    /// Repeat one driver, optionally alternating direction.
    Repeat {
        /// The driver to repeat.
        inner: Box<AnimationDriverSpec>,
        /// Iterations to run, or `-1` to repeat forever.
        count: i32,
        /// Whether alternate iterations run back toward the starting position.
        reverse: bool,
    },
}

impl AnimationDriverSpec {
    /// Instantiate a running driver anchored at a position and velocity.
    ///
    /// `inherited_velocity` is what makes an interrupted animation continuous: a
    /// driver replacing another starts from the velocity the value already had.
    pub fn instantiate(
        &self,
        initial_position: f64,
        inherited_velocity: f64,
    ) -> Result<AnimationDriver, MotionDriverError> {
        AnimationDriver::from_spec(self, initial_position, inherited_velocity)
    }

    fn reversed_for(&self, target: f64) -> Result<Self, MotionDriverError> {
        match self {
            Self::Spring { config, .. } => Ok(Self::Spring {
                target,
                config: *config,
            }),
            Self::Timing { config, .. } => Ok(Self::Timing {
                target,
                config: *config,
            }),
            Self::Decay { velocity, config } => Ok(Self::Decay {
                velocity: velocity.map(|value| -value),
                config: *config,
            }),
            Self::Sequence(_) | Self::Repeat { .. } => {
                Err(MotionDriverError::UnsupportedReverseComposition)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum AnimationDriverState {
    Spring(SpringDriver),
    Timing(TimingDriver),
    Decay(DecayDriver),
    Sequence(SequenceDriver),
    Repeat(RepeatDriver),
}

#[derive(Debug, Clone, PartialEq)]
/// A running driver: one instantiated [`AnimationDriverSpec`] plus its state.
pub struct AnimationDriver {
    state: AnimationDriverState,
    sample: MotionSample,
    terminal_delivered: bool,
}

impl AnimationDriver {
    fn from_spec(
        spec: &AnimationDriverSpec,
        initial_position: f64,
        inherited_velocity: f64,
    ) -> Result<Self, MotionDriverError> {
        require_finite(initial_position, "driver.initial_position")?;
        require_finite(inherited_velocity, "driver.inherited_velocity")?;
        let state = match spec {
            AnimationDriverSpec::Spring { target, config } => AnimationDriverState::Spring(
                SpringDriver::new(initial_position, inherited_velocity, *target, *config)?,
            ),
            AnimationDriverSpec::Timing { target, config } => {
                AnimationDriverState::Timing(TimingDriver::new(initial_position, *target, *config)?)
            }
            AnimationDriverSpec::Decay { velocity, config } => {
                AnimationDriverState::Decay(DecayDriver::new(
                    initial_position,
                    velocity.unwrap_or(inherited_velocity),
                    *config,
                )?)
            }
            AnimationDriverSpec::Sequence(specs) => AnimationDriverState::Sequence(
                SequenceDriver::new(specs.clone(), initial_position, inherited_velocity)?,
            ),
            AnimationDriverSpec::Repeat {
                inner,
                count,
                reverse,
            } => AnimationDriverState::Repeat(RepeatDriver::new(
                (**inner).clone(),
                *count,
                *reverse,
                initial_position,
                inherited_velocity,
            )?),
        };
        Ok(Self {
            state,
            sample: MotionSample {
                position: initial_position,
                velocity: inherited_velocity,
                settled: false,
            },
            terminal_delivered: false,
        })
    }

    /// Advance by `elapsed_seconds`, reporting settlement exactly once.
    pub fn advance(&mut self, elapsed_seconds: f64) -> Result<DriverAdvance, MotionDriverError> {
        require_elapsed(elapsed_seconds)?;
        if self.sample.settled {
            return Ok(DriverAdvance {
                sample: self.sample,
                terminal: None,
            });
        }
        let sample = match &mut self.state {
            AnimationDriverState::Spring(driver) => driver.advance(elapsed_seconds)?,
            AnimationDriverState::Timing(driver) => driver.advance(elapsed_seconds)?,
            AnimationDriverState::Decay(driver) => driver.advance(elapsed_seconds)?,
            AnimationDriverState::Sequence(driver) => driver.advance(elapsed_seconds)?,
            AnimationDriverState::Repeat(driver) => driver.advance(elapsed_seconds)?,
        }
        .validate_finite("driver.sample")?;
        self.sample = sample;
        let terminal = if self.sample.settled && !self.terminal_delivered {
            self.terminal_delivered = true;
            Some(DriverTerminalReason::Settled)
        } else {
            None
        };
        Ok(DriverAdvance {
            sample: self.sample,
            terminal,
        })
    }

    /// The most recently computed sample.
    pub fn sample(&self) -> MotionSample {
        self.sample
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CompositionClock {
    accumulator_seconds: f64,
    reanchored_gap_count: u64,
}

impl CompositionClock {
    fn new() -> Self {
        Self {
            accumulator_seconds: 0.0,
            reanchored_gap_count: 0,
        }
    }

    fn steps(&mut self, elapsed_seconds: f64) -> Result<usize, MotionDriverError> {
        self.accumulator_seconds = add_elapsed(self.accumulator_seconds, elapsed_seconds)?;
        let available =
            (self.accumulator_seconds / MOTION_COMPOSITION_QUANTUM_SECONDS).floor() as usize;
        let steps = available.min(MOTION_MAX_COMPOSITION_CATCH_UP_STEPS);
        self.accumulator_seconds -= steps as f64 * MOTION_COMPOSITION_QUANTUM_SECONDS;
        if available > MOTION_MAX_COMPOSITION_CATCH_UP_STEPS {
            self.reanchored_gap_count += 1;
            self.accumulator_seconds %= MOTION_COMPOSITION_QUANTUM_SECONDS;
        }
        require_finite(
            self.accumulator_seconds,
            "driver.composition_accumulated_elapsed_seconds",
        )?;
        Ok(steps)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SequenceDriver {
    specs: Vec<AnimationDriverSpec>,
    index: usize,
    current: Box<AnimationDriver>,
    clock: CompositionClock,
    sample: MotionSample,
}

impl SequenceDriver {
    fn new(
        specs: Vec<AnimationDriverSpec>,
        initial_position: f64,
        inherited_velocity: f64,
    ) -> Result<Self, MotionDriverError> {
        let Some(first) = specs.first() else {
            return Err(MotionDriverError::EmptySequence);
        };
        let current = Box::new(first.instantiate(initial_position, inherited_velocity)?);
        Ok(Self {
            specs,
            index: 0,
            current,
            clock: CompositionClock::new(),
            sample: MotionSample {
                position: initial_position,
                velocity: inherited_velocity,
                settled: false,
            },
        })
    }

    fn advance(&mut self, elapsed_seconds: f64) -> Result<MotionSample, MotionDriverError> {
        for _ in 0..self.clock.steps(elapsed_seconds)? {
            let advance = self.current.advance(MOTION_COMPOSITION_QUANTUM_SECONDS)?;
            self.sample = advance.sample;
            if self.sample.settled {
                self.index += 1;
                let Some(next) = self.specs.get(self.index) else {
                    return Ok(self.sample);
                };
                *self.current = next.instantiate(self.sample.position, self.sample.velocity)?;
                self.sample.settled = false;
            }
        }
        Ok(self.sample)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RepeatDriver {
    original_spec: AnimationDriverSpec,
    current: Box<AnimationDriver>,
    count: i32,
    completed: i32,
    reverse: bool,
    forward: bool,
    initial_position: f64,
    clock: CompositionClock,
    sample: MotionSample,
}

impl RepeatDriver {
    fn new(
        spec: AnimationDriverSpec,
        count: i32,
        reverse: bool,
        initial_position: f64,
        inherited_velocity: f64,
    ) -> Result<Self, MotionDriverError> {
        if count == 0 || count < -1 {
            return Err(MotionDriverError::InvalidRepeatCount);
        }
        if reverse {
            spec.reversed_for(initial_position)?;
        }
        let current = Box::new(spec.instantiate(initial_position, inherited_velocity)?);
        Ok(Self {
            original_spec: spec,
            current,
            count,
            completed: 0,
            reverse,
            forward: true,
            initial_position,
            clock: CompositionClock::new(),
            sample: MotionSample {
                position: initial_position,
                velocity: inherited_velocity,
                settled: false,
            },
        })
    }

    fn advance(&mut self, elapsed_seconds: f64) -> Result<MotionSample, MotionDriverError> {
        for _ in 0..self.clock.steps(elapsed_seconds)? {
            let advance = self.current.advance(MOTION_COMPOSITION_QUANTUM_SECONDS)?;
            self.sample = advance.sample;
            if !self.sample.settled {
                continue;
            }
            self.completed += 1;
            if self.count != -1 && self.completed >= self.count {
                return Ok(self.sample);
            }
            let next_spec = if self.reverse && self.forward {
                self.original_spec.reversed_for(self.initial_position)?
            } else {
                self.original_spec.clone()
            };
            self.forward = !self.forward;
            *self.current = next_spec.instantiate(self.sample.position, -self.sample.velocity)?;
            self.sample.settled = false;
        }
        Ok(self.sample)
    }
}

fn require_elapsed(elapsed_seconds: f64) -> Result<(), MotionDriverError> {
    require_finite(elapsed_seconds, "driver.elapsed_seconds")?;
    if elapsed_seconds < 0.0 {
        Err(MotionDriverError::NegativeElapsed)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// One value's sample for one frame, with its terminal if it ended.
pub struct DriverPublication {
    /// The value this publication is for.
    pub value_handle: u64,
    /// Identity of the driver that produced it.
    pub driver_sequence: u64,
    /// The sample to write to the value plane.
    pub sample: MotionSample,
    /// Set on the frame the driver stops owning the value.
    pub terminal: Option<DriverTerminal>,
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveDriver {
    sequence: u64,
    driver: AnimationDriver,
    settled: bool,
}

/// Main-owned driver table. The map key makes driver→value exclusivity
/// structural: replacement, app writes, and reset each emit one terminal.
#[derive(Debug, Default)]
pub struct MotionDriverTable {
    next_sequence: u64,
    active: BTreeMap<u64, ActiveDriver>,
}

impl MotionDriverTable {
    /// Reserve a driver identity even when reduced-motion policy resolves a
    /// start directly to its terminal value. Sharing this counter with live
    /// drivers keeps `(valueHandle, driverSeq)` globally unique per table.
    pub fn reserve_sequence(&mut self) -> Result<u64, MotionDriverError> {
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(MotionDriverError::DriverSequenceExhausted)?;
        Ok(self.next_sequence)
    }

    /// Start `driver` on `value_handle`, returning the terminal of whatever it
    /// replaced.
    ///
    /// Driver-to-value exclusivity is structural: the map key is the value, so a
    /// value can never have two drivers writing it.
    pub fn start(
        &mut self,
        value_handle: u64,
        driver: AnimationDriver,
    ) -> Result<Option<DriverTerminal>, MotionDriverError> {
        let sequence = self.reserve_sequence()?;
        let replaced = self.active.insert(
            value_handle,
            ActiveDriver {
                sequence,
                driver,
                settled: false,
            },
        );
        Ok(replaced.map(|previous| DriverTerminal {
            driver_sequence: previous.sequence,
            value_handle,
            reason: DriverTerminalReason::Replaced,
        }))
    }

    /// End the driver on `value_handle` because the application wrote the value.
    pub fn cancel_for_imperative_write(&mut self, value_handle: u64) -> Option<DriverTerminal> {
        self.cancel(value_handle, DriverTerminalReason::ImperativeWrite)
    }

    /// End the driver on `value_handle` because a gesture took the value.
    pub fn cancel_for_gesture_write(&mut self, value_handle: u64) -> Option<DriverTerminal> {
        self.cancel(value_handle, DriverTerminalReason::GestureWrite)
    }

    /// End the driver on `value_handle` for a named reason.
    pub fn cancel(
        &mut self,
        value_handle: u64,
        reason: DriverTerminalReason,
    ) -> Option<DriverTerminal> {
        self.active
            .remove(&value_handle)
            .map(|active| DriverTerminal {
                driver_sequence: active.sequence,
                value_handle,
                reason,
            })
    }

    /// End every driver in the table and return their terminals.
    pub fn reset(&mut self) -> Vec<DriverTerminal> {
        let terminals = self
            .active
            .iter()
            .map(|(value_handle, active)| DriverTerminal {
                driver_sequence: active.sequence,
                value_handle: *value_handle,
                reason: DriverTerminalReason::Reset,
            })
            .collect();
        self.active.clear();
        terminals
    }

    /// Advance every active driver by `elapsed_seconds` and collect publications.
    pub fn advance(
        &mut self,
        elapsed_seconds: f64,
    ) -> Result<Vec<DriverPublication>, MotionDriverError> {
        let mut publications = Vec::with_capacity(self.active.len());
        self.advance_with(elapsed_seconds, |publication| {
            publications.push(publication);
        })?;
        Ok(publications)
    }

    /// Allocation-free advance used by the native FFI. The caller owns a
    /// root-retained publication buffer; terminal membership is marked on the
    /// active entry and removed with `retain`, avoiding a per-frame key Vec.
    pub fn advance_with(
        &mut self,
        elapsed_seconds: f64,
        mut publish: impl FnMut(DriverPublication),
    ) -> Result<usize, MotionDriverError> {
        require_elapsed(elapsed_seconds)?;
        let count = self.active.len();
        for (value_handle, active) in &mut self.active {
            let (sample, terminal_reason) = match active.driver.advance(elapsed_seconds) {
                Ok(advance) => (advance.sample, advance.terminal),
                Err(_) => {
                    // Retire an unexpected numerical fault with the last
                    // finite sample. The host receives an explicit terminal
                    // instead of either NaN or a driver that fails forever.
                    let mut sample = active.driver.sample();
                    sample.settled = true;
                    (sample, Some(DriverTerminalReason::NumericalFailure))
                }
            };
            let terminal = terminal_reason.map(|reason| DriverTerminal {
                driver_sequence: active.sequence,
                value_handle: *value_handle,
                reason,
            });
            active.settled = terminal.is_some();
            publish(DriverPublication {
                value_handle: *value_handle,
                driver_sequence: active.sequence,
                sample,
                terminal,
            });
        }
        self.active.retain(|_, active| !active.settled);
        Ok(count)
    }

    /// Drivers still running.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
