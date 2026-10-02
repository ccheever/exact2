//! `exact_segments` (LLP 1077 D4): the posture and the viewport segments a
//! fold makes, told to the host as one call.

use super::{not_booted, Bridge};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// The posture and the viewport segments changed (LLP 1077 D4).
    pub fn segments(&mut self, posture: u32, cols: u32, rows: u32, count: u32) -> u32 {
        let rects = segment_rects(&self.input, count);
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.set_segments(posture, cols, rows, rects));
        self.emit(out)
    }
}

/// `exact_segments`'s rects: `count × 4` little-endian `f32`s, `x y w h`
/// each, in the input buffer (none when `count` is 0 or the buffer is short).
pub fn segment_rects(input: &[u8], count: u32) -> Vec<exact_kernel::Rect> {
    let bytes = (count as usize).saturating_mul(16);
    if bytes == 0 || input.len() < bytes {
        return Vec::new();
    }
    input[..bytes]
        .chunks_exact(16)
        .map(|r| {
            let f = |i: usize| f32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
            exact_kernel::Rect::new(f(0), f(4), f(8), f(12))
        })
        .collect()
}
