//! The wind fixture's render hooks (app.json `game.presentation`): reeds sway in a
//! custom vertex material and a dusk sky replaces the engine's. Presentation
//! only: the hooks read the world and never step or change it.
use exact_game_render::hooks::{CustomMaterial, Pipelines, MATERIAL_SHADOWS_WGSL, MATERIAL_WGSL};
use exact_game_render::{FrameView, HookGpu, Hooks, Needs, RenderError, RenderWorld};
use exact_gpu::wgpu;

/// The shader pack under `shaders/` (game.presentation.shaders), reflected at build.
/// Its text travels as assets and reloads live; `sky::module()` reads it.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

/// Where the shader pack lives in the source tree, for tests that register it.
#[cfg(not(target_arch = "wasm32"))]
pub fn shader_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders")
}

/// The reed material composes with the engine's WGSL prelude, so it is compiled
/// in rather than shipped in the pack: the pack's shaders validate on their own.
const WIND: &str = include_str!("wind.wgsl");
const FORWARD: &str = include_str!("wind_forward.wgsl");
const SHADOW: &str = include_str!("wind_shadow.wgsl");

#[derive(Default)]
pub struct Wind {
    materials: Vec<CustomMaterial>,
    uniform: Option<wgpu::Buffer>,
    sky: Pipelines<wgpu::RenderPipeline>,
    frame: Option<wgpu::BindGroup>,
}

impl Wind {
    fn reeds(&mut self, gpu: &HookGpu<'_>) {
        let Some(materials) = gpu.materials.as_ref() else {
            return;
        };
        let Some(material) = materials.material("reed.model", 0) else {
            return;
        };
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wind"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wind"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = |label, text: String| {
            gpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(label),
                    source: wgpu::ShaderSource::Wgsl(text.into()),
                })
        };
        let forward = module(
            "wind forward",
            format!("{MATERIAL_WGSL}\n{MATERIAL_SHADOWS_WGSL}\n{WIND}\n{FORWARD}"),
        );
        let shadow = module("wind shadow", format!("{MATERIAL_WGSL}\n{WIND}\n{SHADOW}"));
        self.materials.push(CustomMaterial {
            material,
            forward: materials.pipeline(&forward, &layout, "wind_vs", Some("wind_fs"), false),
            shadow: materials.pipeline(&shadow, &layout, "wind_shadow", None, true),
            resources: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wind"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            }),
        });
        self.uniform = Some(uniform);
    }
}

impl Hooks for Wind {
    fn prepare(
        &mut self,
        gpu: &HookGpu<'_>,
        view: &FrameView<'_>,
        world: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        if self.materials.is_empty() {
            self.reeds(gpu);
        }
        if let Some(uniform) = &self.uniform {
            // Simulation time, interpolated: the same frame always shows the same gust.
            let time = (world.tick() as f32 + view.frame.alpha) / world.hz() as f32;
            let gust = world
                .get::<wind_fixture_logic::Gust>("wind")
                .map_or(1., |g| g.0);
            let words = [time, gust, 0., 0.].map(f32::to_le_bytes).concat();
            gpu.queue.write_buffer(uniform, 0, &words);
        }
        if exact_gpu::shaders::shader_source(shaders::sky::NAME).is_some() {
            self.sky
                .update(gpu.device, view.time.shader_generation, || {
                    let module = gpu.device.create_shader_module(shaders::sky::module());
                    let layout =
                        gpu.device
                            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                                label: Some("dusk sky"),
                                bind_group_layouts: &[Some(gpu.frame_layout)],
                                immediate_size: 0,
                            });
                    sky_pipeline(gpu.device, &layout, &module, view)
                });
        }
        self.frame = Some(gpu.frame_binding.clone());
        Ok(())
    }
    fn needs(&self) -> Needs {
        // The wind blows while the simulation rests.
        if self.sky.pending() {
            Needs::ANIMATE | Needs::PENDING
        } else {
            Needs::ANIMATE
        }
    }
    fn pending_reason(&self) -> &str {
        "dusk sky pipeline validating"
    }
    fn error(&self) -> Option<&str> {
        self.sky.error()
    }
    fn materials(&self) -> &[CustomMaterial] {
        &self.materials
    }
    fn background(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        _: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        if let (Some(sky), Some(frame)) = (self.sky.get(), &self.frame) {
            pass.set_pipeline(sky);
            pass.set_bind_group(0, frame, &[]);
            pass.draw(0..3, 0..1);
        }
        Ok(())
    }
}

/// The engine's own sky state: at the far plane, tested but never written.
fn sky_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    view: &FrameView<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dusk sky"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(shaders::sky::entry::SKY_VS),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(shaders::sky::entry::SKY_FS),
            targets: &[Some(view.color_format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: view.depth_format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: view.sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}
