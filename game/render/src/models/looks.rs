//! In-place look patches: a changed NodeMaterials or MaterialOverrides rewrites
//! its own records' words, not every batch; and the surface words a
//! `MaterialOverride`'s metallic, roughness and `Shimmer` pack into.
use super::INSTANCE_WORDS;
use crate::{buffers::bytes, DrawInstance};
use exact_game::Shimmer;

/// A record's last four words, as model_base.wgsl's `look_surface` and
/// `shimmered` read them: metallic and roughness in eight bits each, then a
/// set flag for each (bits 16, 17) and the wave (bits 24–25: 0 steady,
/// 1 pulse, 2 flicker, 3 hue); the rate and phase as `f32` bits; and the low
/// and high scales in sixteen bits each, in 1/4096ths (so [0, 16)).
pub fn surface_words(metallic: Option<f32>, roughness: Option<f32>, shimmer: Shimmer) -> [u32; 4] {
    let unorm = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    let scale = |v: f32| (v.clamp(0., 65535. / 4096.) * 4096.).round() as u32;
    let mut x = 0;
    if let Some(m) = metallic.filter(|m| m.is_finite()) {
        x |= unorm(m) | 1 << 16;
    }
    if let Some(r) = roughness.filter(|r| r.is_finite()) {
        x |= unorm(r) << 8 | 1 << 17;
    }
    let finite = |v: f32| if v.is_finite() { v } else { 0. };
    let (wave, rate, phase, low, high) = match shimmer {
        Shimmer::Steady => return [x, 0, 0, 0],
        Shimmer::Pulse {
            rate,
            phase,
            low,
            high,
        } => (1, rate, phase, low, high),
        Shimmer::Flicker {
            rate,
            phase,
            low,
            high,
        } => (2, rate, phase, low, high),
        Shimmer::Hue { rate, phase } => (3, rate, phase, 1., 1.),
    };
    // A phase past a few thousand cycles would cost the shader its precision.
    let phase = finite(phase) % 4096.;
    [
        x | wave << 24,
        finite(rate).to_bits(),
        phase.to_bits(),
        scale(finite(low)) | scale(finite(high)) << 16,
    ]
}

impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// Rewrite the looks of records `first..` and part looks `part_first..` in
    /// the instance buffer, leaving every other word and the batches alone.
    pub(crate) fn patch_draw_looks(
        &mut self,
        first: usize,
        records: &[DrawInstance],
        part_first: usize,
        looks: &[[f32; 8]],
    ) {
        let m = &mut self.models;
        let total = m.records.len();
        if first + records.len() > total || part_first + looks.len() > m.part_looks.1.len() {
            return;
        }
        for (i, record) in records.iter().enumerate() {
            let at = (first + i) * INSTANCE_WORDS + 36;
            m.records[first + i].tint = record.tint;
            m.records[first + i].glow = record.glow;
            m.records[first + i].surface = record.surface;
            m.words[at..at + 4].copy_from_slice(&record.tint.map(f32::to_bits));
            m.words[at + 4..at + 7].copy_from_slice(&record.glow.map(f32::to_bits));
            // Word 43 (the part-look link) stays; a patch never changes its shape.
            m.words[at + 8..at + 12].copy_from_slice(&record.surface);
        }
        // Part starts stay the resident absolute ones; only the looks change.
        for (k, look) in looks.iter().enumerate() {
            let entry = &mut m.part_looks.1[part_first + k];
            entry[..7].copy_from_slice(&look[..7]);
            let at = (total + part_first + k) * INSTANCE_WORDS + 36;
            for (word, value) in m.words[at..at + 7].iter_mut().zip(&entry[..7]) {
                *word = value.to_bits();
            }
        }
        let Some(instances) = &mut m.instances else {
            return;
        };
        let span = |a: usize, b: usize| (a * INSTANCE_WORDS, b * INSTANCE_WORDS);
        for (a, b) in [
            span(first, first + records.len()),
            span(total + part_first, total + part_first + looks.len()),
        ] {
            if a < b {
                instances.write(&self.queue, a as u64 * 4, bytes(&m.words[a..b]));
            }
        }
    }
}
