use crate::Resource;

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
    pub fn new(seed: u64) -> Self {
        let mut r = Self { state: 0 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (((old >> 18 ^ old) >> 27) as u32).rotate_right((old >> 59) as u32)
    }
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16777216.0)
    }
}
