//! Slot-indexed, opaque PBR rendering over plain buffers; no simulation ownership.
//!
//! Transforms are ten floats (position xyz, quaternion xyzw, scale xyz), materials
//! twelve (linear base rgba, metallic, roughness, linear emissive rgb, reserved xyz).
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
pub mod world;

/// Simulation vocabulary, also used by module! without a direct dependency.
pub use exact_game;
/// GPU surface ABI, also used by module! without a direct dependency.
pub use exact_gpu;
pub use surface::WorldSurface;
pub use world::Feed;

use exact_gpu::wgpu;
use glam::{Mat4, Vec3};
pub use renderer::Renderer;
use std::ops::Range;
pub use timing::{GPU_PASS_COUNT, GPU_PASS_NAMES};

/// Which transform slots the caller will rewrite after advancing history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rewrite {
    /// Make every live slot current before drawing; swap without copying.
    /// A feed may retain already-matching target pages instead of rewriting them.
    All,
    /// Rewrite a subset; copy history so untouched slots stay still.
    Some,
}

/// A refused arena capacity request. `limit` is an exclusive slot count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderError {
    /// Arena whose requested slot exceeds capacity.
    pub arena: &'static str,
    /// Highest zero-based slot requested.
    pub slot: u64,
    /// Maximum supported slot count (valid slots are below this value).
    pub limit: u64,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    /// Texture coordinates, reserved for textured materials.
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

/// HDR bloom. None on [`FrameInput`] owns no bloom textures and runs no bloom passes.
#[derive(Debug, Clone, Copy)]
pub struct Bloom {
    /// Bright-pass onset in peak linear HDR channel (default 1).
    pub threshold: f32,
    /// Contribution before exposure and tonemapping (default 0.08).
    pub intensity: f32,
    /// Tent upsampling radius in source texels (default 1).
    pub radius: f32,
}
impl Default for Bloom {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            intensity: 0.08,
            radius: 1.0,
        }
    }
}

/// Exponential distance fog with density falling exponentially above world Y=0.
#[derive(Debug, Clone, Copy)]
pub struct Fog {
    /// Linear colour; None uses the environment horizon.
    pub color: Option<[f32; 3]>,
    /// Extinction per world metre at Y=0 (default 0.02).
    pub density: f32,
    /// Density height falloff per world metre (default 0.1).
    pub height_falloff: f32,
}
impl Default for Fog {
    fn default() -> Self {
        Self {
            color: None,
            density: 0.02,
            height_falloff: 0.1,
        }
    }
}

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
        Self {
            zenith: [0.12, 0.22, 0.4],
            horizon: [0.45, 0.6, 0.65],
            ground: [0.04, 0.035, 0.025],
            ambient: 0.5,
            sun_disc: 0.00465,
            fog: None,
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
    /// CPU upload/encoding/submission time in microseconds; zero on wasm32.
    pub encode_us: f64,
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
    ($game:ty) => {
        /// The game's sole surface; shaders are embedded in the renderer.
        pub static REGISTRY: $crate::exact_gpu::Registry = $crate::exact_gpu::Registry {
            surfaces: &[(
                <$game as $crate::exact_game::Game>::NAME,
                <$game as $crate::exact_game::Game>::ARGS.len(),
                || Box::new($crate::WorldSurface::<$game>::default()),
            )],
            shaders: &[],
        };
        $crate::exact_gpu::module!(REGISTRY);
    };
}
