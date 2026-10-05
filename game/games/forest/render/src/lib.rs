//! The art pass's render hooks (app.json `game.render`): wind in the trees. Every
//! material of the baked tree models draws through one custom vertex material
//! that leans the tree about its base and flutters its branch tips, then shades
//! as the engine would (textures, the leaves' cutout, faded crowns). The far
//! levels stay the engine's. Presentation only: the hooks read the world's clock
//! and never step or change it; the greybox loads no tree model and draws as ever.
use exact_game_render::hooks::{CustomMaterial, Pipelines};
use exact_game_render::{FrameView, HookGpu, Hooks, MaterialId, Needs, RenderError, RenderWorld};
use exact_gpu::wgpu;

/// The shader inventory (game.render.shaders): every shader under `shaders/`
/// after its declared preludes (app.json gpu.shaderPreludes), reflected at build.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

/// The trees the art pass bakes (logic `art.rs`, `KINDS`), each loaded on sight.
const TREES: [&str; 5] = [
    "pine_a.model",
    "pine_b.model",
    "pine_c.model",
    "oak.model",
    "birch.model",
];

/// How far the wind moves a vertex, in model units (sway.wgsl): a 2.5% lean of
/// a tree about 10 m tall and the tips' flutter. The GPU cull grows by it.
pub const REACH: f32 = 0.4;

/// The forward and shadow pipelines, the group 2 layout and the wind's binding.
struct Set {
    forward: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    resources: wgpu::BindGroup,
}

#[derive(Default)]
pub struct Wind {
    set: Pipelines<Set>,
    uniform: Option<wgpu::Buffer>,
    /// The tree materials `materials` was made for, so arrivals extend it.
    shaded: Vec<MaterialId>,
    materials: Vec<CustomMaterial>,
}

impl Wind {
    fn prepare_set(&mut self, gpu: &HookGpu<'_>, generation: u32, any: MaterialId) {
        let ready = [shaders::tree_forward::NAME, shaders::tree_shadow::NAME]
            .iter()
            .all(|name| exact_gpu::shaders::shader_source(name).is_some());
        let Some(materials) = gpu.materials.as_ref().filter(|_| ready) else {
            return;
        };
        let uniform = self
            .uniform
            .get_or_insert_with(|| {
                gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("forest wind"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .clone();
        self.set.update(gpu.device, generation, || {
            let layout = gpu
                .device
                .create_bind_group_layout(&shaders::tree_forward::GROUP_2);
            let forward = gpu
                .device
                .create_shader_module(shaders::tree_forward::module());
            let shadow = gpu
                .device
                .create_shader_module(shaders::tree_shadow::module());
            // Every tree material is opaque or masked: one pair serves them all.
            Set {
                forward: materials.pipeline(
                    any,
                    &forward,
                    &layout,
                    shaders::tree_forward::entry::TREE_VS,
                    Some(shaders::tree_forward::entry::TREE_FS),
                    false,
                ),
                shadow: materials.pipeline(
                    any,
                    &shadow,
                    &layout,
                    shaders::tree_shadow::entry::TREE_SHADOW,
                    Some(shaders::tree_shadow::entry::TREE_CUTOUT),
                    true,
                ),
                resources: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("forest wind"),
                    layout: &layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                }),
            }
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
        let Some(materials) = gpu.materials.as_ref() else {
            return Ok(());
        };
        let shaded: Vec<MaterialId> = TREES
            .iter()
            .flat_map(|model| (0..).map_while(move |i| materials.material(model, i)))
            .collect();
        let Some(&any) = shaded.first() else {
            return Ok(());
        };
        let before = self.set.get().map(|s| s.forward.clone());
        self.prepare_set(gpu, view.time.shader_generation, any);
        let Some(set) = self.set.get() else {
            return Ok(());
        };
        if shaded != self.shaded || before.as_ref() != Some(&set.forward) {
            self.materials = shaded
                .iter()
                .map(|&material| CustomMaterial {
                    material,
                    forward: set.forward.clone(),
                    shadow: set.shadow.clone(),
                    resources: set.resources.clone(),
                    reach: REACH,
                })
                .collect();
            self.shaded = shaded;
        }
        // Simulation time, interpolated: the same frame always shows the same gust.
        let time = (world.tick() as f32 + view.frame.alpha) / world.hz() as f32;
        let words = [time, 0., 0., 0.].map(f32::to_le_bytes).concat();
        gpu.queue
            .write_buffer(self.uniform.as_ref().unwrap(), 0, &words);
        Ok(())
    }
    fn needs(&self) -> Needs {
        if self.set.pending() {
            Needs::PENDING
        } else {
            Needs::NONE
        }
    }
    fn pending_reason(&self) -> &str {
        "tree wind pipelines validating"
    }
    fn error(&self) -> Option<&str> {
        self.set.error()
    }
    fn materials(&self) -> &[CustomMaterial] {
        &self.materials
    }
}
