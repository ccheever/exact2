//! Optional screen-space ambient occlusion over the forward pass's depth.
//! Off unless the frame asks (`FrameInput::ambient_occlusion`), so the default
//! renderer allocates nothing and draws the same pixels.
use crate::buffers::{bytes, Targets};
use exact_gpu::wgpu;

pub(crate) struct Ssao {
    size: (u32, u32),
    scale: u32,
    uniform: wgpu::Buffer,
    occlusion: wgpu::TextureView,
    ao_bind: wgpu::BindGroup,
    apply_bind: wgpu::BindGroup,
    ao: wgpu::RenderPipeline,
    apply: wgpu::RenderPipeline,
}

impl Ssao {
    /// Over these targets, which must retain their depth; occlusion at 1/`scale`.
    pub fn new(device: &wgpu::Device, targets: &Targets, scale: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game SSAO"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.wgsl").into()),
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game SSAO"),
            size: 160,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let occlusion = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("game SSAO occlusion"),
                size: wgpu::Extent3d {
                    width: targets.size.0.div_ceil(scale),
                    height: targets.size.1.div_ceil(scale),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding, sample_type, multisampled| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
            count: None,
        };
        let depth = texture(1, wgpu::TextureSampleType::Depth, true);
        let occlusion_entry = texture(
            2,
            wgpu::TextureSampleType::Float { filterable: false },
            false,
        );
        let ao_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game SSAO depth"),
            entries: &[uniform_entry, depth],
        });
        // The upsample reads depth too, for its edge weights.
        let apply_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game SSAO apply"),
            entries: &[uniform_entry, depth, occlusion_entry],
        });
        let ao_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game SSAO depth"),
            layout: &ao_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&targets.depth),
                },
            ],
        });
        let apply_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game SSAO apply"),
            layout: &apply_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&targets.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&occlusion),
                },
            ],
        });
        let pipeline = |layout, entry, format, blend| {
            crate::pipeline::make_pipeline(
                device,
                &shader,
                "game SSAO",
                "vs",
                Some(entry),
                &[Some(layout)],
                &[],
                None,
                1,
                format,
                blend,
                &[],
                false,
            )
        };
        // destination × source: the blurred occlusion darkens the HDR colour.
        let multiply = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::Src,
            operation: wgpu::BlendOperation::Add,
        };
        Self {
            size: targets.size,
            scale,
            ao: pipeline(&ao_layout, "ao", wgpu::TextureFormat::R8Unorm, None),
            apply: pipeline(
                &apply_layout,
                "apply",
                wgpu::TextureFormat::Rgba16Float,
                Some(wgpu::BlendState {
                    color: multiply,
                    alpha: multiply,
                }),
            ),
            uniform,
            occlusion,
            ao_bind,
            apply_bind,
        }
    }

    pub fn fits(&self, targets: &Targets, scale: u32) -> bool {
        self.size == targets.size && self.scale == scale
    }

    /// Estimate occlusion from depth, then multiply it into `color`.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &crate::FrameInput<'_>,
        settings: exact_game::AmbientOcclusion,
        color: &wgpu::TextureView,
        size: (u32, u32),
        split: f32,
        timestamps: Option<&wgpu::QuerySet>,
    ) {
        let mut words = [0f32; 40];
        words[..16].copy_from_slice(&frame.proj.inverse().to_cols_array());
        words[16..32].copy_from_slice(&frame.proj.to_cols_array());
        words[32..36].copy_from_slice(&[
            size.0 as f32,
            size.1 as f32,
            settings.radius.max(1e-3),
            settings.intensity.max(0.),
        ]);
        let (_, samples) = settings.quality.plan();
        words[36..39].copy_from_slice(&[split, self.scale as f32, samples as f32]);
        queue.write_buffer(&self.uniform, 0, bytes(&words));
        for (i, (view, pipeline, bind, load)) in [
            (
                &self.occlusion,
                &self.ao,
                &self.ao_bind,
                wgpu::LoadOp::Clear(wgpu::Color::WHITE),
            ),
            (color, &self.apply, &self.apply_bind, wgpu::LoadOp::Load),
        ]
        .into_iter()
        .enumerate()
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game SSAO"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: crate::timing::writes(
                    timestamps,
                    [crate::timing::SSAO, crate::timing::SSAO_APPLY][i],
                ),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // The occlusion pass runs at the texture's downscaled size.
            let at = if i == 0 {
                (size.0.div_ceil(self.scale), size.1.div_ceil(self.scale))
            } else {
                size
            };
            crate::renderer::viewport(&mut pass, at);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
