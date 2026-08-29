//! `transition`: the declaration, and CSS's rules for running one.
//!
//! @ref LLP 1002 §2; CSS Transitions Level 1 §3 ("Starting of transitions")
//!
//! A [`Transitions`] list is the value of a node's `transition` style row in
//! the kernel — the same row a web host emits as CSS. The rest of this module
//! is what a browser does with that row: start a transition when the target
//! changes, interrupt it from the current value when the target changes
//! again, and shorten it when the change reverses (§3.2's reversing-adjusted
//! start value and reversing shortening factor), so a hover-off halfway
//! through a hover-on takes half the time on every platform, not just the web.

use crate::easing::{Easing, EasingError};
use crate::property::{Property, Value};
use crate::spring::{SpringConfig, SpringError, MAX_DURATION};

/// Most transitions one node may declare.
pub const MAX_TRANSITIONS: usize = 8;

/// What a transition applies to (`transition-property`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionProperty {
    /// `all`.
    All,
    /// One named property.
    Property(Property),
}

impl TransitionProperty {
    /// Whether this declaration covers `property`.
    pub fn covers(self, property: Property) -> bool {
        match self {
            TransitionProperty::All => true,
            TransitionProperty::Property(p) => p == property,
        }
    }
}

/// How a transition progresses (`transition-timing-function`).
#[derive(Debug, Clone, PartialEq)]
pub enum TimingFunction {
    /// A CSS easing over the authored duration.
    Easing(Easing),
    /// A spring; the duration is derived from the spring, not authored.
    Spring(SpringConfig),
}

/// One `transition` declaration: property, duration, delay, timing function.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// What it applies to.
    pub property: TransitionProperty,
    /// `transition-duration`, seconds. Must be `0` for a spring.
    pub duration: f64,
    /// `transition-delay`, seconds. Negative starts partway through (easing
    /// only).
    pub delay: f64,
    /// The curve.
    pub timing: TimingFunction,
}

/// Why a transition declaration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    /// More declarations than [`MAX_TRANSITIONS`].
    TooMany,
    /// A duration or delay was infinite or NaN.
    NonFinite,
    /// `transition-duration` was negative.
    NegativeDuration,
    /// A spring transition authored a duration; a spring's duration is derived.
    SpringDeclaresDuration,
    /// A spring transition authored a negative delay.
    SpringNegativeDelay,
    /// The easing was invalid.
    Easing(EasingError),
    /// The spring parameters were invalid.
    Spring(SpringError),
}

impl Transition {
    /// A transition with no delay.
    pub fn new(property: TransitionProperty, duration: f64, timing: TimingFunction) -> Transition {
        Transition {
            property,
            duration,
            delay: 0.0,
            timing,
        }
    }

    /// Check every rule the evaluator relies on.
    pub fn validate(&self) -> Result<(), TransitionError> {
        if !self.duration.is_finite() || !self.delay.is_finite() {
            return Err(TransitionError::NonFinite);
        }
        if self.duration < 0.0 {
            return Err(TransitionError::NegativeDuration);
        }
        match &self.timing {
            TimingFunction::Easing(easing) => easing.validate().map_err(TransitionError::Easing),
            TimingFunction::Spring(config) => {
                if self.duration != 0.0 {
                    return Err(TransitionError::SpringDeclaresDuration);
                }
                if self.delay < 0.0 {
                    return Err(TransitionError::SpringNegativeDelay);
                }
                config.validate().map_err(TransitionError::Spring)
            }
        }
    }

    /// Whether every number in the declaration is finite.
    pub fn is_finite(&self) -> bool {
        self.duration.is_finite()
            && self.delay.is_finite()
            && match &self.timing {
                TimingFunction::Easing(e) => e.is_finite(),
                TimingFunction::Spring(s) => s.is_finite(),
            }
    }

    /// Whether a change under this declaration starts a transition at all.
    /// CSS: the combined duration (`max(duration, 0) + delay`) must be
    /// positive. A spring's combined duration is its settle time, never zero.
    pub fn starts(&self) -> bool {
        match &self.timing {
            TimingFunction::Easing(_) => self.duration.max(0.0) + self.delay > 0.0,
            TimingFunction::Spring(_) => true,
        }
    }
}

/// A node's `transition` row: zero to [`MAX_TRANSITIONS`] declarations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transitions(pub Vec<Transition>);

impl Transitions {
    /// No transitions: every change is immediate (CSS's initial value,
    /// `transition: all 0s ease 0s`, which starts nothing).
    pub const NONE: Transitions = Transitions(Vec::new());

    /// The declaration governing `property`, if any. When several cover it,
    /// the last wins (CSS Transitions §2.1: "the last one is used").
    pub fn matching(&self, property: Property) -> Option<&Transition> {
        self.0.iter().rev().find(|t| t.property.covers(property))
    }

    /// Validate every declaration and the count.
    pub fn validate(&self) -> Result<(), TransitionError> {
        if self.0.len() > MAX_TRANSITIONS {
            return Err(TransitionError::TooMany);
        }
        self.0.iter().try_for_each(Transition::validate)
    }

    /// Whether every number in every declaration is finite.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(Transition::is_finite)
    }
}

/// A transition in flight for one property of one node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Running {
    /// Value at the start of the transition.
    pub from: Value,
    /// Value at the end (the current target).
    pub to: Value,
    /// Clock time the transition starts moving (change time plus delay).
    pub start: f64,
    /// The curve and its effective duration.
    pub curve: Curve,
    /// CSS §3.2: the value a reversal is measured against.
    pub reversing_adjusted_start: Value,
    /// CSS §3.2: how much of the authored duration this transition uses.
    pub reversing_shortening: f64,
}

/// The shape of a running transition's progress.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Curve {
    /// Eased over `duration` seconds (after any shortening).
    Easing { easing: Easing, duration: f64 },
    /// A spring released at `from` with `velocity`.
    Spring {
        config: SpringConfig,
        velocity: Value,
    },
}

/// What a running transition looks like at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RunningSample {
    pub value: Value,
    pub velocity: Value,
    pub done: bool,
}

impl Running {
    /// Start a transition under `declaration` at `now`.
    pub fn start(
        declaration: &Transition,
        from: Value,
        to: Value,
        velocity: Value,
        now: f64,
        reversing_adjusted_start: Value,
        reversing_shortening: f64,
    ) -> Running {
        let (curve, delay) = match &declaration.timing {
            TimingFunction::Easing(easing) => (
                Curve::Easing {
                    easing: easing.clone(),
                    duration: declaration.duration * reversing_shortening,
                },
                // CSS §3.2: a negative delay is scaled by the shortening factor.
                if declaration.delay >= 0.0 {
                    declaration.delay
                } else {
                    declaration.delay * reversing_shortening
                },
            ),
            TimingFunction::Spring(config) => (
                Curve::Spring {
                    config: *config,
                    velocity,
                },
                declaration.delay,
            ),
        };
        Running {
            from,
            to,
            start: now + delay,
            curve,
            reversing_adjusted_start,
            reversing_shortening,
        }
    }

    /// The easing's output progress at `now` (CSS §3.2 "timing function
    /// output"). A spring has no such number; it reports `1`.
    pub fn easing_progress(&self, now: f64) -> f64 {
        match &self.curve {
            Curve::Easing { easing, duration } => {
                if now <= self.start {
                    easing.progress(0.0)
                } else if *duration <= 0.0 {
                    1.0
                } else {
                    easing.progress((now - self.start) / duration)
                }
            }
            Curve::Spring { .. } => 1.0,
        }
    }

    /// Sample at `now`. Before `start` (in the delay) the value is `from`.
    pub fn sample(&self, now: f64) -> RunningSample {
        match &self.curve {
            Curve::Easing { easing, duration } => {
                if now < self.start {
                    return RunningSample {
                        value: self.from,
                        velocity: Value::ZERO,
                        done: false,
                    };
                }
                if *duration <= 0.0 || now - self.start >= *duration {
                    return RunningSample {
                        value: self.to,
                        velocity: Value::ZERO,
                        done: true,
                    };
                }
                let input = (now - self.start) / duration;
                let progress = easing.progress(input);
                // Velocity by a central difference over one sample step; only
                // a spring started from this value reads it.
                let step = 1.0 / crate::spring::SAMPLE_RATE;
                let before = easing.progress(((now - self.start) - step).max(0.0) / duration);
                let after = easing.progress(((now - self.start) + step).min(*duration) / duration);
                let slope = (after - before) / (2.0 * step);
                let distance = self.to - self.from;
                RunningSample {
                    value: self.from.lerp(self.to, progress),
                    velocity: Value::new(distance.x * slope, distance.y * slope),
                    done: false,
                }
            }
            Curve::Spring { config, velocity } => {
                if now < self.start {
                    return RunningSample {
                        value: self.from,
                        velocity: *velocity,
                        done: false,
                    };
                }
                let elapsed = now - self.start;
                let displacement = self.from - self.to;
                let x = config.sample(displacement.x, velocity.x, elapsed);
                let y = config.sample(displacement.y, velocity.y, elapsed);
                let at_rest = (x.at_rest() && y.at_rest()) || elapsed >= MAX_DURATION;
                if at_rest {
                    return RunningSample {
                        value: self.to,
                        velocity: Value::ZERO,
                        done: true,
                    };
                }
                RunningSample {
                    value: self.to + Value::new(x.displacement, y.displacement),
                    velocity: Value::new(x.velocity, y.velocity),
                    done: false,
                }
            }
        }
    }

    /// A running spring, lowered: its values on the [`SAMPLE_RATE`] grid from
    /// release to rest, evenly spaced, the last exactly `to`. `None` for an
    /// easing (CSS plays those itself). Each value is the same bits
    /// [`Running::sample`] would return at that grid time, so a host that
    /// plays these with linear interpolation shows the native curve.
    ///
    /// [`SAMPLE_RATE`]: crate::spring::SAMPLE_RATE
    pub fn spring_frames(&self) -> Option<(f64, Vec<Value>)> {
        let Curve::Spring { config, velocity } = &self.curve else {
            return None;
        };
        let duration = self.end_time() - self.start;
        if duration <= 0.0 {
            return Some((0.0, vec![self.from, self.to]));
        }
        let displacement = self.from - self.to;
        let count = (duration * crate::spring::SAMPLE_RATE).round() as usize;
        let mut values = Vec::with_capacity(count + 1);
        for n in 0..count {
            let t = n as f64 / crate::spring::SAMPLE_RATE;
            let x = config.sample(displacement.x, velocity.x, t);
            let y = config.sample(displacement.y, velocity.y, t);
            values.push(self.to + Value::new(x.displacement, y.displacement));
        }
        values.push(self.to);
        Some((duration, values))
    }

    /// When the transition ends, on the clock. A spring's end is its settle
    /// time on the sample grid.
    pub fn end_time(&self) -> f64 {
        match &self.curve {
            Curve::Easing { duration, .. } => self.start + duration.max(0.0),
            Curve::Spring { config, velocity } => {
                let displacement = self.from - self.to;
                let x = config.settle_time(displacement.x, velocity.x);
                let y = config.settle_time(displacement.y, velocity.y);
                self.start + x.max(y)
            }
        }
    }
}
