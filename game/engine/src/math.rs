//! Host-independent f32 transcendentals. Game code uses these, never f32::sin.
//! glam as configured (scalar-math + libm, fixed operation order and correctly
//! rounded sqrt) is part of the deterministic set; std's float methods are not.

// std's dynamic clamp pulls its general float formatter into size-optimized builds.
// Keep the same comparisons and rejection, using our existing diagnostic writer.
pub(crate) fn clamp<T: PartialOrd + Copy + Into<f64> + ryu::Float>(
    mut value: T,
    min: T,
    max: T,
) -> T {
    assert!(
        min <= max,
        "clamp bounds must be ordered: {}..={}",
        crate::data::text::Float(min),
        crate::data::text::Float(max)
    );
    if value < min {
        value = min;
    }
    if value > max {
        value = max;
    }
    value
}

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

/// Values supported by the arriving exponential approach.
pub trait Ease: Copy {
    /// Approach using a computed fraction and snap within 0.1 mm (or 1e-4 scalar units).
    fn approach(self, target: Self, fraction: f32) -> Self;
}
fn approach_value(current: f32, target: f32, fraction: f32) -> f32 {
    let delta = target - current;
    if delta.is_finite() {
        current + delta * fraction
    } else {
        (current as f64 + (target as f64 - current as f64) * fraction as f64) as f32
    }
}
impl Ease for f32 {
    fn approach(self, target: Self, fraction: f32) -> Self {
        let next = approach_value(self, target, fraction);
        if (target - next).abs() <= 1e-4 {
            target
        } else {
            next
        }
    }
}
impl Ease for crate::Vec3 {
    fn approach(self, target: Self, fraction: f32) -> Self {
        let next = crate::Vec3::new(
            approach_value(self.x, target.x, fraction),
            approach_value(self.y, target.y, fraction),
            approach_value(self.z, target.z, fraction),
        );
        if (target - next).length_squared() <= 1e-8 {
            target
        } else {
            next
        }
    }
}
/// Exponential approach with time constant `lag` seconds. Zero lag arrives now;
/// otherwise snaps when the remaining distance is at most 1e-4. Uses portable libm.
pub fn ease<T: Ease>(current: T, target: T, lag: f32, dt: f32) -> T {
    assert!(lag.is_finite() && lag >= 0.0 && dt.is_finite() && dt >= 0.0);
    if lag == 0.0 {
        target
    } else {
        current.approach(target, -libm::expm1f(-dt / lag))
    }
}

#[cfg(test)]
mod tests {
    use super::clamp;

    #[test]
    fn compact_clamp_matches_std_bits_and_rejects_the_same_bounds() {
        macro_rules! compare {
            ($ty:ty) => {{
                let edge = [
                    <$ty>::NEG_INFINITY,
                    -<$ty>::MAX,
                    -1.,
                    -0.,
                    0.,
                    1.,
                    <$ty>::from_bits(1),
                    <$ty>::MAX,
                    <$ty>::INFINITY,
                    <$ty>::NAN,
                ];
                for min in edge {
                    for max in edge {
                        if min <= max {
                            for value in edge {
                                assert_eq!(
                                    clamp(value, min, max).to_bits(),
                                    value.clamp(min, max).to_bits()
                                );
                            }
                        } else {
                            assert!(
                                std::panic::catch_unwind(|| clamp(0. as $ty, min, max)).is_err()
                            );
                            assert!(
                                std::panic::catch_unwind(|| (0. as $ty).clamp(min, max)).is_err()
                            );
                        }
                    }
                }
                let mut bits = 1u64;
                let mut next = || {
                    bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1);
                    <$ty>::from_bits(bits as _)
                };
                for _ in 0..100_000 {
                    let value = next();
                    let (a, b) = (next(), next());
                    if a.is_nan() || b.is_nan() {
                        continue;
                    }
                    let (min, max) = if a <= b { (a, b) } else { (b, a) };
                    assert_eq!(
                        clamp(value, min, max).to_bits(),
                        value.clamp(min, max).to_bits()
                    );
                }
            }};
        }
        compare!(f32);
        compare!(f64);
    }
}
