//! The wind fixture's render hooks (app.json `game.render`): reeds sway in a
//! custom vertex material, shaded as the engine shades their textured MASK
//! material, and a dusk sky replaces the engine's. Presentation only: the hooks
//! read the world and never step or change it.
use exact_game_render::hooks::{CustomMaterial, Pipelines};
use exact_game_render::{FrameView, HookGpu, Hooks, Needs, RenderError, RenderWorld};
use exact_gpu::wgpu;

/// The shader inventory (game.render.shaders): every shader under `shaders/`
/// after its declared preludes (app.json gpu.shaderPreludes), reflected at
/// build. The text travels as assets and reloads live; `<name>::module()`
/// reads the registered text, so nothing is assembled at run time.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

/// The assembled inventory as the hosts register it, for tests that load it.
#[cfg(not(target_arch = "wasm32"))]
pub fn shader_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("EXACT_GAME_SHADERS"))
}

/// How far the wind bends a reed's tip (wind.wgsl), at the strongest gust: the
/// GPU cull grows each reed's bounds by this, so a bent reed is never culled.
pub const REACH: f32 = 0.6;

#[derive(Default)]
pub struct Wind {
    reeds: Pipelines<Vec<CustomMaterial>>,
    uniform: Option<wgpu::Buffer>,
    sky: Pipelines<wgpu::RenderPipeline>,
    frame: Option<wgpu::BindGroup>,
}

impl Wind {
    /// The reed material over the registered wind_forward/wind_shadow modules,
    /// validated as one candidate and replaced on each shader generation.
    fn reeds(&mut self, gpu: &HookGpu<'_>, generation: u32) {
        let Some(materials) = gpu.materials.as_ref() else {
            return;
        };
        let Some(material) = materials.material("reed.model", 0) else {
            return;
        };
        let ready = [shaders::wind_forward::NAME, shaders::wind_shadow::NAME]
            .iter()
            .all(|name| exact_gpu::shaders::shader_source(name).is_some());
        if !ready {
            return;
        }
        let uniform = self
            .uniform
            .get_or_insert_with(|| {
                gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("wind"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .clone();
        self.reeds.update(gpu.device, generation, || {
            let layout = gpu
                .device
                .create_bind_group_layout(&shaders::wind_forward::GROUP_2);
            let forward = gpu
                .device
                .create_shader_module(shaders::wind_forward::module());
            let shadow = gpu
                .device
                .create_shader_module(shaders::wind_shadow::module());
            vec![CustomMaterial {
                material,
                forward: materials.pipeline(
                    material,
                    &forward,
                    &layout,
                    shaders::wind_forward::entry::WIND_VS,
                    Some(shaders::wind_forward::entry::WIND_FS),
                    false,
                ),
                shadow: materials.pipeline(
                    material,
                    &shadow,
                    &layout,
                    shaders::wind_shadow::entry::WIND_SHADOW,
                    Some(shaders::wind_shadow::entry::WIND_CUTOUT),
                    true,
                ),
                resources: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("wind"),
                    layout: &layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                }),
                reach: REACH,
            }]
        });
    }
}

impl Hooks for Wind {
    fn prepare(
        &mut self,
        gpu: &HookGpu<'_>,
        view: &FrameView<'_>,
        world: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        self.reeds(gpu, view.time.shader_generation);
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
        if self.sky.pending() || self.reeds.pending() {
            Needs::ANIMATE | Needs::PENDING
        } else {
            Needs::ANIMATE
        }
    }
    fn pending_reason(&self) -> &str {
        "wind and sky pipelines validating"
    }
    fn error(&self) -> Option<&str> {
        self.sky.error().or(self.reeds.error())
    }
    fn materials(&self) -> &[CustomMaterial] {
        self.reeds.get().map_or(&[], Vec::as_slice)
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
