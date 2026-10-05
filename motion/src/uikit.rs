//! UIKit's spring parameterisations, resolved to the springs UIKit builds.
//!
//! @ref LLP 1099 D1–D4 (UIKit's springs, everywhere)
//!
//! UIKit describes a spring as "0.4 s, damping ratio 0.8, velocity 1" and
//! hands Core Animation a physical spring (mass 1, a solved stiffness) that it
//! cuts off at the duration. None of that conversion is documented; this
//! module is the rule as measured on iOS 27.0
//! (`motion/tests/fixtures/uikit-springs/ios-27.0.txt`, recomputed by
//! `fit.mjs` there):
//!
//! - [`duration`]: `animate(withDuration:usingSpringWithDamping:
//!   initialSpringVelocity:)` and `UIViewPropertyAnimator(duration:
//!   dampingRatio:)`. A closed form at zero velocity; otherwise a
//!   reconstruction of UIKit's twelve Newton steps (D2).
//! - [`bounce`]: iOS 17's `animate(springDuration:bounce:)`, exact
//!   coefficients and the end time UIKit computes (D3).
//! - [`response`]: response and damping fraction, the SwiftUI form Signal's
//!   helper feeds `UISpringTimingParameters(mass:stiffness:damping:)`.
//! - [`sample`]: Core Animation's curve, including its overdamped branch,
//!   which pairs each coefficient with the other exponent (D4).
//!
//! Velocities here are UIKit's: moves per second, where 1 is the whole
//! distance in a second. Every transcendental goes through [`crate::math`].

use crate::math;
use crate::spring::{SpringConfig, SpringError, SpringSample};

/// UIKit's settle threshold, as a fraction of the move.
pub const EPSILON: f64 = 1e-3;

/// The shortest duration or response accepted, in seconds.
pub const MIN_DURATION: f64 = 0.001;

/// The latest a timed spring may end, in seconds after it starts (D3).
pub const MAX_END: f64 = 60.0;

/// The smallest damping ratio accepted: the probe measured nothing below.
pub const MIN_RATIO: f64 = 0.01;

/// Where Newton starts, in W = ω·duration (D2).
const START: f64 = 5.0;

/// How many Newton steps UIKit takes. The step cap, not convergence, is
/// what reproduces UIKit's unconverged answers.
const STEPS: u32 = 12;

/// The critical root of `(1 + W)·e^(−W) = ε`: W at zero velocity and ζ = 1.
const CRITICAL_W0: f64 = 9.233413476451585;

/// How Core Animation evaluates a resolved spring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    /// The textbook closed form, [`SpringConfig::sample`], at every ζ. Core
    /// Animation without `allowsOverdamping` clamps ζ to 1; the duration and
    /// response forms clamp ζ when they resolve, so their springs are
    /// already Core Animation's. A physical spring keeps true overdamped
    /// physics (LLP 1099 D4).
    Textbook,
    /// Core Animation with `allowsOverdamping`: above critical, each
    /// coefficient pairs with the other exponent, so the spring starts
    /// faster than its velocity and overshoots (D4).
    Overdamped,
}

/// A UIKit spelling, resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    /// The physical spring (mass 1).
    pub config: SpringConfig,
    /// The initial velocity in moves per second.
    pub velocity: f64,
    /// When the spring ends, in seconds after it starts: the value snaps to
    /// the target then. `None` ends at rest, by exact2's rule.
    pub end: Option<f64>,
    /// How the curve is evaluated.
    pub branch: Branch,
}

/// Why a UIKit spelling was refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UikitError {
    /// An argument was infinite or NaN.
    NonFinite,
    /// A duration or response outside [1 ms, 60 s].
    Duration,
    /// A damping ratio below [`MIN_RATIO`].
    Ratio,
    /// A bounce outside (−1, 1).
    Bounce,
    /// The duration-form procedure found no root (D2's validation).
    NoRoot,
    /// The spring would end more than [`MAX_END`] after it starts.
    EndTooLate,
    /// The resolved coefficients failed [`SpringConfig::validate`].
    Coefficients(SpringError),
}

/// The duration form's settle residual `g(W)`: `|B|·e^(−ζ'W) − ε` below
/// critical, `|−1 + (u/W − 1)·W|·e^(−W) − ε` at it (D2). `zeta` is already
/// clamped to at most 1.
pub fn residual(w: f64, zeta: f64, u: f64) -> f64 {
    let e = math::exp(-zeta * w);
    if zeta < 1.0 {
        let b = (u / w - zeta) / math::sqrt(1.0 - zeta * zeta);
        b.abs() * e - EPSILON
    } else {
        (u - w - 1.0).abs() * e - EPSILON
    }
}

/// W = ω·duration for a damping ratio (clamped to 1) and a normalised
/// velocity `u` = velocity × duration, by D2's procedure. The closed form at
/// zero velocity; otherwise twelve Newton steps from W = 5, halving a step
/// that lands at or below zero, stopping early only on a zero derivative.
/// The answer is not validated; see [`is_root`].
pub fn duration_w(zeta: f64, u: f64) -> f64 {
    let z = zeta.min(1.0);
    if u == 0.0 {
        return if z < 1.0 {
            math::ln(z / (EPSILON * math::sqrt(1.0 - z * z))) / z
        } else {
            CRITICAL_W0
        };
    }
    let mut w = START;
    for _ in 0..STEPS {
        let e = math::exp(-z * w);
        let (g, dg) = if z < 1.0 {
            let s = math::sqrt(1.0 - z * z);
            let b = (u / w - z) / s;
            let sign = if b < 0.0 { -1.0 } else { 1.0 };
            (
                b.abs() * e - EPSILON,
                (sign * (-u / (w * w)) / s - z * b.abs()) * e,
            )
        } else {
            let p = u - w - 1.0;
            let sign = if p < 0.0 { -1.0 } else { 1.0 };
            (p.abs() * e - EPSILON, (-sign - p.abs()) * e)
        };
        if dg == 0.0 {
            break;
        }
        let mut next = w - g / dg;
        if next <= 0.0 {
            next = w / 2.0;
        }
        w = next;
    }
    w
}

/// Whether `w` is an acceptable root: finite, in (0, 10⁴], with
/// `|g(w)| ≤ 0.1·ε` (D2). Where the procedure matches UIKit, |g| is at most
/// 0.059·ε.
pub fn is_root(w: f64, zeta: f64, u: f64) -> bool {
    w.is_finite() && w > 0.0 && w <= 1e4 && residual(w, zeta.min(1.0), u).abs() <= 0.1 * EPSILON
}

/// Whether (ζ, u) lies in the band where UIKit's own solver is chaotic and
/// the reconstruction may pick another root: u > 0 and
/// 0.6 ≤ u/min(ζ, 1) ≤ 8 (LLP 1099 §4.3). It holds every measured miss.
pub fn in_band(zeta: f64, u: f64) -> bool {
    let r = u / zeta.min(1.0);
    u > 0.0 && (0.6..=8.0).contains(&r)
}

fn check_duration(d: f64) -> Result<(), UikitError> {
    if !d.is_finite() {
        return Err(UikitError::NonFinite);
    }
    if !(MIN_DURATION..=MAX_END).contains(&d) {
        return Err(UikitError::Duration);
    }
    Ok(())
}

fn finish(
    config: SpringConfig,
    velocity: f64,
    end: Option<f64>,
    branch: Branch,
) -> Result<Resolved, UikitError> {
    config.validate().map_err(UikitError::Coefficients)?;
    if let Some(end) = end {
        if !end.is_finite() {
            return Err(UikitError::NonFinite);
        }
        if end > MAX_END {
            return Err(UikitError::EndTooLate);
        }
    }
    Ok(Resolved {
        config,
        velocity,
        end,
        branch,
    })
}

/// The duration form: duration `d` seconds, damping ratio `zeta`, initial
/// velocity `velocity` in moves per second. Mass 1, ζ' = min(ζ, 1),
/// k = (W/d)², c = 2ζ'·W/d, ending at `d` (D1–D3).
pub fn duration(d: f64, zeta: f64, velocity: f64) -> Result<Resolved, UikitError> {
    if !zeta.is_finite() || !velocity.is_finite() {
        return Err(UikitError::NonFinite);
    }
    check_duration(d)?;
    if zeta < MIN_RATIO {
        return Err(UikitError::Ratio);
    }
    let z = zeta.min(1.0);
    let u = velocity * d;
    let w = duration_w(z, u);
    if u != 0.0 && !is_root(w, z, u) {
        return Err(UikitError::NoRoot);
    }
    let omega = w / d;
    let config = SpringConfig {
        stiffness: omega * omega,
        damping: 2.0 * z * omega,
        mass: 1.0,
    };
    finish(config, velocity, Some(d), Branch::Textbook)
}

/// The bounce form: iOS 17's `animate(springDuration: d, bounce: b,
/// initialSpringVelocity: velocity)`. k = (2π/d)², c = 4π(1 − b)/d for
/// b ≥ 0 and 4π/(d(1 + b)) below; below b 0 Core Animation's overdamped
/// branch. It ends where UIKit ends it (D3).
pub fn bounce(d: f64, b: f64, velocity: f64) -> Result<Resolved, UikitError> {
    if !b.is_finite() || !velocity.is_finite() {
        return Err(UikitError::NonFinite);
    }
    check_duration(d)?;
    if !(b > -1.0 && b < 1.0) {
        return Err(UikitError::Bounce);
    }
    let tau = 2.0 * core::f64::consts::PI;
    let omega = tau / d;
    let damping = if b >= 0.0 {
        2.0 * tau * (1.0 - b) / d
    } else {
        2.0 * tau / (d * (1.0 + b))
    };
    let config = SpringConfig {
        stiffness: omega * omega,
        damping,
        mass: 1.0,
    };
    // Branch on the resolved damping, not on b: a bounce within an ulp of 0
    // rounds to critical.
    let end = if config.damping < 2.0 * omega {
        core_animation_settle(&config, velocity)
    } else {
        critical_settle(omega, velocity)
    };
    let branch = if b < 0.0 {
        Branch::Overdamped
    } else {
        Branch::Textbook
    };
    finish(config, velocity, Some(end), branch)
}

/// The response form: response `r` seconds and damping fraction `zeta`,
/// k = (2π/r)², c = 4π·min(ζ, 1)/r, ending at rest unless the spelling
/// gives an end time (D1, D3).
pub fn response(r: f64, zeta: f64, velocity: f64) -> Result<Resolved, UikitError> {
    if !zeta.is_finite() || !velocity.is_finite() {
        return Err(UikitError::NonFinite);
    }
    check_duration(r)?;
    if zeta < MIN_RATIO {
        return Err(UikitError::Ratio);
    }
    let tau = 2.0 * core::f64::consts::PI;
    let omega = tau / r;
    let config = SpringConfig {
        stiffness: omega * omega,
        damping: 2.0 * zeta.min(1.0) * omega,
        mass: 1.0,
    };
    finish(config, velocity, None, Branch::Textbook)
}

/// Core Animation's `settlingDuration` below critical damping:
/// `ln((1 + |B|)/ε)/(ζω)` with `B = (v − ζω)/ω_d`, `v` in moves per second
/// (D3). It matches every measured row to 2.2·10⁻¹⁶.
pub fn core_animation_settle(config: &SpringConfig, velocity: f64) -> f64 {
    let omega = math::sqrt(config.stiffness / config.mass);
    let zeta = config.damping / (2.0 * math::sqrt(config.stiffness * config.mass));
    let omega_d = omega * math::sqrt(1.0 - zeta * zeta);
    let b = (velocity - zeta * omega) / omega_d;
    math::ln((1.0 + b.abs()) / EPSILON) / (zeta * omega)
}

/// The critically damped settle time UIKit gives the bounce form at and
/// above critical: the last root of `F(T) = |−1 + (v − ω)T|·e^(−ωT) − ε`,
/// by D3's bracket (the chosen lobe's peak, doubling from max(peak, 1/ω)
/// while F > 0, then 200 bisection steps keeping F(lo) > 0).
pub fn critical_settle(omega: f64, velocity: f64) -> f64 {
    let f = |t: f64| (-1.0 + (velocity - omega) * t).abs() * math::exp(-omega * t) - EPSILON;
    let bracket = |lo: f64| {
        let mut hi = lo.max(1.0 / omega);
        while f(hi) > 0.0 {
            hi *= 2.0;
        }
        let mut lo = lo;
        for _ in 0..200 {
            let m = (lo + hi) / 2.0;
            if f(m) > 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        lo
    };
    if velocity > omega {
        let second = 1.0 / (velocity - omega) + 1.0 / omega;
        if f(second) > 0.0 {
            return bracket(second);
        }
        return if f(0.0) > 0.0 { bracket(0.0) } else { 0.0 };
    }
    let first = if velocity < omega {
        (1.0 / omega - 1.0 / (omega - velocity)).max(0.0)
    } else {
        0.0
    };
    if f(first) > 0.0 {
        bracket(first)
    } else {
        0.0
    }
}

/// The state `elapsed` seconds after release at `displacement` from the
/// target with `velocity` (both in property units), as Core Animation
/// evaluates the spring on `branch` (D4).
pub fn sample(
    config: &SpringConfig,
    branch: Branch,
    displacement: f64,
    velocity: f64,
    elapsed: f64,
) -> SpringSample {
    let alpha = config.damping / (2.0 * config.mass);
    let omega_squared = config.stiffness / config.mass;
    let discriminant = alpha * alpha - omega_squared;
    // Every strictly overdamped spring on the overdamped branch takes Core
    // Animation's pairing, however close to critical. Overdamped means
    // ζ = c/(2√(km)) > 1, as `fit.mjs`'s `caPos` decides it, not a
    // discriminant that rounds positive at ζ = 1.
    let zeta = config.damping / (2.0 * math::sqrt(config.stiffness * config.mass));
    match branch {
        Branch::Overdamped if zeta > 1.0 && discriminant > 0.0 => {
            let root = math::sqrt(discriminant);
            let r1 = -alpha + root;
            let r2 = -alpha - root;
            let on_r2 = -(velocity + r2 * displacement) / (r1 - r2);
            let on_r1 = displacement - on_r2;
            let e1 = math::exp(r1 * elapsed);
            let e2 = math::exp(r2 * elapsed);
            SpringSample {
                displacement: on_r1 * e1 + on_r2 * e2,
                velocity: on_r1 * r1 * e1 + on_r2 * r2 * e2,
            }
        }
        _ => config.sample(displacement, velocity, elapsed),
    }
}
