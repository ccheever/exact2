use super::*;
use crate::hooks::CustomMaterial;

/// Whether cached casters' depth enters each frame's map by a depth-only
/// triangle inside the dynamic caster pass, which saves a copy and an
/// attachment load (on a Pixel 5, 0.18 ms a frame). A fragment shader that
/// writes only `frag_depth` from a depth texture it loads is core WebGPU,
/// Vulkan, Metal and D3D12; GL and WebGL2 copy and load instead.
pub(crate) fn seeds(device: &wgpu::Device) -> bool {
    device.adapter_info().backend != wgpu::Backend::Gl
}

pub(crate) struct StaticShadowCache {
    /// The seed's pipeline and a bind group a layer ([`seeds`]); none where
    /// the cache is copied.
    seed: Option<(wgpu::RenderPipeline, Vec<wgpu::BindGroup>)>,
    texture: wgpu::Texture,
    pub layers: Vec<wgpu::TextureView>,
    signature: Vec<u32>,
    scratch: Vec<u32>,
    pub valid: bool,
    pub encoded: std::cell::Cell<bool>,
    pub groups: Vec<bool>,
    pub dirty: bool,
}

impl StaticShadowCache {
    /// Seed tile depth in the dynamic caster pass, avoiding a texture copy
    /// followed by an attachment load. The unfiltered f32 depth stays exact.
    /// False where the cache is copied instead ([`seeds`]).
    pub fn seed(&self, pass: &mut wgpu::RenderPass<'_>, layer: usize) -> bool {
        let Some((pipeline, bindings)) = &self.seed else {
            return false;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bindings[layer], &[]);
        pass.draw(0..3, 0..1);
        true
    }

    /// Copy layer `layer` into `map`'s, for a pass that loads it, where the
    /// cache is not seeded; false where it is.
    pub fn copy(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        map: &wgpu::Texture,
        layer: usize,
    ) -> bool {
        if self.seed.is_some() {
            return false;
        }
        let copy = |texture| wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer as u32,
            },
            aspect: wgpu::TextureAspect::DepthOnly,
        };
        encoder.copy_texture_to_texture(
            copy(&self.texture),
            copy(map),
            wgpu::Extent3d {
                width: self.texture.width(),
                height: self.texture.height(),
                depth_or_array_layers: 1,
            },
        );
        true
    }

    pub fn new(device: &wgpu::Device, count: u32, resolution: u32, seed: bool) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game cached sun casters"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | if seed {
                    wgpu::TextureUsages::TEXTURE_BINDING
                } else {
                    wgpu::TextureUsages::COPY_SRC
                },
            view_formats: &[],
        });
        let layers: Vec<wgpu::TextureView> = (0..count)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self {
            seed: seed.then(|| seed_pipeline(device, &layers)),
            texture,
            layers,
            signature: Vec::new(),
            scratch: Vec::new(),
            valid: false,
            encoded: std::cell::Cell::new(false),
            groups: Vec::new(),
            dirty: true,
        }
    }
}

/// The seed's pipeline, and a bind group for each of `layers`.
fn seed_pipeline(
    device: &wgpu::Device,
    layers: &[wgpu::TextureView],
) -> (wgpu::RenderPipeline, Vec<wgpu::BindGroup>) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sun cache seed"),
        source: wgpu::ShaderSource::Wgsl(crate::pipeline::SHADOW_SEED.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sun cache seed"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sun cache seed"),
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sun cache seed"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            }),
        ),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[],
        }),
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let binds = layers
        .iter()
        .map(|view| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sun cache seed"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                }],
            })
        })
        .collect();
    (pipeline, binds)
}

impl<const ASSETS: bool> RendererWithAssets<ASSETS> {
    pub(super) fn prepare_shadow_cache(
        &mut self,
        cascades: Option<&Cascades>,
        frame: &FrameInput<'_>,
        custom: &[CustomMaterial],
    ) {
        let enabled = self.shadows.as_ref().is_some_and(|s| s.cache_enabled)
            && self.model_poses_current
            && custom.is_empty();
        if !enabled {
            if let Some(shadows) = &mut self.shadows {
                shadows.cached = None;
            }
            return;
        }
        let cascades = cascades.unwrap();
        let (mut signature, mut groups) = self
            .shadows
            .as_mut()
            .unwrap()
            .cached
            .as_mut()
            .map(|c| {
                (
                    std::mem::take(&mut c.scratch),
                    std::mem::take(&mut c.groups),
                )
            })
            .unwrap_or_default();
        signature.clear();
        groups.clear();
        signature.extend([self.mesh_uploads as u32, (self.mesh_uploads >> 32) as u32]);
        for matrix in &cascades.matrices[..cascades.count as usize] {
            signature.extend(matrix.to_cols_array().map(f32::to_bits));
        }
        groups.extend(self.cull.groups.iter().enumerate().map(|(index, group)| {
            let Some(material) = self.model_material(group.batch) else {
                return false;
            };
            let material = &self.models.materials[material.0];
            if group.flags & 14 == 0
                || group.flags & crate::cull::KEEP_ALL != 0
                || material.alpha != exact_game::asset::AlphaMode::Opaque
                || self.batch_fading(group.batch)
            {
                return false;
            }
            let stationary = group.range.clone().all(|at| {
                let slot = self.slot_list[at as usize];
                if slot < crate::RENDER_SLOT_BASE {
                    return false;
                }
                let index = (slot - crate::RENDER_SLOT_BASE) as usize;
                let record = &self.models.records[index];
                if record.skin.is_some()
                    || frame
                        .attachments
                        .iter()
                        .any(|a| a.entity.index() == record.transform)
                {
                    return false;
                }
                self.models
                    .pose_indices
                    .get(index)
                    .and_then(|&at| self.models.poses.get(at))
                    .is_some_and(|p| p[0] == p[1])
            });
            if !stationary {
                return false;
            }
            signature.extend([
                index as u32,
                group.flags,
                u32::from(group.mirrored),
                u32::from(material.double_sided),
                group.range.start,
                group.range.end,
            ]);
            for at in group.range.clone() {
                let index = (self.slot_list[at as usize] - crate::RENDER_SLOT_BASE) as usize;
                let record = &self.models.records[index];
                let pose = self.models.poses[self.models.pose_indices[index]][0];
                signature.extend([
                    record.transform,
                    record.geometry.0 as u32,
                    record.material.0 as u32,
                    u32::from(self.levels.hidden.get(index).copied().unwrap_or(false)),
                ]);
                signature.extend(record.local.to_cols_array().map(f32::to_bits));
                signature.extend(pose.position.to_array().map(f32::to_bits));
                signature.extend(pose.rotation.to_array().map(f32::to_bits));
                signature.extend(pose.scale.to_array().map(f32::to_bits));
            }
            true
        }));
        let look = self.probe.is_some();
        let shadows = self.shadows.as_mut().unwrap();
        if !groups.iter().any(|&b| b) {
            shadows.cached = None;
            return;
        }
        let cache = shadows.cached.get_or_insert_with(|| {
            self.texture_creations += 1;
            StaticShadowCache::new(
                &self.device,
                shadows.count,
                shadows.resolution,
                shadows.seed,
            )
        });
        // Exact words avoid hash collisions; moving/skinned/fading draws never
        // enter the signature. A fixed sun box therefore survives camera motion.
        // A look frame draws the cached casters again, so that its shadow
        // records count this frame's casters (LLP 1127.000).
        cache.dirty = !cache.valid || cache.signature != signature || look;
        if cache.dirty {
            cache.valid = false;
        }
        cache.encoded.set(false);
        cache.scratch = std::mem::replace(&mut cache.signature, signature);
        cache.groups = groups;
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "shadow_cache_tests.rs"]
mod tests;
