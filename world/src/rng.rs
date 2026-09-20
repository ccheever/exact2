use crate::Resource;
use std::ops::Range;

/// PCG-XSH-RR 64/32, with a fixed odd stream increment and saved 64-bit state.
/// Constants and output permutation are from O'Neill's PCG reference generator.
#[derive(Clone, Debug, Resource)]
pub struct Rng {
    state: u64,
}
impl Default for Rng {
    fn default() -> Self {
        Self::new(0)
    }
}
impl Rng {
    /// Seed the generator using PCG's two-step initialization.
    pub fn new(seed: u64) -> Self {
        let mut r = Self { state: 0 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }
    /// Uniform bits; the state transition is fully specified wrapping arithmetic.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (((old >> 18 ^ old) >> 27) as u32).rotate_right((old >> 59) as u32)
    }
    /// Uniform 24-bit fractions in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16777216.0)
    }
    /// Sample a nonempty half-open integer or finite floating-point range.
    pub fn range<T: RangeValue>(&mut self, range: Range<T>) -> T {
        T::sample(self, range)
    }
    /// A Bernoulli trial; p must be in [0, 1].
    pub fn chance(&mut self, p: f32) -> bool {
        assert!((0.0..=1.0).contains(&p), "probability outside [0, 1]");
        self.next_f32() < p
    }
    /// Choose a slice element, or None for an empty slice.
    pub fn pick<'a, T>(&mut self, values: &'a [T]) -> Option<&'a T> {
        if values.is_empty() {
            None
        } else {
            Some(&values[self.below(values.len() as u64) as usize])
        }
    }
    fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let bits = u64::from(self.next_u32()) << 32 | u64::from(self.next_u32());
            if bits >= threshold {
                return bits % n;
            }
        }
    }
}

/// Numeric ranges supported by the world's generator.
pub trait RangeValue: Sized {
    /// Sample in range, rejecting empty or non-finite bounds.
    fn sample(rng: &mut Rng, range: Range<Self>) -> Self;
}
macro_rules! integer {
    ($($ty:ty),*) => {$(impl RangeValue for $ty {
        fn sample(rng: &mut Rng, range: Range<Self>) -> Self {
            assert!(range.start < range.end, "empty random range");
            let n = (range.end as i128 - range.start as i128) as u64;
            (range.start as i128 + rng.below(n) as i128) as Self
        }
    })*};
}
integer!(u8, u16, u32, u64, i8, i16, i32, i64);
impl RangeValue for f32 {
    fn sample(rng: &mut Rng, range: Range<Self>) -> Self {
        assert!(
            range.start.is_finite() && range.end.is_finite() && range.start < range.end,
            "invalid random range"
        );
        let t = rng.next_f32() as f64;
        let n = ((1.0 - t) * range.start as f64 + t * range.end as f64) as f32;
        n.min(libm::nextafterf(range.end, range.start))
            .max(range.start)
    }
}
impl RangeValue for f64 {
    fn sample(rng: &mut Rng, range: Range<Self>) -> Self {
        assert!(
            range.start.is_finite() && range.end.is_finite() && range.start < range.end,
            "invalid random range"
        );
        let bits = (u64::from(rng.next_u32()) << 21) | u64::from(rng.next_u32() >> 11);
        let t = bits as f64 * (1.0 / 9007199254740992.0);
        let n = (1.0 - t) * range.start + t * range.end;
        n.min(libm::nextafter(range.end, range.start))
            .max(range.start)
    }
}
