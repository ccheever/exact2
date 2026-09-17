//! Retained page fingerprints and coalesced runs. Zero means unknown, never a hash.
use crate::buffers::bytes;

// Independent 64-bit lanes hide multiply latency. Presentation-only, never the
// stable save hash. Eight-byte reads are unaligned and endian-explicit.
pub(super) fn hash(floats: &[f32]) -> u64 {
    const P: u64 = 0x9e3779b185ebca87;
    let mut lanes = [
        P,
        !P,
        P.rotate_left(17),
        P.rotate_left(41),
        P.rotate_left(3),
        P.rotate_left(11),
        P.rotate_left(29),
        P.rotate_left(53),
    ];
    let bytes = bytes(floats);
    let mut chunks = bytes.chunks_exact(64);
    for chunk in &mut chunks {
        for (lane, word) in lanes.iter_mut().zip(chunk.chunks_exact(8)) {
            *lane = (*lane ^ u64::from_le_bytes(word.try_into().unwrap()))
                .rotate_left(27)
                .wrapping_mul(P);
        }
    }
    let mut h = bytes.len() as u64;
    for lane in lanes {
        h = (h ^ lane).rotate_left(27).wrapping_mul(P);
    }
    for word in chunks.remainder().chunks_exact(8) {
        h = (h ^ u64::from_le_bytes(word.try_into().unwrap()))
            .rotate_left(27)
            .wrapping_mul(P);
    }
    debug_assert!(bytes.len().is_multiple_of(8));
    h ^= h >> 33;
    h = h.wrapping_mul(0xc2b2ae3d27d4eb4f);
    (h ^ (h >> 29)).max(1)
}

#[derive(Default)]
pub(super) struct Pages {
    pub hashes: Vec<u64>,
    dense: bool,
    round: u64,
    probe: u8,
    dirty: usize,
    total: usize,
}
impl Pages {
    pub fn invalidate(&mut self, page: usize) {
        if self.hashes.len() <= page {
            self.hashes.resize(page + 1, 0);
        }
        self.hashes[page] = 0;
    }
    pub fn reset(&mut self) {
        self.hashes.fill(0);
        self.dense = false;
        self.round = 0;
        self.probe = 0;
    }
    pub fn inherit_policy(&mut self, other: &Self) {
        self.dense = other.dense;
        self.round = other.round;
        self.probe = other.probe;
    }
    pub fn start(&mut self, count: usize, initial: bool) -> bool {
        self.round += 1;
        self.dirty = 0;
        self.total = 0;
        // Tiny scenes never amortize adaptive probing, and must notice same-value
        // assignments immediately. Probe three feeds to populate both tick roles
        // before deciding whether the target contents actually changed.
        if self.round.is_multiple_of(32) {
            self.probe = 3;
        }
        let hashing = initial || count < 32 || !self.dense || self.probe > 0;
        self.probe = self.probe.saturating_sub(1);
        hashing
    }
    pub fn dirty(&mut self, page: usize, hash: u64, initial: bool) -> bool {
        if self.hashes.len() <= page {
            self.hashes.resize(page + 1, 0);
        }
        self.total += 1;
        let dirty = initial || hash == 0 || self.hashes[page] != hash;
        self.hashes[page] = hash;
        self.dirty += usize::from(dirty);
        dirty
    }
    pub fn finish(&mut self) {
        self.dense = self.total > 0 && self.dirty * 4 >= self.total * 3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fingerprint_observes_every_word_and_record_tail() {
        let mut data = vec![0.; 1024 * 10];
        let base = hash(&data);
        for i in 0..data.len() {
            data[i] = 1.;
            assert_ne!(hash(&data), base, "word {i}");
            data[i] = 0.;
        }
        assert_ne!(hash(&data[..10]), hash(&data[..20]));
    }
    #[test]
    #[ignore = "release bandwidth diagnostic"]
    fn hash_bandwidth() {
        let data = vec![1.25; 500_000 * 10];
        let start = std::time::Instant::now();
        for _ in 0..200 {
            for page in std::hint::black_box(&data).chunks(1024 * 10) {
                std::hint::black_box(hash(page));
            }
        }
        let gbps = (data.len() * 4 * 200) as f64 / start.elapsed().as_secs_f64() / 1e9;
        eprintln!("page hash: {gbps:.2} GB/s");
        assert!(gbps >= 5.0);
    }
}
