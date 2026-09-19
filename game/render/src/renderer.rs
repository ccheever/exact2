use crate::buffers::{bytes, Buffer, Targets};
use crate::pipeline::Pipelines;
use crate::{
    bloom::BloomTargets,
    frame,
    shadows::{Cascades, ShadowMaps},
    timing,
};
use crate::{Batch, FrameInput, MeshId, RenderError, Stats, Vertex};
use exact_gpu::wgpu;
use glam::Vec3;
use std::ops::Range;

pub(crate) const RETIRED_BUDGET: u64 = 64 * 1024 * 1024;

pub(crate) struct Mesh {
    pub(crate) indices: Range<u32>,
    pub(crate) vertex_bytes: u64,
    pub(crate) asset: bool,
    pub(crate) base_vertex: i32,
    center: Vec3,
}

/// Persistent GPU arenas, tick history and draw lists; model pipelines prepare on arrival.
/// Storage capacity is declared by `STORAGE_BINDINGS`; uses 4× MSAA HDR and WebGPU.
pub struct RendererWithAssets<const ASSETS: bool> {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) pipelines: Pipelines,
    pub(crate) models: crate::models::Models,
    pub(crate) quads: crate::quads::Quads,
    model_batches: Vec<Option<crate::MaterialId>>,
    slot_list: Vec<u32>,
    pub(crate) uniform: wgpu::Buffer,
    transforms: [Buffer; 2],
    attachment_matrices: Buffer,
    attachment_words: Vec<f32>,
    current: usize,
    materials: Buffer,
    slots: Buffer,
    scene_binds: [wgpu::BindGroup; 2],
    vertices: Buffer,
    indices: Buffer,
    pub(crate) meshes: Vec<Mesh>,
    pub(crate) mesh_uploads: u64,
    batches: Vec<Batch>,
    targets: Targets,
    counts: Stats,
    shadows: Option<ShadowMaps>,
    bloom: Option<BloomTargets>,
    texture_creations: u64,
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    fn slot_mirrored(&self, slot: u32, frame: &FrameInput<'_>) -> bool {
        let (entity, local, fallback) = if slot >= crate::RENDER_SLOT_BASE {
            let record = &self.models.records[(slot - crate::RENDER_SLOT_BASE) as usize];
            (record.transform, record.local.determinant(), {
                let pair = self.models.poses
                    [self.models.pose_indices[(slot - crate::RENDER_SLOT_BASE) as usize]];
                crate::world::scene::interpolate(pair, frame.alpha)
                    .scale
                    .element_product()
            })
        } else {
            (slot, 1., 1.)
        };
        let owner = frame
            .attachments
            .iter()
            .find(|a| a.entity.index() == entity)
            .map_or(fallback, |a| a.matrix.determinant());
        owner * local < 0.
    }
    // Keep compatible instances batched, splitting only where attachment winding differs.
    fn winding_ranges<'a>(
        &'a self,
        index: usize,
        frame: &'a FrameInput<'_>,
    ) -> impl Iterator<Item = (Range<u32>, bool)> + 'a {
        let mut at = self.batches[index].slots.start;
        let end = self.batches[index].slots.end;
        std::iter::from_fn(move || {
            if at == end {
                return None;
            }
            let start = at;
            let mirrored = self.slot_mirrored(self.slot_list[at as usize], frame);
            at += 1;
            if frame.attachments.is_empty() && self.model_batches[index].is_none() {
                at = end;
            } else {
                while at < end && self.slot_mirrored(self.slot_list[at as usize], frame) == mirrored
                {
                    at += 1;
                }
            }
            Some((start..at, mirrored))
        })
    }
    /// Compile effect pipelines for this output format and retain the device/queue.
    /// Draw targets must match this format; RGBA/BGRA unorm and sRGB are supported.
    /// Particle capacity prepares with emitters; asset arenas may compact retired spans.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let pipelines = Pipelines::new(device, format);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game frame"),
            size: (frame::FLOATS * 4) as u64,
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
        let attachment_matrices = Buffer::new(
            device,
            64,
            wgpu::BufferUsages::STORAGE,
            "game attachment matrices",
        );
        let scene_binds = scene_binds(
            device,
            &pipelines.scene_layout,
            &uniform,
            &transforms,
            &materials,
            &slots,
            &attachment_matrices,
        );
        let targets = Targets::new(device, (64, 64), &pipelines.tone_layout, &uniform);
        Self {
            models: crate::models::Models::default(),
            quads: crate::quads::Quads::new::<ASSETS>(device, queue, &uniform),
            model_batches: Vec::new(),
            slot_list: Vec::new(),
            device: device.clone(),
            queue: queue.clone(),
            pipelines,
            uniform,
            transforms,
            attachment_matrices,
            attachment_words: Vec::new(),
            current: 1,
            materials,
            slots,
            scene_binds,
            vertices: Buffer::new(device, 1024, wgpu::BufferUsages::VERTEX, "game vertices"),
            indices: Buffer::new(device, 1024, wgpu::BufferUsages::INDEX, "game indices"),
            meshes: Vec::new(),
            mesh_uploads: 0,
            batches: Vec::new(),
            targets,
            shadows: None,
            bloom: None,
            texture_creations: 3,
            counts: Stats {
                draws: 1,
                triangles: 1,
                ..Default::default()
            },
        }
    }

    /// Swap history roles. The caller makes every live slot current before drawing.
    /// Feed retains matching target pages instead of copying the previous buffer.
    pub fn begin_tick(&mut self) {
        let previous = self.current;
        self.current = 1 - self.current;
        self.transforms[self.current].live = self.transforms[previous].live;
    }

    /// Upload one contiguous run (ten floats per slot) into the current tick.
    /// Scale is positive; stray negative components draw as their absolute values.
    /// Mirroring belongs to a future double-sided material, not the scale sign.
    /// Exactly one queue write for a nonempty run. Panics on incomplete records;
    /// capacity refusals return the arena, requested slot and exclusive limit.
    pub fn write_transforms(&mut self, first_slot: u32, values: &[f32]) -> Result<(), RenderError> {
        let end = record_end(first_slot, values.len(), 10);
        if values.is_empty() {
            return Ok(());
        }
        self.check_capacity("transforms", end)?;
        self.ensure_slots(end);
        self.transforms[self.current].write(&self.queue, u64::from(first_slot) * 40, bytes(values));
        Ok(())
    }

    // Feed has already patched current via its coalesced run. Snap only history.
    pub(crate) fn write_previous_transforms(
        &mut self,
        first_slot: u32,
        values: &[f32],
    ) -> Result<(), RenderError> {
        let end = record_end(first_slot, values.len(), 10);
        if values.is_empty() {
            return Ok(());
        }
        self.check_capacity("transforms", end)?;
        self.ensure_slots(end);
        self.transforms[1 - self.current].write(
            &self.queue,
            u64::from(first_slot) * 40,
            bytes(values),
        );
        Ok(())
    }

    /// Initialize or teleport a run: write the same transforms into both ticks.
    /// Uses the positive-scale contract and capacity errors of [`Self::write_transforms`].
    pub fn write_transforms_both(
        &mut self,
        first_slot: u32,
        values: &[f32],
    ) -> Result<(), RenderError> {
        let end = record_end(first_slot, values.len(), 10);
        if values.is_empty() {
            return Ok(());
        }
        self.check_capacity("transforms", end)?;
        self.ensure_slots(end);
        for buffer in &mut self.transforms {
            buffer.write(&self.queue, u64::from(first_slot) * 40, bytes(values));
        }
        Ok(())
    }

    /// Upload twelve floats per slot: color RGBA, metallic, roughness, emissive RGB,
    /// then dimensions XYZ (capsules: diameter, half stem, diameter). One queue write.
    /// Negative base alpha enables the grid with spacing = -alpha; geometry is opaque.
    /// Capacity refusals return the arena, requested slot and exclusive limit.
    pub fn write_materials(&mut self, first_slot: u32, values: &[f32]) -> Result<(), RenderError> {
        let end = record_end(first_slot, values.len(), 12);
        if values.is_empty() {
            return Ok(());
        }
        self.check_capacity("materials", end)?;
        self.ensure_slots(end);
        self.materials
            .write(&self.queue, u64::from(first_slot) * 48, bytes(values));
        Ok(())
    }

    /// Replace the persistent draw list. It may be any visible subset of slots:
    /// future culling only needs to replace this list, not the transform arenas.
    /// Panics on invalid mesh IDs/ranges or slots beyond the written high-water
    /// marks. The caller initializes every referenced slot; sparse holes are not
    /// tracked on the CPU. Capacity refusals return an error before changing the list.
    pub fn set_batches(&mut self, batches: &[Batch], slot_list: &[u32]) -> Result<(), RenderError> {
        self.check_capacity("slots", slot_list.len() as u64)?;
        for &slot in slot_list {
            let slot = if ASSETS && slot >= crate::RENDER_SLOT_BASE {
                self.models
                    .records
                    .get((slot - crate::RENDER_SLOT_BASE) as usize)
                    .map_or(slot, |record| record.transform)
            } else {
                slot
            };
            let end = u64::from(slot) + 1;
            self.check_capacity("transforms", end)?;
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
        self.model_batches.clear();
        self.models.transparent.clear();
        for (index, batch) in batches.iter().enumerate() {
            let material = batch.slots.clone().next().and_then(|i| {
                let slot = slot_list[i as usize];
                (slot >= crate::RENDER_SLOT_BASE).then(|| {
                    self.models.records[(slot - crate::RENDER_SLOT_BASE) as usize].material
                })
            });
            if material.is_some_and(|m| {
                self.models.materials[m.0].alpha == exact_game::asset::AlphaMode::Blend
            }) {
                for slot in batch.slots.clone() {
                    self.models.transparent.push((index, slot, 0.0));
                }
            }
            self.model_batches.push(material);
        }
        self.slot_list.clear();
        self.slot_list.extend_from_slice(slot_list);
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
        Ok(())
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
        self.mesh_uploads += 1;
        let id = MeshId(
            self.meshes
                .iter()
                .position(|m| m.asset && m.vertex_bytes == 0)
                .unwrap_or(self.meshes.len()),
        );
        let mesh = Mesh {
            vertex_bytes: size_of_val(vertices) as u64,
            asset: false,
            indices: (index_start / 4) as u32..(index_end / 4) as u32,
            base_vertex: (vertex_start / 32) as i32,
            center,
        };
        if id.0 == self.meshes.len() {
            self.meshes.push(mesh);
        } else {
            self.meshes[id.0] = mesh;
        }
        id
    }

    pub(crate) fn mesh_buffer_bytes(&self) -> u64 {
        self.vertices.raw.size() + self.indices.raw.size()
    }
    pub(crate) fn pack_mesh_buffers(&mut self) {
        let vertices = self.meshes.iter().map(|m| m.vertex_bytes).sum::<u64>();
        let indices = self
            .meshes
            .iter()
            .map(|m| u64::from(m.indices.end - m.indices.start) * 4)
            .sum::<u64>();
        let mut v = Buffer::new(
            &self.device,
            vertices.max(32),
            wgpu::BufferUsages::VERTEX,
            "game packed vertices",
        );
        let mut i = Buffer::new(
            &self.device,
            indices.max(4),
            wgpu::BufferUsages::INDEX,
            "game packed indices",
        );
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let skinned: std::collections::BTreeSet<_> = self
            .models
            .loaded
            .values()
            .flat_map(|m| m.nodes.iter().filter(|n| n.3.is_some()).map(|n| n.0 .0))
            .collect();
        let weight_bytes = skinned
            .iter()
            .map(|&i| self.meshes[i].vertex_bytes)
            .sum::<u64>();
        let mut weights = self.models.skinning.as_ref().map(|_| {
            Buffer::new(
                &self.device,
                weight_bytes.max(64),
                wgpu::BufferUsages::STORAGE,
                "game packed weights",
            )
        });
        // Skin weights share vertex offsets. Pack skinned meshes first so live
        // unskinned geometry cannot leave arbitrarily large holes in that arena.
        let mut order: Vec<_> = (0..self.meshes.len()).collect();
        order.sort_by_key(|i| (!skinned.contains(i), *i));
        for index in order {
            let mesh = &mut self.meshes[index];
            if mesh.vertex_bytes == 0 {
                continue;
            }
            let n = u64::from(mesh.indices.end - mesh.indices.start) * 4;
            encoder.copy_buffer_to_buffer(
                &self.vertices.raw,
                mesh.base_vertex as u64 * 32,
                &v.raw,
                v.live,
                mesh.vertex_bytes,
            );
            encoder.copy_buffer_to_buffer(
                &self.indices.raw,
                u64::from(mesh.indices.start) * 4,
                &i.raw,
                i.live,
                n,
            );
            if let (Some(s), Some(target)) = (&self.models.skinning, &mut weights) {
                let start = mesh.base_vertex as u64 * 32;
                let bytes = mesh.vertex_bytes.min(s.weights.live.saturating_sub(start));
                if bytes > 0 && skinned.contains(&index) {
                    encoder.copy_buffer_to_buffer(
                        &s.weights.raw,
                        start,
                        &target.raw,
                        v.live,
                        bytes,
                    );
                    target.live = v.live + bytes;
                }
            }
            mesh.base_vertex = (v.live / 32) as i32;
            mesh.indices = (i.live / 4) as u32..((i.live + n) / 4) as u32;
            v.live += mesh.vertex_bytes;
            i.live += n;
        }
        self.queue.submit([encoder.finish()]);
        self.vertices = v;
        self.indices = i;
        if let Some(weights) = weights {
            self.models.skinning.as_mut().unwrap().weights = weights;
            self.models.reallocations += 1;
        }
    }

    /// Upload fixed-size frame data and submit the enabled passes.
    /// Zero dimensions become one. Attachments grow in 64-pixel buckets or change
    /// on effect toggles. Steady retained-scene draws allocate no renderer-owned
    /// collections; this excludes wgpu command encoding/staging.
    pub fn draw(
        &mut self,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
    ) -> Stats {
        self.draw_assets(target, size_px, frame)
    }
    pub(crate) fn draw_assets(
        &mut self,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
    ) -> Stats {
        for glow in frame.glows {
            self.write_materials(glow.slot, &glow.material_at(frame.seconds))
                .expect("feed validated glow material slot");
        }
        // Sparse affine overrides preserve joint shear without changing tick TRS arenas.
        let end = frame
            .attachments
            .iter()
            .map(|a| (u64::from(a.entity.index()) + 1) * 64)
            .max()
            .unwrap_or(0);
        if self
            .attachment_matrices
            .grow(&self.device, &self.queue, end)
        {
            self.rebind();
        }
        // Clear last frame's overrides in the same upload as this frame's poses.
        // Storage remains dense through the highest attached entity slot.
        let previous = self.attachment_words.len();
        self.attachment_words
            .resize(previous.max(end as usize / 4), 0.);
        self.attachment_words.fill(0.);
        for attachment in frame.attachments {
            let at = attachment.entity.index() as usize * 16;
            self.attachment_words[at..at + 16].copy_from_slice(&attachment.matrix.to_cols_array());
        }
        self.attachment_matrices
            .write(&self.queue, 0, bytes(&self.attachment_words));
        self.attachment_words.truncate(end as usize / 4);
        let device = &self.device;
        let queue = &self.queue;
        let size = (size_px.0.max(1), size_px.1.max(1));
        if size.0 > self.targets.size.0 || size.1 > self.targets.size.1 {
            let bucket = |n: u32| n.div_ceil(64) * 64;
            let capacity = (
                bucket(size.0).max(self.targets.size.0),
                bucket(size.1).max(self.targets.size.1),
            );
            self.bloom = None;
            self.targets =
                Targets::new(device, capacity, &self.pipelines.tone_layout, &self.uniform);
            self.texture_creations += 3;
        }
        let cascades = frame
            .sun
            .and_then(|s| s.shadows)
            .map(|s| Cascades::new(frame, s));
        if let Some(c) = &cascades {
            if self.shadows.as_ref().is_none_or(|s| s.count != c.count) {
                self.texture_creations += 1;
                self.shadows = Some(ShadowMaps::new(
                    device,
                    c.count,
                    &self.pipelines.shadow_layout,
                    &self.pipelines.camera_layout,
                ));
            }
            self.shadows.as_ref().unwrap().write(queue, c);
        } else {
            self.shadows = None;
        }
        if frame.environment.bloom.is_some() {
            if self.bloom.is_none() {
                self.bloom = Some(BloomTargets::new(
                    device,
                    self.targets.size,
                    &self.pipelines,
                    &self.uniform,
                    &self.targets.resolved,
                ));
                self.texture_creations += 6;
            }
        } else {
            self.bloom = None;
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytes(&frame::uniform(frame, cascades.as_ref(), size)),
        );
        self.quads.frame::<ASSETS>(frame);
        // One total translucent order; opaque/primitive batches remain retained.
        if ASSETS {
            for (_, slot, depth) in &mut self.models.transparent {
                let index = (self.slot_list[*slot as usize] - crate::RENDER_SLOT_BASE) as usize;
                let record = &self.models.records[index];
                let center = record
                    .local
                    .transform_point3(self.meshes[record.geometry.0].center);
                let history = self.models.poses[self.models.pose_indices[index]];
                let pose = frame
                    .attachments
                    .iter()
                    .find(|a| a.entity.index() == record.transform)
                    .map_or_else(
                        || {
                            let t = crate::world::scene::interpolate(history, frame.alpha);
                            glam::Mat4::from_scale_rotation_translation(
                                t.scale, t.rotation, t.position,
                            )
                        },
                        |a| a.matrix,
                    );
                let position = pose.transform_point3(center);
                *depth = -frame.view.transform_point3(position).z;
            }
            for &(index, slot, depth) in &self.models.transparent {
                let record = &self.models.records
                    [(self.slot_list[slot as usize] - crate::RENDER_SLOT_BASE) as usize];
                self.quads.order.push(crate::quads::Order {
                    kind: crate::quads::Kind::Model(index, slot),
                    depth,
                    layer: 0,
                    slot: record.transform,
                    index: slot as usize,
                });
            }
        }
        self.quads.order::<ASSETS>(device, queue);
        let mut encoder = device.create_command_encoder(&Default::default());
        if ASSETS {
            if let Some(skin) = &self.models.skinning {
                skin.encode(&mut encoder, frame.timestamps);
            }
        }
        let mut extra_draws = 0;
        if let Some(shadows) = &self.shadows {
            for i in 0..shadows.count as usize {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("game sun shadow"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &shadows.layers[i],
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: timing::writes(frame.timestamps, i as u32),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipelines.shadow[0]);
                pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                pass.set_bind_group(1, &shadows.cameras[i], &[]);
                pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
                pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
                for (index, batch) in self.batches.iter().enumerate() {
                    if batch.slots.is_empty() || !batch.casts_shadows {
                        continue;
                    }
                    for (slots, mirrored) in self.winding_ranges(index, frame) {
                        if let Some(material) = if ASSETS {
                            self.model_batches[index]
                        } else {
                            None
                        } {
                            let material = &self.models.materials[material.0];
                            if material.alpha == exact_game::asset::AlphaMode::Blend {
                                continue;
                            }
                            pass.set_pipeline(
                                self.pipelines.models.as_ref().unwrap().shadow[usize::from(
                                    material.double_sided,
                                ) + 2
                                    * usize::from(mirrored)]
                                .as_ref()
                                .unwrap(),
                            );
                            pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                            pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                        } else {
                            pass.set_pipeline(&self.pipelines.shadow[usize::from(mirrored)]);
                        }
                        let mesh = &self.meshes[batch.mesh.0];
                        pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slots);
                        extra_draws += 1;
                    }
                }
            }
        }
        {
            let sky = frame
                .environment
                .background
                .unwrap_or(frame.environment.horizon);
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
                timestamp_writes: timing::writes(frame.timestamps, 3),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            viewport(&mut pass, size);
            let variant = usize::from(self.shadows.is_some())
                + 2 * usize::from(frame.environment.fog.is_some());
            pass.set_pipeline(&self.pipelines.forward[variant]);
            if let Some(shadows) = &self.shadows {
                pass.set_bind_group(1, &shadows.sample, &[]);
            }
            pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
            pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
            pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
            for (index, batch) in self.batches.iter().enumerate() {
                if batch.slots.is_empty() {
                    continue;
                }
                for (slots, mirrored) in self.winding_ranges(index, frame) {
                    if let Some(material) = if ASSETS {
                        self.model_batches[index]
                    } else {
                        None
                    } {
                        let material = &self.models.materials[material.0];
                        if material.alpha == exact_game::asset::AlphaMode::Blend {
                            continue;
                        }
                        pass.set_pipeline(
                            self.pipelines.models.as_ref().unwrap().forward[variant
                                + 4 * usize::from(material.double_sided)
                                + 16 * usize::from(mirrored)]
                            .as_ref()
                            .unwrap(),
                        );
                        pass.set_bind_group(
                            1,
                            self.shadows.as_ref().map_or_else(
                                || self.models.no_shadow.as_ref().unwrap(),
                                |s| &s.sample,
                            ),
                            &[],
                        );
                        pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                        pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                    } else {
                        pass.set_pipeline(
                            &self.pipelines.forward[variant + 4 * usize::from(mirrored)],
                        );
                    }
                    let mesh = &self.meshes[batch.mesh.0];
                    if slots.start != batch.slots.start {
                        extra_draws += 1;
                    }
                    pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slots);
                }
            }
            for draw in self.quads.draws.iter().filter(|d| self.quads.opaque(d)) {
                self.quads.draw::<ASSETS>(&mut pass, draw);
                extra_draws += 1;
            }
            if frame::has_sky(frame) {
                pass.set_pipeline(&self.pipelines.sky);
                pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                pass.draw(0..3, 0..1);
                extra_draws += 1;
            }
            for draw in self.quads.draws.iter().filter(|d| !self.quads.opaque(d)) {
                if let crate::quads::Kind::Model(index, slot) = draw.kind {
                    assert!(ASSETS, "model in primitive executor");
                    let batch = &self.batches[index];
                    let material = &self.models.materials[self.model_batches[index].unwrap().0];
                    pass.set_pipeline(
                        self.pipelines.models.as_ref().unwrap().forward[variant
                            + 4 * usize::from(material.double_sided)
                            + 8
                            + 16 * usize::from(
                                self.slot_mirrored(self.slot_list[slot as usize], frame),
                            )]
                        .as_ref()
                        .unwrap(),
                    );
                    pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                    pass.set_bind_group(
                        1,
                        self.shadows
                            .as_ref()
                            .map_or_else(|| self.models.no_shadow.as_ref().unwrap(), |s| &s.sample),
                        &[],
                    );
                    pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                    pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                    pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
                    let mesh = &self.meshes[batch.mesh.0];
                    pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slot..slot + 1);
                } else {
                    self.quads.draw::<ASSETS>(&mut pass, draw);
                }
                extra_draws += 1;
            }
        }
        if let Some(bloom) = &self.bloom {
            extra_draws +=
                bloom.encode(queue, &mut encoder, &self.pipelines, frame.timestamps, size);
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
                timestamp_writes: timing::writes(frame.timestamps, 15),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            viewport(&mut pass, size);
            pass.set_pipeline(&self.pipelines.tone[usize::from(self.bloom.is_some())]);
            if let Some(bloom) = &self.bloom {
                pass.set_bind_group(1, &bloom.binds[0], &[0]);
            }
            pass.set_bind_group(0, &self.targets.tone_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        let mut stats = self.counts;
        stats.draws += extra_draws;
        stats.instances += self.quads.instances();
        stats.triangles += 2 * self.quads.instances();
        stats.draws -= self
            .model_batches
            .iter()
            .flatten()
            .filter(|m| self.models.materials[m.0].alpha == exact_game::asset::AlphaMode::Blend)
            .count() as u32;
        stats.texture_creations = self.texture_creations;
        stats
    }

    /// Maximum slot count under the device's granted adapter limits and the widest
    /// arena (64-byte affine attachments). Valid slot indices are strictly below this count.
    pub fn max_slots(&self) -> u32 {
        (self
            .device
            .limits()
            .max_storage_buffer_binding_size
            .min(self.device.limits().max_buffer_size)
            / 64)
            .min(u64::from(u32::MAX)) as u32
    }

    fn check_capacity(&self, arena: &'static str, end: u64) -> Result<(), RenderError> {
        let limit = u64::from(self.max_slots());
        if end > limit {
            Err(RenderError::Capacity {
                arena,
                slot: end - 1,
                limit,
            })
        } else {
            Ok(())
        }
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
            &self.attachment_matrices,
        );
    }
}

fn record_end(first: u32, len: usize, stride: usize) -> u64 {
    assert!(len.is_multiple_of(stride), "incomplete slot record");
    u64::from(first) + (len / stride) as u64
}

fn scene_binds(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    transforms: &[Buffer; 2],
    materials: &Buffer,
    slots: &Buffer,
    attachments: &Buffer,
) -> [wgpu::BindGroup; 2] {
    std::array::from_fn(|current| {
        let buffers = [
            uniform,
            &transforms[1 - current].raw,
            &transforms[current].raw,
            &materials.raw,
            &slots.raw,
            &attachments.raw,
        ];
        let entries: [_; 6] = std::array::from_fn(|i| wgpu::BindGroupEntry {
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

pub(crate) fn viewport(pass: &mut wgpu::RenderPass<'_>, size: (u32, u32)) {
    pass.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32, 0.0, 1.0);
    pass.set_scissor_rect(0, 0, size.0, size.1);
}

#[cfg(test)]
mod packing_tests {
    use super::*;
    #[test]
    fn oversized_retired_arenas_pack_without_reuploading_live_meshes() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut renderer = RendererWithAssets::<true>::new(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let model =
            exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
                .unwrap();
        renderer.prepare_model("hero.model", &model).unwrap();
        renderer.prepare_model("retired.model", &model).unwrap();
        renderer
            .models
            .loaded
            .get_mut("retired.model")
            .unwrap()
            .active = false;
        let live =
            std::collections::BTreeSet::from(["hero.model".to_owned(), "retired.model".to_owned()]);
        let handles = renderer.models.loaded["hero.model"].nodes.clone();
        let uploads = renderer.mesh_uploads;
        // Actual GPU capacity, including unused tails, must be charged and trimmed.
        renderer
            .vertices
            .grow(&gpu.device, &gpu.queue, 65 * 1024 * 1024);
        assert!(renderer.retired_bytes(&live) > crate::renderer::RETIRED_BUDGET);
        renderer.compact_assets(&live, &Default::default());
        assert!(renderer.retired_bytes(&live) <= crate::renderer::RETIRED_BUDGET);
        assert_eq!(renderer.mesh_uploads, uploads);
        assert_eq!(renderer.models.loaded["hero.model"].nodes, handles);
        assert!(renderer.meshes[handles[0].0 .0].vertex_bytes > 0);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod e10_tests {
    use exact_game::*;
    use exact_gpu::{fixture, Frame, Surface};
    struct Beacon;
    impl Game for Beacon {
        const ID: &'static str = "e10-glow-pixels";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.insert_resource(Environment {
                background: Some([0.; 3]),
                fog: None,
                ..Default::default()
            });
            w.spawn((Transform::at(0., 0., 5.), Camera::orthographic(4.)));
            let mut glow = Glow::default();
            glow.0.to(w.now(), 1., 0.5);
            w.spawn((
                Transform::default(),
                Mesh::sphere(0.5),
                Material::glow([16.; 3]),
                glow,
            ));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    #[test]
    fn full_glow_blooms_without_clipping_the_lit_pixel_to_white() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut surface = crate::WorldSurface::<Beacon>::default();
        surface.bind(&[], None).unwrap();
        let mut frame = Frame {
            width: 64.,
            height: 64.,
            scale: 1.,
            now_ms: 0.,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
        };
        let dark = fixture::render(&gpu, &mut surface, &frame)
            .unwrap()
            .0
            .at(32, 32);
        frame.now_ms = 500.;
        let lit = fixture::render(&gpu, &mut surface, &frame)
            .unwrap()
            .0
            .at(32, 32);
        assert!(
            lit[0] > dark[0] + 100,
            "tween must light the beacon: {dark:?} -> {lit:?}"
        );
        assert_ne!(
            &lit[..3],
            &[255, 255, 255],
            "full glow retains highlight headroom"
        );
    }
}

#[cfg(test)]
mod attachment_upload_tests {
    use super::*;
    #[test]
    fn one_dense_attachment_upload_clears_removed_overrides_on_next_frame() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut r = crate::Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
        let mut w = exact_game::World::new(60, 0);
        let a = w.spawn(exact_game::Transform::default());
        let b = w.spawn(exact_game::Transform::default());
        let matrix = glam::Mat4::from_translation(glam::Vec3::X);
        let attachments = [a, b].map(|entity| crate::DisplayedAttachment {
            entity,
            matrix,
            pose: Default::default(),
        });
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        let mut frame = FrameInput {
            attachments: &attachments,
            sun: None,
            ..Default::default()
        };
        frame.environment.bloom = None;
        r.draw(&target, (16, 16), &frame);
        let size = (u64::from(b.index()) + 1) * 64;
        let read = |r: &crate::Renderer| {
            crate::skinning::tests::read(&gpu, &r.attachment_matrices.raw, size)
        };
        let before = read(&r);
        assert_eq!(
            &before[a.index() as usize * 64..(a.index() as usize + 1) * 64],
            bytes(&matrix.to_cols_array())
        );
        frame.attachments = &attachments[..1];
        r.draw(&target, (16, 16), &frame);
        let after = read(&r);
        assert_eq!(
            &after[a.index() as usize * 64..(a.index() as usize + 1) * 64],
            bytes(&matrix.to_cols_array())
        );
        assert!(after[b.index() as usize * 64..].iter().all(|b| *b == 0));
    }
}
