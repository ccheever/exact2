//! Host-independent f32 transcendentals. Game code uses these, never f32::sin.

// Direct exports keep the portable scalar operations without another wrapper layer.
pub use libm::{
    acosf as acos, asinf as asin, atan2f as atan2, atanf as atan, ceilf as ceil, cosf as cos,
    expf as exp, floorf as floor, logf as ln, powf, roundf as round, sinf as sin, sqrtf as sqrt,
    tanf as tan,
};
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
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

fn approach(current: f32, target: f32, fraction: f32) -> f32 {
    let delta = target - current;
    let next = if delta.is_finite() {
        current + delta * fraction
    } else {
        (current as f64 + (target as f64 - current as f64) * fraction as f64) as f32
    };
    if (target - next).abs() <= 1e-4 {
        target
    } else {
        next
    }
}
pub fn ease(current: f32, target: f32, lag: f32, dt: f32) -> f32 {
    assert!(lag.is_finite() && lag >= 0.0 && dt.is_finite() && dt >= 0.0);
    if lag == 0.0 {
        target
    } else {
        approach(current, target, -libm::expm1f(-dt / lag))
    }
}
