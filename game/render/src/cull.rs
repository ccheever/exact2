//! GPU frustum culling: per-view compacted slot lists and indirect draws.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
//!
//! Transforms live on the GPU, so the test runs there too: one compute pass after
//! skinning tests every opaque item's interpolation-swept sphere against the camera
//! and each sun cascade, then writes each view's visible slots, in retained order,
//! into its own 256-byte-aligned region. Draws read their region through the scene
//! group's dynamic `slots` offset and take counts from `draw_indexed_indirect` with
//! a zero first instance, so WebGPU's optional `indirect-first-instance` is unused.
use crate::buffers::{bytes, Buffer};
use exact_gpu::wgpu;
use glam::Mat4;
use std::ops::Range;

/// Camera plus up to three sun cascades.
pub(crate) const VIEWS: u32 = 4;
/// Words per draw-group record in `setup`, mirrored by cull.wgsl.
pub(crate) const GROUP_WORDS: usize = 16;
/// Words per model record and per skinned record.
pub(crate) const RECORD_WORDS: usize = 8;
pub(crate) const SKIN_WORDS: usize = 20;
/// Group flag: keep every item (game-deformed vertices have no engine bound).
pub(crate) const KEEP_ALL: u32 = 16;
pub(crate) const CAPSULE: u32 = 64;
pub(crate) const PLAIN: u32 = 128;
const CHUNK: u32 = 64;
const UNIFORM_BYTES: u64 = 24 * 16 + 3 * 16;

/// One indirect draw per view: a contiguous slot-list range with one pipeline.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    pub batch: usize,
    pub range: Range<u32>,
    pub mirrored: bool,
    pub flags: u32,
}

pub(crate) struct Cull {
    test: wgpu::ComputePipeline,
    scan: wgpu::ComputePipeline,
    scatter: wgpu::ComputePipeline,
    test_layout: wgpu::BindGroupLayout,
    list_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    setup: Buffer,
    scratch: Buffer,
    pub compacted: Buffer,
    pub indirect: Buffer,
    no_palette: wgpu::Buffer,
    test_binds: Option<([wgpu::BindGroup; 2], [wgpu::Buffer; 9])>,
    list_bind: Option<(wgpu::BindGroup, [wgpu::Buffer; 5])>,
    /// This frame's draw groups, in draw order.
    pub groups: Vec<Group>,
    uploaded: Vec<Group>,
    uploaded_epoch: Option<u64>,
    /// Bumped whenever slots, records or meshes change structure.
    pub epoch: u64,
    pub words: Vec<u32>,
    regions: Vec<u32>,
    sections: [u32; 4],
    chunks: u32,
    stride: u32,
    views: u32,
    /// Vertex binding window of the compacted buffer, in bytes.
    pub window: u64,
    pub dispatches: u64,
    /// Diagnostic: keep every item, drawing exactly what an unculled frame drew.
    pub keep_all: bool,
    /// The lists exceed this device's storage limits: draw every group directly.
    pub direct: bool,
    // Regions start on the device's dynamic storage offset alignment, in words.
    align: u32,
    limit: u64,
    counts: Option<Counts>,
}

// Optional asynchronous readback of each view's instance totals (perf diagnostics).
struct Counts {
    buffer: wgpu::Buffer,
    done: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
    phase: u8,
    groups: usize,
    views: usize,
    last: Option<[u64; VIEWS as usize]>,
}

impl Cull {
    pub fn new(device: &wgpu::Device) -> Self {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = wgpu::BufferBindingType::Uniform;
        let read = wgpu::BufferBindingType::Storage { read_only: true };
        let write = wgpu::BufferBindingType::Storage { read_only: false };
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        // Eight storage buffers: the portable per-stage limit, exactly.
        let test_layout = layout(
            "game cull test",
            &[
                entry(0, uniform),
                entry(1, read),
                entry(2, read),
                entry(3, read),
                entry(4, read),
                entry(5, read),
                entry(6, uniform),
                entry(7, read),
                entry(8, read),
                entry(9, write),
            ],
        );
        let list_layout = layout(
            "game cull lists",
            &[
                entry(4, read),
                entry(6, uniform),
                entry(7, read),
                entry(9, write),
                entry(10, write),
                entry(11, write),
            ],
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game cull"),
            source: wgpu::ShaderSource::Wgsl(crate::pipeline::cull_source().into()),
        });
        let pipeline = |layout: &wgpu::BindGroupLayout, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("game cull"),
                layout: Some(
                    &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("game cull"),
                        bind_group_layouts: &[Some(layout)],
                        immediate_size: 0,
                    }),
                ),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let storage = |label| Buffer::new(device, 256, wgpu::BufferUsages::STORAGE, label);
        Self {
            test: pipeline(&test_layout, "test"),
            scan: pipeline(&list_layout, "scan"),
            scatter: pipeline(&list_layout, "scatter"),
            test_layout,
            list_layout,
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game cull views"),
                size: UNIFORM_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            setup: storage("game cull setup"),
            scratch: storage("game cull scratch"),
            compacted: storage("game culled slots"),
            indirect: Buffer::new(
                device,
                256,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
                "game culled draws",
            ),
            no_palette: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game cull no palette"),
                size: 64,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            test_binds: None,
            list_bind: None,
            groups: Vec::new(),
            uploaded: Vec::new(),
            uploaded_epoch: None,
            epoch: 0,
            words: Vec::new(),
            regions: Vec::new(),
            sections: [0; 4],
            chunks: 0,
            stride: 0,
            views: 0,
            window: 256,
            dispatches: 0,
            keep_all: false,
            direct: false,
            align: (device.limits().min_storage_buffer_offset_alignment / 4).max(1),
            limit: device
                .limits()
                .max_storage_buffer_binding_size
                .min(device.limits().max_buffer_size),
            counts: None,
        }
    }

    /// Start or stop reading back per-view instance totals.
    pub fn count(&mut self, device: &wgpu::Device, on: bool) {
        if !on {
            self.counts = None;
        } else if self.counts.is_none() {
            self.counts = Some(Counts {
                buffer: counts_buffer(device, 256),
                done: Default::default(),
                phase: 0,
                groups: 0,
                views: 0,
                last: None,
            });
        }
    }
    /// Copy this frame's indirect arguments for reading, when armed and idle.
    pub fn copy_counts(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        let Some(counts) = &mut self.counts else {
            return;
        };
        if counts.phase != 0 || self.groups.is_empty() || self.direct {
            return;
        }
        let bytes = u64::from(VIEWS) * self.groups.len() as u64 * 20;
        if counts.buffer.size() < bytes {
            counts.buffer = counts_buffer(device, bytes.next_power_of_two());
        }
        encoder.copy_buffer_to_buffer(&self.indirect.raw, 0, &counts.buffer, 0, bytes);
        counts.groups = self.groups.len();
        counts.views = self.views as usize;
        counts.phase = 1;
    }
    /// After submission: request the mapping of a copied frame.
    pub fn map_counts(&mut self) {
        let Some(counts) = &mut self.counts else {
            return;
        };
        if counts.phase == 1 {
            let done = counts.done.clone();
            counts
                .buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| {
                    *done.lock().unwrap() = Some(r.is_ok())
                });
            counts.phase = 2;
        }
    }
    /// The most recently read instance totals per view (camera, cascades 0–2);
    /// views that were not drawn report zero.
    pub fn culled(&mut self, device: &wgpu::Device) -> Option<[u64; VIEWS as usize]> {
        let counts = self.counts.as_mut()?;
        if counts.phase == 2 {
            #[cfg(not(target_arch = "wasm32"))]
            let _ = device.poll(wgpu::PollType::Poll);
            #[cfg(target_arch = "wasm32")]
            let _ = device;
            if let Some(ok) = counts.done.lock().unwrap().take() {
                if ok {
                    let data = counts.buffer.slice(..).get_mapped_range().unwrap();
                    let mut totals = [0u64; VIEWS as usize];
                    for (v, total) in totals.iter_mut().enumerate().take(counts.views) {
                        for g in 0..counts.groups {
                            let at = ((v * counts.groups + g) * 5 + 1) * 4;
                            *total +=
                                u64::from(u32::from_ne_bytes(data[at..at + 4].try_into().unwrap()));
                        }
                    }
                    drop(data);
                    counts.last = Some(totals);
                }
                counts.buffer.unmap();
                counts.phase = 0;
            }
        }
        counts.last
    }

    /// Whether this frame's groups differ from the uploaded setup.
    pub fn stale(&self) -> bool {
        self.uploaded_epoch != Some(self.epoch) || self.uploaded != self.groups
    }

    /// Lay out each group's output region and chunk count; the caller then appends
    /// group, record and skin words before `finish_setup`.
    pub fn begin_setup(&mut self) {
        self.words.clear();
        self.regions.clear();
        let mut region = 0;
        let mut chunk = 0;
        for group in &self.groups {
            let count = group.range.end - group.range.start;
            self.regions.push(region);
            region += count.div_ceil(self.align) * self.align;
            chunk += count.div_ceil(CHUNK);
        }
        self.stride = region.max(self.align);
        self.chunks = chunk;
    }
    /// Group `index`'s fixed words: item range, chunks and output region.
    pub fn group_words(&self, index: usize, chunk_start: u32) -> [u32; 4] {
        let group = &self.groups[index];
        [
            group.range.start,
            group.range.end - group.range.start,
            chunk_start,
            (group.range.end - group.range.start).div_ceil(CHUNK),
        ]
    }
    pub fn region(&self, index: usize) -> u32 {
        self.regions[index]
    }
    /// Finish the setup the caller appended in `words` (groups, then records and
    /// skins at the given word offsets) and upload it with the chunk table.
    pub fn finish_setup(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        items: u32,
        records: u32,
        skins: u32,
    ) {
        let chunks_at = self.words.len() as u32;
        for (index, group) in self.groups.iter().enumerate() {
            let count = (group.range.end - group.range.start).div_ceil(CHUNK);
            self.words
                .extend(std::iter::repeat_n(index as u32, count as usize));
        }
        self.sections = [0, chunks_at, records, skins];
        self.uploaded.clone_from(&self.groups);
        self.uploaded_epoch = Some(self.epoch);
        let window = self
            .groups
            .iter()
            .map(|g| (g.range.end - g.range.start).div_ceil(self.align) * self.align)
            .max()
            .unwrap_or(self.align)
            .max(self.align);
        let sizes = [
            (self.words.len() as u64 * 4).max(4),
            (u64::from(items) + u64::from(self.chunks) * 5).max(1) * 4,
            (u64::from(VIEWS) * u64::from(self.stride) + u64::from(window)) * 4,
            (u64::from(VIEWS) * self.groups.len() as u64 * 20).max(20),
        ];
        // Per-slot list storage is below the 64-byte attachment arena behind
        // max_slots(); only region padding can exceed a tiny limit. Then draw directly.
        self.direct = sizes.iter().any(|&size| size > self.limit);
        if self.direct {
            return;
        }
        self.setup.grow(device, queue, sizes[0]);
        self.setup.write(queue, 0, bytes(&self.words));
        self.scratch.grow(device, queue, sizes[1]);
        self.window = u64::from(window) * 4;
        self.compacted.grow(device, queue, sizes[2]);
        self.indirect.grow(device, queue, sizes[3]);
    }

    /// Byte offsets of group `index` in view `view`: the slot region and its draw.
    pub fn offsets(&self, view: u32, index: usize) -> (u32, u64) {
        (
            (view * self.stride + self.regions[index]) * 4,
            (u64::from(view) * self.groups.len() as u64 + index as u64) * 20,
        )
    }

    /// Per-frame view planes and counts; `items` is the slot-list length.
    pub fn write_views(&mut self, queue: &wgpu::Queue, views: &[Mat4], items: u32) {
        self.views = views.len() as u32;
        let mut data = [0u32; UNIFORM_BYTES as usize / 4];
        for (v, matrix) in views.iter().enumerate() {
            for (i, plane) in planes(*matrix).iter().enumerate() {
                let at = (v * 6 + i) * 4;
                for k in 0..4 {
                    data[at + k] = plane[k].to_bits();
                }
            }
        }
        let tail = 24 * 4;
        data[tail..tail + 4].copy_from_slice(&[
            views.len() as u32,
            self.chunks,
            self.groups.len() as u32,
            self.stride,
        ]);
        data[tail + 4..tail + 8].copy_from_slice(&self.sections);
        data[tail + 8..tail + 12].copy_from_slice(&[items, items + self.chunks, 0, 0]);
        queue.write_buffer(&self.uniform, 0, bytes(&data));
    }

    /// Recreate bind groups whose buffers were replaced.
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        &mut self,
        device: &wgpu::Device,
        frame: &wgpu::Buffer,
        transforms: [&wgpu::Buffer; 2],
        materials: &wgpu::Buffer,
        slots: &wgpu::Buffer,
        attachments: &wgpu::Buffer,
        palette: Option<&wgpu::Buffer>,
    ) {
        let palette = palette.unwrap_or(&self.no_palette);
        let key = [
            frame.clone(),
            transforms[0].clone(),
            transforms[1].clone(),
            materials.clone(),
            slots.clone(),
            attachments.clone(),
            palette.clone(),
            self.setup.raw.clone(),
            self.scratch.raw.clone(),
        ];
        if self.test_binds.as_ref().is_none_or(|(_, old)| *old != key) {
            let binds = std::array::from_fn(|current| {
                let buffers = [
                    (0, frame),
                    (1, transforms[1 - current]),
                    (2, transforms[current]),
                    (3, materials),
                    (4, slots),
                    (5, attachments),
                    (6, &self.uniform),
                    (7, &self.setup.raw),
                    (8, palette),
                    (9, &self.scratch.raw),
                ];
                let entries = buffers.map(|(binding, buffer)| wgpu::BindGroupEntry {
                    binding,
                    resource: buffer.as_entire_binding(),
                });
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("game cull test"),
                    layout: &self.test_layout,
                    entries: &entries,
                })
            });
            self.test_binds = Some((binds, key));
        }
        let key = [
            slots.clone(),
            self.setup.raw.clone(),
            self.scratch.raw.clone(),
            self.compacted.raw.clone(),
            self.indirect.raw.clone(),
        ];
        if self.list_bind.as_ref().is_none_or(|(_, old)| *old != key) {
            let buffers = [
                (4, slots),
                (6, &self.uniform),
                (7, &self.setup.raw),
                (9, &self.scratch.raw),
                (10, &self.compacted.raw),
                (11, &self.indirect.raw),
            ];
            let entries = buffers.map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game cull lists"),
                layout: &self.list_layout,
                entries: &entries,
            });
            self.list_bind = Some((bind, key));
        }
    }
    /// Encode the three dispatches. Nothing is encoded without a group.
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        current: usize,
        timestamps: Option<&wgpu::QuerySet>,
    ) {
        if self.groups.is_empty() || self.direct {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("game cull"),
            timestamp_writes: timestamps.map(|query_set| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(crate::timing::CULL * 2),
                end_of_pass_write_index: Some(crate::timing::CULL * 2 + 1),
            }),
        });
        let grid = |n: u32| (n.min(65535), n.div_ceil(65535));
        let (chunks, groups) = (grid(self.chunks), grid(self.groups.len() as u32));
        pass.set_pipeline(&self.test);
        pass.set_bind_group(0, &self.test_binds.as_ref().unwrap().0[current], &[]);
        pass.dispatch_workgroups(chunks.0, chunks.1, 1);
        let lists = &self.list_bind.as_ref().unwrap().0;
        pass.set_pipeline(&self.scan);
        pass.set_bind_group(0, lists, &[]);
        pass.dispatch_workgroups(groups.0, groups.1, 1);
        pass.set_pipeline(&self.scatter);
        pass.dispatch_workgroups(chunks.0, chunks.1, 1);
        self.dispatches += 3;
    }
}

fn counts_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("game cull counts"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

/// Whether a sphere may touch the clip volume: the CPU side of cull.wgsl's test,
/// with the same rounding margin. Non-finite input is kept.
pub(crate) fn sphere_visible(planes: &[[f32; 4]; 6], center: glam::Vec3, radius: f32) -> bool {
    if !center.is_finite() || !radius.is_finite() {
        return true;
    }
    let margin = radius * 1.0001 + 1e-4 + 1e-6 * center.abs().max_element();
    planes
        .iter()
        .all(|p| glam::Vec3::new(p[0], p[1], p[2]).dot(center) + p[3] >= -margin)
}

/// Six normalized inside-positive planes of a 0–1 depth clip volume. A degenerate
/// plane (an infinite far plane) keeps everything.
pub(crate) fn planes(m: Mat4) -> [[f32; 4]; 6] {
    let [r0, r1, r2, r3] = [m.row(0), m.row(1), m.row(2), m.row(3)];
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].map(|p| {
        let length = p.truncate().length();
        if length > 1e-20 && p.is_finite() {
            (p / length).to_array()
        } else {
            [0., 0., 0., 1.]
        }
    })
}
