//! Clustered local lights: every point and spot light the frame carries, culled
//! on the CPU into a 16 × 9 × 24 view-frustum grid (screen tiles × exponential
//! depth slices) and read by the forward and model shaders (`lights.wgsl`).
//!
//! A light joins every cluster its range sphere may touch, so a fragment skips
//! only lights whose windowed contribution is exactly zero there. Each cluster
//! lists its lights in frame order (nearest the camera first); the sum is the
//! one an unculled loop over the same lights computes.
use crate::buffers::{bytes, Buffer};
use crate::FrameInput;
use exact_gpu::wgpu;
use glam::{Vec3, Vec4Swizzles};

/// Screen tiles across, down, and depth slices.
pub(crate) const CLUSTERS: [u32; 3] = [16, 9, 24];
const CELLS: usize = (CLUSTERS[0] * CLUSTERS[1] * CLUSTERS[2]) as usize;
/// u32 words per light record (lights.wgsl).
pub(crate) const RECORD_WORDS: usize = 16;

pub(crate) struct Lights {
    pub buffer: Buffer,
    /// Per-slot screen-door fade (`1 - Opacity`), zero past the end; read beside
    /// the lights by the same fragment shaders (`faded` in lights.wgsl).
    pub opacity: Buffer,
    opacity_words: Vec<f32>,
    words: Vec<u32>,
    counts: Vec<u32>,
    // Per light: tile x, y and slice ranges (inclusive), or None when culled.
    spans: Vec<Option<[u32; 6]>>,
    /// Lights in the buffer, camera near distance, slices per log2 metre, and
    /// the grid's word offset: the frame uniform's `lights_info`.
    pub info: [f32; 4],
}

impl Lights {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            buffer: Buffer::new(device, 64, wgpu::BufferUsages::STORAGE, "game lights"),
            opacity: Buffer::new(device, 16, wgpu::BufferUsages::STORAGE, "game opacity"),
            opacity_words: Vec::new(),
            words: Vec::new(),
            counts: Vec::new(),
            spans: Vec::new(),
            info: [0.; 4],
        }
    }

    /// Replace the per-slot opacities; true when the buffer was replaced and the
    /// scene groups must rebind. The table holds `1 - opacity`, so zeroed and
    /// unwritten slots draw opaque; it never shrinks, so a cleared fade is rewritten.
    pub fn set_opacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        values: &[(u32, f32)],
    ) -> bool {
        let needed = values
            .iter()
            .map(|&(slot, _)| slot as usize + 1)
            .max()
            .unwrap_or(0);
        let len = self.opacity_words.len().max(needed);
        self.opacity_words.clear();
        self.opacity_words.resize(len, 0.);
        for &(slot, value) in values {
            let opacity = if value.is_nan() {
                1.
            } else {
                value.clamp(0., 1.)
            };
            self.opacity_words[slot as usize] = 1. - opacity;
        }
        let grew = self
            .opacity
            .grow(device, queue, (self.opacity_words.len().max(1) * 4) as u64);
        self.opacity.write(queue, 0, bytes(&self.opacity_words));
        grew
    }

    /// Pack this frame's lights and their clusters; true when the buffer was
    /// replaced and the scene groups must rebind.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &FrameInput<'_>,
        size: (u32, u32),
        shadows: &crate::local_shadows::Plan,
    ) -> bool {
        let lights = &frame.lights[..frame.lights.len().min(crate::MAX_LIGHTS)];
        self.info = [0.; 4];
        if lights.is_empty() {
            return false;
        }
        let inverse = frame.proj.inverse();
        let near = (-inverse.project_point3(Vec3::ZERO).z).max(1e-4);
        let view = frame.view;
        self.spans.clear();
        let mut far = near * 2.;
        for light in lights {
            let centre = view.transform_point3(light.position);
            let r = light.range.max(0.);
            let (front, back) = (-centre.z - r, -centre.z + r);
            if back < near || r.is_nan() || r <= 0. || !centre.is_finite() {
                self.spans.push(None);
                continue;
            }
            far = far.max(back);
            // The sphere's view-space box projects inside its corners' hull while
            // every corner is in front of the camera; otherwise cover the screen.
            let (mut lo, mut hi) = (glam::Vec2::splat(-1.), glam::Vec2::splat(1.));
            if front > near {
                (lo, hi) = (
                    glam::Vec2::splat(f32::INFINITY),
                    glam::Vec2::splat(-f32::INFINITY),
                );
                for corner in 0..8 {
                    let offset = Vec3::new(
                        if corner & 1 == 0 { -r } else { r },
                        if corner & 2 == 0 { -r } else { r },
                        if corner & 4 == 0 { -r } else { r },
                    );
                    let clip = frame.proj * (centre + offset).extend(1.);
                    let ndc = clip.xy() / clip.w;
                    lo = lo.min(ndc);
                    hi = hi.max(ndc);
                }
            }
            // NDC y is up; framebuffer rows run down. Pad one pixel either side.
            let pad = glam::Vec2::new(2. / size.0 as f32, 2. / size.1 as f32);
            let tile = |ndc_x: f32, ndc_y: f32| {
                let x = ((ndc_x * 0.5 + 0.5) * CLUSTERS[0] as f32).floor();
                let y = ((0.5 - ndc_y * 0.5) * CLUSTERS[1] as f32).floor();
                (
                    x.clamp(0., (CLUSTERS[0] - 1) as f32) as u32,
                    y.clamp(0., (CLUSTERS[1] - 1) as f32) as u32,
                )
            };
            let (x0, y0) = tile(lo.x - pad.x, hi.y + pad.y);
            let (x1, y1) = tile(hi.x + pad.x, lo.y - pad.y);
            if lo.x - pad.x > 1. || hi.x + pad.x < -1. || lo.y - pad.y > 1. || hi.y + pad.y < -1. {
                self.spans.push(None);
                continue;
            }
            self.spans.push(Some([x0, x1, y0, y1, 0, 0]));
            // Depth slices are filled below, once the far bound is known.
            let last = self.spans.len() - 1;
            if let Some(span) = &mut self.spans[last] {
                span[4] = front.max(near).to_bits();
                span[5] = back.to_bits();
            }
        }
        let scale = CLUSTERS[2] as f32 / (far / near).log2().max(1e-6);
        let slice = |depth: f32| {
            ((depth / near).log2() * scale)
                .floor()
                .clamp(0., (CLUSTERS[2] - 1) as f32) as u32
        };
        self.counts.clear();
        self.counts.resize(CELLS, 0);
        for span in self.spans.iter_mut().flatten() {
            // One slice of margin either side absorbs log2 rounding in the shader.
            let (front, back) = (f32::from_bits(span[4]), f32::from_bits(span[5]));
            span[4] = slice(front).saturating_sub(1);
            span[5] = (slice(back) + 1).min(CLUSTERS[2] - 1);
            for_cells(span, |cell| self.counts[cell] += 1);
        }
        let records = lights.len() * RECORD_WORDS;
        let grid = records;
        let indices = grid + 2 * CELLS;
        let total: u32 = self.counts.iter().sum();
        let matrices = indices + total as usize;
        self.words.clear();
        self.words.resize(matrices + shadows.views.len() * 16, 0);
        for (i, view) in shadows.views.iter().enumerate() {
            self.words[matrices + i * 16..matrices + (i + 1) * 16]
                .copy_from_slice(&view.to_cols_array().map(f32::to_bits));
        }
        for (i, light) in lights.iter().enumerate() {
            let [inner, outer] = light.cone.unwrap_or([-2., -2.]);
            let shadow = shadows.layers.get(i).copied().flatten();
            let record: [f32; RECORD_WORDS] = [
                light.position.x,
                light.position.y,
                light.position.z,
                light.range,
                light.color.x,
                light.color.y,
                light.color.z,
                light.intensity,
                light.direction.x,
                light.direction.y,
                light.direction.z,
                outer,
                inner,
                shadow.map_or(-1., |(layer, _)| layer as f32),
                shadow.map_or(0., |(layer, _)| (matrices + layer as usize * 16) as f32),
                shadow.map_or(0., |(_, texel)| texel),
            ];
            self.words[i * RECORD_WORDS..(i + 1) * RECORD_WORDS]
                .copy_from_slice(&record.map(f32::to_bits));
        }
        let mut at = indices as u32;
        for (cell, &count) in self.counts.iter().enumerate() {
            self.words[grid + 2 * cell] = at;
            at += count;
        }
        // Reuse counts as each cell's fill cursor; lights append in frame order.
        self.counts.fill(0);
        for (i, span) in self.spans.iter().enumerate() {
            let Some(span) = span else { continue };
            for_cells(span, |cell| {
                let start = self.words[grid + 2 * cell];
                self.words[(start + self.counts[cell]) as usize] = i as u32;
                self.counts[cell] += 1;
            });
        }
        for (cell, &count) in self.counts.iter().enumerate() {
            self.words[grid + 2 * cell + 1] = count;
        }
        self.info = [lights.len() as f32, near, scale, grid as f32];
        let grew = self
            .buffer
            .grow(device, queue, (self.words.len() * 4) as u64);
        self.buffer.write(queue, 0, bytes(&self.words));
        grew
    }
}

fn for_cells(span: &[u32; 6], mut f: impl FnMut(usize)) {
    for z in span[4]..=span[5] {
        for y in span[2]..=span[3] {
            for x in span[0]..=span[1] {
                f(((z * CLUSTERS[1] + y) * CLUSTERS[0] + x) as usize);
            }
        }
    }
}
