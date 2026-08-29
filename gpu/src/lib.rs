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
//! cross. Failures are per canvas, reported by [`Module::error`], never a
//! refusal of anything else.
//!
//! - [`Surface`], [`Frame`] — what an app implements.
//! - [`Module`] — the device and the instances, one per canvas node.
//! - [`json`] — the values as the batch carries them.
//! - [`module!`] — the exports for one app's registry.
//! - `fixture` (native) — a surface rendered and read back, for fixtures.

#![deny(missing_docs)]

use std::collections::HashMap;

pub use exact_plan::Value;
pub use wgpu;

pub mod json;

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

/// Why a surface refused its inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceError(pub String);

/// What an app implements per canvas.
pub trait Surface {
    /// The canvas's inputs from the plan, as typed values; before the
    /// first render and whenever they change. A refusal names the input.
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError>;
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
    /// Whether the surface samples the canvas's children (LLP 1014 D2). A
    /// host that can paint a subtree then hands it to [`Surface::children`]
    /// and stops compositing the children itself; a host that cannot, or a
    /// surface that answers `false`, leaves them composited over the surface.
    fn wants_children(&self) -> bool {
        false
    }
    /// The canvas's children as a texture (LLP 1014 D2, D3) — laid
    /// out by the kernel in the canvas's box, painted by the host at the
    /// canvas's scale, premultiplied RGBA — for the surface to sample;
    /// `None` when there are none. Called when the texture is created or
    /// replaced; its contents update in place.
    fn children(&mut self, _texture: Option<&wgpu::TextureView>) {}
}

/// Makes a surface.
pub type Factory = fn() -> Box<dyn Surface>;

/// An app's surfaces: name, arity, factory.
pub struct Registry(pub &'static [(&'static str, usize, Factory)]);

/// The device and every canvas's surface.
pub struct Module {
    registry: &'static Registry,
    gpu: Option<Gpu>,
    instances: HashMap<u32, Instance>,
    next: u32,
    error: String,
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
    target: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    bound: bool,
    dirty: bool,
    children: Option<Children>,
}

/// A canvas's children, painted by the host, on the device (LLP 1014 D3).
struct Children {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
}

impl Module {
    /// A module with no device yet.
    pub fn new(registry: &'static Registry) -> Module {
        Module {
            registry,
            gpu: None,
            instances: HashMap::new(),
            next: 0,
            error: String::new(),
        }
    }

    /// Adopt a device (the platform-specific loader made it).
    pub fn set_gpu(&mut self, gpu: Gpu) {
        self.gpu = Some(gpu);
    }

    /// The device, when loaded.
    pub fn gpu(&self) -> Option<&Gpu> {
        self.gpu.as_ref()
    }

    /// The last failure's text, for the presenter to report.
    pub fn error(&self) -> &str {
        &self.error
    }

    fn fail<T>(&mut self, e: impl Into<String>) -> Option<T> {
        self.error = e.into();
        None
    }

    /// Create a canvas's surface by name on a platform target; `None` (and
    /// [`Module::error`]) when the name is unknown or the target refused.
    pub fn create(
        &mut self,
        name: &str,
        target: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Option<u32> {
        let Some(gpu) = self.gpu.as_ref() else {
            return self.fail("no device");
        };
        let Some((_, _, factory)) = self.registry.0.iter().find(|(n, _, _)| *n == name) else {
            return self.fail(format!("no surface named `{name}` in this module"));
        };
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
        self.next += 1;
        let id = self.next;
        self.instances.insert(
            id,
            Instance {
                surface: factory(),
                target,
                config,
                bound: false,
                dirty: false,
                children: None,
            },
        );
        Some(id)
    }

    /// Whether a canvas's surface samples its children (LLP 1014 D2).
    pub fn wants_children(&self, id: u32) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|i| i.surface.wants_children())
    }

    /// The canvas's children, painted by the host (LLP 1014 D3): `width`×`height`
    /// premultiplied RGBA, rows top-down, tightly packed. Creates or
    /// replaces the texture at a new size, writes the pixels, and marks the
    /// canvas dirty.
    pub fn texture(&mut self, id: u32, width: u32, height: u32, bytes: &[u8]) -> bool {
        let expected = width as usize * height as usize * 4;
        if width == 0 || height == 0 || bytes.len() != expected {
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
        let same = matches!(&inst.children, Some(c) if c.width == width && c.height == height);
        if !same {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("children"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            inst.surface.children(Some(&view));
            inst.children = Some(Children {
                texture,
                width,
                height,
            });
        }
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
    pub fn bind(&mut self, id: u32, inputs: &[Value]) -> bool {
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail::<()>("no such canvas").is_some();
        };
        match inst.surface.bind(inputs) {
            Ok(()) => {
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

    /// Whether a canvas has something to render: inputs it has not shown.
    pub fn dirty(&self, id: u32) -> bool {
        self.instances.get(&id).is_some_and(|i| i.dirty)
    }

    /// Render one frame for a canvas at the given size; returns whether the
    /// surface wants another frame. Nothing happens before the first bind.
    pub fn render(&mut self, id: u32, frame: &Frame) -> Option<bool> {
        let (w, h) = frame.pixels();
        let Some(gpu) = self.gpu.as_ref() else {
            return self.fail("no device");
        };
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail("no such canvas");
        };
        if !inst.bound {
            return Some(false);
        }
        if inst.config.width != w || inst.config.height != h {
            inst.config.width = w;
            inst.config.height = h;
            inst.target.configure(&gpu.device, &inst.config);
        }
        use wgpu::CurrentSurfaceTexture as Current;
        let texture = match inst.target.get_current_texture() {
            Current::Success(t) => t,
            Current::Suboptimal(t) => {
                inst.target.configure(&gpu.device, &inst.config);
                t
            }
            // Nothing to draw into this frame; the inputs stay dirty.
            Current::Timeout | Current::Occluded => return Some(true),
            other => {
                self.error = format!("surface: {other:?}");
                return None;
            }
        };
        let view = texture.texture.create_view(&Default::default());
        let wants = inst
            .surface
            .render(frame, &gpu.device, &gpu.queue, &view, inst.config.format);
        gpu.queue.present(texture);
        inst.dirty = false;
        Some(wants)
    }

    /// Drop a canvas's surface.
    pub fn destroy(&mut self, id: u32) {
        self.instances.remove(&id);
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
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("exact"),
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
