use crate::buffers::{bytes, Buffer, Targets};
use crate::pipeline::Pipelines;
use crate::{Batch, FrameInput, MeshId, Stats, Vertex};
use exact_gpu::wgpu;
use glam::Vec3;
use std::ops::Range;

struct Mesh {
    indices: Range<u32>,
    base_vertex: i32,
    center: Vec3,
    radius: f32,
}

/// Persistent GPU arenas, tick history, draw lists and eagerly compiled pipelines.
/// Uses four storage bindings and 4× MSAA HDR; requires WebGPU (not WebGL).
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    pipelines: Pipelines,
    uniform: wgpu::Buffer,
    transforms: [Buffer; 2],
    current: usize,
    materials: Buffer,
    slots: Buffer,
    scene_binds: [wgpu::BindGroup; 2],
    vertices: Buffer,
    indices: Buffer,
    meshes: Vec<Mesh>,
    batches: Vec<Batch>,
    targets: Targets,
    counts: Stats,
}

impl Renderer {
    /// Compile both pipelines for this output format. `draw` must use the same
    /// device, queue and format. RGBA/BGRA unorm and sRGB targets are supported.
    /// Starts small; all arenas grow on demand and never shrink.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let pipelines = Pipelines::new(device, format);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game frame"),
            size: 164 * 4,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let transforms = [
            Buffer::new(
                device,
                16 * 40,
                wgpu::BufferUsages::STORAGE,
                "game transforms a",
            ),
            Buffer::new(
                device,
                16 * 40,
                wgpu::BufferUsages::STORAGE,
                "game transforms b",
            ),
        ];
        let materials = Buffer::new(
            device,
            16 * 48,
            wgpu::BufferUsages::STORAGE,
            "game materials",
        );
        let slots = Buffer::new(device, 64, wgpu::BufferUsages::STORAGE, "game slots");
        let scene_binds = scene_binds(
            device,
            &pipelines.scene_layout,
            &uniform,
            &transforms,
            &materials,
            &slots,
        );
        let targets = Targets::new(device, (1, 1), &pipelines.tone_layout, &uniform);
        Self {
            device: device.clone(),
            queue: queue.clone(),
            format,
            pipelines,
            uniform,
            transforms,
            current: 1,
            materials,
            slots,
            scene_binds,
            vertices: Buffer::new(device, 1024, wgpu::BufferUsages::VERTEX, "game vertices"),
            indices: Buffer::new(device, 1024, wgpu::BufferUsages::INDEX, "game indices"),
            meshes: Vec::new(),
            batches: Vec::new(),
            targets,
            counts: Stats {
                draws: 1,
                triangles: 1,
                ..Default::default()
            },
        }
    }

    /// Advance history, then copy the previous tick into the new current buffer.
    /// Untouched slots therefore stay still. One GPU copy of the written high-water
    /// range, submitted before subsequent write calls for this tick.
    pub fn begin_tick(&mut self) {
        let previous = self.current;
        self.current = 1 - self.current;
        let size = self.transforms[previous].live;
        if size != 0 {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(
                &self.transforms[previous].raw,
                0,
                &self.transforms[self.current].raw,
                0,
                size,
            );
            self.queue.submit([encoder.finish()]);
        }
        self.transforms[self.current].live = size;
    }

    /// Upload one contiguous run (ten floats per slot) into the current tick.
    /// Exactly one queue write for a nonempty run. Panics on incomplete records
    /// or a range larger than the device's storage binding limit.
    pub fn write_transforms(&mut self, first_slot: u32, values: &[f32]) {
        let end = record_end(first_slot, values.len(), 10);
        if values.is_empty() {
            return;
        }
        self.ensure_slots(end);
        self.transforms[self.current].write(&self.queue, u64::from(first_slot) * 40, bytes(values));
    }

    /// Initialize or teleport a run: write the same transforms into both ticks.
    pub fn write_transforms_both(&mut self, first_slot: u32, values: &[f32]) {
        let end = record_end(first_slot, values.len(), 10);
        if values.is_empty() {
            return;
        }
        self.ensure_slots(end);
        for buffer in &mut self.transforms {
            buffer.write(&self.queue, u64::from(first_slot) * 40, bytes(values));
        }
    }

    /// Upload contiguous material records (twelve floats per slot), one queue write.
    /// Base alpha and the final three floats are reserved; all geometry is opaque.
    pub fn write_materials(&mut self, first_slot: u32, values: &[f32]) {
        let end = record_end(first_slot, values.len(), 12);
        if values.is_empty() {
            return;
        }
        self.ensure_slots(end);
        self.materials
            .write(&self.queue, u64::from(first_slot) * 48, bytes(values));
    }

    /// Replace the persistent draw list. It may be any visible subset of slots:
    /// future culling only needs to replace this list, not the transform arenas.
    /// Panics on invalid mesh IDs/ranges or slots beyond the written high-water
    /// marks. The caller initializes every referenced slot; sparse holes are not
    /// tracked on the CPU.
    pub fn set_batches(&mut self, batches: &[Batch], slot_list: &[u32]) {
        assert!(slot_list.len() <= u32::MAX as usize);
        for &slot in slot_list {
            let end = u64::from(slot) + 1;
            assert!(
                self.transforms.iter().all(|buffer| end * 40 <= buffer.live)
                    && end * 48 <= self.materials.live,
                "slot has not been initialized"
            );
        }
        let mut counts = Stats {
            draws: 1,
            triangles: 1,
            ..Default::default()
        };
        for batch in batches {
            let mesh = &self.meshes[batch.mesh.0];
            assert!(
                batch.slots.start <= batch.slots.end && batch.slots.end as usize <= slot_list.len()
            );
            let instances = u64::from(batch.slots.end - batch.slots.start);
            if instances != 0 {
                counts.draws += 1;
                counts.instances += instances;
                counts.triangles +=
                    instances * u64::from(mesh.indices.end - mesh.indices.start) / 3;
            }
        }
        if self
            .slots
            .grow(&self.device, &self.queue, size_of_val(slot_list) as u64)
        {
            self.rebind();
        }
        self.slots.write(&self.queue, 0, bytes(slot_list));
        self.batches.clear();
        self.batches.extend_from_slice(batches);
        self.counts = counts;
    }

    /// Append a triangle mesh to the shared vertex/index arenas. Indices are local
    /// to this mesh; winding is counterclockwise. Panics on empty/invalid geometry.
    pub fn add_mesh(&mut self, vertices: &[Vertex], indices: &[u32]) -> MeshId {
        assert!(!vertices.is_empty() && !indices.is_empty() && indices.len().is_multiple_of(3));
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
        let vertex_start = self.vertices.live;
        let index_start = self.indices.live;
        let vertex_end = vertex_start + size_of_val(vertices) as u64;
        let index_end = index_start + size_of_val(indices) as u64;
        assert!(vertex_end / 32 <= i32::MAX as u64 && index_end / 4 <= u64::from(u32::MAX));
        self.vertices.grow(&self.device, &self.queue, vertex_end);
        self.indices.grow(&self.device, &self.queue, index_end);
        self.vertices
            .write(&self.queue, vertex_start, bytes(vertices));
        self.indices.write(&self.queue, index_start, bytes(indices));
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for vertex in vertices {
            let p = Vec3::from_array(vertex.position);
            assert!(p.is_finite());
            low = low.min(p);
            high = high.max(p);
        }
        let center = (low + high) * 0.5;
        let radius = vertices
            .iter()
            .map(|v| Vec3::from_array(v.position).distance(center))
            .fold(0.0, f32::max);
        let id = MeshId(self.meshes.len());
        self.meshes.push(Mesh {
            indices: (index_start / 4) as u32..(index_end / 4) as u32,
            base_vertex: (vertex_start / 32) as i32,
            center,
            radius,
        });
        id
    }

    /// Local-space bounding sphere (AABB center and maximum vertex distance).
    /// Retained independently of the visible list for a future culling caller.
    pub fn mesh_bounds(&self, mesh: MeshId) -> (Vec3, f32) {
        let mesh = &self.meshes[mesh.0];
        (mesh.center, mesh.radius)
    }

    /// Upload one frame uniform and submit the forward/MSAA and ACES tonemap passes.
    /// Zero dimensions become one. Attachments only change on resize; this method
    /// allocates no CPU collections in steady state (wgpu manages its own encoding).
    /// Panics if `format` differs from the construction format.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
    ) -> Stats {
        #[cfg(not(target_arch = "wasm32"))]
        let start = std::time::Instant::now();
        assert_eq!(
            format, self.format,
            "output format is fixed at Renderer::new"
        );
        let size = (size_px.0.max(1), size_px.1.max(1));
        if self.targets.size != size {
            self.targets = Targets::new(device, size, &self.pipelines.tone_layout, &self.uniform);
        }
        queue.write_buffer(&self.uniform, 0, bytes(&frame_uniform(frame)));
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let sky = frame.environment.sky;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game forward"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.color,
                    resolve_target: Some(&self.targets.resolved),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky[0] as f64,
                            g: sky[1] as f64,
                            b: sky[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.forward);
            pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
            pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
            pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
            for batch in &self.batches {
                if batch.slots.is_empty() {
                    continue;
                }
                let mesh = &self.meshes[batch.mesh.0];
                pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, batch.slots.clone());
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game ACES"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.tone);
            pass.set_bind_group(0, &self.targets.tone_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        let mut stats = self.counts;
        #[cfg(not(target_arch = "wasm32"))]
        {
            stats.encode_us = start.elapsed().as_secs_f64() * 1_000_000.0;
        }
        #[cfg(target_arch = "wasm32")]
        {
            stats.encode_us = 0.0;
        }
        stats
    }

    fn ensure_slots(&mut self, end: u64) {
        let changed = self.transforms[0].grow(&self.device, &self.queue, end * 40)
            | self.transforms[1].grow(&self.device, &self.queue, end * 40)
            | self.materials.grow(&self.device, &self.queue, end * 48);
        if changed {
            self.rebind();
        }
    }

    fn rebind(&mut self) {
        self.scene_binds = scene_binds(
            &self.device,
            &self.pipelines.scene_layout,
            &self.uniform,
            &self.transforms,
            &self.materials,
            &self.slots,
        );
    }
}

fn record_end(first: u32, len: usize, stride: usize) -> u64 {
    assert!(len.is_multiple_of(stride), "incomplete slot record");
    let end = u64::from(first) + (len / stride) as u64;
    assert!(end <= u64::from(u32::MAX) + 1, "slot range overflow");
    end
}

fn scene_binds(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    transforms: &[Buffer; 2],
    materials: &Buffer,
    slots: &Buffer,
) -> [wgpu::BindGroup; 2] {
    std::array::from_fn(|current| {
        let buffers = [
            uniform,
            &transforms[1 - current].raw,
            &transforms[current].raw,
            &materials.raw,
            &slots.raw,
        ];
        let entries: [_; 5] = std::array::from_fn(|i| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: buffers[i].as_entire_binding(),
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game scene"),
            layout,
            entries: &entries,
        })
    })
}

fn frame_uniform(frame: &FrameInput<'_>) -> [f32; 164] {
    let mut data = [0.0; 164];
    data[..16].copy_from_slice(&(frame.proj * frame.view).to_cols_array());
    data[16..19].copy_from_slice(&frame.camera_position.to_array());
    data[19] = frame.alpha.clamp(0.0, 1.0);
    if let Some(sun) = frame.sun {
        data[20..23].copy_from_slice(&sun.direction.to_array());
        data[23] = sun.illuminance;
        data[24..27].copy_from_slice(&sun.color.to_array());
    }
    data[27] = frame.points.len().min(16) as f32;
    data[28..31].copy_from_slice(&frame.environment.sky);
    data[31] = frame.environment.ambient;
    data[32..35].copy_from_slice(&frame.environment.ground);
    data[35] = frame.exposure;
    for (point, out) in frame
        .points
        .iter()
        .take(16)
        .zip(data[36..].chunks_exact_mut(8))
    {
        out[..3].copy_from_slice(&point.position.to_array());
        out[3] = point.range;
        out[4..7].copy_from_slice(&point.color.to_array());
        out[7] = point.intensity;
    }
    data
}
