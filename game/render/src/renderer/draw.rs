//! One frame: resolve its state once, then encode the passes in order.
use super::passes::hook_error;
use super::*;

/// Everything the passes share after preparation.
pub(super) struct Resolved {
    pub size: (u32, u32),
    pub needs: crate::Needs,
    pub scene_copy: bool,
    pub retained: bool,
    /// Forward variant: shadowed + 2 × fogged.
    pub variant: usize,
    /// CPU stage times: prepare, compute, opaque, background, surface, post.
    pub times: [Option<f64>; 6],
    /// Engine draws beyond the retained per-batch count.
    pub draws: u32,
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
        self.cull.map_counts();
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
            sample_count: 4,
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
            self.models
                .custom_data(&self.queue, |slot| hooks.instance_data(slot));
        }
        let (scene_copy, retained) = self.prepare_targets(size, needs);
        let cascades = self.prepare_effects(frame);
        let map = frame.environment_map.and_then(|m| {
            let texture = self.models.textures.get(m.texture).filter(|t| t.active)?;
            Some(crate::ibl::MapSource {
                view: &texture.view,
                digest: texture.digest,
                intensity: m.intensity,
                rgbm: m.rgbm,
            })
        });
        self.environment
            .prepare(&self.device, &self.queue, &frame.environment, map);
        if self
            .lights
            .prepare(&self.device, &self.queue, frame, size, &self.local_plan)
        {
            self.rebind();
        }
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytes(&frame::uniform(
                frame,
                cascades.as_ref(),
                size,
                &self.environment.irradiance,
                self.lights.info,
            )),
        );
        self.quads
            .frame::<ASSETS>(frame, &self.models.textures, !self.cull.keep_all);
        self.order_translucent(frame);
        self.quads.order::<ASSETS>(&self.device, &self.queue);
        self.prepare_cull(frame, cascades.as_ref(), hooks.materials());
        let mut state = Resolved {
            size,
            needs,
            scene_copy,
            retained,
            variant: usize::from(self.shadows.is_some())
                + 2 * usize::from(frame.environment.fog.is_some()),
            times,
            draws: 0,
        };
        self.environment.encode(encoder);
        if self.environment.mapped() {
            // The GPU-projected SH replaces the uniform's CPU irradiance.
            encoder.copy_buffer_to_buffer(&self.environment.sh, 0, &self.uniform, 144 * 4, 144);
        }
        if ASSETS {
            if let Some(skin) = &self.models.skinning {
                skin.encode(encoder, frame.timestamps);
            }
        }
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::encoder_stamp(&self.device, encoder, frame.timestamps, 17, false);
            hooks
                .compute(encoder, &view)
                .map_err(|e| hook_error("compute", e))?;
            timing::encoder_stamp(&self.device, encoder, frame.timestamps, 17, true);
            state.times[1] = start.map(|s| s.elapsed());
        }
        self.cull.encode(encoder, self.current, frame.timestamps);
        self.encode_local_culls(encoder);
        state.draws += self.encode_shadows(encoder, frame, hooks.materials());
        state.draws += self.encode_local_shadows(encoder, hooks.materials());
        self.encode_forward(encoder, &mut state, frame, hooks, &view)?;
        if scene_copy {
            self.encode_surface(encoder, &mut state, frame, hooks, &view)?;
        }
        self.encode_post(encoder, target, &mut state, frame, hooks, &view)?;
        self.cull.copy_counts(&self.device, encoder);
        Ok(self.finish_stats(hooks, &state))
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
    fn prepare_targets(&mut self, size: (u32, u32), needs: crate::Needs) -> (bool, bool) {
        let scene_copy = needs.contains(crate::Needs::SCENE_COPY);
        let retained = scene_copy || needs.contains(crate::Needs::FINAL_DEPTH);
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
            self.targets = Targets::with_retention(
                &self.device,
                capacity,
                &self.pipelines.tone_layout,
                &self.uniform,
                retained,
            );
            self.texture_creations += 3;
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
        (scene_copy, retained)
    }

    // Shadow maps and bloom follow the frame's effects; disabled effects release them.
    fn prepare_effects(&mut self, frame: &FrameInput<'_>) -> Option<Cascades> {
        let cascades = frame
            .sun
            .filter(|sun| sun.illuminance != 0.0)
            .and_then(|s| s.shadows)
            .map(|s| Cascades::new(frame, s));
        if let Some(c) = &cascades {
            if self.shadows.as_ref().is_none_or(|s| s.count != c.count) {
                self.texture_creations += 1;
                self.shadows = Some(ShadowMaps::new(
                    &self.device,
                    c.count,
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
    fn order_translucent(&mut self, frame: &FrameInput<'_>) {
        if !ASSETS {
            return;
        }
        let camera = crate::cull::planes(frame.proj * frame.view);
        for (_, slot, depth) in &mut self.models.transparent {
            let index = (self.slot_list[*slot as usize] - crate::RENDER_SLOT_BASE) as usize;
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
        stats.instances += self.quads.instances();
        stats.triangles += 2 * self.quads.instances();
        if ASSETS {
            stats.draws -= self
                .model_batches
                .iter()
                .flatten()
                .filter(|m| self.models.materials[m.0].alpha == exact_game::asset::AlphaMode::Blend)
                .count() as u32;
        }
        stats.texture_creations = self.texture_creations;
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
            metrics.attachment_bytes =
                u64::from(self.targets.size.0) * u64::from(self.targets.size.1) * (56 + extra);
            metrics.record(
                state.times,
                hooks.work(),
                !needs.contains(crate::Needs::PENDING) && hooks.error().is_none(),
            );
        }
        stats
    }
}
