//! Game-owned passes in an engine-owned frame.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d1-the-shape-a-type-parameter-like-presentation
use crate::{FrameInput, RenderError};
use exact_game::{Component, Entity, Mat4, Ref, Resource, Target, Transform, World};
use exact_gpu::wgpu;

pub(crate) mod gpu_timing;
mod materials;
pub(crate) mod metrics;
mod pipelines;
mod targets;
pub(crate) use materials::MaterialBindings;
pub use materials::{CustomMaterial, MaterialGpu, MATERIAL_WGSL};
pub use pipelines::Pipelines;
pub(crate) use targets::{FrameBinding, HookTargets};

/// The engine's shared frame uniform and lighting helpers. Append game entry points.
pub const FRAME_WGSL: &str = include_str!("shaders/frame.wgsl");

/// Reflected frame-uniform interface, generated from [`FRAME_WGSL`].
pub mod frame_interface {
    include!(concat!(env!("OUT_DIR"), "/frame_interface.rs"));
}

/// Restricted borrowing of simulation values. No mutable query or raw World escape.
/// `&World` itself permits mutation, so it must not cross this boundary.
pub struct RenderWorld<'a>(pub(crate) &'a World);
impl<'a> RenderWorld<'a> {
    /// Read one component by entity or name.
    pub fn get<C: Component>(&self, target: impl Target) -> Option<Ref<'a, C>> {
        self.0.get(target)
    }
    /// Read an optional resource.
    pub fn resource<R: Resource>(&self) -> Option<Ref<'a, R>> {
        self.0.try_resource::<R>()
    }
    /// Visit one component column in stable entity order, with immutable values.
    pub fn for_each<C: Component>(&self, mut visit: impl FnMut(Entity, &C)) {
        for (entity, value) in self.0.query::<&C>().iter() {
            visit(entity, value);
        }
    }
    /// Resolve a named entity, including its generation.
    pub fn named(&self, name: &str) -> Option<Entity> {
        self.0.named(name)
    }
    /// Completed simulation tick.
    pub fn tick(&self) -> u64 {
        self.0.tick()
    }
    /// Simulation ticks per second.
    pub fn hz(&self) -> u32 {
        self.0.hz()
    }
    /// Current global transform; displayed poses are supplied by FrameView.
    pub fn global(&self, entity: Entity) -> Option<Mat4> {
        self.0.global(entity).map(Mat4::from)
    }
}

/// Optional attachments and presentation scheduling for the current frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Needs(u8);
impl Needs {
    /// No optional work.
    pub const NONE: Self = Self(0);
    /// Opaque HDR/depth snapshot and a multisampled refracting-surface stage.
    pub const SCENE_COPY: Self = Self(1);
    /// HDR input/output before bloom and tone.
    pub const HDR_POST: Self = Self(2);
    /// Sampleable final scene depth for post, independently of SCENE_COPY.
    pub const FINAL_DEPTH: Self = Self(4);
    /// Request another visible frame even when the simulation is paused.
    pub const ANIMATE: Self = Self(8);
    /// Presentation resources are still preparing. Does not stop simulation ticks.
    pub const PENDING: Self = Self(16);
    /// Whether all the requested bits are present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub(crate) const fn attachments(self) -> Self {
        Self(self.0 & 7)
    }
}
impl std::ops::BitOr for Needs {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Host clock and lifecycle facts, separate from saved game time.
#[derive(Clone, Copy, Debug)]
pub struct HookTime {
    /// Seconds since the last draw; zero on resets, clamped to 100 ms on live gaps.
    pub dt: f32,
    /// Histories must be primed before this draw.
    pub reset: bool,
    /// The agent owns time, rather than the display.
    pub seekable: bool,
    /// Simulation is paused; presentation may independently animate.
    pub paused: bool,
    /// Logical dimensions in CSS points.
    pub points: (f32, f32),
    /// Device pixels per logical point.
    pub scale: f32,
    /// Shader asset generation; candidate pipelines replace valid ones atomically.
    pub shader_generation: u32,
}
impl Default for HookTime {
    fn default() -> Self {
        Self {
            dt: 0.,
            reset: true,
            seekable: true,
            paused: false,
            points: (1., 1.),
            scale: 1.,
            shader_generation: 0,
        }
    }
}

/// The camera, lights, interpolation and attachment dimensions of this exact draw.
pub struct FrameView<'a> {
    /// Engine inputs used by its own pipelines, including view/projection and time.
    pub frame: &'a FrameInput<'a>,
    /// Host clock/lifecycle facts.
    pub time: HookTime,
    /// Active pixel rectangle, starting at (0, 0).
    pub pixels: (u32, u32),
    /// Allocation extent; sampling must respect the active rectangle.
    pub capacity: (u32, u32),
    /// Forward colour format; post outputs use the same HDR format.
    pub color_format: wgpu::TextureFormat,
    /// Forward depth format.
    pub depth_format: wgpu::TextureFormat,
    /// Geometry pipeline sample count. Fullscreen post uses one sample.
    pub sample_count: u32,
    pub(crate) poses: &'a Poses,
}
impl FrameView<'_> {
    /// Displayed transform of an entity registered through Hooks::entities.
    pub fn pose(&self, entity: Entity) -> Option<Mat4> {
        if let Some(a) = self.frame.attachments.iter().find(|a| a.entity == entity) {
            return Some(a.matrix);
        }
        self.poses.at(entity, self.frame.alpha)
    }
    /// Projection inverse, valid for both perspective and orthographic cameras.
    pub fn inverse_projection(&self) -> Mat4 {
        self.frame.proj.inverse()
    }
}

/// Borrowed GPU access for preparing resources; the engine owns command submission.
pub struct HookGpu<'a> {
    /// Optional model material support, present for an assets-enabled game.
    pub materials: Option<MaterialGpu<'a>>,
    /// Current device; handles must be discarded on device loss.
    pub device: &'a wgpu::Device,
    /// Uniform/buffer uploads for this frame.
    pub queue: &'a wgpu::Queue,
    /// Engine frame uniform at group 0, binding 0, matching FRAME_WGSL.
    pub frame_binding: &'a wgpu::BindGroup,
    /// Reflected layout of frame_binding.
    pub frame_layout: &'a wgpu::BindGroupLayout,
}

/// Immutable pre-surface inputs. Linear depth is positive view-space Z; zero is sky.
pub struct SceneCopy<'a> {
    /// Resolved linear HDR colour, distinct from the continuation's resolve target.
    pub color: &'a wgpu::TextureView,
    /// R32Float nearest-sample linear view depth. Sample with textureLoad.
    pub depth: &'a wgpu::TextureView,
}
/// Fullscreen post inputs. Output must be completely written over the active rectangle.
pub struct PostInputs<'a> {
    /// Final resolved linear HDR scene.
    pub input: &'a wgpu::TextureView,
    /// Separate linear HDR output consumed by both bloom and tone.
    pub output: &'a wgpu::TextureView,
    /// Final linear depth, only when FINAL_DEPTH was requested.
    pub depth: Option<&'a wgpu::TextureView>,
    /// Earlier pre-surface depth, only when SCENE_COPY was requested.
    pub opaque_depth: Option<&'a wgpu::TextureView>,
}

/// Cumulative game-reported GPU creation work; separate from engine-observed work.
#[derive(Clone, Copy, Debug, Default)]
pub struct HookWork {
    /// Pipeline creations.
    pub pipelines: u64,
    /// Buffer creations or replacements.
    pub buffers: u64,
    /// Texture creations or replacements.
    pub textures: u64,
}
impl HookWork {
    pub(crate) fn json(self) -> String {
        format!(
            "{{\"pipelines\":{},\"buffers\":{},\"textures\":{}}}",
            self.pipelines, self.buffers, self.textures
        )
    }
    pub(crate) fn since(self, old: Self) -> Self {
        Self {
            pipelines: self.pipelines.saturating_sub(old.pipelines),
            buffers: self.buffers.saturating_sub(old.buffers),
            textures: self.textures.saturating_sub(old.textures),
        }
    }
}

/// Optional game rendering. Every callback borrows resources for this frame only.
/// @ref llp/1046.006.000-render-hooks.rfc.md#d1-the-shape-a-type-parameter-like-presentation
pub trait Hooks: Default {
    /// Compile away all hook preparation for the default implementation.
    const ENABLED: bool = true;
    /// Allocate/resize resources, extract immutable world data, and upload uniforms.
    fn prepare(
        &mut self,
        _gpu: &HookGpu<'_>,
        _view: &FrameView<'_>,
        _world: &RenderWorld<'_>,
    ) -> Result<(), RenderError> {
        Ok(())
    }
    /// Optional resources and independent demand for another frame, read after prepare.
    fn needs(&self) -> Needs {
        Needs::NONE
    }
    /// Whether callbacks can draw while a replacement is pending.
    fn drawable(&self) -> bool {
        true
    }
    /// Entity histories retained at tick boundaries. New registrations prime current/current.
    fn entities(&self) -> &[Entity] {
        &[]
    }
    /// A named readiness reason, while PENDING is set.
    fn pending_reason(&self) -> &str {
        "render hook resources pending"
    }
    /// Last rejected shader/pipeline update, even if a previous set still draws.
    fn error(&self) -> Option<&str> {
        None
    }
    /// Cumulative work performed through the raw GPU handles.
    fn work(&self) -> Option<HookWork> {
        None
    }
    /// Custom shading for loaded unskinned opaque model materials.
    fn materials(&self) -> &[CustomMaterial] {
        &[]
    }
    /// Extra per-instance word; presentation-only and never saved.
    fn instance_data(&self, _transform: u32) -> u32 {
        0
    }
    /// Compute/offscreen passes, after skinning and before shadows.
    fn compute(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        _view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        Ok(())
    }
    /// Additional opaque geometry. Depth writes are required for occluding geometry.
    fn opaque(
        &mut self,
        _pass: &mut wgpu::RenderPass<'_>,
        _view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        Ok(())
    }
    /// Background content after sky, before the snapshot. The game owns classification
    /// for transparent content that belongs behind its refractor.
    fn background(
        &mut self,
        _pass: &mut wgpu::RenderPass<'_>,
        _view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        Ok(())
    }
    /// Refracting geometry in the 4× continuation, with writable depth.
    fn surface(
        &mut self,
        _pass: &mut wgpu::RenderPass<'_>,
        _scene: &SceneCopy<'_>,
        _view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        Ok(())
    }
    /// Linear HDR effects, before bloom and the engine's tone curve.
    fn post(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        _inputs: &PostInputs<'_>,
        _view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        Ok(())
    }
    /// Drop device-owned state immediately; the next prepare receives reset=true.
    fn device_lost(&mut self) {
        *self = Self::default();
    }
}
impl Hooks for () {
    const ENABLED: bool = false;
}

#[derive(Default)]
pub(crate) struct Poses {
    entries: Vec<(Entity, [Transform; 2])>,
    tick: Option<u64>,
    generation: u64,
    parent_revision: u64,
}
impl Poses {
    pub fn track(&mut self, entities: &[Entity], world: &World) {
        self.entries.retain(|(e, _)| entities.contains(e));
        for &e in entities {
            if self.entries.iter().any(|(old, _)| *old == e) {
                continue;
            }
            if let Some(p) = crate::world::scene::pose(world, e) {
                self.entries.push((e, [p; 2]));
            }
        }
    }
    pub fn sync(&mut self, world: &World) {
        let reset = self.generation != world.presentation_generation();
        self.generation = world.presentation_generation();
        let parent_revision = world.revision::<exact_game::Parent>();
        let parent_changed = self.parent_revision != parent_revision;
        self.parent_revision = parent_revision;
        let next = self.tick != Some(world.tick());
        self.tick = Some(world.tick());
        self.entries.retain_mut(|(e, pair)| {
            let Some(pose) = crate::world::scene::pose(world, *e) else {
                return false;
            };
            if next {
                pair[0] = pair[1];
            }
            pair[1] = pose;
            if reset || crate::world::scene::snap(world, *e, parent_changed) {
                pair[0] = pose;
            }
            true
        });
    }
    fn at(&self, e: Entity, alpha: f32) -> Option<Mat4> {
        let (_, pair) = self.entries.iter().find(|(id, _)| *id == e)?;
        let pose = crate::world::scene::interpolate(*pair, alpha);
        Some(Mat4::from_scale_rotation_translation(
            pose.scale,
            pose.rotation,
            pose.position,
        ))
    }
}

#[derive(Default)]
pub(crate) struct HookClock {
    last: Option<(f64, bool)>,
    generation: Option<u64>,
}
impl HookClock {
    pub fn reset(&mut self) {
        self.last = None;
    }
    pub fn frame(&mut self, frame: &exact_gpu::Frame, generation: u64, paused: bool) -> HookTime {
        let reset = self.generation != Some(generation)
            || self.last.is_none_or(|(t, seekable)| {
                frame.now_ms < t || frame.seekable != seekable || frame.now_ms - t > 1000.
            });
        let dt = if reset {
            0.
        } else {
            self.last.map_or(0., |(t, _)| {
                ((frame.now_ms - t) / 1000.).clamp(0., 0.1) as f32
            })
        };
        self.last = Some((frame.now_ms, frame.seekable));
        self.generation = Some(generation);
        HookTime {
            dt,
            reset,
            paused,
            seekable: frame.seekable,
            points: (frame.width, frame.height),
            scale: frame.scale,
            shader_generation: frame.shader_generation,
        }
    }
}
