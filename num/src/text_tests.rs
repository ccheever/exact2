//! Differential tests: every text is core's, byte for byte. (A heavier run
//! — every 97th `f32`, ten million random `f64` bit patterns, ten million
//! decimal-like values, `{:.N}` to twenty places — matched too.)

// The literals are the values under test, digits and all.
#![allow(clippy::excessive_precision)]

use super::{Exponent, Fixed, Shortest, Shortest32, ShortestDebug};

/// xorshift64*, deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

fn check64(v: f64) {
    let bits = v.to_bits();
    assert_eq!(
        Shortest(v).to_string(),
        format!("{v}"),
        "{{}} of {bits:#018x}"
    );
    assert_eq!(
        Exponent(v).to_string(),
        format!("{v:e}"),
        "{{:e}} of {bits:#018x}"
    );
    assert_eq!(
        format!("{:?}", ShortestDebug(v)),
        format!("{v:?}"),
        "{{:?}} of {bits:#018x}"
    );
    assert_eq!(
        format!("{:#?}", ShortestDebug(v)),
        format!("{v:#?}"),
        "{{:#?}} of {bits:#018x}"
    );
}

fn check32(v: f32) {
    let bits = v.to_bits();
    assert_eq!(
        Shortest32(v).to_string(),
        format!("{v}"),
        "{{}} of {bits:#010x}"
    );
}

fn check_fixed(v: f64) {
    for frac in 0..=6 {
        let bits = v.to_bits();
        assert_eq!(
            Fixed(v, frac).to_string(),
            format!("{v:.frac$}"),
            "{{:.{frac}}} of {bits:#018x}"
        );
    }
}

#[test]
fn specials_and_edges_print_as_core_prints_them() {
    for v in [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.1,
        0.5,
        1.5,
        2.0,
        10.0,
        100.0,
        1e15,
        1e16,
        1e17,
        1e21,
        1e22,
        1e23,
        9007199254740992.0,
        9007199254740993.0,
        4503599627370496.5,
        1e-4,
        9.999e-5,
        1e-5,
        1e-7,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        5e-324,
        1e-323,
        2.2250738585072009e-308,
        f64::EPSILON,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        123.456,
        0.30000000000000004,
        1.7976931348623157e308,
        33554432.5,
        8388608.5,
        0.07,
        0.25,
        0.35,
        2.5,
        3.5,
        1e100,
        1.5e300,
        1e-300,
        1125899906842624.25,
        1125899906842624.75,
        562949953421312.125,
        562949953421312.375,
        0.125,
        0.375,
        1e-20,
        0.1 + 0.2,
        1.0 / 3.0,
    ] {
        check64(v);
        check_fixed(v);
    }
    for v in [
        0.0f32,
        -0.0,
        1.0,
        0.1,
        0.5,
        8388608.5,
        16777216.0,
        16777218.0,
        123456789.0,
        3.4028235e38,
        f32::MIN_POSITIVE,
        1e-45,
        1.1754942e-38,
        f32::INFINITY,
        f32::NAN,
        1e-4,
        9.99e-5,
        1e16,
        1.2345678e-10,
        65504.0,
    ] {
        check32(v);
    }
    // Powers of two, and their neighbours, across the whole range.
    for e in -1074..=1023 {
        let v = 2f64.powi(e);
        for v in [
            v,
            f64::from_bits(v.to_bits() + 1),
            f64::from_bits(v.to_bits().saturating_sub(1)),
        ] {
            check64(v);
        }
    }
    for e in -149..=127 {
        let v = 2f32.powi(e);
        for v in [
            v,
            f32::from_bits(v.to_bits() + 1),
            f32::from_bits(v.to_bits().saturating_sub(1)),
        ] {
            check32(v);
        }
    }
}

#[test]
fn random_bit_patterns_print_as_core_prints_them() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let bits: Vec<(u64, u32)> = (0..50_000)
        .map(|_| (rng.next(), rng.next() as u32))
        .collect();
    crate::tests::par(&bits, |&(a, b)| {
        check64(f64::from_bits(a));
        check32(f32::from_bits(b));
    });
}

#[test]
fn everyday_numbers_print_as_core_prints_them() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for _ in 0..10_000 {
        // What CSS, clocks and data carry: short decimals, integers,
        // computed values, in a wide middle range.
        let places = rng.next() % 6;
        let whole = (rng.next() % 1_000_000) as f64;
        let short = whole / 10f64.powi(places as i32);
        let computed = (rng.next() % 100_000) as f64 / 3.0 * 0.1;
        let scale = 10f64.powi((rng.next() % 40) as i32 - 20);
        for v in [
            short,
            -short,
            computed,
            short * scale,
            (rng.next() >> 11) as f64,
        ] {
            check64(v);
            check32(v as f32);
            check_fixed(v);
        }
    }
}
