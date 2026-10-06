//! The pinned deterministic-math profile.
//!
//! @ref LLP 1003 §5 (determinism)
//!
//! Every transcendental the evaluator calls routes through this module, so the
//! same transition advanced to the same time produces the same bits on every
//! target this crate compiles for. Rust does not contract separate
//! multiply/add expressions into FMA, so the authored rounding survives too.

/// The `libm` release whose software kernels define the profile.
pub const LIBM_PINNED_VERSION: &str = "0.2.16";

/// Deterministic software exponential.
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Deterministic software sine.
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Deterministic software cosine.
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Deterministic software natural logarithm (LLP 1099 D2, amending LLP 1003
/// §7): UIKit spring conversions solve with it.
#[inline]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// IEEE 754 binary64 square root — correctly rounded by the standard, so the
/// platform intrinsic is already architecture-independent for finite inputs.
#[inline]
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_lock_matches_the_pinned_libm() {
        let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock"))
            .expect("read workspace Cargo.lock");
        let mut versions = Vec::new();
        let mut name = None;
        for line in lock.lines() {
            if line == "[[package]]" {
                name = None;
            } else if let Some(v) = line.strip_prefix("name = \"") {
                name = Some(v.trim_end_matches('"'));
            } else if let (Some("libm"), Some(v)) = (name, line.strip_prefix("version = \"")) {
                versions.push(v.trim_end_matches('"').to_string());
            }
        }
        assert_eq!(
            versions,
            [LIBM_PINNED_VERSION],
            "Cargo.lock's libm moved; move LIBM_PINNED_VERSION with it and re-check the easing and spring fixtures"
        );
    }

    #[test]
    fn kernels_route_to_libm() {
        assert_eq!(exp(1.0).to_bits(), libm::exp(1.0).to_bits());
        assert_eq!(sin(1.0).to_bits(), libm::sin(1.0).to_bits());
        assert_eq!(cos(1.0).to_bits(), libm::cos(1.0).to_bits());
        assert_eq!(ln(2.0).to_bits(), libm::log(2.0).to_bits());
    }
}
