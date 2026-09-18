//! Slot-indexed, opaque PBR rendering over plain buffers; no simulation ownership.
//!
//! Transforms are ten floats (position xyz, quaternion xyzw, scale xyz), materials
//! twelve (linear base rgb, alpha/negative grid spacing, metallic, roughness, linear emissive rgb, primitive dimensions xyz).
//! Rotations must be unit quaternions. New slots must be initialized in both ticks.
//! Matrices use WebGPU's 0–1 depth range, with near at zero (for example
//! [`glam::camera::rh::proj::directx::perspective`]). All lights and environment colours are linear.
#![deny(missing_docs)]
#![deny(unsafe_code)]

mod bloom;
mod buffers;
mod frame;
mod perf;
mod pipeline;
mod renderer;
mod shadows;
pub mod shapes;
mod surface;
mod timing;
mod trace;
pub mod world;

/// Simulation vocabulary, also used by module! without a direct dependency.
pub use exact_game;
/// GPU surface ABI, also used by module! without a direct dependency.
pub use exact_gpu;
pub use surface::{Presentation, WorldSurface};
pub use world::Feed;

use exact_gpu::wgpu;
use glam::{Mat4, Vec3};
pub use renderer::Renderer;
use std::ops::Range;
pub use timing::{GPU_PASS_COUNT, GPU_PASS_NAMES};

/// A refused arena capacity request. `limit` is an exclusive slot count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderError {
    /// Additional named scene refusal, if this is not an arena slot error.
    pub detail: Option<String>,
    /// Arena whose requested slot exceeds capacity.
    pub arena: &'static str,
    /// Highest zero-based slot requested.
    pub slot: u64,
    /// Maximum supported slot count (valid slots are below this value).
    pub limit: u64,
}

impl RenderError {
    pub(crate) fn scene(detail: String) -> Self {
        Self {
            arena: "mesh",
            slot: 0,
            limit: 4096,
            detail: Some(detail),
        }
    }
}
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(detail) = &self.detail {
            return f.write_str(detail);
        }
        write!(
            f,
            "{} arena: slot {} exceeds limit {}",
            self.arena, self.slot, self.limit
        )
    }
}
impl std::error::Error for RenderError {}

/// A mesh in one renderer's append-only arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshId(pub(crate) usize);

/// One tightly packed, 32-byte mesh vertex.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Local position.
    pub position: [f32; 3],
    /// Local unit normal.
    pub normal: [f32; 3],
    /// Primitive deformation: capsule cap sign and capsule flag; zero for other meshes.
    pub uv: [f32; 2],
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
}

impl Batch {
    /// A retained draw that casts shadows by default.
    pub fn new(mesh: MeshId, slots: Range<u32>) -> Self {
        Self {
            mesh,
            slots,
            casts_shadows: true,
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
    /// Incident illuminance multiplier.
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

pub use exact_game::{Bloom, Fog};

/// An inverse-square point light, smoothly extinguished at its range.
#[derive(Debug, Clone, Copy, Default)]
pub struct PointLightInput {
    /// World-space position.
    pub position: Vec3,
    /// Linear RGB tint.
    pub color: Vec3,
    /// Radiant intensity multiplier.
    pub intensity: f32,
    /// Positive cutoff distance in world units.
    pub range: f32,
}

/// Shared sky gradient and hemisphere illumination, all in linear RGB.
#[derive(Debug, Clone, Copy)]
pub struct Environment {
    /// Optional flat background, independent of ambient illumination.
    pub background: Option<[f32; 3]>,
    /// Radiance directly overhead.
    pub zenith: [f32; 3],
    /// Radiance at the horizon; also the default fog colour.
    pub horizon: [f32; 3],
    /// Radiance directly below.
    pub ground: [f32; 3],
    /// Hemisphere illumination multiplier (does not scale the background).
    pub ambient: f32,
    /// Sun disc angular radius in radians; zero disables both disc and glow.
    pub sun_disc: f32,
    /// None selects a forward pipeline with no fog calculations.
    pub fog: Option<Fog>,
}
impl Default for Environment {
    fn default() -> Self {
        let engine = exact_game::Environment::default();
        Self {
            background: engine.background,
            zenith: engine.zenith,
            horizon: engine.horizon,
            ground: engine.ground,
            ambient: engine.ambient,
            sun_disc: engine.sun_disc,
            fog: engine.fog,
        }
    }
}

/// Constant-size displayed-frame input; transforms stay in the tick buffers.
pub struct FrameInput<'a> {
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
    /// Point lights; only the first sixteen are used.
    pub points: &'a [PointLightInput],
    /// Hemisphere lighting and background.
    pub environment: Environment,
    /// Linear exposure multiplier before the ACES-fitted curve.
    pub exposure: f32,
    /// Optional HDR bloom; allocation only on enable/resize, released on disable.
    pub bloom: Option<Bloom>,
    /// Optional pass timestamps. Requires TIMESTAMP_QUERY on the device.
    /// Reserve [`GPU_PASS_COUNT`] pairs in the query set. Resolve/read outside draw.
    pub timestamps: Option<&'a wgpu::QuerySet>,
}

impl Default for FrameInput<'_> {
    fn default() -> Self {
        Self {
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
            points: &[],
            environment: Environment::default(),
            exposure: 1.0,
            bloom: Some(Bloom::default()),
            timestamps: None,
        }
    }
}

/// Work submitted by a frame, including its fullscreen tonemap triangle.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// All submitted draws, including shadows, sky, bloom and tonemapping.
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
    ($game:ty) => { $crate::module!($game, hook ()); };
    ($game:ty, audio) => {
        #[derive(Default)]
        struct GameAudio(exact_game_audio::SurfacePlayer);
        impl $crate::Presentation for GameAudio {
            fn sync(&mut self, world: &$crate::exact_game::World, generation: u64, playing: bool, seekable: bool) {
                self.0.sync(world, generation, playing, seekable);
            }
            fn unlock(&mut self) { self.0.unlock(); }
        }
        $crate::module!($game, hook GameAudio);
    };
    ($game:ty, hook $hook:ty) => {
        /// The game's sole surface; shaders are embedded in the renderer.
        pub static REGISTRY: $crate::exact_gpu::Registry = $crate::exact_gpu::Registry {
            surfaces: &[(
                <$game as $crate::exact_game::Game>::NAME,
                <<$game as $crate::exact_game::Game>::Args as $crate::exact_game::Args>::FIELDS
                    .len(),
                || Box::new($crate::WorldSurface::<$game, $hook>::default()),
            )],
            shaders: &[],
        };
        $crate::exact_gpu::module!(REGISTRY);
    };
}
