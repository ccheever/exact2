//! Aurora: a full-screen fragment shader driven by the frame clock — the
//! canvas that wants a frame every frame (LLP 1009 D4), with a uniform
//! buffer and the canvas's children behind the bind-group layout the shader
//! declares (`shaders::aurora`, reflected at build).

use crate::shaders::aurora::{entry, module, Uniforms, CHILDREN, CHILDREN_SAMPLER, GROUP_0, U};
use exact_gpu::json::text;
use exact_gpu::wgpu;
use exact_gpu::{Frame, Surface, SurfaceError, Value};

/// The aurora surface: one input, a seed string (the selected station).
#[derive(Default)]
pub struct AuroraSurface {
    seed: f32,
    gpu: Option<Gpu>,
    /// The canvas's children, painted by the host, when there are any (LLP 1014 D2).
    children: Option<wgpu::TextureView>,
    /// The bind group must be rebuilt around a new children texture.
    rebind: bool,
}

struct Gpu {
    format: wgpu::TextureFormat,
    /// The shader generation the pipeline was built at (LLP 1030 D8).
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// A 1×1 transparent texture: the children when there are none.
    blank: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl AuroraSurface {
    /// A fresh surface.
    pub fn new() -> AuroraSurface {
        AuroraSurface::default()
    }

    /// The seed for a string: a stable number in `[0, 1)`.
    pub fn seed_of(s: &str) -> f32 {
        let mut h: u32 = 2166136261;
        for b in s.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        (h % 1000) as f32 / 1000.0
    }
}

impl Surface for AuroraSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [seed] = inputs else {
            return Err(SurfaceError(format!(
                "aurora: expected 1 input, got {}",
                inputs.len()
            )));
        };
        self.seed = AuroraSurface::seed_of(&text(seed, "seed")?);
        Ok(())
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        if self.gpu.as_ref().map(|g| (g.format, g.generation))
            != Some((format, frame.shader_generation))
        {
            self.gpu = Some(build(device, queue, format, frame.shader_generation));
            self.rebind = true;
        }
        let children = self.children.as_ref();
        let gpu = self.gpu.as_mut().unwrap();
        if self.rebind {
            gpu.bind_group = bind_group(
                device,
                &gpu.layout,
                &gpu.uniforms,
                children.unwrap_or(&gpu.blank),
                &gpu.sampler,
            );
            self.rebind = false;
        }
        let (w, h) = frame.pixels();
        let uniforms = Uniforms {
            time: frame.now_ms as f32,
            width: w as f32,
            height: h as f32,
            seed: self.seed,
        };
        queue.write_buffer(&gpu.uniforms, 0, &uniforms.bytes());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aurora"),
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
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &gpu.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        // Lit from the clock: another frame, always.
        true
    }

    fn children_mode(&self) -> exact_gpu::ChildrenMode {
        exact_gpu::ChildrenMode::Composite { previous: false }
    }

    fn children(&mut self, texture: Option<&wgpu::TextureView>) {
        self.children = texture.cloned();
        self.rebind = true;
    }
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    children: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("aurora"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: U.binding,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: CHILDREN.binding,
                resource: wgpu::BindingResource::TextureView(children),
            },
            wgpu::BindGroupEntry {
                binding: CHILDREN_SAMPLER.binding,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn build(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    generation: u32,
) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("children"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let blank_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("no children"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &blank_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0, 0, 0, 0],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let blank = blank_texture.create_view(&Default::default());
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("aurora uniforms"),
        size: Uniforms::SIZE as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = bind_group(device, &layout, &uniforms, &blank, &sampler);
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("aurora"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("aurora"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(entry::VS),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry::FS),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    Gpu {
        format,
        generation,
        pipeline,
        uniforms,
        layout,
        sampler,
        blank,
        bind_group,
    }
}
