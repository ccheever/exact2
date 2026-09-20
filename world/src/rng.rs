use crate::Resource;

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
}
