//! Host-independent f32 transcendentals. Game code uses these, never f32::sin.

/// Sine in radians.
pub fn sin(x: f32) -> f32 {
    libm::sinf(x)
}
/// Cosine in radians.
pub fn cos(x: f32) -> f32 {
    libm::cosf(x)
}
/// Tangent in radians.
pub fn tan(x: f32) -> f32 {
    libm::tanf(x)
}
/// Inverse sine in radians.
pub fn asin(x: f32) -> f32 {
    libm::asinf(x)
}
/// Inverse cosine in radians.
pub fn acos(x: f32) -> f32 {
    libm::acosf(x)
}
/// Inverse tangent in radians.
pub fn atan(x: f32) -> f32 {
    libm::atanf(x)
}
/// Quadrant-aware inverse tangent in radians.
pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}
/// Natural exponential.
pub fn exp(x: f32) -> f32 {
    libm::expf(x)
}
/// Natural logarithm.
pub fn ln(x: f32) -> f32 {
    libm::logf(x)
}
/// Raise x to a floating-point power.
pub fn powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}
/// Nonnegative square root.
pub fn sqrt(x: f32) -> f32 {
    libm::sqrtf(x)
}
/// Round down.
pub fn floor(x: f32) -> f32 {
    libm::floorf(x)
}
/// Round up.
pub fn ceil(x: f32) -> f32 {
    libm::ceilf(x)
}
/// Round to the nearest integer, halfway away from zero.
pub fn round(x: f32) -> f32 {
    libm::roundf(x)
}
/// Linear interpolation, with t allowed outside [0, 1].
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
/// Cubic interpolation between distinct increasing edges, clamped to [0, 1].
pub fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// Wrap radians to [-pi, pi).
pub fn wrap_angle(x: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let tau = std::f32::consts::TAU;
    let y = libm::fmodf(x, tau);
    if y < -pi {
        y + tau
    } else if y >= pi {
        y - tau
    } else {
        y
    }
}

fn approach_value(current: f32, target: f32, fraction: f32) -> f32 {
    let delta = target - current;
    if delta.is_finite() {
        current + delta * fraction
    } else {
        (current as f64 + (target as f64 - current as f64) * fraction as f64) as f32
    }
}
fn approach(current: f32, target: f32, fraction: f32) -> f32 {
    let next = approach_value(current, target, fraction);
    if (target - next).abs() <= 1e-4 {
        target
    } else {
        next
    }
}
/// Exponential approach with time constant `lag` seconds. Zero lag arrives now;
/// otherwise snaps when the remaining distance is at most 1e-4. Uses portable libm.
pub fn ease(current: f32, target: f32, lag: f32, dt: f32) -> f32 {
    assert!(lag.is_finite() && lag >= 0.0 && dt.is_finite() && dt >= 0.0);
    if lag == 0.0 {
        target
    } else {
        approach(current, target, -libm::expm1f(-dt / lag))
    }
}
