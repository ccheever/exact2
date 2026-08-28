//! The pinned deterministic-math profile.
//!
//! @ref RFC 0492 M-A (deterministic clock and agent evidence)
//!
//! Every transcendental the evaluator calls routes through this module, so the
//! same plan advanced by the same clock produces the same bits on every target
//! this crate compiles for. Profile v1 forbids FMA contraction: Rust does not
//! automatically contract separate multiply/add expressions, so the evaluator's
//! authored rounding boundaries survive.

/// The `libm` release whose software kernels define profile v1.
pub const LIBM_PINNED_VERSION: &str = "0.2.16";

/// One named function implementation in a deterministic-math profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeterministicMathKernel {
    /// Function name used by evaluator call sites.
    pub name: &'static str,
    /// Pinned implementation identity.
    pub source: &'static str,
}

/// The evaluator's math and comparison declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeterministicMathProfile {
    /// Stable profile identity, included in value-sequence evidence.
    pub profile_id: &'static str,
    /// Floating-point representation used by the evaluator.
    pub float_format: &'static str,
    /// Whether separate multiply/add expressions may be contracted.
    pub fma_contraction: &'static str,
    /// Equality rule for native value-sequence evidence.
    pub comparison_boundary: &'static str,
    /// Ordered function-to-implementation declarations.
    pub kernels: &'static [DeterministicMathKernel],
}

const LIBM_SOURCE: &str = "libm 0.2.16";
const SQRT_SOURCE: &str = "ieee754-correctly-rounded (core::f64::sqrt)";

const KERNELS: &[DeterministicMathKernel] = &[
    DeterministicMathKernel {
        name: "exp",
        source: LIBM_SOURCE,
    },
    DeterministicMathKernel {
        name: "ln",
        source: LIBM_SOURCE,
    },
    DeterministicMathKernel {
        name: "sin",
        source: LIBM_SOURCE,
    },
    DeterministicMathKernel {
        name: "cos",
        source: LIBM_SOURCE,
    },
    DeterministicMathKernel {
        name: "sqrt",
        source: SQRT_SOURCE,
    },
];

/// The deterministic kernels and raw-bit comparison boundary the evaluator uses.
pub const DETERMINISTIC_MATH_PROFILE: DeterministicMathProfile = DeterministicMathProfile {
    profile_id: "exact-motion-math-v1",
    float_format: "ieee754-binary64",
    fma_contraction: "forbidden",
    comparison_boundary: "f64-bits",
    kernels: KERNELS,
};

/// Deterministic software exponential pinned by profile v1.
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Deterministic software natural logarithm pinned by profile v1.
#[inline]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// Deterministic software sine pinned by profile v1.
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Deterministic software cosine pinned by profile v1.
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// IEEE 754 binary64 square root.
///
/// IEEE 754 requires a correctly rounded square root, so Rust's platform
/// intrinsic already has an architecture-independent result for finite
/// binary64 inputs and does not need a software transcendental kernel.
#[inline]
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repository_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(relative)
    }

    #[test]
    fn cargo_lock_matches_the_profile_libm_pin() {
        let lock_path = repository_path("Cargo.lock");
        let lock = fs::read_to_string(&lock_path).expect("read workspace Cargo.lock");
        let mut in_package = false;
        let mut package_name: Option<&str> = None;
        let mut package_version: Option<&str> = None;
        let mut libm_versions = Vec::new();

        for line in lock.lines().chain(std::iter::once("[[package]]")) {
            if line == "[[package]]" {
                if in_package && package_name == Some("libm") {
                    libm_versions.push(package_version.expect("libm package has a version"));
                }
                in_package = true;
                package_name = None;
                package_version = None;
            } else if in_package {
                if let Some(value) = line
                    .strip_prefix("name = \"")
                    .and_then(|value| value.strip_suffix('"'))
                {
                    package_name = Some(value);
                } else if let Some(value) = line
                    .strip_prefix("version = \"")
                    .and_then(|value| value.strip_suffix('"'))
                {
                    package_version = Some(value);
                }
            }
        }

        assert_eq!(
            libm_versions,
            [LIBM_PINNED_VERSION],
            "Cargo.lock libm version moved; update the named profile"
        );
        assert_eq!(
            LIBM_SOURCE.strip_prefix("libm "),
            Some(LIBM_PINNED_VERSION),
            "the profile's libm source string must match LIBM_PINNED_VERSION"
        );
    }

    #[test]
    fn exponential_routes_to_the_pinned_libm_kernel() {
        assert_eq!(exp(1.0).to_bits(), libm::exp(1.0).to_bits());
    }
}
