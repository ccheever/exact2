use super::*;
impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    pub(crate) fn draw_assets(
        &mut self,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
    ) -> Stats {
        self.draw_hooked(
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

    // @ref llp/1046.006.000-render-hooks.rfc.md#d3-optional-services-the-scene-copy
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_hooked<H: crate::Hooks>(
        &mut self,
        target: &wgpu::TextureView,
        size_px: (u32, u32),
        frame: &FrameInput<'_>,
        hooks: &mut H,
        world: Option<&exact_game::World>,
        time: crate::HookTime,
        poses: &crate::hooks::Poses,
    ) -> Result<Stats, RenderError> {
        let mut hook_times = [None; 6];
        if H::ENABLED && self.hook_metrics.is_none() {
            self.hook_metrics = Some(Box::default());
        }
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
        let needs = if H::ENABLED {
            let binding = self
                .hook_binding
                .get_or_insert_with(|| crate::hooks::FrameBinding::new(device, &self.uniform));
            if let Some(world) = world {
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
                        empty: &binds.empty,
                    })
                } else {
                    None
                };
                let gpu = crate::HookGpu {
                    materials,
                    device,
                    queue,
                    frame_binding: &binding.bind,
                    frame_layout: &binding.layout,
                };
                let start = (!time.seekable).then(crate::perf::Stamp::now);
                hooks
                    .prepare(&gpu, &view, &crate::RenderWorld(world))
                    .map_err(|e| hook_error("prepare", e))?;
                hook_times[0] = start.map(|s| s.elapsed());
            }
            hooks.needs()
        } else {
            crate::Needs::NONE
        };
        if !hooks.drawable() {
            self.hook_metrics
                .as_mut()
                .unwrap()
                .record(hook_times, hooks.work(), false);
            return Ok(self.draw_assets(target, size_px, frame));
        }
        if H::ENABLED && ASSETS {
            for custom in hooks.materials() {
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
            self.models
                .custom_data(queue, |slot| hooks.instance_data(slot));
        }
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
                device,
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
                device,
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
        let cascades = frame
            .sun
            .filter(|sun| sun.illuminance != 0.0)
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
                    self.hook_targets
                        .as_ref()
                        .map_or(&self.targets.resolved, |t| t.hdr(&self.targets.resolved)),
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
        self.quads.frame::<ASSETS>(frame, &self.models.textures);
        // One total translucent order; opaque/primitive batches remain retained.
        if ASSETS {
            for (_, slot, depth) in &mut self.models.transparent {
                let index = (self.slot_list[*slot as usize] - crate::RENDER_SLOT_BASE) as usize;
                let record = &self.models.records[index];
                let center = record
                    .local
                    .transform_point3(self.meshes[record.geometry.0].center);
                let history = self.models.poses[self.models.pose_indices[index]];
                let pose = attachment_matrix(&self.attachment_words, record.transform)
                    .unwrap_or_else(|| {
                        let t = crate::world::scene::interpolate(history, frame.alpha);
                        glam::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
                    });
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
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::encoder_stamp(device, &mut encoder, frame.timestamps, 17, false);
            hooks
                .compute(&mut encoder, &view)
                .map_err(|e| hook_error("compute", e))?;
            timing::encoder_stamp(device, &mut encoder, frame.timestamps, 17, true);
            hook_times[1] = start.map(|s| s.elapsed());
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
                            if let Some(custom) =
                                hooks.materials().iter().find(|c| c.material == material)
                            {
                                let binds = self.custom_bindings.as_ref().unwrap();
                                pass.set_pipeline(&custom.shadow);
                                pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                                pass.set_bind_group(1, &shadows.cameras[i], &[]);
                                pass.set_bind_group(2, &custom.resources, &[]);
                                pass.set_bind_group(3, &binds.instances.as_ref().unwrap().1, &[]);
                                let mesh = &self.meshes[batch.mesh.0];
                                pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slots);
                                extra_draws += 1;
                                continue;
                            }
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
                    resolve_target: Some(
                        self.hook_targets
                            .as_ref()
                            .and_then(|t| t.scene())
                            .map_or(&self.targets.resolved, |scene| scene.color),
                    ),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky[0] as f64,
                            g: sky[1] as f64,
                            b: sky[2] as f64,
                            a: 1.0,
                        }),
                        store: if scene_copy {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: if retained {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
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
                        if let Some(custom) =
                            hooks.materials().iter().find(|c| c.material == material)
                        {
                            let binds = self.custom_bindings.as_ref().unwrap();
                            pass.set_pipeline(&custom.forward);
                            pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                            pass.set_bind_group(1, &binds.empty_bind, &[]);
                            pass.set_bind_group(2, &custom.resources, &[]);
                            pass.set_bind_group(3, &binds.instances.as_ref().unwrap().1, &[]);
                            let mesh = &self.meshes[batch.mesh.0];
                            pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slots);
                            continue;
                        }
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
                        pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                        if let Some(shadows) = &self.shadows {
                            pass.set_bind_group(1, &shadows.sample, &[]);
                        }
                    }
                    let mesh = &self.meshes[batch.mesh.0];
                    if slots.start != batch.slots.start {
                        extra_draws += 1;
                    }
                    pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slots);
                }
            }
            for draw in self.quads.draws.iter().filter(|d| self.quads.opaque(d)) {
                self.quads
                    .draw::<ASSETS>(&mut pass, draw, &self.models.textures);
                extra_draws += 1;
            }
            if H::ENABLED {
                let start = (!time.seekable).then(crate::perf::Stamp::now);
                timing::pass_stamp(device, &mut pass, frame.timestamps, 18, false);
                hooks
                    .opaque(&mut pass, &view)
                    .map_err(|e| hook_error("opaque", e))?;
                timing::pass_stamp(device, &mut pass, frame.timestamps, 18, true);
                hook_times[2] = start.map(|s| s.elapsed());
                viewport(&mut pass, size);
            }
            if frame::has_sky(frame) {
                pass.set_pipeline(&self.pipelines.sky);
                pass.set_bind_group(0, &self.scene_binds[self.current], &[]);
                pass.draw(0..3, 0..1);
                extra_draws += 1;
            }
            if H::ENABLED {
                let start = (!time.seekable).then(crate::perf::Stamp::now);
                timing::pass_stamp(device, &mut pass, frame.timestamps, 19, false);
                hooks
                    .background(&mut pass, &view)
                    .map_err(|e| hook_error("background", e))?;
                timing::pass_stamp(device, &mut pass, frame.timestamps, 19, true);
                hook_times[3] = start.map(|s| s.elapsed());
            }
            if !scene_copy {
                extra_draws += self.translucent(&mut pass, frame, variant, size);
            }
        }
        if scene_copy {
            let targets = self.hook_targets.as_ref().unwrap();
            targets.resolve_depth(&mut encoder, queue, frame, size, true);
            extra_draws += 1;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game surface + translucent"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.color,
                    resolve_target: Some(&self.targets.resolved),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: if needs.contains(crate::Needs::FINAL_DEPTH) {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timing::writes(frame.timestamps, 20),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            viewport(&mut pass, size);
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            hooks
                .surface(&mut pass, &targets.scene().unwrap(), &view)
                .map_err(|e| hook_error("surface", e))?;
            hook_times[4] = start.map(|s| s.elapsed());
            let variant = usize::from(self.shadows.is_some())
                + 2 * usize::from(frame.environment.fog.is_some());
            extra_draws += self.translucent(&mut pass, frame, variant, size);
        }
        if let Some(targets) = &self.hook_targets {
            targets.resolve_depth(&mut encoder, queue, frame, size, false);
            if needs.contains(crate::Needs::FINAL_DEPTH) {
                extra_draws += 1;
            }
            if let Some(inputs) = targets.post(&self.targets.resolved) {
                let start = (!time.seekable).then(crate::perf::Stamp::now);
                timing::encoder_stamp(device, &mut encoder, frame.timestamps, 21, false);
                hooks
                    .post(&mut encoder, &inputs, &view)
                    .map_err(|e| hook_error("post", e))?;
                timing::encoder_stamp(device, &mut encoder, frame.timestamps, 21, true);
                hook_times[5] = start.map(|s| s.elapsed());
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
            pass.set_bind_group(
                0,
                self.hook_targets
                    .as_ref()
                    .map_or(&self.targets.tone_bind, |t| t.tone(&self.targets.tone_bind)),
                &[],
            );
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        let mut stats = self.counts;
        stats.draws += extra_draws;
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
            let metrics = self.hook_metrics.as_mut().unwrap();
            let extra = if scene_copy { 12 } else { 0 }
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
                hook_times,
                hooks.work(),
                !needs.contains(crate::Needs::PENDING) && hooks.error().is_none(),
            );
        }
        Ok(stats)
    }
    fn translucent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frame: &FrameInput<'_>,
        variant: usize,
        size: (u32, u32),
    ) -> u32 {
        viewport(pass, size);
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut draws = 0;
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
                self.quads.draw::<ASSETS>(pass, draw, &self.models.textures);
            }
            draws += 1;
        }
        draws
    }
}
fn hook_error(stage: &str, error: RenderError) -> RenderError {
    RenderError::Scene(format!("render hook {stage}: {error}"))
}
