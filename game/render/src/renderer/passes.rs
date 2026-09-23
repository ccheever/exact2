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
            pass.set_bind_group(1, &shadows.cameras[i], &[]);
            pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
            pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
            let view = 1 + i as u32;
            for (index, group) in self.cull.groups.iter().enumerate() {
                if group.flags & (1 << view) == 0 {
                    continue;
                }
                if let Some(material) = self.model_material(group.batch) {
                    if let Some(custom) = custom.iter().find(|c| c.material == material) {
                        let binds = self.custom_bindings.as_ref().unwrap();
                        pass.set_pipeline(&custom.shadow);
                        pass.set_bind_group(1, &shadows.cameras[i], &[]);
                        pass.set_bind_group(2, &custom.resources, &[]);
                        pass.set_bind_group(3, &binds.instances.as_ref().unwrap().1, &[]);
                    } else {
                        let material = &self.models.materials[material.0];
                        pass.set_pipeline(
                            self.pipelines.models.as_ref().unwrap().shadow[usize::from(
                                material.double_sided,
                            ) + 2 * usize::from(
                                group.mirrored,
                            )]
                            .as_ref()
                            .unwrap(),
                        );
                        pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                        pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                    }
                } else {
                    pass.set_pipeline(&self.pipelines.shadow[usize::from(group.mirrored)]);
                }
                self.draw_group(&mut pass, view, index);
                draws += 1;
            }
        }
        draws
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
                    store: if state.scene_copy {
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
            timestamp_writes: timing::writes(frame.timestamps, 3),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        viewport(&mut pass, state.size);
        let variant = state.variant;
        pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        for (index, group) in self.cull.groups.iter().enumerate() {
            if let Some(material) = self.model_material(group.batch) {
                if let Some(custom) = hooks.materials().iter().find(|c| c.material == material) {
                    let binds = self.custom_bindings.as_ref().unwrap();
                    pass.set_pipeline(&custom.forward);
                    pass.set_bind_group(1, &binds.empty_bind, &[]);
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
                    pass.set_bind_group(
                        1,
                        self.shadows
                            .as_ref()
                            .map_or_else(|| self.models.no_shadow.as_ref().unwrap(), |s| &s.sample),
                        &[],
                    );
                    pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                    pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                }
            } else {
                pass.set_pipeline(
                    &self.pipelines.forward[variant + 4 * usize::from(group.mirrored)],
                );
                if let Some(shadows) = &self.shadows {
                    pass.set_bind_group(1, &shadows.sample, &[]);
                }
            }
            if group.range.start != self.batches[group.batch].slots.start {
                state.draws += 1;
            }
            self.draw_group(&mut pass, 0, index);
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
            timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 18, true);
            state.times[2] = start.map(|s| s.elapsed());
            viewport(&mut pass, state.size);
        }
        if frame::has_sky(frame) {
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
            timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 19, true);
            state.times[3] = start.map(|s| s.elapsed());
        }
        if !state.scene_copy {
            state.draws += self.translucent(&mut pass, frame, variant, state.size);
        }
        Ok(())
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
        targets.resolve_depth(encoder, &self.queue, frame, state.size, true);
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
            timestamp_writes: timing::writes(frame.timestamps, 20),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        viewport(&mut pass, state.size);
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
            targets.resolve_depth(encoder, &self.queue, frame, state.size, false);
            if state.needs.contains(crate::Needs::FINAL_DEPTH) {
                state.draws += 1;
            }
            if let Some(inputs) = targets.post(&self.targets.resolved) {
                let start = (!time.seekable).then(crate::perf::Stamp::now);
                timing::encoder_stamp(&self.device, encoder, frame.timestamps, 21, false);
                hooks
                    .post(encoder, &inputs, view)
                    .map_err(|e| hook_error("post", e))?;
                timing::encoder_stamp(&self.device, encoder, frame.timestamps, 21, true);
                state.times[5] = start.map(|s| s.elapsed());
            }
        }
        if let Some(bloom) = &self.bloom {
            state.draws += bloom.encode(
                &self.queue,
                encoder,
                &self.pipelines,
                frame.timestamps,
                state.size,
            );
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
            timestamp_writes: timing::writes(frame.timestamps, 15),
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
                pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
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

pub(super) fn hook_error(stage: &str, error: RenderError) -> RenderError {
    RenderError::Scene(format!("render hook {stage}: {error}"))
}
