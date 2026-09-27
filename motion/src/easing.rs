//! CSS easing functions.
//!
//! @ref LLP 1002 §2; CSS Easing Functions Level 1 (`<easing-function>`)
//!
//! Every curve here is CSS's, with CSS's output: `linear`, the four keyword
//! cubic Béziers, `cubic-bezier()`, `steps()`, and `linear()`. The browser is
//! the oracle: `tests/easing.rs` pins outputs to the values a browser computes
//! for the same inputs. Nothing here is a Reanimated or UIKit curve.

/// Where `steps()` places its jumps (CSS `<step-position>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepPosition {
    /// `jump-start` / `start`: the first jump happens at input 0.
    JumpStart,
    /// `jump-end` / `end` (the default): the last jump happens at input 1.
    #[default]
    JumpEnd,
    /// `jump-none`: no jump at either end; `count - 1` jumps.
    JumpNone,
    /// `jump-both`: a jump at both ends; `count + 1` jumps.
    JumpBoth,
}

impl StepPosition {
    /// Every position, in wire order.
    pub const ALL: [StepPosition; 4] = [
        StepPosition::JumpStart,
        StepPosition::JumpEnd,
        StepPosition::JumpNone,
        StepPosition::JumpBoth,
    ];

    /// From the wire discriminant.
    pub fn from_wire(value: u8) -> Option<StepPosition> {
        StepPosition::ALL.get(value as usize).copied()
    }

    /// The CSS keyword.
    pub fn name(self) -> &'static str {
        match self {
            StepPosition::JumpStart => "jump-start",
            StepPosition::JumpEnd => "jump-end",
            StepPosition::JumpNone => "jump-none",
            StepPosition::JumpBoth => "jump-both",
        }
    }
}

/// One stop of a `linear()` easing: input progress → output progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearStop {
    /// Input progress, `[0, 1]`, nondecreasing across the list.
    pub input: f64,
    /// Output progress at that input.
    pub output: f64,
}

/// Most stops a `linear()` easing may carry on the wire.
pub const MAX_LINEAR_STOPS: usize = 64;

/// A CSS `<easing-function>`.
#[derive(Debug, Clone, PartialEq)]
pub enum Easing {
    /// `linear`.
    Linear,
    /// `ease` = `cubic-bezier(0.25, 0.1, 0.25, 1)`.
    Ease,
    /// `ease-in` = `cubic-bezier(0.42, 0, 1, 1)`.
    EaseIn,
    /// `ease-out` = `cubic-bezier(0, 0, 0.58, 1)`.
    EaseOut,
    /// `ease-in-out` = `cubic-bezier(0.42, 0, 0.58, 1)`.
    EaseInOut,
    /// `cubic-bezier(x1, y1, x2, y2)`; `x1`, `x2` in `[0, 1]`.
    CubicBezier {
        /// First control point, input axis.
        x1: f64,
        /// First control point, output axis.
        y1: f64,
        /// Second control point, input axis.
        x2: f64,
        /// Second control point, output axis.
        y2: f64,
    },
    /// `steps(count, position)`.
    Steps {
        /// Number of intervals; at least one (two for `jump-none`).
        count: u16,
        /// Where the jumps fall.
        position: StepPosition,
    },
    /// `linear(<stop>, …)`: piecewise-linear through the stops.
    PiecewiseLinear(Vec<LinearStop>),
}

/// Why an easing was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingError {
    /// A control point or stop was infinite or NaN.
    NonFinite,
    /// A `cubic-bezier` `x` control point lay outside `[0, 1]`.
    ControlPointOutOfRange,
    /// `steps(0, …)`.
    ZeroSteps,
    /// `steps(1, jump-none)` has no interval to jump across.
    JumpNoneNeedsTwoSteps,
    /// `linear()` needs at least two stops.
    TooFewStops,
    /// `linear()` stops must have nondecreasing inputs.
    StopsNotSorted,
    /// A `linear()` stop input lay outside `[0, 1]`.
    StopOutOfRange,
    /// More `linear()` stops than the wire carries.
    TooManyStops,
}

impl Easing {
    /// Check every rule the evaluator relies on. Validated once, at the
    /// boundary; after that `progress` may assume the shape.
    pub fn validate(&self) -> Result<(), EasingError> {
        match self {
            Easing::Linear
            | Easing::Ease
            | Easing::EaseIn
            | Easing::EaseOut
            | Easing::EaseInOut => Ok(()),
            Easing::CubicBezier { x1, y1, x2, y2 } => {
                if ![*x1, *y1, *x2, *y2].iter().all(|v| v.is_finite()) {
                    return Err(EasingError::NonFinite);
                }
                if !(0.0..=1.0).contains(x1) || !(0.0..=1.0).contains(x2) {
                    return Err(EasingError::ControlPointOutOfRange);
                }
                Ok(())
            }
            Easing::Steps { count, position } => {
                if *count == 0 {
                    return Err(EasingError::ZeroSteps);
                }
                if *count == 1 && *position == StepPosition::JumpNone {
                    return Err(EasingError::JumpNoneNeedsTwoSteps);
                }
                Ok(())
            }
            Easing::PiecewiseLinear(stops) => {
                if stops.len() < 2 {
                    return Err(EasingError::TooFewStops);
                }
                if stops.len() > MAX_LINEAR_STOPS {
                    return Err(EasingError::TooManyStops);
                }
                let mut last = 0.0;
                for stop in stops {
                    if !stop.input.is_finite() || !stop.output.is_finite() {
                        return Err(EasingError::NonFinite);
                    }
                    if !(0.0..=1.0).contains(&stop.input) {
                        return Err(EasingError::StopOutOfRange);
                    }
                    if stop.input < last {
                        return Err(EasingError::StopsNotSorted);
                    }
                    last = stop.input;
                }
                Ok(())
            }
        }
    }

    /// Whether every number in the easing is finite.
    pub fn is_finite(&self) -> bool {
        !matches!(self.validate(), Err(EasingError::NonFinite))
    }

    /// Output progress at `input` (clamped to `[0, 1]`).
    pub fn progress(&self, input: f64) -> f64 {
        let x = input.clamp(0.0, 1.0);
        match self {
            Easing::Linear => x,
            Easing::Ease => bezier(0.25, 0.1, 0.25, 1.0, x),
            Easing::EaseIn => bezier(0.42, 0.0, 1.0, 1.0, x),
            Easing::EaseOut => bezier(0.0, 0.0, 0.58, 1.0, x),
            Easing::EaseInOut => bezier(0.42, 0.0, 0.58, 1.0, x),
            Easing::CubicBezier { x1, y1, x2, y2 } => bezier(*x1, *y1, *x2, *y2, x),
            Easing::Steps { count, position } => steps(*count, *position, x),
            Easing::PiecewiseLinear(stops) => piecewise(stops, x),
        }
    }
}

/// A cubic Bézier from (0,0) through the two control points to (1,1),
/// evaluated as a function of the input axis (CSS Easing §2.3.1).
fn bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let t = solve_bezier_t(x1, x2, x);
    bezier_axis(y1, y2, t)
}

fn bezier_axis(c1: f64, c2: f64, t: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * c1 + 3.0 * inverse * t * t * c2 + t * t * t
}

fn bezier_axis_derivative(c1: f64, c2: f64, t: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * c1 + 6.0 * inverse * t * (c2 - c1) + 3.0 * t * t * (1.0 - c2)
}

/// Newton's method with a bisection fallback, the way browsers solve it. With
/// `x1`, `x2` in `[0, 1]` the input axis is monotonic, so the root is unique.
fn solve_bezier_t(x1: f64, x2: f64, x: f64) -> f64 {
    const EPSILON: f64 = 1e-7;
    let mut t = x;
    for _ in 0..8 {
        let error = bezier_axis(x1, x2, t) - x;
        if error.abs() < EPSILON {
            return t;
        }
        let slope = bezier_axis_derivative(x1, x2, t);
        if slope.abs() < 1e-6 {
            break;
        }
        t -= error / slope;
    }
    let (mut low, mut high) = (0.0, 1.0);
    t = x;
    for _ in 0..64 {
        if high - low <= EPSILON {
            break;
        }
        if bezier_axis(x1, x2, t) < x {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) * 0.5;
    }
    t
}

/// CSS Easing §2.3.2, the step-easing output procedure (before flag unset).
fn steps(count: u16, position: StepPosition, x: f64) -> f64 {
    let count = count as f64;
    let mut current = (x * count).floor();
    if matches!(position, StepPosition::JumpStart | StepPosition::JumpBoth) {
        current += 1.0;
    }
    if current < 0.0 {
        current = 0.0;
    }
    let jumps = match position {
        StepPosition::JumpStart | StepPosition::JumpEnd => count,
        StepPosition::JumpNone => count - 1.0,
        StepPosition::JumpBoth => count + 1.0,
    };
    if current > jumps {
        current = jumps;
    }
    current / jumps
}

/// CSS Easing Level 2 `linear()`: interpolate between the two stops that
/// bracket `x`; before the first or after the last stop, hold that stop.
fn piecewise(stops: &[LinearStop], x: f64) -> f64 {
    let first = stops[0];
    let last = stops[stops.len() - 1];
    if x <= first.input {
        return first.output;
    }
    if x >= last.input {
        return last.output;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if x >= a.input && x <= b.input {
            if b.input == a.input {
                return b.output;
            }
            let p = (x - a.input) / (b.input - a.input);
            return a.output + (b.output - a.output) * p;
        }
    }
    last.output
}
