//! Local light shadows: the frame's plan, its depth layers and their culls.
use super::*;
use crate::cull::{Cull, KEEP_ALL};

/// A cull over up to four local shadow views and the scene groups over its lists.
pub(crate) struct LocalCull {
    pub cull: Cull,
    binds: Option<([wgpu::BindGroup; 2], (wgpu::Buffer, u64))>,
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    /// Plan this frame's local shadow layers and keep their maps and culls.
    pub(super) fn prepare_local(&mut self, frame: &FrameInput<'_>) {
        self.local_plan.of(frame.lights);
        let views = self.local_plan.views.len() as u32;
        if views == 0 {
            return;
        }
        // Grow only: a flashlight toggled off and on keeps its layers.
        if self.local.as_ref().is_none_or(|l| l.count < views) {
            self.texture_creations += 1;
            self.local = Some(crate::local_shadows::LocalMaps::new(
                &self.device,
                views,
                &self.pipelines.camera_layout,
            ));
            self.shadow_sample_stale = true;
        }
        self.local
            .as_ref()
            .unwrap()
            .write(&self.queue, &self.local_plan.views);
        while self.local_culls.len() < views.div_ceil(crate::cull::VIEWS) as usize {
            self.local_culls.push(LocalCull {
                cull: Cull::new(&self.device),
                binds: None,
            });
        }
    }

    /// Group 1 of every forward pass: cascades, comparison sampler, local maps.
    pub(super) fn refresh_shadow_sample(&mut self) {
        if !std::mem::take(&mut self.shadow_sample_stale) {
            return;
        }
        fn view<'a>(
            v: Option<&'a wgpu::TextureView>,
            placeholder: &'a wgpu::TextureView,
        ) -> wgpu::BindingResource<'a> {
            wgpu::BindingResource::TextureView(v.unwrap_or(placeholder))
        }
        let placeholder = &self.shadow_placeholder;
        self.shadow_sample = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game shadow sample"),
            layout: &self.pipelines.shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view(self.shadows.as_ref().map(|s| &s.view), placeholder),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_comparison),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view(self.local.as_ref().map(|l| &l.sample), placeholder),
                },
            ],
        });
    }

    /// The local culls' groups: every caster in each of its four views.
    pub(super) fn local_groups(&mut self) {
        let used = self
            .local_plan
            .views
            .len()
            .div_ceil(crate::cull::VIEWS as usize);
        for (i, local) in self.local_culls.iter_mut().enumerate() {
            local.cull.groups.clear();
            if i >= used {
                continue;
            }
            local.cull.epoch = self.cull.epoch;
            let views = (self.local_plan.views.len() - i * 4).min(4);
            let bits = (1 << views) - 1;
            for group in &self.cull.groups {
                let casts = self.batches[group.batch].casts_shadows;
                local.cull.groups.push(crate::cull::Group {
                    flags: if casts { bits } else { 0 } | (group.flags & KEEP_ALL),
                    ..group.clone()
                });
            }
        }
    }
    pub(super) fn locals_stale(&self) -> bool {
        self.local_culls
            .iter()
            .any(|l| !l.cull.groups.is_empty() && l.cull.stale())
    }
    /// Upload the main setup's words, with each group's view flags, to each local cull.
    pub(super) fn local_setups(&mut self, words: &[u32], items: u32, records: u32, skins: u32) {
        for local in &mut self.local_culls {
            if local.cull.groups.is_empty() {
                continue;
            }
            local.cull.begin_setup();
            local.cull.words.extend_from_slice(words);
            for (index, group) in local.cull.groups.iter().enumerate() {
                let at = index * crate::cull::GROUP_WORDS + 7;
                local.cull.words[at] = (local.cull.words[at] & !15) | (group.flags & 15);
            }
            local
                .cull
                .finish_setup(&self.device, &self.queue, items, records, skins);
        }
    }
    /// Bind each used local cull and write its views.
    pub(super) fn local_views(&mut self) {
        let items = self.slot_list.len() as u32;
        for (i, local) in self.local_culls.iter_mut().enumerate() {
            if local.cull.groups.is_empty() || local.cull.direct {
                continue;
            }
            local.cull.bind(
                &self.device,
                &self.uniform,
                [&self.transforms[0].raw, &self.transforms[1].raw],
                &self.materials.raw,
                &self.slots.raw,
                &self.attachment_matrices.raw,
                self.models.skinning.as_ref().map(|s| &s.palette.raw),
            );
            let key = (local.cull.compacted.raw.clone(), local.cull.window);
            if local.binds.as_ref().is_none_or(|(_, old)| *old != key) {
                let binds = scene_binds(
                    &self.device,
                    &self.pipelines.scene_layout,
                    &self.uniform,
                    &self.transforms,
                    &self.materials,
                    (&local.cull.compacted.raw, Some(local.cull.window)),
                    &self.attachment_matrices,
                    &self.environment,
                    &self.lights,
                );
                local.binds = Some((binds, key));
            }
            let views = &self.local_plan.views[i * 4..(i * 4 + 4).min(self.local_plan.views.len())];
            local.cull.write_views(&self.queue, views, items);
        }
    }
    /// The scene groups changed buffers: rebuild every local cull's on next use.
    pub(super) fn rebind_locals(&mut self) {
        for local in &mut self.local_culls {
            local.binds = None;
        }
    }

    pub(super) fn encode_local_culls(&self, encoder: &mut wgpu::CommandEncoder) {
        for local in &self.local_culls {
            if !local.cull.groups.is_empty() {
                local.cull.encode(encoder, self.current, None);
            }
        }
    }

    /// One depth pass per local shadow layer over that layer's culled casters.
    pub(super) fn encode_local_shadows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        custom: &[crate::hooks::CustomMaterial],
    ) -> u32 {
        let Some(maps) = &self.local else {
            return 0;
        };
        let mut draws = 0;
        for layer in 0..self.local_plan.views.len() {
            let local = &self.local_culls[layer / 4];
            let view = (layer % 4) as u32;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game local shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &maps.layers[layer],
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
            pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
            pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
            for (index, group) in local.cull.groups.iter().enumerate() {
                if group.flags & (1 << view) == 0 {
                    continue;
                }
                self.set_depth_pipeline(&mut pass, group, &maps.cameras[layer], custom);
                if self.cull.direct {
                    let mesh = &self.meshes[self.batches[group.batch].mesh.0];
                    pass.set_bind_group(0, &self.scene_binds[self.current], &[0]);
                    pass.draw_indexed(mesh.indices.clone(), mesh.base_vertex, group.range.clone());
                } else {
                    let (region, draw) = local.cull.offsets(view, index);
                    let binds = &local.binds.as_ref().unwrap().0;
                    pass.set_bind_group(0, &binds[self.current], &[region]);
                    pass.draw_indexed_indirect(&local.cull.indirect.raw, draw);
                }
                draws += 1;
            }
        }
        draws
    }
}
