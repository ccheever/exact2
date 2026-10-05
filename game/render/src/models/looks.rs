//! In-place look patches: a changed NodeMaterials or MaterialOverrides rewrites
//! its own records' words, not every batch.
use super::INSTANCE_WORDS;
use crate::{buffers::bytes, DrawInstance};

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
            m.words[at..at + 4].copy_from_slice(&record.tint.map(f32::to_bits));
            m.words[at + 4..at + 7].copy_from_slice(&record.glow.map(f32::to_bits));
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
