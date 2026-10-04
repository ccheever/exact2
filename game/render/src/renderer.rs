mod culling;
mod draw;
mod local;
mod passes;
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
    /// Position box half extents around `center`.
    half: Vec3,
    /// Primitive bound: the largest |position| per axis and |capsule offset sign|,
    /// and whether any vertex takes the capsule (uniform) or per-axis scale.
    reach: Vec3,
    cap: f32,
    capsule: bool,
    plain: bool,
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
    pub(crate) slot_list: Vec<u32>,
    pub(crate) uniform: wgpu::Buffer,
    transforms: [Buffer; 2],
    attachment_matrices: Buffer,
    attachment_words: Vec<f32>,
    current: usize,
    materials: Buffer,
    slots: Buffer,
    scene_binds: [wgpu::BindGroup; 2],
    /// The same scene group over the culled lists: binding 4 is a dynamic window.
    culled_binds: [wgpu::BindGroup; 2],
    culled_key: (wgpu::Buffer, u64),
    pub(crate) cull: crate::cull::Cull,
    pub(crate) environment: crate::ibl::EnvironmentLight,
    pub(crate) lights: crate::lights::Lights,
    /// Timestamp pairs this frame wrote (`timing::GPU_PASS_NAMES` bits), so a
    /// readback never reports a pass that did not run.
    pub(crate) timed: std::cell::Cell<u64>,
    pub(crate) local: Option<crate::local_shadows::LocalMaps>,
    pub(crate) local_plan: crate::local_shadows::Plan,
    pub(crate) local_culls: Vec<local::LocalCull>,
    /// Group 1 of forward passes; rebuilt when a shadow texture is replaced.
    shadow_sample: wgpu::BindGroup,
    shadow_sample_stale: bool,
    shadow_placeholder: wgpu::TextureView,
    shadow_comparison: wgpu::Sampler,
    vertices: Buffer,
    indices: Buffer,
    pub(crate) meshes: Vec<Mesh>,
    pub(crate) mesh_uploads: u64,
    batches: Vec<Batch>,
    /// A drawn batch is in the viewmodel layer this frame.
    viewmodels: bool,
    targets: Targets,
    counts: Stats,
    shadows: Option<ShadowMaps>,
    bloom: Option<BloomTargets>,
    ssao: Option<crate::ssao::Ssao>,
    texture_creations: u64,
    hook_binding: Option<crate::hooks::FrameBinding>,
    custom_bindings: Option<crate::hooks::MaterialBindings>,
    hook_targets: Option<crate::hooks::HookTargets>,
    pub(crate) hook_metrics: Option<Box<crate::hooks::metrics::Metrics>>,
}

// Use the uploaded affine records and the same presence test as transform.wgsl.
fn attachment_matrix(words: &[f32], slot: u32) -> Option<glam::Mat4> {
    let values = words.chunks_exact(16).nth(slot as usize)?;
    (values[15] == 1.).then(|| glam::Mat4::from_cols_slice(values))
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    fn slot_mirrored(&self, slot: u32, frame: &FrameInput<'_>) -> bool {
        let (entity, local, fallback) = if ASSETS && slot >= crate::RENDER_SLOT_BASE {
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
        let owner =
            attachment_matrix(&self.attachment_words, entity).map_or(fallback, |m| m.determinant());
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
            // A model batch shares its nodes' parity; only owners with a negative
            // scale axis or an attachment can differ within it.
            if frame.attachments.is_empty()
                && (!ASSETS
                    || self.model_batches[index].is_none()
                    || !self.models.any_mirrored_owner())
            {
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
        let environment = crate::ibl::EnvironmentLight::new(device, false);
        let lights = crate::lights::Lights::new(device);
        let retained_binds = scene_binds(
            device,
            &pipelines.scene_layout,
            &uniform,
            &transforms,
            &materials,
            (&slots.raw, None),
            &attachment_matrices,
            &environment,
            &lights,
        );
        let cull = crate::cull::Cull::new(device);
        let culled_key = (cull.compacted.raw.clone(), cull.window);
        let culled_binds = scene_binds(
            device,
            &pipelines.scene_layout,
            &uniform,
            &transforms,
            &materials,
            (&cull.compacted.raw, Some(cull.window)),
            &attachment_matrices,
            &environment,
            &lights,
        );
        let targets = Targets::new(device, (64, 64), &pipelines.tone_layout, &uniform);
        let shadow_placeholder = crate::local_shadows::placeholder(device);
        let shadow_comparison = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_sample = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game shadow sample"),
            layout: &pipelines.shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_placeholder),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_comparison),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&shadow_placeholder),
                },
            ],
        });
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
            scene_binds: retained_binds,
            culled_binds,
            culled_key,
            cull,
            environment,
            lights,
            timed: std::cell::Cell::new(0),
            local: None,
            local_plan: Default::default(),
            local_culls: Vec::new(),
            shadow_sample,
            shadow_sample_stale: false,
            shadow_placeholder,
            shadow_comparison,
            vertices: Buffer::new(device, 1024, wgpu::BufferUsages::VERTEX, "game vertices"),
            indices: Buffer::new(device, 1024, wgpu::BufferUsages::INDEX, "game indices"),
            meshes: Vec::new(),
            mesh_uploads: 0,
            batches: Vec::new(),
            viewmodels: false,
            targets,
            shadows: None,
            bloom: None,
            ssao: None,
            texture_creations: 4,
            hook_binding: None,
            custom_bindings: None,
            hook_targets: None,
            hook_metrics: None,
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
        // Slots past the history's high-water mark have never had a previous
        // pose: an entity spawned in an earlier tick of a multi-tick advance
        // is no longer `fresh` when the presentation feeds. It starts with its
        // current pose as history, as a fresh one does.
        let history = &mut self.transforms[1 - self.current];
        let known = history.live / 40;
        if end > known {
            let from = known.max(u64::from(first_slot));
            let skip = (from - u64::from(first_slot)) as usize * 10;
            history.write(&self.queue, from * 40, bytes(&values[skip..]));
        }
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

    /// Replace the persistent draw list. Each frame the GPU culls it per view
    /// (camera, sun cascades), preserving list order within each drawn range.
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
        if ASSETS {
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
        self.cull.epoch += 1;
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
        assert!(
            vertex_end / size_of::<Vertex>() as u64 <= i32::MAX as u64
                && index_end / 4 <= u64::from(u32::MAX)
        );
        self.vertices.grow(&self.device, &self.queue, vertex_end);
        self.indices.grow(&self.device, &self.queue, index_end);
        self.vertices
            .write(&self.queue, vertex_start, bytes(vertices));
        self.indices.write(&self.queue, index_start, bytes(indices));
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        let (mut reach, mut cap, mut capsule, mut plain) = (Vec3::ZERO, 0f32, false, false);
        for vertex in vertices {
            let p = Vec3::from_array(vertex.position);
            assert!(p.is_finite());
            low = low.min(p);
            high = high.max(p);
            reach = reach.max(p.abs());
            cap = cap.max(vertex.uv[0].abs());
            capsule |= vertex.uv[1] == 1.0;
            plain |= vertex.uv[1] != 1.0;
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
            base_vertex: (vertex_start / size_of::<Vertex>() as u64) as i32,
            center,
            half: (high - low) * 0.5,
            reach,
            cap,
            capsule,
            plain,
        };
        self.cull.epoch += 1;
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
                mesh.base_vertex as u64 * size_of::<Vertex>() as u64,
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
                let bytes = (mesh.vertex_bytes / size_of::<Vertex>() as u64 * 32)
                    .min(s.weights.live.saturating_sub(start));
                let target_start = v.live / size_of::<Vertex>() as u64 * 32;
                if bytes > 0 && skinned.contains(&index) {
                    encoder.copy_buffer_to_buffer(
                        &s.weights.raw,
                        start,
                        &target.raw,
                        target_start,
                        bytes,
                    );
                    target.live = target_start + bytes;
                }
            }
            mesh.base_vertex = (v.live / size_of::<Vertex>() as u64) as i32;
            mesh.indices = (i.live / 4) as u32..((i.live + n) / 4) as u32;
            v.live += mesh.vertex_bytes;
            i.live += n;
        }
        self.queue.submit([encoder.finish()]);
        self.vertices = v;
        self.indices = i;
        self.cull.epoch += 1;
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

    /// Diagnostic readback of the GPU's per-view culling results. While on, each
    /// frame copies its indirect draw arguments and maps them asynchronously.
    pub fn count_culled(&mut self, on: bool) {
        self.cull.count(&self.device, on);
    }
    /// Instances drawn per view in the latest completed readback: camera, then sun
    /// cascades 0–2 (zero where absent). `None` until one arrives.
    pub fn culled(&mut self) -> Option<[u64; 4]> {
        self.cull.culled(&self.device)
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

    /// Record that this frame wrote timestamp pair `pair`.
    pub(crate) fn mark(&self, pair: u32) {
        self.timed.set(self.timed.get() | 1 << pair);
    }
    pub(crate) fn rebind(&mut self) {
        self.scene_binds = scene_binds(
            &self.device,
            &self.pipelines.scene_layout,
            &self.uniform,
            &self.transforms,
            &self.materials,
            (&self.slots.raw, None),
            &self.attachment_matrices,
            &self.environment,
            &self.lights,
        );
        self.rebind_culled();
        self.rebind_locals();
    }
    fn rebind_culled(&mut self) {
        self.culled_key = (self.cull.compacted.raw.clone(), self.cull.window);
        self.culled_binds = scene_binds(
            &self.device,
            &self.pipelines.scene_layout,
            &self.uniform,
            &self.transforms,
            &self.materials,
            (&self.cull.compacted.raw, Some(self.cull.window)),
            &self.attachment_matrices,
            &self.environment,
            &self.lights,
        );
    }
}

fn record_end(first: u32, len: usize, stride: usize) -> u64 {
    assert!(len.is_multiple_of(stride), "incomplete slot record");
    u64::from(first) + (len / stride) as u64
}

// `slots` is the retained list (whole buffer, offset zero) or the culled lists
// through a fixed window whose dynamic offset selects one view's group region.
#[allow(clippy::too_many_arguments)]
fn scene_binds(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    transforms: &[Buffer; 2],
    materials: &Buffer,
    slots: (&wgpu::Buffer, Option<u64>),
    attachments: &Buffer,
    environment: &crate::ibl::EnvironmentLight,
    lights: &crate::lights::Lights,
) -> [wgpu::BindGroup; 2] {
    std::array::from_fn(|current| {
        let slots = match slots.1 {
            Some(size) => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: slots.0,
                offset: 0,
                size: std::num::NonZeroU64::new(size),
            }),
            None => slots.0.as_entire_binding(),
        };
        let resources = [
            uniform.as_entire_binding(),
            transforms[1 - current].raw.as_entire_binding(),
            transforms[current].raw.as_entire_binding(),
            materials.raw.as_entire_binding(),
            slots,
            attachments.raw.as_entire_binding(),
            wgpu::BindingResource::TextureView(&environment.view),
            wgpu::BindingResource::Sampler(&environment.sampler),
            lights.buffer.raw.as_entire_binding(),
            lights.opacity.raw.as_entire_binding(),
            wgpu::BindingResource::TextureView(&environment.sky_view),
            wgpu::BindingResource::Sampler(&environment.sky_sampler),
        ];
        let entries = resources.map({
            let mut binding = 0;
            move |resource| {
                binding += 1;
                wgpu::BindGroupEntry {
                    binding: binding - 1,
                    resource,
                }
            }
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
    /// A slot first written after a history swap (an entity spawned in an
    /// earlier tick of a multi-tick advance, so never `fresh` when the
    /// presentation feeds) has no history yet; it draws without interpolation
    /// instead of refusing the batch.
    #[test]
    fn slots_first_written_after_a_swap_take_their_current_pose_as_history() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut r = RendererWithAssets::<false>::new(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let (vertices, indices) = crate::shapes::plane();
        let mesh = r.add_mesh(&vertices, &indices);
        let pose = [0., 0., 0., 0., 0., 0., 1., 1., 1., 1.];
        r.write_transforms_both(0, &pose).unwrap();
        r.write_materials(0, &[1.; 12 * 3000]).unwrap();
        r.begin_tick();
        r.write_transforms(0, &pose.repeat(3000)).unwrap();
        r.set_batches(&[Batch::new(mesh, 0..3000)], &(0..3000).collect::<Vec<_>>())
            .unwrap();
        assert!(r.transforms.iter().all(|b| b.live == 3000 * 40));
    }
    #[test]
    fn primitive_batches_skip_model_allocations_and_keep_attachment_winding() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut r = RendererWithAssets::<false>::new(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let (vertices, indices) = crate::shapes::plane();
        let mesh = r.add_mesh(&vertices, &indices);
        let pose = [0., 0., 0., 0., 0., 0., 1., 1., 1., 1.];
        r.write_transforms_both(0, &pose.repeat(2)).unwrap();
        r.write_materials(0, &[1.; 24]).unwrap();
        for _ in 0..3 {
            r.set_batches(&[Batch::new(mesh, 0..2)], &[0, 1]).unwrap();
        }
        assert_eq!(r.model_batches.capacity(), 0);
        assert_eq!(r.models.transparent.capacity(), 0);
        let mut world = exact_game::World::new(60, 0);
        world.spawn(exact_game::Transform::default());
        let mirrored = world.spawn(exact_game::Transform::default());
        let attachments = [crate::DisplayedAttachment {
            entity: mirrored,
            matrix: glam::Mat4::from_scale(Vec3::new(-1., 1., 1.)),
            pose: Default::default(),
        }];
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
        assert_eq!(
            r.winding_ranges(0, &frame).collect::<Vec<_>>(),
            [(0..1, false), (1..2, true)]
        );
        frame.attachments = &[];
        r.draw(&target, (16, 16), &frame);
        assert_eq!(
            r.winding_ranges(0, &frame).collect::<Vec<_>>(),
            [(0..2, false)]
        );
    }
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
            // Exercise amplified light; intensity one preserves authored emission.
            glow.0.to(w.now(), 16., 0.5);
            w.spawn((
                Transform::default(),
                Mesh::sphere(0.5),
                Material::glow([1.; 3]),
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
        let hole = w.spawn(exact_game::Transform::default());
        let b = w.spawn(exact_game::Transform::default());
        let matrix = glam::Mat4::from_scale_rotation_translation(
            Vec3::new(-1., 2., 3.),
            glam::Quat::from_rotation_z(0.25),
            Vec3::X,
        );
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
        assert_eq!(
            attachment_matrix(&r.attachment_words, a.index()),
            Some(matrix)
        );
        assert_eq!(attachment_matrix(&r.attachment_words, hole.index()), None);
        assert_eq!(attachment_matrix(&r.attachment_words, u32::MAX), None);
        assert!(r.slot_mirrored(a.index(), &frame));
        assert!(r.slot_mirrored(b.index(), &frame));
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
        assert!(r.slot_mirrored(a.index(), &frame));
        assert!(!r.slot_mirrored(b.index(), &frame));
        let after = read(&r);
        assert_eq!(
            &after[a.index() as usize * 64..(a.index() as usize + 1) * 64],
            bytes(&matrix.to_cols_array())
        );
        assert!(after[b.index() as usize * 64..].iter().all(|b| *b == 0));
        frame.attachments = &[];
        r.draw(&target, (16, 16), &frame);
        assert!(r.attachment_words.is_empty());
        assert_eq!(attachment_matrix(&r.attachment_words, a.index()), None);
        assert!(read(&r).iter().all(|b| *b == 0));
        // A singular affine override is present even though its determinant is zero.
        let collapsed = [crate::DisplayedAttachment {
            entity: b,
            matrix: glam::Mat4::from_scale(Vec3::new(0., 1., 1.)),
            pose: Default::default(),
        }];
        frame.attachments = &collapsed;
        r.draw(&target, (16, 16), &frame);
        assert_eq!(attachment_matrix(&r.attachment_words, a.index()), None);
        assert_eq!(
            attachment_matrix(&r.attachment_words, b.index()),
            Some(collapsed[0].matrix)
        );
        assert!(!r.slot_mirrored(b.index(), &frame));
        // Repeated direct-call overrides use the last upload on both CPU and GPU.
        let repeated = [collapsed[0], attachments[1]];
        frame.attachments = &repeated;
        r.draw(&target, (16, 16), &frame);
        assert_eq!(
            attachment_matrix(&r.attachment_words, b.index()),
            Some(matrix)
        );
        assert!(r.slot_mirrored(b.index(), &frame));
    }
}
