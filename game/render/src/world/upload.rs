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

#[derive(Default, Clone, Copy)]
struct Stamp {
    generation: Option<u64>,
    hash: u64,
    /// Checks left that write the page without hashing it: set when a hash
    /// found it changed, as a page written every tick (a spinning crowd) is.
    unhashed: u8,
}
/// Checks after a changed hash that skip hashing. A page that stops changing is
/// written at most this many more times; one that keeps changing is hashed on
/// two checks in `UNHASHED + 2` (the first after the run, against a hash the
/// run left unknown, cannot tell a change).
const UNHASHED: u8 = 7;
#[derive(Default, Clone)]
pub(super) struct Pages(Vec<Stamp>);
impl Pages {
    pub fn invalidate(&mut self, page: usize) {
        if let Some(stamp) = self.0.get_mut(page) {
            *stamp = Stamp::default();
        }
    }
    pub fn reset(&mut self) {
        self.0.fill(Stamp::default());
    }
    pub fn needs_check(&self, page: usize, generation: u64) -> bool {
        self.0
            .get(page)
            .is_none_or(|s| s.generation != Some(generation))
    }
    pub fn dirty(&mut self, page: usize, generation: u64, values: &[f32], filter: bool) -> bool {
        if self.0.len() <= page {
            self.0.resize(page + 1, Stamp::default());
        }
        let stamp = &mut self.0[page];
        if filter && stamp.unhashed > 0 {
            // Writing an unchanged page is correct; hashing a changed one is waste.
            *stamp = Stamp {
                generation: Some(generation),
                hash: 0,
                unhashed: stamp.unhashed - 1,
            };
            return true;
        }
        let hash = if filter { hash(values) } else { 0 };
        let dirty = hash == 0 || stamp.hash != hash;
        *stamp = Stamp {
            generation: Some(generation),
            hash,
            unhashed: if filter && dirty && stamp.hash != 0 {
                UNHASHED
            } else {
                0
            },
        };
        dirty
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
    fn a_page_changing_every_check_is_written_unhashed_and_stops_when_it_stops() {
        let mut pages = Pages::default();
        let (a, b) = (vec![1.0; 20], vec![2.0; 20]);
        let mut written = Vec::new();
        // Alternating values: every check is a change, including back to an earlier value.
        for generation in 0..20u64 {
            let values = if generation % 2 == 0 { &a } else { &b };
            written.push(pages.dirty(0, generation, values, true));
        }
        assert!(written.iter().all(|&w| w), "{written:?}");
        let hashed = pages.0[0].unhashed;
        // Then unchanged: written until a hash can see it stopped, then never.
        let mut tail = Vec::new();
        for generation in 20..40u64 {
            tail.push(pages.dirty(0, generation, &a, true));
        }
        let last = tail.iter().rposition(|&w| w).unwrap();
        assert!(last <= UNHASHED as usize + 1, "{tail:?} after {hashed}");
        assert!(tail[last + 1..].iter().all(|&w| !w));
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
