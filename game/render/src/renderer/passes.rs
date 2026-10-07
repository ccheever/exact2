//! The frame's passes, each reading one resolved frame state.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d3-optional-services-the-scene-copy
use super::draw::Resolved;
use super::*;
use crate::hooks::CustomMaterial;

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    fn model_material(&self, batch: usize) -> Option<crate::MaterialId> {
        if ASSETS {
            self.model_batches[batch]
        } else {
            None
        }
    }
    /// The world's depth range, or the viewmodel layer's: while any viewmodel
    /// draws, it takes the nearest `VIEWMODEL_DEPTH` and the world the rest.
    /// Where the viewmodel layer's depth ends this frame: zero without one.
    pub(super) fn depth_split(&self) -> f32 {
        if self.viewmodels {
            crate::VIEWMODEL_DEPTH
        } else {
            0.
        }
    }
    fn scene_viewport(&self, pass: &mut wgpu::RenderPass<'_>, size: (u32, u32), viewmodel: bool) {
        let split = self.depth_split();
        let (near, far) = if viewmodel { (0., split) } else { (split, 1.) };
        pass.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32, near, far);
        pass.set_scissor_rect(0, 0, size.0, size.1);
    }
    fn draw_group(&self, pass: &mut wgpu::RenderPass<'_>, view: u32, group: usize) {
        if self.cull.direct {
            let mesh = &self.meshes[self.batches[self.cull.groups[group].batch].mesh.0];
            pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
            pass.draw_indexed(
                mesh.indices.clone(),
                mesh.base_vertex,
                self.cull.groups[group].range.clone(),
            );
            return;
        }
        let (region, draw) = self.cull.offsets(view, group);
        pass.set_bind_group(0, &self.culled_binds[self.current], &[region]);
        pass.draw_indexed_indirect(&self.cull.indirect.raw, draw);
    }

    /// One depth pass per cascade over that cascade's culled casters.
    pub(super) fn encode_shadows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &FrameInput<'_>,
        custom: &[CustomMaterial],
    ) -> u32 {
        let Some(shadows) = &self.shadows else {
            return 0;
        };
        let mut draws = 0;
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
            if frame.timestamps.is_some() {
                self.mark(i as u32);
            }
            pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
            pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
            let view = 1 + i as u32;
            for (index, group) in self.cull.groups.iter().enumerate() {
                if group.flags & (1 << view) == 0 {
                    continue;
                }
                self.set_depth_pipeline(&mut pass, group, &shadows.cameras[i], custom);
                self.draw_group(&mut pass, view, index);
                draws += 1;
            }
        }
        draws
    }

    /// A depth-only caster pipeline and its groups 1-3 for one draw group.
    pub(super) fn set_depth_pipeline(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        group: &crate::cull::Group,
        camera: &wgpu::BindGroup,
        custom: &[CustomMaterial],
    ) {
        if let Some(material) = self.model_material(group.batch) {
            if let Some(custom) = custom.iter().find(|c| c.material == material) {
                let binds = self.custom_bindings.as_ref().unwrap();
                pass.set_pipeline(&custom.shadow);
                pass.set_bind_group(2, &custom.resources, &[]);
                pass.set_bind_group(3, &binds.instances.as_ref().unwrap().1, &[]);
            } else {
                let material = &self.models.materials[material.0];
                pass.set_pipeline(
                    self.pipelines.models.as_ref().unwrap().shadow
                        [usize::from(material.double_sided) + 2 * usize::from(group.mirrored)]
                    .as_ref()
                    .unwrap(),
                );
                pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
            }
        } else {
            pass.set_pipeline(&self.pipelines.shadow[usize::from(group.mirrored)]);
        }
        pass.set_bind_group(1, camera, &[]);
    }

    /// Opaque groups and quads, the hook's opaque stage, sky and background; with
    /// no scene copy the translucent pass continues here.
    pub(super) fn encode_forward<H: crate::Hooks>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        state: &mut Resolved,
        frame: &FrameInput<'_>,
        hooks: &mut H,
        view: &crate::FrameView<'_>,
    ) -> Result<(), RenderError> {
        let time = view.time;
        // Soft particles: translucency follows in its own pass over this depth.
        let split = self.quads.soft_active();
        let sky = frame
            .environment
            .background
            .unwrap_or(frame.environment.horizon);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game forward"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.color,
                resolve_target: (!split).then(|| {
                    self.hook_targets
                        .as_ref()
                        .and_then(|t| t.scene())
                        .map_or(&self.targets.resolved, |scene| scene.color)
                }),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: sky[0] as f64,
                        g: sky[1] as f64,
                        b: sky[2] as f64,
                        a: 1.0,
                    }),
                    store: if state.scene_copy || split {
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
                    store: if state.retained {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                }),
                stencil_ops: None,
            }),
            timestamp_writes: {
                if frame.timestamps.is_some() {
                    self.mark(3);
                }
                timing::writes(frame.timestamps, 3)
            },
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.scene_viewport(&mut pass, state.size, false);
        let mut layer = false;
        let variant = state.variant;
        pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        for (index, group) in self.cull.groups.iter().enumerate() {
            if let Some(material) = self.model_material(group.batch) {
                if let Some(custom) = hooks.materials().iter().find(|c| c.material == material) {
                    let binds = self.custom_bindings.as_ref().unwrap();
                    pass.set_pipeline(&custom.forward);
                    pass.set_bind_group(1, &self.shadow_sample, &[]);
                    pass.set_bind_group(2, &custom.resources, &[]);
                    pass.set_bind_group(3, &binds.instances.as_ref().unwrap().1, &[]);
                } else {
                    let material = &self.models.materials[material.0];
                    pass.set_pipeline(
                        self.pipelines.models.as_ref().unwrap().forward[variant
                            + 4 * usize::from(material.double_sided)
                            + 16 * usize::from(group.mirrored)]
                        .as_ref()
                        .unwrap(),
                    );
                    pass.set_bind_group(1, &self.shadow_sample, &[]);
                    pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                    pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                }
            } else {
                pass.set_pipeline(
                    &self.pipelines.forward[variant + 4 * usize::from(group.mirrored)],
                );
                pass.set_bind_group(1, &self.shadow_sample, &[]);
            }
            if group.range.start != self.batches[group.batch].slots.start {
                state.draws += 1;
            }
            let viewmodel = self.batches[group.batch].viewmodel;
            if viewmodel != layer {
                self.scene_viewport(&mut pass, state.size, viewmodel);
                layer = viewmodel;
            }
            self.draw_group(&mut pass, 0, index);
        }
        if layer {
            self.scene_viewport(&mut pass, state.size, false);
        }
        for draw in self.quads.draws.iter().filter(|d| self.quads.opaque(d)) {
            self.quads
                .draw::<ASSETS>(&mut pass, draw, &self.models.textures);
            state.draws += 1;
        }
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 18, false);
            hooks
                .opaque(&mut pass, view)
                .map_err(|e| hook_error("opaque", e))?;
            if timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 18, true) {
                self.mark(18);
            }
            state.times[2] = start.map(|s| s.elapsed());
            self.scene_viewport(&mut pass, state.size, false);
        }
        if frame::has_sky(frame, self.sky_ready) {
            pass.set_pipeline(&self.pipelines.sky);
            pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
            pass.draw(0..3, 0..1);
            state.draws += 1;
        }
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 19, false);
            hooks
                .background(&mut pass, view)
                .map_err(|e| hook_error("background", e))?;
            if timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 19, true) {
                self.mark(19);
            }
            state.times[3] = start.map(|s| s.elapsed());
        }
        if !state.scene_copy && !split {
            timing::pass_stamp(
                &self.device,
                &mut pass,
                frame.timestamps,
                timing::TRANSLUCENT,
                false,
            );
            state.draws += self.translucent(&mut pass, frame, variant, state.size);
            if timing::pass_stamp(
                &self.device,
                &mut pass,
                frame.timestamps,
                timing::TRANSLUCENT,
                true,
            ) {
                self.mark(timing::TRANSLUCENT);
            }
        }
        Ok(())
    }

    /// Translucency over the opaque pass's stored colour, its depth read-only and
    /// sampled by soft particles.
    pub(super) fn encode_translucent(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        state: &mut Resolved,
        frame: &FrameInput<'_>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game translucent"),
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
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: {
                if frame.timestamps.is_some() {
                    self.mark(timing::TRANSLUCENT);
                }
                timing::writes(frame.timestamps, timing::TRANSLUCENT)
            },
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.scene_viewport(&mut pass, state.size, false);
        state.draws += self.translucent(&mut pass, frame, state.variant, state.size);
    }

    /// Scene-copy continuation: depth snapshot, the refracting surface, translucency.
    pub(super) fn encode_surface<H: crate::Hooks>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        state: &mut Resolved,
        frame: &FrameInput<'_>,
        hooks: &mut H,
        view: &crate::FrameView<'_>,
    ) -> Result<(), RenderError> {
        let time = view.time;
        let targets = self.hook_targets.as_ref().unwrap();
        if targets.resolve_depth(
            encoder,
            &self.queue,
            frame,
            state.size,
            true,
            self.depth_split(),
        ) && frame.timestamps.is_some()
        {
            self.mark(22);
        }
        state.draws += 1;
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
                    store: if state.needs.contains(crate::Needs::FINAL_DEPTH) {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                }),
                stencil_ops: None,
            }),
            timestamp_writes: {
                if frame.timestamps.is_some() {
                    self.mark(20);
                }
                timing::writes(frame.timestamps, 20)
            },
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.scene_viewport(&mut pass, state.size, false);
        let start = (!time.seekable).then(crate::perf::Stamp::now);
        hooks
            .surface(&mut pass, &targets.scene().unwrap(), view)
            .map_err(|e| hook_error("surface", e))?;
        state.times[4] = start.map(|s| s.elapsed());
        state.draws += self.translucent(&mut pass, frame, state.variant, state.size);
        Ok(())
    }

    /// Final depth, the hook's HDR post, bloom and the tone curve into `target`.
    pub(super) fn encode_post<H: crate::Hooks>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        state: &mut Resolved,
        frame: &FrameInput<'_>,
        hooks: &mut H,
        view: &crate::FrameView<'_>,
    ) -> Result<(), RenderError> {
        let time = view.time;
        if let Some(targets) = &self.hook_targets {
            if targets.resolve_depth(
                encoder,
                &self.queue,
                frame,
                state.size,
                false,
                self.depth_split(),
            ) && frame.timestamps.is_some()
            {
                self.mark(23);
            }
            if state.needs.contains(crate::Needs::FINAL_DEPTH) {
                state.draws += 1;
            }
            if let Some(inputs) = targets.post(&self.targets.resolved) {
                let start = (!time.seekable).then(crate::perf::Stamp::now);
                timing::encoder_stamp(&self.device, encoder, frame.timestamps, 21, false);
                hooks
                    .post(encoder, &inputs, view)
                    .map_err(|e| hook_error("post", e))?;
                if timing::encoder_stamp(&self.device, encoder, frame.timestamps, 21, true) {
                    self.mark(21);
                }
                state.times[5] = start.map(|s| s.elapsed());
            }
        }
        if let Some(bloom) = &self.bloom {
            let draws = bloom.encode(
                &self.queue,
                encoder,
                &self.pipelines,
                frame.timestamps,
                state.size,
            );
            if frame.timestamps.is_some() {
                // Levels down from pair 4, then back up from pair 10.
                let levels = draws.div_ceil(2);
                for level in 0..levels {
                    self.mark(4 + level);
                    if level + 1 < levels {
                        self.mark(10 + level);
                    }
                }
            }
            state.draws += draws;
        }
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
            timestamp_writes: {
                if frame.timestamps.is_some() {
                    self.mark(15);
                }
                timing::writes(frame.timestamps, 15)
            },
            occlusion_query_set: None,
            multiview_mask: None,
        });
        viewport(&mut pass, state.size);
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
        Ok(())
    }

    // Ordered translucency: blended models one instance at a time from the retained
    // list, then quads; the CPU order already excludes what the camera cannot see.
    fn translucent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frame: &FrameInput<'_>,
        variant: usize,
        size: (u32, u32),
    ) -> u32 {
        self.scene_viewport(pass, size, false);
        let mut layer = false;
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut draws = 0;
        for draw in self.quads.draws.iter().filter(|d| !self.quads.opaque(d)) {
            // Blended viewmodel parts keep the viewmodel layer's depth range.
            let viewmodel = matches!(draw.kind, crate::quads::Kind::Model(index, _)
                if self.batches[index].viewmodel);
            if viewmodel != layer {
                self.scene_viewport(pass, size, viewmodel);
                layer = viewmodel;
            }
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
                pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
                pass.set_bind_group(1, &self.shadow_sample, &[]);
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
        if layer {
            self.scene_viewport(pass, size, false);
        }
        draws
    }
}

pub(super) fn hook_error(stage: &str, error: RenderError) -> RenderError {
    RenderError::Scene(format!("render hook {stage}: {error}"))
}
