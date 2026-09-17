//! Slot-indexed, opaque PBR rendering over plain buffers; no simulation ownership.
//!
//! Transforms are ten floats (position xyz, quaternion xyzw, scale xyz), materials
//! twelve (linear base rgba, metallic, roughness, linear emissive rgb, reserved xyz).
//! Rotations must be unit quaternions. New slots must be initialized in both ticks.
//! Matrices use WebGPU's 0–1 depth range, with near at zero (for example
//! [`glam::camera::rh::proj::directx::perspective`]). All lights and environment colours are linear.
#![deny(missing_docs)]
#![deny(unsafe_code)]

mod buffers;
mod pipeline;
mod renderer;
pub mod shapes;

use glam::{Mat4, Vec3};
pub use renderer::Renderer;
use std::ops::Range;

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
}

/// Directional light, with no shadow map.
#[derive(Debug, Clone, Copy)]
pub struct Sun {
    /// Direction the light travels (towards the scene); nonzero.
    pub direction: Vec3,
    /// Linear RGB tint.
    pub color: Vec3,
    /// Incident illuminance multiplier.
    pub illuminance: f32,
}

/// An inverse-square point light, smoothly extinguished at its range.
#[derive(Debug, Clone, Copy)]
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

/// Hemisphere illumination and the flat background.
#[derive(Debug, Clone, Copy)]
pub struct Environment {
    /// Upward hemisphere radiance; also the background before exposure/tonemapping.
    pub sky: [f32; 3],
    /// Downward hemisphere radiance.
    pub ground: [f32; 3],
    /// Hemisphere illumination multiplier (does not scale the background).
    pub ambient: f32,
}

/// The only data uploaded per displayed frame (656 bytes).
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
}

/// Work submitted by a frame, including its fullscreen tonemap triangle.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// Nonempty batch draws plus one tonemap draw.
    pub draws: u32,
    /// Mesh instances (excludes the fullscreen triangle).
    pub instances: u64,
    /// Mesh triangles plus one fullscreen triangle.
    pub triangles: u64,
    /// CPU upload/encoding/submission time in microseconds; zero on wasm32.
    pub encode_us: f64,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[test]
    fn every_shader_validates_without_an_adapter() {
        use exact_gpu::wgpu::naga;
        for source in [super::pipeline::FORWARD, super::pipeline::TONEMAP] {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap();
        }
    }
}
