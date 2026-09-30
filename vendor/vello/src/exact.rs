// Copyright 2026 the Exact authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! EXACT: vello's bump-allocated buffers sized to the scene.
//!
//! vello 0.10 allocates its bump buffers at fixed sizes picked for its test
//! scenes (`BufferSizes::new`: ~150 MB, whatever the scene). Here they start
//! small, grow when a render overflows them (the GPU counts what each stage
//! needed; the render runs again), and drift back down when scenes shrink.

use vello_encoding::{BufferSize, BumpAllocators, Layout, RenderConfig};

/// Pooled buffers no render has taken in this many renders are dropped.
pub const EVICT_RENDERS: u64 = 32;

/// The floor: enough for a small canvas's scene without a retry.
const FLOOR: BumpSizes = BumpSizes {
    lines: 1 << 13,
    binning: 1 << 12,
    tiles: 1 << 13,
    seg_counts: 1 << 13,
    blend: 1 << 12,
    ptcl: 1 << 13,
    peak: [0; 6],
};

/// Sizes of the bump-allocated buffers, in elements (the dynamic part of
/// the bin data and the per-tile command list; the fixed parts are sized
/// from the scene's layout and the target).
#[derive(Clone, Copy, Debug)]
pub struct BumpSizes {
    pub lines: u32,
    pub binning: u32,
    pub tiles: u32,
    pub seg_counts: u32,
    pub blend: u32,
    pub ptcl: u32,
    /// Each counter's decaying peak over recent renders.
    peak: [u32; 6],
}

impl Default for BumpSizes {
    fn default() -> Self {
        FLOOR
    }
}

/// The counters in [`BumpSizes`] order.
fn counts(b: &BumpAllocators) -> [u32; 6] {
    [b.lines, b.binning, b.tile, b.seg_counts, b.blend, b.ptcl]
}

/// Room for `need`: a quarter again, rounded up to a 4 KiB multiple of
/// elements.
fn room(need: u32) -> u32 {
    (need.saturating_add(need / 4).saturating_add(1024)).next_multiple_of(1024)
}

impl BumpSizes {
    fn get(&self) -> [u32; 6] {
        [self.lines, self.binning, self.tiles, self.seg_counts, self.blend, self.ptcl]
    }

    fn set(&mut self, v: [u32; 6]) {
        [self.lines, self.binning, self.tiles, self.seg_counts, self.blend, self.ptcl] = v;
    }

    /// Use these sizes for a render's buffers and its config.
    pub(crate) fn apply(&self, c: &mut RenderConfig, layout: &Layout) {
        let b = &mut c.buffer_sizes;
        let g = &mut c.gpu;
        b.lines = BufferSize::new(self.lines);
        g.lines_size = self.lines;
        b.bin_data = BufferSize::new(layout.bin_data_start + self.binning);
        g.binning_size = self.binning;
        b.tiles = BufferSize::new(self.tiles);
        g.tiles_size = self.tiles;
        b.seg_counts = BufferSize::new(self.seg_counts);
        g.seg_counts_size = self.seg_counts;
        // Segments are allocated per segment counted, which is checked.
        b.segments = BufferSize::new(self.seg_counts);
        g.segments_size = self.seg_counts;
        b.blend_spill = BufferSize::new(self.blend);
        g.blend_size = self.blend;
        // The per-tile command list: a fixed block per tile, then the
        // dynamic part, then one increment of headroom.
        let fixed = g.width_in_tiles * g.height_in_tiles * 64;
        let ptcl = fixed + self.ptcl + 256;
        b.ptcl = BufferSize::new(ptcl);
        g.ptcl_size = ptcl;
    }

    /// A render overflowed: make room for what every stage that ran counted
    /// (a stage after the one that failed did not run, so its count is low
    /// and grows on a later try).
    pub(crate) fn grow(&mut self, b: &BumpAllocators) {
        let mut v = self.get();
        let mut grew = false;
        for (s, need) in v.iter_mut().zip(counts(b)) {
            if need > *s {
                *s = room(need).max(s.saturating_mul(2));
                grew = true;
            }
        }
        if !grew {
            // Failed with every count in range: double them all.
            v = v.map(|s| s.saturating_mul(2));
        }
        self.set(v);
    }

    /// A render fit: remember the peak, and shrink a size that is far
    /// larger than any recent render needed.
    pub(crate) fn settle(&mut self, b: &BumpAllocators) {
        let mut v = self.get();
        let floor = FLOOR.get();
        for (i, need) in counts(b).into_iter().enumerate() {
            let p = &mut self.peak[i];
            *p = need.max(*p - *p / 64);
            let want = room(*p).max(floor[i]);
            if v[i] > want.saturating_mul(3) {
                v[i] = want.saturating_mul(2).max(floor[i]);
            }
        }
        self.set(v);
    }
}

/// What one [`crate::Renderer::render_exact`] did.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExactStats {
    /// Renders run (more than one when the bump buffers overflowed).
    pub renders: u32,
    /// Milliseconds recording and submitting.
    pub encode_ms: f64,
    /// Milliseconds waiting for the GPU.
    pub gpu_ms: f64,
    /// The last render's counters.
    pub bump: BumpAllocators,
}
