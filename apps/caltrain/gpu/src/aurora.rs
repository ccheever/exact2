//! Aurora: a full-screen fragment shader driven by the frame clock — the
//! canvas that wants a frame every frame (LLP 1009 D4), with a uniform
//! buffer behind an explicit bind-group layout.

use exact_gpu::json::text;
use exact_gpu::wgpu;
use exact_gpu::{Frame, Surface, SurfaceError, Value};

/// The shader, validated at build (`build.rs`).
pub const AURORA_WGSL: &str = include_str!("../shaders/aurora.wgsl");

/// The aurora surface: one input, a seed string (the selected station).
#[derive(Default)]
pub struct AuroraSurface {
    seed: f32,
    gpu: Option<Gpu>,
}

struct Gpu {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
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
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> {
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
        if self.gpu.as_ref().map(|g| g.format) != Some(format) {
            self.gpu = Some(build(device, format));
        }
        let gpu = self.gpu.as_ref().unwrap();
        let (w, h) = frame.pixels();
        let uniforms: [f32; 4] = [frame.now_ms as f32, w as f32, h as f32, self.seed];
        queue.write_buffer(
            &gpu.uniforms,
            0,
            &uniforms
                .iter()
                .flat_map(|f| f.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
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
}

fn build(device: &wgpu::Device, format: wgpu::TextureFormat) -> Gpu {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("aurora"),
        source: wgpu::ShaderSource::Wgsl(AURORA_WGSL.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("aurora uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(16),
            },
            count: None,
        }],
    });
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("aurora uniforms"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("aurora"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniforms.as_entire_binding(),
        }],
    });
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
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
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
        pipeline,
        uniforms,
        bind_group,
    }
}
