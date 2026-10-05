//! Slot-indexed, opaque PBR rendering over plain buffers; no simulation ownership.
//!
//! Transforms are ten floats (position xyz, quaternion xyzw, scale xyz), materials
//! twelve (linear base rgb, alpha/negative grid spacing, metallic, roughness, linear emissive rgb, primitive dimensions xyz).
//! Rotations must be unit quaternions. New slots must be initialized in both ticks.
//! Matrices use WebGPU's 0–1 depth range, with near at zero (for example
//! [`glam::camera::rh::proj::directx::perspective`]). All lights and environment colours are linear.
#![deny(missing_docs)]
#![deny(unsafe_code)]

// Shared GPU test helpers use the same renderer path inside and outside this crate.
#[cfg(test)]
extern crate self as exact_game_render;

mod bloom;
mod buffers;
mod cull;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod cull_tests;
mod frame;
pub mod hooks;
mod ibl;
mod lights;
mod local_shadows;
mod lod;
mod model_pipeline;
mod models;
mod perf;
mod pipeline;
mod placed;
#[cfg(test)]
mod quad_tests;
mod quads;
mod renderer;
mod shadows;
pub mod shapes;
mod skinning;
mod ssao;
mod surface;
mod timing;
mod trace;
pub mod world;

/// Simulation vocabulary, also used by module! without a direct dependency.
pub use exact_game;
/// GPU surface ABI, also used by module! without a direct dependency.
pub use exact_gpu;
pub use hooks::{
    FrameView, HookGpu, HookTime, HookWork, Hooks, Needs, PostInputs, RenderWorld, SceneCopy,
};
pub use models::{surface_words, ModelExecutor};
pub use surface::{Executor, WorldSurface};
pub use world::scene::{DisplayedAttachment, PHOTOMETRIC_SCALE};
pub use world::Feed;

use exact_gpu::wgpu;
use glam::{Mat4, Vec3};
/// Full renderer for direct clients. Game modules select their concrete capability.
pub type Renderer = renderer::RendererWithAssets<true>;
use std::ops::Range;
pub use timing::{GPU_PASS_COUNT, GPU_PASS_NAMES};

/// A refused capacity request or invalid scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// A slot outside the arena's exclusive limit.
    Capacity {
        /// Arena name.
        arena: &'static str,
        /// Highest zero-based requested slot.
        slot: u64,
        /// Exclusive slot limit.
        limit: u64,
    },
    /// Named scene validation failure.
    Scene(String),
}
impl RenderError {
    pub(crate) fn scene(detail: String) -> Self {
        Self::Scene(detail)
    }
}
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scene(detail) => f.write_str(detail),
            Self::Capacity { arena, slot, limit } => {
                write!(f, "{arena} arena: slot {slot} exceeds limit {limit}")
            }
        }
    }
}
impl std::error::Error for RenderError {}

/// A mesh in one renderer's append-only arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshId(pub(crate) usize);

/// A renderer-owned baked material; primitives use their existing per-entity floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaterialId(pub(crate) usize);
/// Render slots live outside the entity index space and do not allocate entities.
pub const RENDER_SLOT_BASE: u32 = 1 << 31;
/// Maximum vertex-stage storage bindings: five scene buffers plus three model buffers.
pub const STORAGE_BINDINGS: u32 = SCENE_STORAGE_BINDINGS + 3;
/// Scene storage shared by the primitive and model pipelines.
pub const SCENE_STORAGE_BINDINGS: u32 = 5;
/// One model mesh node under an entity. Geometry/material are batch keys; the
/// transform slot and full affine offset are uploaded into the instance buffer.
#[derive(Debug, Clone)]
pub struct DrawInstance {
    /// Entity transform and tint slot (the existing page upload stays unchanged).
    pub transform: u32,
    /// Mesh geometry in this renderer.
    pub geometry: MeshId,
    /// Baked material in this renderer.
    pub material: MaterialId,
    /// Presentation-only custom material payload; zero for ordinary drawing.
    pub data: u32,
    /// Composed model node offset.
    pub local: Mat4,
    /// Renderer-owned skin template; absent for unskinned nodes.
    pub skin: Option<u32>,
    /// This node's base-colour multiplier (`NodeMaterials`); `[1; 4]` keeps it.
    pub tint: [f32; 4],
    /// Linear emission added to this node; zero adds none.
    pub glow: [f32; 3],
    /// `MaterialOverride`'s metallic, roughness and `Shimmer`, packed as the
    /// model shader reads them ([`surface_words`]); `[0; 4]` changes nothing.
    pub surface: [u32; 4],
}

/// One tightly packed, 48-byte mesh vertex.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Local position.
    pub position: [f32; 3],
    /// Local unit normal.
    pub normal: [f32; 3],
    /// Primitive deformation: capsule cap sign and capsule flag; zero for other meshes.
    pub uv: [f32; 2],
    /// Linear RGBA multiplier; white preserves the material.
    pub color: [f32; 4],
}

/// One indexed draw; the range addresses the supplied slot list, not the slots.
#[derive(Debug, Clone)]
pub struct Batch {
    /// Geometry shared by all instances in this draw.
    pub mesh: MeshId,
    /// Instance range in the renderer's persistent slot list.
    pub slots: Range<u32>,
    /// Whether this draw participates in sun shadow passes; true in [`Batch::new`].
    pub casts_shadows: bool,
    /// Drawn in the viewmodel layer: the nearest [`VIEWMODEL_DEPTH`] of the depth
    /// range, in front of the whole world; false in [`Batch::new`].
    pub viewmodel: bool,
    /// The level of detail its instances draw at (0, their own model, in
    /// [`Batch::new`]). Each frame one level per entity draws; direct drawing
    /// (no GPU cull) draws level 0 only.
    pub level: u8,
}

/// The depth range the viewmodel layer takes while any viewmodel draws; the
/// world draws behind it in the rest. Without a viewmodel the world takes 0–1.
pub const VIEWMODEL_DEPTH: f32 = 0.05;

impl Batch {
    /// A retained draw that casts shadows by default.
    pub fn new(mesh: MeshId, slots: Range<u32>) -> Self {
        Self {
            mesh,
            slots,
            casts_shadows: true,
            viewmodel: false,
            level: 0,
        }
    }
    /// This draw in the viewmodel layer, casting no shadows.
    pub fn viewmodel(self) -> Self {
        Self {
            casts_shadows: false,
            viewmodel: true,
            ..self
        }
    }
}

/// Directional light, optionally casting stable cascaded shadows.
#[derive(Debug, Clone, Copy)]
pub struct Sun {
    /// Direction the light travels (towards the scene); nonzero.
    pub direction: Vec3,
    /// Linear RGB tint.
    pub color: Vec3,
    /// Incident radiance: `Feed` supplies lux times [`PHOTOMETRIC_SCALE`].
    pub illuminance: f32,
    /// None skips all shadow work and releases the shadow textures.
    pub shadows: Option<Shadows>,
}

impl Default for Sun {
    fn default() -> Self {
        Self {
            direction: Vec3::new(1.0, -2.0, -1.0),
            color: Vec3::ONE,
            illuminance: 3.0,
            shadows: Some(Shadows::default()),
        }
    }
}

/// Sun shadow quality and reach. Up to three 2048² depth layers.
#[derive(Debug, Clone, Copy)]
pub struct Shadows {
    /// Camera view depth reached by shadows, in metres (default 60).
    pub distance: f32,
    /// Number of cascades, clamped to 1–3 (default 3).
    pub cascades: u32,
    /// 3×3 PCF radius in shadow texels (default 1.5).
    pub softness: f32,
}
impl Default for Shadows {
    fn default() -> Self {
        Self {
            distance: 60.0,
            cascades: 3,
            softness: 1.5,
        }
    }
}

pub use exact_game::{Bloom, Environment, Fog};

/// An equirectangular environment texture ([`FrameInput::environment_map`]).
#[derive(Debug, Clone, Copy)]
pub struct EnvironmentMapInput<'a> {
    /// A texture added with `add_texture` (or delivered as a `.tex` asset).
    pub texture: &'a str,
    /// Linear radiance multiplier.
    pub intensity: f32,
    /// RGBM range, or zero for plain RGB.
    pub rgbm: f32,
    /// Also draw it as the visible sky.
    pub visible: bool,
    /// Yaw about +Y in radians.
    pub rotation: f32,
}

/// Local lights drawn per frame, nearest the camera first. Further eligible
/// lights are counted in `FrameInput::lights_dropped`, never silently lost.
pub const MAX_LIGHTS: usize = 256;

/// An inverse-square point or spot light, smoothly extinguished at its range.
#[derive(Debug, Clone, Copy, Default)]
pub struct LightInput {
    /// World-space position.
    pub position: Vec3,
    /// Linear RGB tint.
    pub color: Vec3,
    /// Renderer radiance at 1 m: `Feed` supplies `PointLight.intensity` (candela)
    /// times [`PHOTOMETRIC_SCALE`], the sun's lux scale.
    pub intensity: f32,
    /// Positive cutoff distance in world units.
    pub range: f32,
    /// Unit direction a spot light points (its −Z); ignored without a cone.
    pub direction: Vec3,
    /// A spot light's cosines of its inner and outer half-angles: full intensity
    /// inside the inner, smoothly zero at the outer. None is a point light.
    pub cone: Option<[f32; 2]>,
    /// Wants a shadow map; the renderer shadows the first few such lights.
    pub shadows: bool,
}

/// Retained emissive tween and material for one entity; only presentation samples it.
/// The shoulder compresses only Glow output above both 2 and the authored channel;
/// intensity 1 preserves authored emission, including values greater than 2.
#[derive(Clone)]
pub struct GlowInput {
    /// Material slot (entity index).
    pub slot: u32,
    pub(crate) material: [f32; 12],
    pub(crate) tween: exact_game::Tween,
    pub(crate) hz: u32,
    pub(crate) model: bool,
}
impl GlowInput {
    pub(crate) fn material_at(&self, seconds: f64) -> [f32; 12] {
        let intensity = self.tween.value_at(seconds, self.hz);
        let mut material = self.material;
        if self.model {
            material[9] = intensity.max(0.);
        }
        for value in &mut material[6..9] {
            let emissive = *value * intensity;
            let shoulder = value.max(2.);
            let excess = (emissive - shoulder).max(0.);
            *value = emissive.min(shoulder) + excess / (1. + excess * 0.5);
        }
        material
    }
}

/// Constant-size displayed-frame input; transforms stay in the tick buffers.
pub struct FrameInput<'a> {
    /// Retained emissive curves, sampled without a world query.
    pub glows: &'a [GlowInput],
    /// Displayed simulation time in seconds.
    pub seconds: f64,
    /// World-to-view matrix.
    pub view: Mat4,
    /// View-to-clip matrix, conventional 0–1 depth, not reverse-Z.
    pub proj: Mat4,
    /// World-space eye position.
    pub camera_position: Vec3,
    /// Tick interpolation fraction, clamped to 0–1.
    pub alpha: f32,
    /// Optional directional light.
    pub sun: Option<Sun>,
    /// A second, never shadowed directional light: the moon, or a fill.
    pub fill: Option<Sun>,
    /// Point and spot lights, nearest first; at most [`MAX_LIGHTS`] are used.
    pub lights: &'a [LightInput],
    /// Eligible lights left out beyond [`MAX_LIGHTS`].
    pub lights_dropped: usize,
    /// Displayed socket attachments, evaluated from the interpolated local chain.
    pub attachments: &'a [DisplayedAttachment],
    /// Hemisphere lighting and background.
    pub environment: Environment,
    /// An authored environment map lighting the scene in place of the sky:
    /// a texture added under this name, its intensity and RGBM range.
    pub environment_map: Option<EnvironmentMapInput<'a>>,
    /// Screen-space ambient occlusion; None (the default) draws none.
    pub ambient_occlusion: Option<exact_game::AmbientOcclusion>,
    /// Optional pass timestamps. Requires TIMESTAMP_QUERY on the device.
    /// Reserve [`GPU_PASS_COUNT`] pairs in the query set. Resolve/read outside draw.
    pub timestamps: Option<&'a wgpu::QuerySet>,
    /// How far above SDR white the tone curve reaches (LLP 1100 D12b); 1 for
    /// an SDR target.
    pub headroom: f32,
}

impl Default for FrameInput<'_> {
    fn default() -> Self {
        Self {
            glows: &[],
            seconds: 0.,
            view: Mat4::IDENTITY,
            proj: glam::camera::rh::proj::directx::perspective(
                60f32.to_radians(),
                16.0 / 9.0,
                0.1,
                100.0,
            ),
            camera_position: Vec3::ZERO,
            alpha: 1.0,
            sun: Some(Sun::default()),
            fill: None,
            lights: &[],
            lights_dropped: 0,
            attachments: &[],
            environment: Environment::default(),
            environment_map: None,
            ambient_occlusion: None,
            timestamps: None,
            headroom: 1.0,
        }
    }
}

/// Work submitted by a frame, including its fullscreen tonemap triangle.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// Engine-issued draws, including shadows, sky, bloom and tonemapping.
    /// Raw hook draws are outside these counters.
    pub draws: u32,
    /// Forward mesh instances (shadow draws do not count again).
    pub instances: u64,
    /// Forward mesh triangles plus the tonemap triangle (baseline comparable).
    pub triangles: u64,
    /// Cumulative attachment textures created by this renderer, including bloom/shadows.
    pub texture_creations: u64,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[test]
    fn every_shader_validates_without_an_adapter() {
        use exact_gpu::wgpu::naga;
        for source in super::pipeline::shader_sources() {
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap();
        }
    }
}

/// Export one game's surface and the native or wasm GPU module ABI.
#[macro_export]
macro_rules! module {
    ($game:ty, hooks = $hooks:ty) => { $crate::module!($game, render_hooks (), false, $hooks, &[]); };
    ($game:ty, hooks = $hooks:ty, shaders = $shaders:expr) => { $crate::module!($game, render_hooks (), false, $hooks, $shaders); };
    ($game:ty, assets, hooks = $hooks:ty, shaders = $shaders:expr) => { $crate::module!($game, render_hooks $crate::ModelExecutor, true, $hooks, $shaders); };
    ($game:ty, assets, hooks = $hooks:ty) => { $crate::module!($game, render_hooks $crate::ModelExecutor, true, $hooks, &[]); };
    ($game:ty) => { $crate::module!($game, hook (), false); };
    ($game:ty, assets) => { $crate::module!($game, hook $crate::ModelExecutor, true); };
    ($game:ty, audio) => { $crate::module!($game, audio_mode false); };
    ($game:ty, audio, assets) => { $crate::module!($game, audio_mode true); };
    ($game:ty, audio, hooks = $hooks:ty, shaders = $shaders:expr) => { $crate::module!($game, audio_mode false, $hooks, $shaders); };
    ($game:ty, audio, assets, hooks = $hooks:ty, shaders = $shaders:expr) => { $crate::module!($game, audio_mode true, $hooks, $shaders); };
    ($game:ty, audio, hooks = $hooks:ty) => { $crate::module!($game, audio_mode false, $hooks, &[]); };
    ($game:ty, audio, assets, hooks = $hooks:ty) => { $crate::module!($game, audio_mode true, $hooks, &[]); };
    ($game:ty, audio_mode $assets:tt) => { $crate::module!($game, audio_mode $assets, (), &[]); };
    ($game:ty, audio_mode $assets:tt, $hooks:ty, $shaders:expr) => {
        #[derive(Default)]
        struct GameAudio(exact_game_audio::SurfacePlayer, Option<$crate::exact_game::audio::Sounds>);
        impl $crate::Executor for GameAudio {
            fn wants_audio(&self) -> bool { true }
            fn clock(&mut self, seekable: bool) { self.0.clock(seekable); }
            fn suspend(&mut self, suspended: bool) {
                if let Err(error) = self.0.suspend(suspended) { eprintln!("{error}"); }
            }
            fn sync(&mut self, world: &$crate::exact_game::World, generation: u64, playing: bool, seekable: bool) {
                self.0.sync(world, generation, playing, seekable);
            }
            fn before_restore(&mut self, world: &$crate::exact_game::World, mode: $crate::exact_gpu::Restore) {
                self.1 = if mode == $crate::exact_gpu::Restore::Carry { world.try_resource::<$crate::exact_game::audio::Sounds>().map(|s| s.clone()) } else { None };
            }
            fn after_restore(&mut self, world: &mut $crate::exact_game::World, mode: $crate::exact_gpu::Restore) {
                if mode == $crate::exact_gpu::Restore::Carry {
                    if let Some(fresh) = self.1.take() {
                        if world.try_resource::<$crate::exact_game::audio::Sounds>().is_some() { world.resource_mut::<$crate::exact_game::audio::Sounds>().0.extend(fresh.0); }
                    }
                }
            }
            fn unlock(&mut self) { self.0.unlock(); }
        }
        $crate::module!($game, audio_hook GameAudio, $assets, $hooks, $shaders);
    };
    ($game:ty, audio_hook $hook:ty, false, $hooks:ty, $shaders:expr) => { $crate::module!($game, render_hooks $hook, false, $hooks, $shaders); };
    ($game:ty, audio_hook $hook:ty, true, $hooks:ty, $shaders:expr) => { $crate::module!($game, render_hooks $crate::ModelExecutor<$hook>, true, $hooks, $shaders); };
    ($game:ty, hook $hook:ty, $assets:literal) => {
        $crate::module!($game, render_hooks $hook, $assets, (), &[]);
    };
    ($game:ty, render_hooks $hook:ty, $assets:literal, $hooks:ty, $shaders:expr) => {
        /// The game's sole surface; shaders are embedded in the renderer.
        pub static REGISTRY: $crate::exact_gpu::Registry = $crate::exact_gpu::Registry {
            surfaces: &[(
                <$game as $crate::exact_game::Game>::NAME,
                <<$game as $crate::exact_game::Game>::Args as $crate::exact_game::Args>::FIELDS
                    .len(),
                || Box::new($crate::WorldSurface::<$game, $hook, $assets, $hooks>::default()),
            )],
            shaders: $shaders,
        };
        $crate::exact_gpu::module!(REGISTRY);
    };
}

impl FrameInput<'_> {
    pub(crate) fn displayed_matrix(
        &self,
        entity: exact_game::Entity,
        fallback: exact_game::Transform,
    ) -> Mat4 {
        world::scene::displayed_matrix(self.attachments, entity, fallback)
    }
}

#[cfg(test)]
#[path = "../tests/fixture/mod.rs"]
mod test_game;

#[cfg(test)]
#[path = "../tests/fixture/model.rs"]
mod test_model;

#[cfg(test)]
#[path = "../tests/fixture/device.rs"]
mod test_device;

#[cfg(test)]
mod visibility_tests;
