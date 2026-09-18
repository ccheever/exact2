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

pub(crate) struct Mesh {
    pub(crate) indices: Range<u32>,
    pub(crate) vertex_bytes: u64,
    pub(crate) asset: bool,
    pub(crate) base_vertex: i32,
    center: Vec3,
}

/// Persistent GPU arenas, tick history and draw lists; model pipelines prepare on arrival.
/// Uses four storage bindings and 4× MSAA HDR; requires WebGPU (not WebGL).
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
    fn model_mirrored(&self, index: usize) -> bool {
        let slot = self.slot_list[self.batches[index].slots.start as usize];
        self.models.records[(slot - crate::RENDER_SLOT_BASE) as usize]
            .local
            .determinant()
            < 0.
    }
    /// Compile effect pipelines for this output format and retain the device/queue.
    /// Draw targets must match this format; RGBA/BGRA unorm and sRGB are supported.
    /// Reserves quad capacity up front; asset arenas may compact retired spans.
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
        let scene_binds = scene_binds(
            device,
            &pipelines.scene_layout,
            &uniform,
            &transforms,
            &materials,
            &slots,
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
        // Attachments are frame dependencies, never interpolated composed endpoints.
        // Restore these ordinary tick arenas after submission so detaching cannot
        // leave a stale displayed transform behind or poison page-write filtering.
        for attachment in frame.attachments {
            let values = crate::world::floats(attachment.pose);
            for buffer in &self.transforms {
                self.queue.write_buffer(
                    &buffer.raw,
                    u64::from(attachment.entity.index()) * 40,
                    bytes(&values),
                );
            }
        }
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
        self.quads.frame::<ASSETS>(device, queue, frame);
        // One total translucent order; opaque/primitive batches remain retained.
        if ASSETS {
            for (_, slot, depth) in &mut self.models.transparent {
                let index = (self.slot_list[*slot as usize] - crate::RENDER_SLOT_BASE) as usize;
                let record = &self.models.records[index];
                let center = record
                    .local
                    .transform_point3(self.meshes[record.geometry.0].center);
                let history = self.models.poses[index];
                let pose = frame
                    .attachments
                    .iter()
                    .find(|a| a.entity.index() == record.transform)
                    .map_or_else(
                        || crate::world::scene::interpolate(history, frame.alpha),
                        |a| a.pose,
                    );
                let position = pose.position + pose.rotation * (pose.scale * center);
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
        self.quads.order(device, queue);
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
                pass.set_pipeline(&self.pipelines.shadow);
                pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                pass.set_bind_group(1, &shadows.cameras[i], &[]);
                pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
                pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
                for (index, batch) in self.batches.iter().enumerate() {
                    if batch.slots.is_empty() || !batch.casts_shadows {
                        continue;
                    }
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
                            ) + 2 * usize::from(
                                self.model_mirrored(index),
                            )]
                            .as_ref()
                            .unwrap(),
                        );
                        pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                        pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                    } else {
                        pass.set_pipeline(&self.pipelines.shadow);
                    }
                    let mesh = &self.meshes[batch.mesh.0];
                    pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, batch.slots.clone());
                    extra_draws += 1;
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
                            + 16 * usize::from(self.model_mirrored(index))]
                        .as_ref()
                        .unwrap(),
                    );
                    pass.set_bind_group(
                        1,
                        self.shadows
                            .as_ref()
                            .map_or_else(|| self.models.no_shadow.as_ref().unwrap(), |s| &s.sample),
                        &[],
                    );
                    pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                    pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                } else {
                    pass.set_pipeline(&self.pipelines.forward[variant]);
                }
                let mesh = &self.meshes[batch.mesh.0];
                pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, batch.slots.clone());
            }
            for draw in self.quads.draws.iter().filter(|d| self.quads.opaque(d)) {
                self.quads.draw(&mut pass, draw);
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
                    let batch = &self.batches[index];
                    let material = &self.models.materials[self.model_batches[index].unwrap().0];
                    pass.set_pipeline(
                        self.pipelines.models.as_ref().unwrap().forward[variant
                            + 4 * usize::from(material.double_sided)
                            + 8
                            + 16 * usize::from(self.model_mirrored(index))]
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
                    self.quads.draw(&mut pass, draw);
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
        for attachment in frame.attachments {
            for (buffer, pose) in [
                (&self.transforms[1 - self.current], attachment.history[0]),
                (&self.transforms[self.current], attachment.history[1]),
            ] {
                queue.write_buffer(
                    &buffer.raw,
                    u64::from(attachment.entity.index()) * 40,
                    bytes(&crate::world::floats(pose)),
                );
            }
        }
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
    /// arena (48-byte materials). Valid slot indices are strictly below this count.
    pub fn max_slots(&self) -> u32 {
        (self
            .device
            .limits()
            .max_storage_buffer_binding_size
            .min(self.device.limits().max_buffer_size)
            / 48)
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

pub(crate) fn viewport(pass: &mut wgpu::RenderPass<'_>, size: (u32, u32)) {
    pass.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32, 0.0, 1.0);
    pass.set_scissor_rect(0, 0, size.0, size.1);
}

#[cfg(test)]
mod packing_tests {
    use super::*;
    #[test]
    fn oversized_retired_arenas_pack_without_reuploading_live_meshes() {
        let gpu = exact_gpu::fixture::device().unwrap();
        let mut renderer = RendererWithAssets::<true>::new(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let model = exact_game::bin::from_slice(include_bytes!(
            "../../games/asset-fixture/assets/crate.model"
        ))
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
        assert!(renderer.retired_bytes(&live) > 64 * 1024 * 1024);
        renderer.compact_assets(wgpu::TextureFormat::Rgba8Unorm, &live);
        assert!(renderer.retired_bytes(&live) <= 64 * 1024 * 1024);
        assert_eq!(renderer.mesh_uploads, uploads);
        assert_eq!(renderer.models.loaded["hero.model"].nodes, handles);
        assert!(renderer.meshes[handles[0].0 .0].vertex_bytes > 0);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
}
