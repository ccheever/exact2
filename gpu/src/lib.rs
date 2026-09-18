//! The GPU canvas: the trait an app's surfaces implement and the module
//! that runs them on a wgpu device (LLP 1009).
//!
//! **wgpu is the one GPU API, on every host** (D1): a surface is written
//! against wgpu's own types and runs on Metal, Vulkan, or the browser's
//! WebGPU unchanged. **The module is loaded on demand** (D2): an app's GPU
//! crate compiles its surfaces and this crate into one artifact — a
//! `dylib` natively, a wasm with wasm-bindgen glue on the web — that the
//! presenter loads the first time a canvas is on screen, after the first
//! pixel. Inputs cross as JSON-encoded plan values; wgpu objects never
//! cross. Failures are per canvas, reported by [`Module::take_error`], never a
//! refusal of anything else.
//!
//! - [`Surface`], [`Frame`] — what an app implements.
//! - [`Module`] — the device and the instances, one per canvas node.
//! - [`json`] — the values as the batch carries them.
//! - [`shaders`] — the WGSL by name, registered at run time (LLP 1030 D8).
//! - [`module!`] — the exports for one app's registry.
//! - `fixture` (native) — a surface rendered and read back, for fixtures.

#![deny(missing_docs)]

use std::collections::{BTreeSet, HashMap};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub use exact_plan::Value;
pub use wgpu;

mod input;
pub use input::{InputEvent, PointerKind, PointerPhase};
pub mod json;
pub mod shaders;

/// One frame's context.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The canvas's width in points.
    pub width: f32,
    /// The canvas's height in points.
    pub height: f32,
    /// Device pixels per point.
    pub scale: f32,
    /// The host's presentable clock, milliseconds.
    pub now_ms: f64,
    /// The agent owns time: honour every millisecond, including time off screen.
    /// Otherwise the display owns time and a surface may drop unseen time.
    pub seekable: bool,
    /// How many times the canvas's children texture has been uploaded (LLP
    /// 1014): a surface that keeps the previous children crossfades when
    /// this changes. The module sets it; a host passes `0`.
    pub children_generation: u32,
    /// How many times a shader has been registered (LLP 1030 D8,
    /// [`shaders::shader_generation`]): a surface keys its cached pipeline
    /// by this beside the target format, so a registered edit is a new
    /// pipeline at the next frame. The module sets it; a fixture passes
    /// what [`shaders::shader_generation`] says.
    pub shader_generation: u32,
}

impl Frame {
    /// The drawable's size in device pixels, at least one.
    pub fn pixels(&self) -> (u32, u32) {
        (
            ((self.width * self.scale).round() as u32).max(1),
            ((self.height * self.scale).round() as u32).max(1),
        )
    }
}

/// Why a surface refused its inputs or could not draw committed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceError(pub String);

/// What an app implements per canvas.
pub trait Surface {
    /// The canvas's inputs from the plan, as typed values; before the
    /// first render and whenever they change. A refusal names the input.
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError>;
    /// Bind at the host's optional commit clock, without requiring a frame.
    fn bind_at(&mut self, inputs: &[Value], _at_ms: Option<f64>) -> Result<(), SurfaceError> {
        self.bind(inputs)
    }
    /// Names under the app's assets directory that this surface wants; drained by the module.
    fn assets(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// Deliver GPU-ready bytes, or None when the host has no such file.
    fn asset(&mut self, _name: &str, _bytes: Option<&[u8]>) {}
    /// State as bytes this surface can later restore: a save or a dev reload's carry.
    /// None means this surface has nothing worth carrying.
    fn carry(&mut self) -> Option<Vec<u8>> {
        None
    }
    /// Take back a carry, possibly from an older build. Err leaves state unchanged.
    fn restore(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Err("this surface carries no state".into())
    }
    /// One frame into `target` (of `format`). Returns whether another
    /// frame is wanted without new inputs.
    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool;
    /// A presentation target acquired a device, before its first draw.
    fn device_ready(&mut self) {}
    /// Presentation was lost; release device resources without discarding owned state.
    fn device_lost(&mut self) {}
    /// Drain an error discovered while rendering or advancing committed state.
    fn take_error(&mut self) -> Option<SurfaceError> {
        None
    }
    /// Raw input inside this canvas (LLP 1041.002 S1); app gestures elsewhere are untouched.
    fn wants_input(&self) -> bool {
        false
    }
    /// One device event in canvas points, stamped with the host's clock.
    fn input(&mut self, _event: &InputEvent) {}
    /// Strings posted to the app, drained after render, input and agent calls (S2).
    fn messages(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// The surface's public record — one JSON object — when it changed since last
    /// asked. The host offers it to the app as `exactSurface("<name>")`.
    /// The first live instance owns a surface name; other instances cannot publish or clear its record.
    fn published(&mut self) -> Option<String> {
        None
    }
    /// An agent request and reply as JSON objects; the host adds clock and size (S3).
    fn agent(&mut self, _request: &str) -> Option<String> {
        None
    }
    /// Whether the surface samples the canvas's children (LLP 1014 D2). A
    /// host that can paint a subtree then hands it to [`Surface::children`]
    /// and stops compositing the children itself; a host that cannot, or a
    /// surface that answers `false`, leaves them composited over the surface.
    fn wants_children(&self) -> bool {
        false
    }
    /// Whether the surface also wants the children as they were before the
    /// latest upload (LLP 1014): the module keeps a copy, handed over once by
    /// [`Surface::previous_children`] and refreshed before every upload;
    /// [`Frame::children_generation`] says when the pair changed.
    fn wants_previous_children(&self) -> bool {
        false
    }
    /// The children before the latest upload (see
    /// [`Surface::wants_previous_children`]); `None` when there are none.
    fn previous_children(&mut self, _texture: Option<&wgpu::TextureView>) {}
    /// Whether the surface wants each direct child of the canvas as its own
    /// texture with its kernel frame (LLP 1014 D5, the browser's `drawable`):
    /// the host then captures every child separately, hands each one over
    /// through [`Surface::child`], and asks [`Surface::placement`] after each
    /// frame where the surface put it, for hit-testing and accessibility.
    fn wants_children_each(&self) -> bool {
        false
    }
    /// The `index`th direct child's texture (created or resized; contents
    /// update in place) and its frame in the canvas's points — `x, y, width,
    /// height`. `None` when the child is gone.
    fn child(&mut self, _index: usize, _texture: Option<&wgpu::TextureView>, _frame: [f32; 4]) {}
    /// How many direct children there are now (children past it are gone).
    fn children_count(&mut self, _count: usize) {}
    /// Where the surface put the `index`th child: a 3×3 homography, row
    /// major, from the child's own points (origin at its top-left corner) to
    /// the canvas's points — the browser's `canvasTransform` — and its depth,
    /// larger nearer the eye, which orders hit-testing where children
    /// overlap (the browser's hit-test stack follows draw order). `None` is
    /// the kernel's frame, untouched. The host inverts the homography to
    /// hit-test and reports the mapped box to accessibility.
    fn placement(&self, _index: usize) -> Option<Placement> {
        None
    }
    /// The canvas's children as a texture (LLP 1014 D2, D3) — laid
    /// out by the kernel in the canvas's box, painted by the host at the
    /// canvas's scale, premultiplied RGBA — for the surface to sample;
    /// `None` when there are none. Called when the texture is created or
    /// replaced; its contents update in place.
    fn children(&mut self, _texture: Option<&wgpu::TextureView>) {}
}

/// Where a surface put a child (LLP 1014 D5): see [`Surface::placement`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Child points to canvas points, row major, projective.
    pub homography: [f32; 9],
    /// Larger nearer the eye.
    pub depth: f32,
}

/// Makes a surface.
pub type Factory = fn() -> Box<dyn Surface>;

/// An app's surfaces and the shaders they compile against.
pub struct Registry {
    /// Every surface: name, arity, factory.
    pub surfaces: &'static [(&'static str, usize, Factory)],
    /// Every shader the surfaces were reflected against: name and interface
    /// digest (the generated `SHADERS`, `exact-gpu-reflect`). A surface is
    /// not created until each has registered text at that interface
    /// ([`shaders`]).
    pub shaders: &'static [(&'static str, u64)],
}

/// The device and every canvas's surface.
pub struct Module {
    registry: &'static Registry,
    gpu: Option<Gpu>,
    device_lost: Arc<AtomicBool>,
    instances: HashMap<u32, Instance>,
    next: u32,
    error: String,
    seekable: bool,
}

/// The wgpu device.
pub struct Gpu {
    /// The instance.
    pub instance: wgpu::Instance,
    /// The adapter.
    pub adapter: wgpu::Adapter,
    /// The device.
    pub device: wgpu::Device,
    /// Its queue.
    pub queue: wgpu::Queue,
}

struct Instance {
    surface: Box<dyn Surface>,
    messages: Vec<String>,
    published: Option<String>,
    presentation: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    outstanding: BTreeSet<String>,
    bound: bool,
    dirty: bool,
    children: Option<Children>,
    children_generation: u32,
    /// Per-child textures (LLP 1014 D5), by index.
    each: Vec<Option<ChildTexture>>,
}

impl Instance {
    fn drain(&mut self) {
        self.messages.extend(self.surface.messages());
        if let Some(record) = self.surface.published() {
            self.published = Some(record);
        }
    }
}

/// One direct child's texture on the device (LLP 1014 D5).
struct ChildTexture {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
}

/// A canvas's children, painted by the host, on the device (LLP 1014 D3),
/// and — for a surface that asked — the children before the latest upload.
struct Children {
    texture: wgpu::Texture,
    previous: Option<wgpu::Texture>,
    width: u32,
    height: u32,
    /// A Metal texture the host owns, imported by its pointer (LLP 1008 §9):
    /// the host renders the children into it and hands it over with no
    /// copy; `texture` is the import. A host that alternates between two
    /// hands each over in turn, so the import is kept by pointer.
    metal: Option<usize>,
}

impl Module {
    /// A module with no device yet.
    pub fn new(registry: &'static Registry) -> Module {
        Module {
            registry,
            gpu: None,
            device_lost: Arc::new(AtomicBool::new(false)),
            instances: HashMap::new(),
            next: 0,
            error: String::new(),
            seekable: false,
        }
    }

    /// Adopt a device (the platform-specific loader made it).
    pub fn set_gpu(&mut self, gpu: Gpu) {
        self.device_lost = Arc::new(AtomicBool::new(false));
        let lost = self.device_lost.clone();
        gpu.device.set_device_lost_callback(move |_, _| {
            lost.store(true, Ordering::Release);
        });
        self.gpu = Some(gpu);
    }

    fn check_device(&mut self) {
        if self.gpu.is_some() && self.device_lost.load(Ordering::Acquire) {
            self.lose_device();
        }
    }

    /// The device, when loaded.
    pub fn gpu(&self) -> Option<&Gpu> {
        self.gpu
            .as_ref()
            .filter(|_| !self.device_lost.load(Ordering::Acquire))
    }

    /// Consume the last failure's text, for the presenter to report once.
    pub fn take_error(&mut self) -> String {
        std::mem::take(&mut self.error)
    }

    /// The interface digest the module's Rust binds for shader `name`
    /// (LLP 1030 D8) — what a registered text must match; `None` for a
    /// name no surface here uses.
    pub fn expected_digest(&self, name: &str) -> Option<u64> {
        self.registry
            .shaders
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, d)| *d)
    }

    /// The shaders this module's surfaces bind against, by name.
    pub fn shader_names(&self) -> Vec<&'static str> {
        self.registry.shaders.iter().map(|(n, _)| *n).collect()
    }

    /// Register the text of shader `name` for this module's surfaces
    /// (LLP 1030 D8): validated, its interface checked against the one the
    /// binary binds. Every canvas is marked dirty on success, so the next
    /// frame renders through the new pipeline. `false`, with the reason in
    /// [`Module::take_error`], on a refusal.
    pub fn set_shader(&mut self, name: &str, text: String) -> bool {
        let Some(expected) = self.expected_digest(name) else {
            self.error = format!("no shader named `{name}` in this module");
            return false;
        };
        match shaders::set_shader(name, text, Some(expected)) {
            Ok(()) => {
                for inst in self.instances.values_mut() {
                    inst.dirty = true;
                }
                true
            }
            Err(e) => {
                self.error = e;
                false
            }
        }
    }

    fn fail<T>(&mut self, e: impl Into<String>) -> Option<T> {
        self.error = e.into();
        None
    }

    /// Create a canvas's surface by name on a platform target; `None` (and
    /// [`Module::take_error`]) when the name is unknown or the target refused.
    pub fn create(
        &mut self,
        name: &str,
        target: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Option<u32> {
        self.check_device();
        let Some(gpu) = self.gpu.as_ref() else {
            return self.fail("no device");
        };
        let Some((_, _, factory)) = self.registry.surfaces.iter().find(|(n, _, _)| *n == name)
        else {
            return self.fail(format!("no surface named `{name}` in this module"));
        };
        // Every shader this module's surfaces bind against has its text, at
        // the interface the binary was built for (LLP 1030 D8): a pipeline
        // built over nothing would be wgpu's error, not a refusal by name.
        if let Some(why) = shaders::missing(self.registry.shaders) {
            return self.fail(why);
        }
        let Some(mut config) = target.get_default_config(&gpu.adapter, width.max(1), height.max(1))
        else {
            return self.fail("the adapter cannot present to this target");
        };
        // The browser's canvas default is linear `bgra8unorm`; a native
        // executor that picked an sRGB format would show every color lighter
        // (LLP 1009 D1: the browser is the oracle). Prefer a non-sRGB format.
        let formats = target.get_capabilities(&gpu.adapter).formats;
        if let Some(f) = formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
            config.view_formats = vec![];
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        target.configure(&gpu.device, &config);
        self.insert(*factory, Some((target, config)))
    }

    /// Create surface ownership without a device, target, or registered shaders.
    pub fn create_headless(&mut self, name: &str) -> Option<u32> {
        let Some((_, _, factory)) = self.registry.surfaces.iter().find(|(n, _, _)| *n == name)
        else {
            return self.fail(format!("no surface named `{name}` in this module"));
        };
        self.insert(*factory, None)
    }

    fn insert(
        &mut self,
        factory: Factory,
        presentation: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    ) -> Option<u32> {
        self.next += 1;
        let id = self.next;
        let mut surface = factory();
        if presentation.is_some() {
            surface.device_ready();
        }
        self.instances.insert(
            id,
            Instance {
                surface,
                messages: Vec::new(),
                published: None,
                presentation,
                outstanding: BTreeSet::new(),
                bound: false,
                dirty: false,
                children: None,
                children_generation: 0,
                each: Vec::new(),
            },
        );
        Some(id)
    }

    /// Release presentation resources while preserving every surface's state.
    pub fn lose_device(&mut self) {
        for inst in self.instances.values_mut() {
            inst.surface.device_lost();
            inst.presentation = None;
            inst.children = None;
            inst.each.clear();
        }
        self.gpu = None;
    }

    /// Whether this canvas has a device and presentation target.
    pub fn has_device(&self, id: u32) -> bool {
        self.gpu.is_some()
            && !self.device_lost.load(Ordering::Acquire)
            && self
                .instances
                .get(&id)
                .is_some_and(|i| i.presentation.is_some())
    }

    /// Set once by an agent host: every frame honours the seekable clock.
    pub fn set_seekable(&mut self, on: bool) {
        self.seekable = on;
    }

    /// Whether this canvas asks for raw device input.
    pub fn wants_input(&self, id: u32) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|i| i.surface.wants_input())
    }

    /// Deliver one device event and mark the canvas dirty.
    pub fn input(&mut self, id: u32, event: &InputEvent) -> bool {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail::<()>("no such canvas").is_some();
        };
        inst.surface.input(event);
        inst.drain();
        if let Some(SurfaceError(e)) = inst.surface.take_error() {
            self.error = e;
            return false;
        }
        inst.dirty = true;
        true
    }

    /// Parse and deliver one ABI event; malformed input is refused by name.
    pub fn input_json(&mut self, id: u32, text: &str) -> bool {
        match json::parse_input(text) {
            Ok(event) => self.input(id, &event),
            Err(error) => self.fail::<()>(error).is_some(),
        }
    }

    /// Drain the strings posted since the host last asked, exactly once.
    pub fn take_messages(&mut self, id: u32) -> Vec<String> {
        self.instances
            .get_mut(&id)
            .map(|i| std::mem::take(&mut i.messages))
            .unwrap_or_default()
    }

    /// Take the latest changed public record exactly once.
    pub fn take_published(&mut self, id: u32) -> Option<String> {
        self.instances.get_mut(&id).and_then(|i| i.published.take())
    }

    /// Ask this canvas an agent question; an answer or posted message marks it dirty.
    pub fn agent(&mut self, id: u32, request: &str) -> Option<String> {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail("no such canvas");
        };
        let reply = inst.surface.agent(request);
        let messages = inst.surface.messages();
        inst.dirty |= reply.is_some() || !messages.is_empty();
        inst.messages.extend(messages);
        if let Some(record) = inst.surface.published() {
            inst.published = Some(record);
        }
        if let Some(SurfaceError(e)) = inst.surface.take_error() {
            self.error = e;
            return None;
        }
        reply
    }

    /// Whether a canvas's surface samples its children (LLP 1014 D2).
    pub fn wants_children(&self, id: u32) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|i| i.surface.wants_children())
    }

    /// Whether a canvas's surface wants each child as its own texture (LLP
    /// 1014 D5).
    pub fn wants_children_each(&self, id: u32) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|i| i.surface.wants_children_each())
    }

    /// The `index`th direct child of a canvas, painted by the host (LLP 1014
    /// D5): `frame` in the canvas's points, `width`×`height` premultiplied
    /// RGBA pixels. Creates or replaces the texture at a new size, writes
    /// the pixels, tells the surface, and marks the canvas dirty.
    pub fn child(
        &mut self,
        id: u32,
        index: usize,
        frame: [f32; 4],
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> bool {
        self.check_device();
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(bytes.len()) {
            self.error = format!("child {index}: {} bytes for {width}x{height}", bytes.len());
            return false;
        }
        let Some(gpu) = self.gpu.as_ref() else {
            self.error = "no device".into();
            return false;
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        // Children arrive in order: the next index appends, an earlier one
        // replaces; a gap is a host's mistake, refused — never an allocation
        // the host's number sizes.
        if index > inst.each.len() {
            self.error = format!(
                "child {index}: out of order ({} children so far)",
                inst.each.len()
            );
            return false;
        }
        if inst.each.len() == index {
            inst.each.push(None);
        }
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let same = matches!(&inst.each[index], Some(c) if c.width == width && c.height == height);
        if !same {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("child"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            inst.surface.child(index, Some(&view), frame);
            inst.each[index] = Some(ChildTexture {
                texture,
                width,
                height,
            });
        } else {
            // The same texture; the frame may have moved.
            let texture = &inst.each[index].as_ref().expect("checked").texture;
            let view = texture.create_view(&Default::default());
            inst.surface.child(index, Some(&view), frame);
        }
        let child = inst.each[index].as_ref().expect("just set");
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &child.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: None,
            },
            size,
        );
        inst.children_generation += 1;
        inst.dirty = true;
        true
    }

    /// How many direct children a canvas has now (LLP 1014 D5): the textures
    /// past it are dropped and the surface told.
    pub fn children_count(&mut self, id: u32, count: usize) -> bool {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        if inst.each.len() > count {
            for index in count..inst.each.len() {
                inst.surface.child(index, None, [0.0; 4]);
            }
            inst.each.truncate(count);
        }
        inst.surface.children_count(count);
        inst.dirty = true;
        true
    }

    /// Where a canvas's surface put its `index`th child (LLP 1014 D5).
    pub fn placement(&self, id: u32, index: usize) -> Option<Placement> {
        self.instances
            .get(&id)
            .and_then(|i| i.surface.placement(index))
    }

    /// The canvas's children, painted by the host (LLP 1014 D3): `width`×`height`
    /// premultiplied RGBA, rows top-down, tightly packed. Creates or
    /// replaces the texture at a new size, writes the pixels, and marks the
    /// canvas dirty.
    pub fn texture(&mut self, id: u32, width: u32, height: u32, bytes: &[u8]) -> bool {
        self.check_device();
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(bytes.len()) {
            self.error = format!("children: {} bytes for {width}x{height}", bytes.len());
            return false;
        }
        let Some(gpu) = self.gpu.as_ref() else {
            self.error = "no device".into();
            return false;
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let same = matches!(&inst.children, Some(c) if c.width == width && c.height == height && c.metal.is_none());
        if !same {
            let make = |label: &str| {
                gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            };
            let texture = make("children");
            let view = texture.create_view(&Default::default());
            inst.surface.children(Some(&view));
            let previous = inst.surface.wants_previous_children().then(|| {
                let previous = make("previous children");
                let view = previous.create_view(&Default::default());
                inst.surface.previous_children(Some(&view));
                previous
            });
            inst.children = Some(Children {
                texture,
                previous,
                width,
                height,
                metal: None,
            });
        } else if let Some(Children {
            texture,
            previous: Some(previous),
            ..
        }) = &inst.children
        {
            // What the children were, before the write below lands: a copy
            // submitted now runs before a write enqueued after it.
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_texture(
                texture.as_image_copy(),
                previous.as_image_copy(),
                size,
            );
            gpu.queue.submit([encoder.finish()]);
        }
        inst.children_generation += 1;
        let children = inst.children.as_ref().expect("just set");
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &children.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: None,
            },
            size,
        );
        inst.dirty = true;
        true
    }

    /// New inputs for a canvas; a refusal is reported and the surface keeps
    /// its last accepted inputs.
    pub fn bind(&mut self, id: u32, inputs: &[Value], at_ms: Option<f64>) -> bool {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail::<()>("no such canvas").is_some();
        };
        match inst.surface.bind_at(inputs, at_ms) {
            Ok(()) => {
                inst.drain();
                inst.bound = true;
                inst.dirty = true;
                true
            }
            Err(SurfaceError(e)) => {
                self.error = e;
                false
            }
        }
    }

    /// Drain wanted asset names once; refuse absolute, escaping or non-ASCII paths.
    pub fn take_assets(&mut self, id: u32) -> Vec<String> {
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return Vec::new();
        };
        let mut wanted = Vec::new();
        for name in inst.surface.assets() {
            if !asset_name(&name) {
                self.error =
                    format!("asset `{name}`: expected a relative asset path without .. segments");
            } else if inst.outstanding.insert(name.clone()) {
                wanted.push(name);
            }
        }
        wanted
    }

    /// Deliver one requested asset, with None for a missing file; works without a device.
    pub fn asset(&mut self, id: u32, name: &str, bytes: Option<&[u8]>) -> bool {
        if !asset_name(name) {
            self.error = format!("asset `{name}`: invalid relative asset path");
            return false;
        }
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        if !inst.outstanding.remove(name) {
            self.error = format!("asset `{name}`: not requested by this surface");
            return false;
        }
        inst.surface.asset(name, bytes);
        inst.drain();
        inst.dirty = true;
        if let Some(SurfaceError(error)) = inst.surface.take_error() {
            self.error = error;
            return false;
        }
        true
    }

    /// Capture state without advancing the surface or consuming its publications.
    pub fn carry(&mut self, id: u32) -> Option<Vec<u8>> {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail("no such canvas");
        };
        inst.surface.carry()
    }

    /// Restore atomically; successful state is published before the next frame.
    pub fn restore(&mut self, id: u32, bytes: &[u8]) -> bool {
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        match inst.surface.restore(bytes) {
            Ok(()) => {
                // Outputs from the replaced state (including fresh setup) must not
                // be delivered alongside the restored state. Refusals keep them.
                inst.messages.clear();
                inst.published = None;
                inst.drain();
                inst.dirty = true;
                true
            }
            Err(error) => {
                self.error = error;
                false
            }
        }
    }

    /// Whether a canvas has something to render: inputs it has not shown.
    pub fn dirty(&self, id: u32) -> bool {
        self.has_device(id) && self.instances.get(&id).is_some_and(|i| i.dirty)
    }

    /// Render one frame for a canvas at the given size; returns whether the
    /// surface wants another frame. None with no error means no device/target.
    /// Nothing happens before the first bind.
    pub fn render(&mut self, id: u32, frame: &Frame) -> Option<bool> {
        self.check_device();
        let (w, h) = frame.pixels();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail("no such canvas");
        };
        let gpu = self.gpu.as_ref()?;
        if !inst.bound {
            return Some(false);
        }
        let (target, config) = inst.presentation.as_mut()?;
        if config.width != w || config.height != h {
            config.width = w;
            config.height = h;
            target.configure(&gpu.device, config);
        }
        use wgpu::CurrentSurfaceTexture as Current;
        let texture = match target.get_current_texture() {
            Current::Success(t) => t,
            Current::Suboptimal(t) => {
                target.configure(&gpu.device, config);
                t
            }
            // Nothing to draw into this frame; the inputs stay dirty.
            Current::Timeout | Current::Occluded => return Some(true),
            Current::Lost => {
                self.lose_device();
                return None;
            }
            other => {
                self.error = format!("surface: {other:?}");
                return None;
            }
        };
        let view = texture.texture.create_view(&Default::default());
        let frame = Frame {
            seekable: self.seekable,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
            ..*frame
        };
        let wants = inst
            .surface
            .render(&frame, &gpu.device, &gpu.queue, &view, config.format);
        inst.drain();
        if let Some(SurfaceError(e)) = inst.surface.take_error() {
            self.error = e;
            return None;
        }
        gpu.queue.present(texture);
        inst.dirty = false;
        Some(wants)
    }

    /// Every command submitted to the device so far, complete (LLP 1008 §9):
    /// a host that hands the module textures it renders itself waits here
    /// before drawing into one the module may still be reading — sampling
    /// it, or copying it into the previous children.
    pub fn sync(&self) -> bool {
        match self.gpu() {
            Some(gpu) => gpu
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })
                .is_ok(),
            None => false,
        }
    }

    /// Drop a canvas's surface.
    pub fn destroy(&mut self, id: u32) {
        self.instances.remove(&id);
    }

    /// The canvas's children as a Metal texture the host rendered (LLP 1008
    /// §9): `raw` is an `MTLTexture` — `width`×`height`, `rgba8Unorm`,
    /// readable by shaders — that the host keeps alive; it is retained and
    /// imported as it is, no bytes crossing. The same pointer again reuses
    /// the import — and a host should keep to one texture: a new import is a
    /// new children view to the surface, which takes it as a fresh set (the
    /// glass crossfades). The previous children (for a surface that
    /// crossfades) are copied out of the texture at each hand-over — the
    /// host hands over after drawing, so the copy is of the frame before.
    ///
    /// # Safety
    /// `raw` is a live `MTLTexture` of that size and format, valid until
    /// the canvas is destroyed or another texture replaces it.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub unsafe fn texture_from_metal(
        &mut self,
        id: u32,
        width: u32,
        height: u32,
        raw: *mut std::ffi::c_void,
    ) -> bool {
        self.check_device();
        use objc2::rc::Retained;
        use objc2::runtime::ProtocolObject;
        use objc2_metal::MTLTexture;
        if width == 0 || height == 0 || raw.is_null() {
            self.error = format!("children: a {width}x{height} Metal texture at {raw:?}");
            return false;
        }
        let Some(gpu) = self.gpu.as_ref() else {
            self.error = "no device".into();
            return false;
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let same = matches!(&inst.children, Some(c) if c.width == width && c.height == height && c.metal == Some(raw as usize));
        if !same {
            // SAFETY: the caller's contract — a live MTLTexture; `retain`
            // takes its own reference to it.
            let Some(retained) =
                (unsafe { Retained::retain(raw as *mut ProtocolObject<dyn MTLTexture>) })
            else {
                self.error = "children: the Metal texture could not be retained".into();
                return false;
            };
            // SAFETY: the texture's own size and format, as the host made it.
            let hal = unsafe {
                wgpu::hal::metal::Device::texture_from_raw(
                    retained,
                    wgpu::TextureFormat::Rgba8Unorm,
                    objc2_metal::MTLTextureType::Type2D,
                    1,
                    1,
                    wgpu::hal::CopyExtent {
                        width,
                        height,
                        depth: 1,
                    },
                    None,
                )
            };
            // SAFETY: the hal texture matches the descriptor.
            let texture = unsafe {
                gpu.device.create_texture_from_hal::<wgpu::hal::api::Metal>(
                    hal,
                    &wgpu::TextureDescriptor {
                        label: Some("children (metal)"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    },
                    wgpu::TextureUses::RESOURCE,
                )
            };
            // The previous children, out of the texture bound until now,
            // before the surface hears of the new one.
            let previous = match inst.children.take() {
                Some(Children {
                    texture: old,
                    previous: Some(previous),
                    width: w,
                    height: h,
                    ..
                }) if w == width && h == height => {
                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    encoder.copy_texture_to_texture(
                        old.as_image_copy(),
                        previous.as_image_copy(),
                        size,
                    );
                    gpu.queue.submit([encoder.finish()]);
                    Some(previous)
                }
                _ => inst.surface.wants_previous_children().then(|| {
                    let previous = gpu.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("previous children"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    let view = previous.create_view(&Default::default());
                    inst.surface.previous_children(Some(&view));
                    previous
                }),
            };
            let view = texture.create_view(&Default::default());
            inst.surface.children(Some(&view));
            inst.children = Some(Children {
                texture,
                previous,
                width,
                height,
                metal: Some(raw as usize),
            });
        } else if let Some(Children {
            texture,
            previous: Some(previous),
            ..
        }) = &inst.children
        {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_texture(
                texture.as_image_copy(),
                previous.as_image_copy(),
                size,
            );
            gpu.queue.submit([encoder.finish()]);
        }
        inst.children_generation += 1;
        inst.dirty = true;
        true
    }

    /// A canvas's picture as pixels, rendered again into a module-owned
    /// texture (LLP 1014: a canvas nested under a canvas painted through its
    /// surface paints this into its ancestor's capture), and whether the
    /// surface wants another frame. Nothing before the first bind.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn readback(&mut self, id: u32, frame: &Frame) -> Option<(fixture::Pixels, bool)> {
        self.check_device();
        let gpu = self.gpu.as_ref()?;
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return None;
        };
        if !inst.bound {
            return None;
        }
        let frame = Frame {
            seekable: self.seekable,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
            ..*frame
        };
        let result = fixture::render(gpu, inst.surface.as_mut(), &frame);
        inst.drain();
        if let Some(SurfaceError(e)) = inst.surface.take_error() {
            self.error = e;
            return None;
        }
        match result {
            Ok((pixels, wants)) => {
                // The picture was taken: nothing is unshown any more.
                inst.dirty = false;
                Some((pixels, wants))
            }
            Err(e) => {
                self.error = e;
                None
            }
        }
    }
}

/// Run a future to completion on this thread. wgpu's adapter and device
/// requests are futures that complete synchronously on native backends;
/// this is the whole executor they need.
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    // SAFETY: the vtable's functions do nothing and the data pointer is null and never read.
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// Create the device: the first adapter that can present, with default
/// limits. Natively synchronous; on the web, awaited by the loader.
pub async fn load_gpu(
    instance: wgpu::Instance,
    compatible: Option<&wgpu::Surface<'_>>,
) -> Result<Gpu, String> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: compatible,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no adapter: {e}"))?;
    // The limits: wgpu's defaults where the adapter meets them; else its
    // downlevel defaults with this adapter's texture resolution — what the
    // iOS simulator's Metal is (its device is below the Apple4 family and
    // passes 15 inter-stage variables to the default's 16; an iPhone since
    // the A11 passes 31). Everything a surface here uses fits both; a
    // device is never refused for a limit no surface needs.
    let available = adapter.limits();
    let required_limits = if wgpu::Limits::default().check_limits(&available) {
        wgpu::Limits::default()
    } else {
        wgpu::Limits::downlevel_defaults().using_resolution(available)
    };
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("exact"),
            required_limits,
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no device: {e}"))?;
    Ok(Gpu {
        instance,
        adapter,
        device,
        queue,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub mod fixture;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(test)]
mod device_loss_tests {
    use super::*;
    struct Probe;
    impl Surface for Probe {
        fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn render(
            &mut self,
            _: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            panic!("lost device rendered")
        }
        fn agent(&mut self, _: &str) -> Option<String> {
            Some("alive".into())
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || Box::new(Probe))],
        shaders: &[],
    };
    #[test]
    fn device_loss_guards_each_gpu_entry_before_any_other_call() {
        for operation in 0..7 {
            let Ok(gpu) = fixture::device() else { return };
            let mut m = Module::new(&REGISTRY);
            m.set_gpu(gpu);
            let id = m.create_headless("probe").unwrap();
            m.bind(id, &[], None);
            // Model the asynchronous callback, without calling lose_device first.
            m.device_lost.store(true, Ordering::Release);
            match operation {
                0 => assert!(!m.child(id, 0, [0., 0., 1., 1.], 1, 1, &[0; 4])),
                1 => assert!(!m.texture(id, 1, 1, &[0; 4])),
                2 => assert!(m
                    .readback(
                        id,
                        &Frame {
                            width: 1.,
                            height: 1.,
                            scale: 1.,
                            now_ms: 0.,
                            seekable: true,
                            children_generation: 0,
                            shader_generation: 0
                        }
                    )
                    .is_none()),
                3 => assert!(!m.dirty(id)),
                4 => assert!(!m.sync()),
                5 => assert!(m.gpu().is_none()),
                _ => {
                    #[cfg(any(target_os = "macos", target_os = "ios"))]
                    // A sentinel must never be retained/imported after loss.
                    assert!(!unsafe { m.texture_from_metal(id, 1, 1, std::ptr::dangling_mut()) });
                }
            }
            assert_eq!(m.agent(id, "state").as_deref(), Some("alive"));
            assert!(!m.dirty(id));
        }
    }
}

/// A relative path under assets/, using the portable ASCII filename vocabulary.
pub fn asset_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b))
        && name.split('/').all(|part| part != "..")
}
