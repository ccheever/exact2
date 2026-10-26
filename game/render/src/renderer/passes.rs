//! The frame's passes, each reading one resolved frame state.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d3-optional-services-the-scene-copy
use super::draw::Resolved;
use super::probe::{Drew, Probe, Stage};
use super::*;
use crate::hooks::CustomMaterial;

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    pub(super) fn model_material(&self, batch: usize) -> Option<crate::MaterialId> {
        if ASSETS {
            self.model_batches[batch]
        } else {
            None
        }
    }
    /// The world's depth range, or the viewmodel layer's: while any viewmodel
    /// draws, it takes the nearest `VIEWMODEL_DEPTH` and the world the rest.
    /// Where the viewmodel layer's depth ends this frame: zero without one.
    pub(crate) fn depth_split(&self) -> f32 {
        if self.viewmodels {
            crate::VIEWMODEL_DEPTH
        } else {
            0.
        }
    }
    pub(crate) fn scene_viewport(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        size: (u32, u32),
        viewmodel: bool,
    ) {
        let split = self.depth_split();
        let (near, far) = if viewmodel { (0., split) } else { (split, 1.) };
        pass.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32, near, far);
        pass.set_scissor_rect(0, 0, size.0, size.1);
    }
    /// Whether a batch's records are posed by skin palettes, as a crowd's
    /// avatars are. Those skip the depth prepass: small, many and moving, a
    /// second vertex pass over them costs more than the overdraw it saves.
    pub(crate) fn batch_skinned(&self, batch: usize) -> bool {
        let start = self.batches[batch].slots.start as usize;
        self.slot_list.get(start).is_some_and(|&slot| {
            slot >= crate::RENDER_SLOT_BASE
                && self.models.records[(slot - crate::RENDER_SLOT_BASE) as usize]
                    .skin
                    .is_some()
        })
    }
    pub(crate) fn draw_group<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'_>,
        bound: &mut Bound<'a>,
        view: u32,
        group: usize,
    ) {
        let began = self.probe_begin(pass, || Drew::Group {
            group,
            view,
            variant: None,
        });
        if self.cull.direct {
            let mesh = &self.meshes[self.batches[self.cull.groups[group].batch].mesh.0];
            bound.scene(pass, &self.scene_binds[self.current], 0);
            bound.flush(pass);
            pass.draw_indexed(
                mesh.indices.clone(),
                mesh.base_vertex,
                self.cull.groups[group].range.clone(),
            );
        } else {
            let (offset, draw) = self.cull.draw(view, group);
            bound.scene(pass, &self.culled_binds[self.current], offset);
            bound.indirect(pass, &self.cull.indirect.raw, draw);
        }
        Probe::end(pass, began);
    }

    pub(crate) fn draw_outline_group<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'_>,
        bound: &mut Bound<'a>,
        group: usize,
    ) {
        if self.cull.direct {
            let mesh = &self.meshes[self.batches[self.cull.groups[group].batch].mesh.0];
            bound.scene(pass, &self.scene_binds[self.current], 0);
            pass.draw_indexed(
                mesh.indices.clone(),
                0,
                self.cull.groups[group].range.clone(),
            );
        } else {
            let (region, _) = self.cull.draw(0, group);
            bound.scene(pass, &self.culled_binds[self.current], region);
            pass.draw_indexed_indirect(&self.cull.indirect.raw, self.cull.hull_draw(group));
        }
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
        // A look frame draws every caster itself and leaves the cache as the
        // live frames left it (`prepare_shadow_cache`).
        let cache = shadows.cached.as_ref().filter(|_| self.probe.is_none());
        for i in 0..shadows.count as usize {
            let view = 1 + i as u32;
            self.probe_stage(Stage::SunShadow(i as u8));
            let stamped = cache.is_some()
                && timing::encoder_stamp(&self.device, encoder, frame.timestamps, i as u32, false);
            if let Some(cache) = cache {
                if cache.dirty {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("game static sun casters"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &cache.layers[i],
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
                    pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
                    let mut bound = Bound::merging(self.probe.is_none());
                    for (index, group) in self.cull.groups.iter().enumerate() {
                        if !cache.groups[index] || group.flags & (1 << view) == 0 {
                            continue;
                        }
                        self.set_depth_pipeline(
                            &mut pass,
                            &mut bound,
                            group,
                            &shadows.cameras[i],
                            custom,
                        );
                        self.draw_group(&mut pass, &mut bound, view, index);
                        draws += 1;
                    }
                    bound.flush(&mut pass);
                }
            }
            let copied = cache.is_some_and(|c| c.copy(encoder, shadows.view.texture(), i));
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game sun shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadows.layers[i],
                    depth_ops: Some(wgpu::Operations {
                        load: if copied {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(1.)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timing::span(frame.timestamps, i as u32, !stamped, true),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if cache.is_some_and(|c| c.seed(&mut pass, i)) {
                draws += 1;
            }
            if frame.timestamps.is_some() {
                self.mark(i as u32);
            }
            pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
            pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
            let mut bound = Bound::merging(self.probe.is_none());
            for (index, group) in self.cull.groups.iter().enumerate() {
                if group.flags & (1 << view) == 0 || cache.is_some_and(|c| c.groups[index]) {
                    continue;
                }
                self.set_depth_pipeline(&mut pass, &mut bound, group, &shadows.cameras[i], custom);
                self.draw_group(&mut pass, &mut bound, view, index);
                draws += 1;
            }
            bound.flush(&mut pass);
        }
        if let Some(cache) = cache {
            cache.encoded.set(true);
        }
        draws
    }

    /// Last frame's camera lists of convex primitives, depth only at this
    /// frame's poses, then the occlusion pyramid this frame's cull reads
    /// (occlusion.rs).
    pub(super) fn encode_occluders(&self, encoder: &mut wgpu::CommandEncoder) {
        let occlusion = self.occlusion.as_ref().unwrap();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game occluders"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: occlusion.depth(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(occlusion.occluder());
        pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut bound = Bound::default();
        self.probe_stage(Stage::Occluders);
        for &(index, offset) in &occlusion.groups {
            pass.set_bind_group(1, &occlusion.uniforms_bind, &[offset]);
            self.draw_group(&mut pass, &mut bound, 0, index);
        }
        drop(pass);
        occlusion.reduce(encoder);
    }

    /// A depth-only caster pipeline and its groups 1-3 for one draw group.
    pub(super) fn set_depth_pipeline<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'_>,
        bound: &mut Bound<'a>,
        group: &crate::cull::Group,
        camera: &'a wgpu::BindGroup,
        custom: &'a [CustomMaterial],
    ) {
        if let Some(material) = self.model_material(group.batch) {
            if let Some(custom) = custom.iter().find(|c| c.material == material) {
                let binds = self.custom_bindings.as_ref().unwrap();
                bound.pipeline(pass, &custom.shadow);
                self.probe_variant(usize::MAX);
                bound.group(pass, 2, &custom.resources);
                bound.group(pass, 3, &binds.instances.as_ref().unwrap().1);
                bound.single();
            } else {
                let material = &self.models.materials[material.0];
                let opaque = material.alpha == exact_game::asset::AlphaMode::Opaque
                    && !self.batch_fading(group.batch);
                let key = 4 * usize::from(material.double_sided)
                    + 16 * usize::from(group.mirrored)
                    + 32 * usize::from(opaque);
                self.probe_variant(key);
                bound.pipeline(pass, self.pipelines.model_variant(&self.device, key, true));
                bound.group(pass, 2, self.depth_material(material, opaque));
                bound.group(pass, 3, self.models.bind.as_ref().unwrap());
            }
        } else {
            self.probe_variant(usize::from(group.mirrored));
            bound.pipeline(pass, &self.pipelines.shadow[usize::from(group.mirrored)]);
        }
        bound.group(pass, 1, camera);
    }

    /// Group 2 of a model's depth program: an opaque one reads no material.
    pub(super) fn depth_material<'a>(
        &'a self,
        material: &'a crate::models::Material,
        opaque: bool,
    ) -> &'a wgpu::BindGroup {
        if opaque {
            self.pipelines.no_material()
        } else {
            material.bind.as_ref().unwrap()
        }
    }

    /// Opaque groups and quads, the hook's opaque stage, sky and background; with
    /// no scene copy the translucent pass continues here.
    pub(super) fn encode_forward<H: crate::Hooks>(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        state: &mut Resolved,
        frame: &FrameInput<'_>,
        hooks: &mut H,
        view: &crate::FrameView<'_>,
    ) -> Result<(), RenderError> {
        let time = view.time;
        // Soft particles: translucency follows in its own pass over this depth.
        let split = self.quads.split();
        let sky = frame
            .environment
            .background
            .map_or(frame.environment.horizon, |c| c.0);
        // A direct frame draws into the target, through its own MSAA attachment at
        // 4x, which resolves into the target after the frame's last pass.
        let direct = state.direct.then(|| self.direct_attachments());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game forward"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: direct.map_or_else(
                    || {
                        if self.targets.samples == 1 && state.scene_copy {
                            self.hook_targets.as_ref().unwrap().scene().unwrap().color
                        } else {
                            &self.targets.color
                        }
                    },
                    |d| d.color.as_ref().unwrap_or(target),
                ),
                resolve_target: if self.targets.samples == 1 || split {
                    None
                } else if direct.is_some() {
                    Some(target)
                } else {
                    Some(
                        self.hook_targets
                            .as_ref()
                            .and_then(|t| t.scene())
                            .map_or(&self.targets.resolved, |scene| scene.color),
                    )
                },
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(match direct {
                        Some(d) => frame::tone(sky, frame, !d.format.is_srgb()),
                        None => frame::clear_color(frame),
                    }),
                    store: if self.targets.samples == 1 || state.scene_copy || split {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: direct.map_or(&self.targets.depth, |d| &d.depth),
                depth_ops: Some(wgpu::Operations {
                    load: if self.prepass_active && !state.direct {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    },
                    store: if state.depth_read {
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
            occlusion_query_set: self.probe_queries(Stage::Forward),
            multiview_mask: None,
        });
        if state.direct {
            state.draws += self.draw_depth_prepass(&mut pass, state.size, true);
        }
        self.probe_stage(Stage::Forward);
        self.scene_viewport(&mut pass, state.size, false);
        let mut layer = false;
        let variant = state.variant;
        pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut bound = Bound::merging(self.probe.is_none());
        bound.group(&mut pass, 1, &self.shadow_sample);
        for (index, group) in self.cull.groups.iter().enumerate() {
            if let Some(material) = self.model_material(group.batch) {
                if let Some(custom) = hooks.materials().iter().find(|c| c.material == material) {
                    let binds = self.custom_bindings.as_ref().unwrap();
                    bound.pipeline(&mut pass, &custom.forward);
                    bound.group(&mut pass, 2, &custom.resources);
                    bound.group(&mut pass, 3, &binds.instances.as_ref().unwrap().1);
                    bound.single();
                } else {
                    let material = &self.models.materials[material.0];
                    let prepassed = self.prepass_active && !self.batch_skinned(group.batch);
                    let key = variant
                        + crate::pipeline::PREPASS * usize::from(prepassed)
                        + 64 * usize::from(material.untextured)
                        + 128 * usize::from(self.shadows.as_ref().is_some_and(|s| s.count == 1))
                        + 4 * usize::from(material.double_sided)
                        + 16 * usize::from(group.mirrored)
                        + 32 * usize::from(
                            material.alpha == exact_game::asset::AlphaMode::Opaque
                                && !self.batch_fading(group.batch),
                        )
                        + crate::pipeline::MODEL_DIRECT * usize::from(state.direct)
                        + crate::pipeline::MODEL_BILINEAR * usize::from(state.bilinear)
                        + crate::pipeline::MODEL_LIGHTS
                            * usize::from(self.lights.info[0] != 0. || frame.fill.is_some());
                    self.probe_variant(key);
                    let pipeline = self.pipelines.model_variant(&self.device, key, false);
                    bound.pipeline(&mut pass, pipeline);
                    bound.group(&mut pass, 2, material.bind.as_ref().unwrap());
                    bound.group(&mut pass, 3, self.models.bind.as_ref().unwrap());
                }
            } else {
                let key = variant
                    + 4 * usize::from(group.mirrored)
                    + (state.primitive
                        & if self.batch_fading(group.batch) {
                            !crate::pipeline::EARLY
                        } else {
                            usize::MAX
                        })
                    + crate::pipeline::PREPASS * usize::from(self.prepass_active);
                self.probe_variant(key);
                bound.pipeline(&mut pass, self.pipelines.forward_variant(&self.device, key));
            }
            if group.range.start != self.batches[group.batch].slots.start {
                state.draws += 1;
            }
            let viewmodel = self.batches[group.batch].viewmodel;
            if viewmodel != layer {
                bound.flush(&mut pass);
                self.scene_viewport(&mut pass, state.size, viewmodel);
                layer = viewmodel;
            }
            self.draw_group(&mut pass, &mut bound, 0, index);
        }
        bound.flush(&mut pass);
        drop(bound);
        if layer {
            self.scene_viewport(&mut pass, state.size, false);
        }
        for (index, _) in
            (self.quads.draws.iter().enumerate()).filter(|(_, d)| self.quads.opaque(d))
        {
            let pipeline =
                (self.quads).bind::<ASSETS>(&mut pass, index, &self.models.textures, state.direct);
            let began = self.probe_begin(&mut pass, || self.quads.record(index, pipeline));
            self.quads.submit(&mut pass, index);
            Probe::end(&mut pass, began);
            state.draws += 1;
        }
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 18, false);
            self.probe_note(|| Drew::Summary("hook opaque"));
            hooks
                .opaque(&mut pass, view)
                .map_err(|e| hook_error("opaque", e))?;
            if timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 18, true) {
                self.mark(18);
            }
            state.times[2] = start.map(|s| s.elapsed());
            self.scene_viewport(&mut pass, state.size, false);
        }
        let hulls = self.draw_outlines(&mut pass, state.size, state.direct);
        if hulls.draws > 0 {
            self.probe_note(|| Drew::Hulls { draws: hulls.draws });
        }
        state.draws += hulls.draws;
        state.hull_instances += hulls.instances;
        state.hull_triangles += hulls.triangles;
        if frame::has_sky(frame, self.sky_ready) {
            pass.set_pipeline(if state.direct {
                self.pipelines.sky_direct(&self.device)
            } else {
                &self.pipelines.sky
            });
            pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
            pass.draw(0..3, 0..1);
            self.probe_note(|| Drew::Fixed("sky"));
            state.draws += 1;
        }
        if H::ENABLED {
            let start = (!time.seekable).then(crate::perf::Stamp::now);
            timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 19, false);
            self.probe_note(|| Drew::Summary("hook background"));
            hooks
                .background(&mut pass, view)
                .map_err(|e| hook_error("background", e))?;
            if timing::pass_stamp(&self.device, &mut pass, frame.timestamps, 19, true) {
                self.mark(19);
            }
            state.times[3] = start.map(|s| s.elapsed());
        }
        if frame.water.is_some() {
            // A background hook may have left its own viewport and scissor.
            self.scene_viewport(&mut pass, state.size, false);
            let above = crate::water::from_above(frame);
            (self.water).draw_surface(&mut pass, &self.scene_binds[self.current], above);
            self.probe_note(|| Drew::Fixed("water surface"));
            state.draws += 1;
        }
        if !state.scene_copy && !split {
            timing::pass_stamp(
                &self.device,
                &mut pass,
                frame.timestamps,
                timing::TRANSLUCENT,
                false,
            );
            state.draws += self.translucent(&mut pass, frame, state, state.size);
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
    /// sampled by soft particles and a look frame's particle counts: the HDR
    /// image, or a direct frame's target (`direct_attachments`).
    pub(super) fn encode_translucent(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        state: &mut Resolved,
        frame: &FrameInput<'_>,
    ) {
        let direct = state.direct.then(|| self.direct_attachments());
        let (color, depth) = direct.map_or((&self.targets.color, &self.targets.depth), |d| {
            (d.color.as_ref().unwrap_or(target), &d.depth)
        });
        let resolved = direct.map_or(&self.targets.resolved, |_| target);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game translucent"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: (self.targets.samples > 1).then_some(resolved),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: if self.targets.samples == 1 {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: {
                if frame.timestamps.is_some() {
                    self.mark(timing::TRANSLUCENT);
                }
                timing::writes(frame.timestamps, timing::TRANSLUCENT)
            },
            occlusion_query_set: self.probe_queries(Stage::Translucent),
            multiview_mask: None,
        });
        self.probe_stage(Stage::Translucent);
        self.scene_viewport(&mut pass, state.size, false);
        state.draws += self.translucent(&mut pass, frame, state, state.size);
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
        if self.targets.samples == 1 {
            encoder.copy_texture_to_texture(
                targets.scene().unwrap().color.texture().as_image_copy(),
                self.targets.color.texture().as_image_copy(),
                self.targets.color.texture().size(),
            );
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game surface + translucent"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.color,
                resolve_target: (self.targets.samples > 1).then_some(&self.targets.resolved),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: if self.targets.samples == 1 {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: if state.needs.contains(crate::Needs::FINAL_DEPTH) || state.depth_after {
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
            occlusion_query_set: self.probe_queries(Stage::Translucent),
            multiview_mask: None,
        });
        self.probe_stage(Stage::Translucent);
        self.scene_viewport(&mut pass, state.size, false);
        let start = (!time.seekable).then(crate::perf::Stamp::now);
        self.probe_note(|| Drew::Summary("hook surface"));
        hooks
            .surface(&mut pass, &targets.scene().unwrap(), view)
            .map_err(|e| hook_error("surface", e))?;
        state.times[4] = start.map(|s| s.elapsed());
        state.draws += self.translucent(&mut pass, frame, state, state.size);
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
                self.probe_note(|| Drew::Summary("hook post"));
                hooks
                    .post(encoder, &inputs, view)
                    .map_err(|e| hook_error("post", e))?;
                if timing::encoder_stamp(&self.device, encoder, frame.timestamps, 21, true) {
                    self.mark(21);
                }
                state.times[5] = start.map(|s| s.elapsed());
            }
        }
        if self.bloom.is_some() {
            self.probe_note(|| Drew::Summary("bloom"));
        }
        self.probe_note(|| Drew::Fixed("tone"));
        if let Some(bloom) = &self.bloom {
            let passes = bloom.encode(
                &self.queue,
                encoder,
                &self.pipelines,
                frame.timestamps,
                state.size,
            );
            if frame.timestamps.is_some() {
                // Levels down from pair 4, then back up from pair 10.
                let levels = passes.div_ceil(2);
                for level in 0..levels {
                    self.mark(4 + level);
                    if level + 1 < levels {
                        self.mark(10 + level);
                    }
                }
            }
            state.draws += passes;
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
        state: &Resolved,
        size: (u32, u32),
    ) -> u32 {
        let variant = state.variant;
        self.scene_viewport(pass, size, false);
        let mut layer = false;
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut draws = 0;
        for (index, draw) in
            (self.quads.draws.iter().enumerate()).filter(|(_, d)| !self.quads.opaque(d))
        {
            // Blended viewmodel parts keep the viewmodel layer's depth range.
            let viewmodel = matches!(draw.kind, crate::quads::Kind::Model(index, _)
                if self.batches[index].viewmodel)
                || matches!(
                    draw.kind,
                    crate::quads::Kind::Particle(_, _, _, true)
                        | crate::quads::Kind::Primitive(true)
                );
            if viewmodel != layer {
                self.scene_viewport(pass, size, viewmodel);
                layer = viewmodel;
            }
            if let crate::quads::Kind::Model(batch_index, slot) = draw.kind {
                let index = batch_index;
                assert!(ASSETS, "model in primitive executor");
                let batch = &self.batches[index];
                let material = &self.models.materials[self.model_batches[index].unwrap().0];
                let mirrored = self.slot_mirrored(self.slot_list[slot as usize], frame);
                let key = variant
                    + 64 * usize::from(material.untextured)
                    + 128 * usize::from(self.shadows.as_ref().is_some_and(|s| s.count == 1))
                    + 4 * usize::from(material.double_sided)
                    + 8
                    + 16 * usize::from(mirrored)
                    + crate::pipeline::MODEL_DIRECT * usize::from(state.direct)
                    + crate::pipeline::MODEL_BILINEAR * usize::from(state.bilinear)
                    + crate::pipeline::MODEL_LIGHTS
                        * usize::from(self.lights.info[0] != 0. || frame.fill.is_some());
                self.probe_variant(key);
                pass.set_pipeline(self.pipelines.model_variant(&self.device, key, false));
                pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
                pass.set_bind_group(1, &self.shadow_sample, &[]);
                pass.set_bind_group(2, material.bind.as_ref().unwrap(), &[]);
                pass.set_bind_group(3, self.models.bind.as_ref().unwrap(), &[]);
                pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
                let mesh = &self.meshes[batch.mesh.0];
                let began = self.probe_begin(pass, || Drew::Blended {
                    batch: index,
                    variant: None,
                    mirrored,
                });
                pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, slot..slot + 1);
                Probe::end(pass, began);
            } else {
                let pipeline =
                    (self.quads).bind::<ASSETS>(pass, index, &self.models.textures, state.direct);
                let began = self.probe_begin(pass, || self.quads.record(index, pipeline));
                self.quads.submit(pass, index);
                Probe::end(pass, began);
            }
            draws += 1;
        }
        if layer {
            self.scene_viewport(pass, size, false);
        }
        draws
    }
}

/// What a run of draws has bound, so it sets only what changes. Group 0, the
/// scene group with its dynamic `slots` offset, is `draw_group`'s.
///
/// A merging run (`Bound::merging`) also holds back consecutive indirect draws
/// over the same state and issues them as one multi-draw when the state next
/// changes or the run is flushed; its loop flushes before anything else it sets
/// on the pass, and at its end. A binning GPU replays a pass's draw commands
/// in each bin, so fewer commands cost it less.
#[derive(Default)]
pub(crate) struct Bound<'a> {
    pipeline: Option<&'a wgpu::RenderPipeline>,
    groups: [Option<&'a wgpu::BindGroup>; 4],
    offset: u32,
    merge: bool,
    /// The next draw goes out alone (`Bound::single`).
    single: bool,
    /// Held draws: the arguments' buffer, the first one's byte offset, the count.
    run: Option<(&'a wgpu::Buffer, u64, u32)>,
}
impl<'a> Bound<'a> {
    /// A run that merges, unless a look frame's probe brackets each draw.
    pub fn merging(merge: bool) -> Self {
        Self {
            merge,
            ..Self::default()
        }
    }
    /// Issue the next draw alone: a custom material's shader may read
    /// `draw_index`, which a multi-draw counts through its run.
    pub fn single(&mut self) {
        self.single = true;
    }
    /// Issue the held draws.
    pub fn flush(&mut self, pass: &mut wgpu::RenderPass<'_>) {
        match self.run.take() {
            Some((buffer, offset, 1)) => pass.draw_indexed_indirect(buffer, offset),
            Some((buffer, offset, count)) => {
                pass.multi_draw_indexed_indirect(buffer, offset, count)
            }
            None => {}
        }
    }
    /// An indexed indirect draw, held to join the next when this run merges.
    pub fn indirect(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        buffer: &'a wgpu::Buffer,
        offset: u64,
    ) {
        const ARGS: u64 = size_of::<wgpu::util::DrawIndexedIndirectArgs>() as u64;
        if !self.merge || std::mem::take(&mut self.single) {
            self.flush(pass);
            pass.draw_indexed_indirect(buffer, offset);
            return;
        }
        if let Some((held, first, count)) = &mut self.run {
            if *held == buffer && *first + u64::from(*count) * ARGS == offset {
                *count += 1;
                return;
            }
        }
        self.flush(pass);
        self.run = Some((buffer, offset, 1));
    }
    pub fn pipeline(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &'a wgpu::RenderPipeline,
    ) {
        if self.pipeline != Some(pipeline) {
            self.flush(pass);
            pass.set_pipeline(pipeline);
            self.pipeline = Some(pipeline);
        }
    }
    pub fn group(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        index: u32,
        group: &'a wgpu::BindGroup,
    ) {
        if self.groups[index as usize] != Some(group) {
            self.flush(pass);
            pass.set_bind_group(index, group, &[]);
            self.groups[index as usize] = Some(group);
        }
    }
    pub(super) fn scene(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        group: &'a wgpu::BindGroup,
        offset: u32,
    ) {
        if self.groups[0] != Some(group) || self.offset != offset {
            self.flush(pass);
            pass.set_bind_group(0, group, &[offset]);
            self.groups[0] = Some(group);
            self.offset = offset;
        }
    }
}

/// A loop that ends without its flush loses the draws it held.
impl Drop for Bound<'_> {
    fn drop(&mut self) {
        debug_assert!(
            self.run.is_none() || std::thread::panicking(),
            "a merging run was dropped holding draws"
        );
    }
}

#[cfg(test)]
#[path = "bound_tests.rs"]
mod bound_tests;

pub(super) fn hook_error(stage: &str, error: RenderError) -> RenderError {
    RenderError::Scene(format!("render hook {stage}: {error}"))
}
