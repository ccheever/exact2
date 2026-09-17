use crate::{pipeline::Pipelines, timing};
use exact_gpu::wgpu;

pub(crate) struct BloomTargets {
    pub levels: Vec<wgpu::TextureView>,
    pub binds: Vec<wgpu::BindGroup>,
    source: wgpu::BindGroup,
}

impl BloomTargets {
    pub fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        pipelines: &Pipelines,
        uniform: &wgpu::Buffer,
        hdr: &wgpu::TextureView,
    ) -> Self {
        // RGBA16F is filterable and blendable in baseline WebGPU. No optional
        // Rg11b10Ufloat renderability/device feature is required from the host.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game bloom linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game bloom source"),
                layout: &pipelines.bloom_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let source = bind(hdr);
        let mut levels = Vec::with_capacity(6);
        let mut binds = Vec::with_capacity(6);
        let (mut width, mut height) = ((size.0 / 2).max(1), (size.1 / 2).max(1));
        loop {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("game bloom octave"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            binds.push(bind(&view));
            levels.push(view);
            if levels.len() == 6 || width / 2 < 8 || height / 2 < 8 {
                break;
            }
            width /= 2;
            height /= 2;
        }
        Self {
            levels,
            binds,
            source,
        }
    }

    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &Pipelines,
        timestamps: Option<&wgpu::QuerySet>,
    ) -> u32 {
        for i in 0..self.levels.len() {
            let bind = if i == 0 {
                &self.source
            } else {
                &self.binds[i - 1]
            };
            self.pass(
                encoder,
                &pipelines.bloom[usize::from(i != 0)],
                i,
                bind,
                false,
                timestamps,
                4 + i as u32,
            );
        }
        for i in (0..self.levels.len() - 1).rev() {
            self.pass(
                encoder,
                &pipelines.bloom[2],
                i,
                &self.binds[i + 1],
                true,
                timestamps,
                10 + i as u32,
            );
        }
        (self.levels.len() * 2 - 1) as u32
    }

    #[allow(clippy::too_many_arguments)]
    fn pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        level: usize,
        bind: &wgpu::BindGroup,
        add: bool,
        timestamps: Option<&wgpu::QuerySet>,
        slot: u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(if add {
                "game bloom up"
            } else {
                "game bloom down"
            }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.levels[level],
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if add {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timing::writes(timestamps, slot),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
