//! One frame: resolve its state once, then encode the passes in order.
use super::passes::hook_error;
use super::*;

/// Everything the passes share after preparation.
pub(super) struct Resolved {
    pub size: (u32, u32),
    pub needs: crate::Needs,
    pub scene_copy: bool,
    /// Something after the forward pass reads its depth this frame (a scene
    /// copy, final depth, SSAO, soft particles): the pass stores it.
    pub depth_read: bool,
    /// Shared forward bits: shadowed, fogged and PCF-soft.
    pub variant: usize,
    /// Primitive forward bits beyond `variant` (pipeline.rs `LEAN`, `EARLY`, `DIRECT`,
    /// `UNIFORM`).
    pub primitive: usize,
    /// The frame needs no HDR image (`direct_frame`): it draws tonemapped into
    /// the target (resolving into it at 4x), and no tone pass follows.
    pub direct: bool,
    pub bilinear: bool,
    /// CPU stage times: prepare, compute, opaque, background, surface, post.
    pub times: [Option<f64>; 6],
    /// Engine draws beyond the retained per-batch count.
    pub draws: u32,
    pub hull_instances: u64,
    pub hull_triangles: u64,
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// Encode, submit, and map this frame's readbacks: a renderer that owns its queue.
    pub(crate) fn draw_assets(
        &mut self,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
    ) -> Stats {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let stats = self.encode_assets(&mut encoder, target, size_px, frame);
        self.queue.submit([encoder.finish()]);
        self.submitted();
        stats
    }

    fn encode_assets(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
    ) -> Stats {
        self.encode_hooked(
            encoder,
            target,
            size_px,
            frame,
            &mut (),
            None,
            crate::HookTime::default(),
            &Default::default(),
        )
        .expect("empty hooks cannot refuse")
    }

    /// After the encoded frame's submit: map what it copied for reading.
    pub(crate) fn submitted(&mut self) {
        if let Some(cache) = self.shadows.as_mut().and_then(|s| s.cached.as_mut()) {
            if cache.encoded.replace(false) {
                cache.valid = true;
            }
        }
        self.pipelines.reap_specializations();
        self.cull.map_counts();
        if self.pipelines.cache.handle.is_some() {
            let work = self.residency_work().pipeline_creations;
            self.pipelines.cache.save(work);
        }
    }

    // Order: skinning → hook compute → culling → shadows → forward (opaque, hook
    // opaque, sky, background[, translucent]) → [surface + translucent] → post →
    // bloom → tone. @ref llp/1046.006.000-render-hooks.rfc.md#d3-optional-services-the-scene-copy
    #[allow(clippy::too_many_arguments)]
    /// Encode the frame into `encoder` (the GPU module's, LLP 1009 D7); the
    /// caller submits it and then calls [`Self::submitted`].
    pub(crate) fn encode_hooked<H: crate::Hooks>(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
        hooks: &mut H,
        world: Option<&exact_game::World>,
        time: crate::HookTime,
        poses: &crate::hooks::Poses,
    ) -> Result<Stats, RenderError> {
        self.timed.set(0);
        if H::ENABLED && self.hook_metrics.is_none() {
            self.hook_metrics = Some(Box::default());
        }
        self.upload_overrides(frame);
        let size = (size_px.0.max(1), size_px.1.max(1));
        let capacity = (
            size.0.div_ceil(64).max(self.targets.size.0 / 64) * 64,
            size.1.div_ceil(64).max(self.targets.size.1 / 64) * 64,
        );
        let view = crate::FrameView {
            frame,
            time,
            pixels: size,
            capacity,
            color_format: wgpu::TextureFormat::Rgba16Float,
            depth_format: wgpu::TextureFormat::Depth32Float,
            sample_count: self.pipelines.samples,
            poses,
        };
        let mut times = [None; 6];
        let needs = if H::ENABLED {
            self.prepare_hooks(hooks, world, &view, &mut times)?;
            hooks.needs()
        } else {
            crate::Needs::NONE
        };
        if !hooks.drawable() {
            self.hook_metrics
                .as_mut()
                .unwrap()
                .record(times, hooks.work(), false);
            return Ok(self.encode_assets(encoder, target, size_px, frame));
        }
        if H::ENABLED && ASSETS {
            self.check_custom_materials(hooks.materials())?;
            // A change in custom materials rebatches: those models draw unmerged.
            let custom = hooks.materials();
            if custom.len() != self.models.custom.len()
                || custom
                    .iter()
                    .any(|c| !self.models.custom.contains(&c.material))
            {
                self.models.custom = custom.iter().map(|c| c.material).collect();
                self.models.revision += 1;
            }
            self.models
                .custom_data(&self.queue, |slot| hooks.instance_data(slot));
        }
        self.prepass_active = self.pipelines.depth_prepass && hooks.materials().is_empty();
        // This frame's quads and translucent order first: the split below
        // follows the soft particles actually drawn.
        self.quads
            .frame::<ASSETS>(frame, &self.models.textures, !self.cull.keep_all);
        self.select_levels(frame);
        self.order_translucent(frame);
        self.quads.order::<ASSETS>(&self.device, &self.queue);
        // Soft particles read the opaque depth: the translucent pass splits from
        // the opaque one (not under a scene copy, whose continuation draws them
        // hard). Depth stays retained once they have drawn, so toggling them
        // never recreates the targets (and drops bloom and SSAO).
        let soft = self.quads.soft_drawn() && !needs.contains(crate::Needs::SCENE_COPY);
        self.soft_retained |= soft;
        let direct = self.direct_frame::<H>(frame, soft);
        // A direct frame leaves the HDR attachments as they are, unused.
        let scene_copy = if direct {
            false
        } else {
            self.prepare_targets(
                size,
                needs,
                frame.ambient_occlusion.is_some() || self.soft_retained,
            )
        };
        let cascades = self.prepare_effects(frame);
        let map = frame.environment_map.and_then(|m| {
            let texture = self.models.textures.get(m.texture).filter(|t| t.active)?;
            Some(crate::ibl::MapSource {
                view: &texture.view,
                digest: texture.digest,
                intensity: m.intensity,
                rgbm: m.rgbm,
                rotation: m.rotation,
            })
        });
        if self
            .environment
            .prepare(&self.device, &self.queue, &frame.environment, map)
        {
            self.rebind();
        }
        if self
            .lights
            .prepare(&self.device, &self.queue, frame, size, &self.local_plan)
        {
            self.rebind();
        }
        self.prepare_batch_coverage();
        // A visible sky map draws once resident; the procedural sky until then.
        self.sky_ready = frame.environment_map.is_some_and(|m| {
            m.visible
                && self
                    .models
                    .textures
                    .get(m.texture)
                    .is_some_and(|t| t.active)
        });
        let mut uniform = frame::uniform(
            frame,
            self.sky_ready,
            cascades.as_ref(),
            size,
            &self.environment.irradiance,
            self.lights.info,
        );
        // An authored map's intensity scales its filtered light at sample time.
        uniform[31] *= self.environment.ambient_scale();
        self.queue.write_buffer(&self.uniform, 0, bytes(&uniform));
        self.prepare_cull(frame, cascades.as_ref(), hooks.materials(), size);
        self.quads.soft_pass(
            &self.device,
            &self.queue,
            soft.then_some(&self.targets.depth),
            (frame.proj * frame.view).inverse(),
            self.depth_split(),
        );
        self.prepare_shadow_cache(cascades.as_ref(), frame, hooks.materials());
        if direct {
            self.prepare_direct(target.texture().size());
        }
        let lean = self.lights.info[0] == 0. && frame.fill.is_none() && self.grids == 0;
        let early = self.pipelines.early;
        let e = &frame.environment;
        let uniform =
            frame.environment_map.is_none() && e.zenith == e.horizon && e.horizon == e.ground;
        let bilinear = frame
            .sun
            .and_then(|s| s.shadows)
            .is_some_and(|s| matches!(s.filter, crate::ShadowFilter::Bilinear { .. }));
        let mut state = Resolved {
            bilinear,
            size,
            needs,
            scene_copy,
            depth_read: scene_copy
                || needs.contains(crate::Needs::FINAL_DEPTH)
                || frame.ambient_occlusion.is_some()
                || soft,
            variant: usize::from(self.shadows.is_some())
                + 2 * usize::from(frame.environment.fog.is_some())
                + crate::pipeline::PCF_SOFT
                    * usize::from(
                        self.shadows.is_some()
                            && frame.sun.and_then(|s| s.shadows).is_some_and(|s| {
                                matches!(
                                    s.filter,
                                    crate::ShadowFilter::PcfSoft { .. }
                                        | crate::ShadowFilter::Bilinear { .. }
                                )
                            }),
                    ),
            primitive: if bilinear {
                crate::pipeline::BILINEAR
            } else {
                0
            } | if lean { crate::pipeline::LEAN } else { 0 }
                | if early { crate::pipeline::EARLY } else { 0 }
                | if direct { crate::pipeline::DIRECT } else { 0 }
                | if uniform { crate::pipeline::UNIFORM } else { 0 }
                | if self.shadows.as_ref().is_some_and(|s| s.count == 1) {
                    crate::pipeline::SINGLE_CASCADE
                } else {
                    0
                },
            direct,
            times,
            draws: 0,
            hull_instances: 0,
            hull_triangles: 0,
        };
        if self.environment.encode(encoder, frame.timestamps) {
            self.mark(timing::ENVIRONMENT);
        }
        if self.environment.mapped() {
            // The GPU-projected SH replaces the uniform's CPU irradiance.
            encoder.copy_buffer_to_buffer(&self.environment.sh, 0, &self.uniform, 144 * 4, 144);
        }
        if ASSETS {
            if let Some(skin) = &self.models.skinning {
                if skin.encode(encoder, frame.timestamps) && frame.timestamps.is_some() {
                    self.mark(16);
                }
            }
        }
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::encoder_stamp(&self.device, encoder, frame.timestamps, 17, false);
            hooks
                .compute(encoder, &view)
                .map_err(|e| hook_error("compute", e))?;
            if timing::encoder_stamp(&self.device, encoder, frame.timestamps, 17, true) {
                self.mark(17);
            }
            state.times[1] = start.map(|s| s.elapsed());
        }
        if self.occluding {
            self.encode_occluders(encoder);
        }
        self.cull_drew = self.cull.encode(encoder, self.current, frame.timestamps);
        if self.cull_drew && frame.timestamps.is_some() {
            self.mark(timing::CULL);
        }
        self.encode_local_culls(encoder);
        state.draws += self.encode_shadows(encoder, frame, hooks.materials());
        state.draws += self.encode_local_shadows(encoder, hooks.materials(), frame.timestamps);
        state.draws += self.encode_depth_prepass(encoder, frame, state.size, state.direct);
        self.encode_forward(encoder, target, &mut state, frame, hooks, &view)?;
        if self.quads.soft_active() {
            self.encode_translucent(encoder, &mut state, frame);
        }
        if scene_copy {
            self.encode_surface(encoder, &mut state, frame, hooks, &view)?;
        }
        if let Some(settings) = frame.ambient_occlusion {
            let (scale, _) = settings.quality.plan();
            if self
                .ssao
                .as_ref()
                .is_none_or(|s| !s.fits(&self.targets, scale))
            {
                self.ssao = Some(crate::ssao::Ssao::new(&self.device, &self.targets, scale));
                self.texture_creations += 1;
            }
            if frame.timestamps.is_some() {
                self.mark(timing::SSAO);
                self.mark(timing::SSAO_APPLY);
            }
            self.ssao.as_ref().unwrap().encode(
                &self.queue,
                encoder,
                frame,
                settings,
                &self.targets.resolved,
                size,
                self.depth_split(),
                frame.timestamps,
            );
            state.draws += 2;
        } else {
            self.ssao = None;
        }
        if !state.direct {
            self.encode_post(encoder, target, &mut state, frame, hooks, &view)?;
        }
        self.cull.copy_counts(&self.device, encoder);
        Ok(self.finish_stats(hooks, &state))
    }

    /// Whether this frame draws straight into the target, with no HDR image (README
    /// "Direct frames"): nothing after the forward pass reads one (no hook, bloom,
    /// SSAO or soft particle), and whatever blends, blends as it would into one. A
    /// blend is linear in its inputs, so it commutes with exposure, all the tone
    /// curve `ToneMapping::None` applies, and an sRGB target blends in linear
    /// light; under ACES, or into a target the shader encodes, it would mix other
    /// values than the tone pass's. A tiling GPU then stores no HDR image and skips
    /// the full-screen tone pass's read and write: at 4x, with MSAA in the target's
    /// format, half the bytes a sample too (on an Adreno 620, 6 ms of a
    /// 32,000-cube frame).
    fn direct_frame<H: crate::Hooks>(&self, frame: &FrameInput<'_>, soft: bool) -> bool {
        let blends = self.quads.draws.iter().any(|d| !self.quads.opaque(d));
        self.allow_direct
            && !H::ENABLED
            && frame.environment.bloom.is_none()
            && frame.ambient_occlusion.is_none()
            && !soft
            && (!blends
                || (frame.environment.tone_mapping == exact_game::ToneMapping::None
                    && self.pipelines.format().is_srgb()))
    }

    /// A direct frame's attachments at the target's size, and its quad pipelines.
    fn prepare_direct(&mut self, extent: wgpu::Extent3d) {
        let format = self.pipelines.format();
        let retained = false;
        if self
            .direct_targets
            .as_ref()
            .is_none_or(|t| t.format != format || t.size != extent || t.retained != retained)
        {
            let targets = crate::buffers::DirectTargets::new(
                &self.device,
                format,
                extent,
                self.pipelines.samples,
                retained,
            );
            self.texture_creations += targets.textures();
            self.direct_targets = Some(targets);
        }
        self.quads.prepare_direct(&self.device);
        self.direct_frames += 1;
    }

    fn upload_overrides(&mut self, frame: &FrameInput<'_>) {
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
    }

    fn prepare_hooks<H: crate::Hooks>(
        &mut self,
        hooks: &mut H,
        world: Option<&exact_game::World>,
        view: &crate::FrameView<'_>,
        times: &mut [Option<f64>; 6],
    ) -> Result<(), RenderError> {
        let device = &self.device;
        let binding = self
            .hook_binding
            .get_or_insert_with(|| crate::hooks::FrameBinding::new(device, &self.uniform));
        let Some(world) = world else {
            return Ok(());
        };
        let materials = if ASSETS {
            let binds = self
                .custom_bindings
                .get_or_insert_with(|| crate::hooks::MaterialBindings::new(device));
            binds.sync(device, &self.models);
            Some(crate::hooks::MaterialGpu {
                device,
                models: &self.models,
                pipelines: &self.pipelines,
                instance: &binds.layout,
            })
        } else {
            None
        };
        let gpu = crate::HookGpu {
            materials,
            device,
            queue: &self.queue,
            frame_binding: &binding.bind,
            frame_layout: &binding.layout,
        };
        let start = (!view.time.seekable).then(crate::perf::Stamp::now);
        hooks
            .prepare(&gpu, view, &crate::RenderWorld(world))
            .map_err(|e| hook_error("prepare", e))?;
        times[0] = start.map(|s| s.elapsed());
        Ok(())
    }

    fn check_custom_materials(
        &self,
        custom: &[crate::hooks::CustomMaterial],
    ) -> Result<(), RenderError> {
        for custom in custom {
            for (name, model) in &self.models.loaded {
                for (index, node) in model.nodes.iter().enumerate() {
                    if node.1 == custom.material && self.meshes[node.0 .0].outline.is_some() {
                        return Err(RenderError::scene(format!(
                            "{name}: mesh {} (node {:?}): outline with custom material is unsupported",
                            node.0.0, model.names[index]
                        )));
                    }
                }
            }
            if let Some(record) = self.models.records.iter().find(|r| {
                r.material == custom.material && self.meshes[r.geometry.0].outline.is_some()
            }) {
                return Err(RenderError::scene(format!(
                    "mesh {}: outline with custom material is unsupported",
                    record.geometry.0
                )));
            }
            if self
                .models
                .materials
                .get(custom.material.0)
                .is_none_or(|m| m.alpha != exact_game::asset::AlphaMode::Opaque)
                || self
                    .models
                    .records
                    .iter()
                    .any(|r| r.material == custom.material && r.skin.is_some())
            {
                return Err(RenderError::Scene(
                    "custom materials require a loaded opaque unskinned model".into(),
                ));
            }
        }
        Ok(())
    }

    // Attachments grow in 64-pixel buckets; hook services add retained targets.
    // Whether the frame copies its scene for a hook.
    fn prepare_targets(&mut self, size: (u32, u32), needs: crate::Needs, occlusion: bool) -> bool {
        let scene_copy = needs.contains(crate::Needs::SCENE_COPY);
        // Occlusion reads the forward pass's depth too.
        let retained = self.pipelines.depth_prepass
            || scene_copy
            || occlusion
            || needs.contains(crate::Needs::FINAL_DEPTH);
        let replace_targets = size.0 > self.targets.size.0
            || size.1 > self.targets.size.1
            || retained != self.targets.retained;
        if replace_targets {
            let bucket = |n: u32| n.div_ceil(64) * 64;
            let capacity = (
                bucket(size.0).max(self.targets.size.0),
                bucket(size.1).max(self.targets.size.1),
            );
            self.bloom = None;
            self.ssao = None;
            self.targets = Targets::with_retention(
                &self.device,
                capacity,
                &self.pipelines.tone_layout,
                &self.uniform,
                retained,
                self.pipelines.samples,
            );
            self.texture_creations += if self.pipelines.samples == 1 { 2 } else { 3 };
        }
        if needs.attachments() == crate::Needs::NONE {
            if self.hook_targets.take().is_some() {
                self.bloom = None;
            }
        } else if replace_targets
            || self
                .hook_targets
                .as_ref()
                .is_none_or(|t| t.size != self.targets.size || t.needs != needs.attachments())
        {
            let targets = crate::hooks::HookTargets::new(
                &self.device,
                &self.targets,
                &self.pipelines,
                &self.uniform,
                needs,
            );
            self.texture_creations += targets.texture_count();
            if let Some(metrics) = &mut self.hook_metrics {
                metrics.engine.textures += targets.texture_count();
                metrics.engine.pipelines += u64::from(retained);
                metrics.engine.buffers += u64::from(retained);
            }
            self.hook_targets = Some(targets);
            self.bloom = None;
        }
        scene_copy
    }

    // Shadow maps and bloom follow the frame's effects; disabled effects release them.
    fn prepare_effects(&mut self, frame: &FrameInput<'_>) -> Option<Cascades> {
        let cascades = frame
            .sun
            .filter(|sun| sun.illuminance != 0.0)
            .and_then(|s| s.shadows)
            .map(|s| Cascades::new(frame, s));
        if let Some(c) = &cascades {
            if self.shadows.as_ref().is_none_or(|s| {
                s.count != c.count
                    || s.resolution != c.resolution
                    || s.cache_enabled != c.cache_enabled
            }) {
                self.texture_creations += 1;
                self.shadows = Some(ShadowMaps::new(
                    &self.device,
                    c.count,
                    c.resolution,
                    c.cache_enabled,
                    &self.pipelines.camera_layout,
                ));
                self.shadow_sample_stale = true;
            }
            self.shadows.as_ref().unwrap().write(&self.queue, c);
        } else if self.shadows.take().is_some() {
            self.shadow_sample_stale = true;
        }
        self.prepare_local(frame);
        self.refresh_shadow_sample();
        if frame.environment.bloom.is_some() {
            if self.bloom.is_none() {
                self.bloom = Some(BloomTargets::new(
                    &self.device,
                    self.targets.size,
                    &self.pipelines,
                    &self.uniform,
                    self.hook_targets
                        .as_ref()
                        .map_or(&self.targets.resolved, |t| t.hdr(&self.targets.resolved)),
                ));
                self.texture_creations += 6;
            }
        } else {
            self.bloom = None;
        }
        cascades
    }

    // One total translucent order; opaque/primitive batches remain retained.
    // Blended models the camera cannot see stay out of it (NaN depth marks them).
    /// One level of detail per entity, from its displayed position.
    fn select_levels(&mut self, frame: &FrameInput<'_>) {
        if !ASSETS || self.levels.entries.is_empty() {
            return;
        }
        let (poses, indices) = (&self.models.poses, &self.models.pose_indices);
        let words = &self.attachment_words;
        self.levels.select(|e| {
            let position = attachment_matrix(words, e.slot).map_or_else(
                || {
                    let history = poses[indices[e.record as usize]];
                    crate::world::scene::interpolate(history, frame.alpha).position
                },
                |m| m.w_axis.truncate(),
            );
            position.distance(frame.camera_position)
        });
        if self.levels.changed {
            if let Some(skinning) = &mut self.models.skinning {
                skinning.activate(&self.queue, &self.levels.hidden);
            }
        }
    }

    fn order_translucent(&mut self, frame: &FrameInput<'_>) {
        if !ASSETS {
            return;
        }
        let camera = crate::cull::planes(frame.proj * frame.view);
        for (_, slot, depth) in &mut self.models.transparent {
            let index = (self.slot_list[*slot as usize] - crate::RENDER_SLOT_BASE) as usize;
            if self.levels.hidden.get(index).copied().unwrap_or(false) {
                *depth = f32::NAN;
                continue;
            }
            let record = &self.models.records[index];
            let mesh = &self.meshes[record.geometry.0];
            let history = self.models.poses[self.models.pose_indices[index]];
            let pose =
                attachment_matrix(&self.attachment_words, record.transform).unwrap_or_else(|| {
                    let t = crate::world::scene::interpolate(history, frame.alpha);
                    glam::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
                });
            let position = pose.transform_point3(record.local.transform_point3(mesh.center));
            *depth = -frame.view.transform_point3(position).z;
            // Skinned vertices have no CPU bound here; the GPU pass keeps them too.
            let m = pose * record.local;
            let abs = glam::Mat3::from_cols(
                m.x_axis.truncate().abs(),
                m.y_axis.truncate().abs(),
                m.z_axis.truncate().abs(),
            );
            if record.skin.is_none()
                && !self.cull.keep_all
                && !crate::cull::sphere_visible(&camera, position, (abs * mesh.half).length())
            {
                *depth = f32::NAN;
            }
        }
        for &(index, slot, depth) in &self.models.transparent {
            if depth.is_nan() {
                continue;
            }
            let record = &self.models.records
                [(self.slot_list[slot as usize] - crate::RENDER_SLOT_BASE) as usize];
            self.quads.order.push(crate::quads::Order {
                viewmodel: self.batches[index].viewmodel,
                kind: crate::quads::Kind::Model(index, slot),
                depth,
                layer: 0,
                slot: record.transform,
                index: slot as usize,
            });
        }
    }

    fn finish_stats<H: crate::Hooks>(&mut self, hooks: &H, state: &Resolved) -> Stats {
        let mut stats = self.counts;
        stats.draws += state.draws;
        stats.instances += self.quads.instances() + state.hull_instances;
        stats.triangles += 2 * self.quads.instances() + state.hull_triangles;
        if ASSETS {
            stats.draws -= self
                .model_batches
                .iter()
                .flatten()
                .filter(|m| self.models.materials[m.0].alpha == exact_game::asset::AlphaMode::Blend)
                .count() as u32;
        }
        stats.texture_creations = self.texture_creations;
        stats.direct_frames = self.direct_frames;
        if H::ENABLED {
            let needs = state.needs;
            let metrics = self.hook_metrics.as_mut().unwrap();
            let extra = if state.scene_copy { 12 } else { 0 }
                + if needs.contains(crate::Needs::FINAL_DEPTH) {
                    4
                } else {
                    0
                }
                + if needs.contains(crate::Needs::HDR_POST) {
                    8
                } else {
                    0
                };
            metrics.attachment_bytes = u64::from(self.targets.size.0)
                * u64::from(self.targets.size.1)
                * (if self.pipelines.samples == 1 { 12 } else { 56 } + extra);
            metrics.record(
                state.times,
                hooks.work(),
                !needs.contains(crate::Needs::PENDING) && hooks.error().is_none(),
            );
        }
        stats
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use exact_game::*;
    use exact_gpu::{fixture, wgpu};

    #[test]
    fn single_sample_town_settings_draw_direct_or_keep_one_hdr_attachment() {
        let Some(gpu) = crate::test_device::device_or_skip(fixture::device()) else {
            return;
        };
        let mut world = World::new(60, 1);
        world.spawn((Transform::default(), Mesh::cube(1.)));
        let mut renderer = crate::Renderer::with_samples(
            &gpu.device,
            &gpu.queue,
            wgpu::TextureFormat::Rgba8Unorm,
            SceneSamples::One,
        );
        crate::Feed::default().feed(&world, &mut renderer).unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("town attachment audit"),
            size: wgpu::Extent3d {
                width: 1080,
                height: 2340,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let frame = crate::FrameInput {
            environment: Environment {
                background: Some([0.5; 3]),
                fog: Some(Fog::linear(18., 60.)),
                bloom: None,
                tone_mapping: ToneMapping::None,
                ..Default::default()
            },
            ..Default::default()
        };
        let draw = |renderer: &mut crate::Renderer| {
            let stats = renderer.draw(
                &texture.create_view(&Default::default()),
                (1080, 2340),
                &frame,
            );
            assert_eq!(
                stats.draws, 5,
                "three shadow draws, opaque draw, output transfer"
            );
            assert!(renderer.hook_targets.is_none());
            assert!(renderer.bloom.is_none());
            assert!(renderer.ssao.is_none());
            assert!(!renderer.quads.soft_active());
            assert!(!renderer.occluding);
            assert_eq!(renderer.shadows.as_ref().unwrap().count, 3);
            stats.direct_frames
        };
        // Direct: one transient depth at the target's size, and the HDR
        // attachments as they were made, one 64-pixel bucket.
        for frames in 1..=2 {
            assert_eq!(draw(&mut renderer), frames);
            assert_eq!(renderer.targets.size, (64, 64));
            let direct = renderer.direct_targets.as_ref().unwrap();
            assert!(direct.color.is_none() && !direct.retained);
            assert_eq!((direct.size.width, direct.size.height), (1080, 2340));
        }
        renderer.set_direct(false);
        for _ in 0..2 {
            assert_eq!(draw(&mut renderer), 2);
            assert_eq!(renderer.targets.size, (1088, 2368));
            assert_eq!(renderer.targets.samples, 1);
            assert_eq!(renderer.targets.color, renderer.targets.resolved);
            assert!(!renderer.targets.retained);
        }
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
}
