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

#[cfg(not(target_arch = "wasm32"))]
mod acquire;
mod binding;
mod children;
mod frame;
mod input;
pub use input::{InputEvent, PointerKind, PointerPhase};
pub mod json;
pub mod shaders;
mod uniform;
pub use uniform::FrameUniform;

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
    /// The display's frame period in milliseconds as the host knows it (the web
    /// host's paced clock, CADisplayLink's duration); 0 while unknown or headless.
    /// Any animated surface may pace or look ahead by it; a world schedules its
    /// ticks and draws its interpolated pose against it.
    pub period_ms: f64,
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

/// Host presentation lifecycle, independent of saved surface data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lifecycle {
    /// The surface is no longer shown.
    Hidden,
    /// The surface is shown again.
    Visible,
    /// An external interruption of this surface's device work (call, alert, route change).
    Interrupted,
    /// The external interruption ended; device work may resume.
    Resumed,
}

/// Delivery failure, independent of GPU readiness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// The requested file does not exist.
    Missing,
    /// Terminal transport or integrity failure after bounded retries.
    Failed(String),
}
/// Why saved state is being restored.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Restore {
    /// Open an authored save, preserving its saved definitions.
    #[default]
    Open = 0,
    /// Carry dynamic state to newly loaded code and definitions.
    Carry = 1,
}
impl Restore {
    /// Decode the host ABI's explicit restore purpose.
    pub fn from_code(code: u32) -> Result<Self, String> {
        match code {
            0 => Ok(Self::Open),
            1 => Ok(Self::Carry),
            _ => Err(format!("invalid restore purpose {code}")),
        }
    }
}

/// How a surface composes its canvas children; the host owns overlay composition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChildrenMode {
    /// Ordinary host composition over the canvas.
    #[default]
    Overlay,
    /// One captured subtree, optionally retaining the previous upload.
    Composite {
        /// Keep the subtree texture from before the latest upload.
        previous: bool,
    },
    /// Separate captured children, with per-child placement.
    Each,
}

impl ChildrenMode {
    /// Host ABI: overlay=0, composite=1, composite with history=2, each=3.
    pub fn code(self) -> u32 {
        match self {
            Self::Overlay => 0,
            Self::Composite { previous: false } => 1,
            Self::Composite { previous: true } => 2,
            Self::Each => 3,
        }
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
/// One asset delivery transaction. Retirements precede new requests.
pub struct AssetChanges {
    /// Relative asset paths to deliver.
    pub requests: Vec<String>,
    /// Previous deliveries/flights to forget, including names requested again.
    pub retired: Vec<String>,
}
impl AssetChanges {
    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"requests\":{},\"retired\":{}}}",
            json::strings(&self.requests),
            json::strings(&self.retired)
        )
    }
}

/// What an app implements per canvas.
pub trait Surface {
    /// Device work follows visibility: a hidden chart stops its ticker, a video
    /// stops decoding, and both resume when shown. This never advances saved
    /// state; events still arrive when the agent owns the clock.
    fn lifecycle(&mut self, _event: Lifecycle) {}
    /// Clock ownership, before first input and whenever it changes. A seekable
    /// chart/video uses explicit time and must not open a live device.
    fn clock(&mut self, _seekable: bool) {}
    /// Named inputs and their defaults, in bind order. An empty declaration
    /// accepts positional inputs only. Names are resolved before `bind` runs.
    fn arguments(&self) -> Vec<(&'static str, Value)> {
        Vec::new()
    }
    /// The canvas's inputs from the plan, as typed values; before the
    /// first render and whenever they change. A refusal names the input.
    fn bind(&mut self, inputs: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError>;
    /// Drain asset requests and retirements together; hosts cancel old flights first.
    fn assets(&mut self) -> AssetChanges {
        AssetChanges::default()
    }
    /// Deliver encoded content, a missing name, or a terminal transport failure.
    fn asset(&mut self, _name: &str, _bytes: Result<&[u8], AssetError>) {}
    /// Complete device preparation inside asset delivery, before reporting readiness.
    fn prepare_assets(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _format: wgpu::TextureFormat,
    ) {
    }
    /// Device resources are validating asynchronously; a fixed clock still needs redraws.
    fn preparing(&self) -> bool {
        false
    }
    /// State as bytes this surface can later restore: a save or a dev reload's carry.
    /// None means this surface has nothing worth carrying.
    fn carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok(None)
    }
    /// Take back a carry, possibly from an older build. Err leaves state unchanged.
    fn restore(&mut self, _bytes: &[u8], _mode: Restore) -> Result<(), String> {
        Err("this surface carries no state".into())
    }
    /// One frame into `target` (of `format`), recorded into `encoder`.
    /// Returns whether another frame is wanted without new inputs.
    ///
    /// The encoder is the module's, shared by every canvas the host renders
    /// in this tick, and submitted once after the last of them (LLP 1009
    /// D7): a surface records its passes and never submits. Writes through
    /// `queue` (`write_buffer`, `write_texture`) land before any of the
    /// tick's commands run — so a resource another canvas's commands read in
    /// the same tick is not rewritten here; per-instance resources behave as
    /// if this canvas were submitted alone.
    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool;
    /// The commands the last `render` recorded were submitted: map what it
    /// copied for reading (a map cannot precede the submit of the copy).
    fn submitted(&mut self) {}
    /// A presentation target acquired a device, before its first draw. Its
    /// granted features (e.g. block-compressed textures) choose what to fetch.
    fn device_ready(&mut self, _features: wgpu::Features) {}
    /// Presentation was lost; release device resources without discarding owned state.
    fn device_lost(&mut self) {}
    /// Drain an error discovered while rendering or advancing committed state.
    fn take_error(&mut self) -> Option<SurfaceError> {
        None
    }
    /// Raw input inside this canvas (LLP 1046.002 S1); app gestures elsewhere are untouched.
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
    /// Child composition selected by this surface (LLP 1014).
    fn children_mode(&self) -> ChildrenMode {
        ChildrenMode::Overlay
    }
    /// The `index`th direct child's texture (created or resized; contents
    /// update in place) and its frame in the canvas's points — `x, y, width,
    /// height`, and its Contract `testId` (empty if unnamed). A `None` texture
    /// means the host composites this child (web/Linux). An empty name, no
    /// texture and an empty frame together mean it is gone.
    fn child(
        &mut self,
        _index: usize,
        _name: &str,
        _texture: Option<&wgpu::TextureView>,
        _frame: [f32; 4],
    ) {
    }
    /// Where the surface put the `index`th child: a 3×3 homography, row
    /// major, from the child's own points (origin at its top-left corner) to
    /// the canvas's points — the browser's `canvasTransform` — and its depth,
    /// larger nearer the eye, which orders hit-testing where children
    /// overlap (the browser's hit-test stack follows draw order). `None` is
    /// the kernel's frame, untouched. The host inverts the homography to
    /// hit-test and reports the mapped box to accessibility. A hidden placement
    /// excludes the subtree from painting, hit-testing and accessibility.
    fn placement(&self, _index: usize) -> Option<Placement> {
        None
    }
    /// The canvas's children as a texture (LLP 1014 D2, D3) — laid
    /// out by the kernel in the canvas's box, painted by the host at the
    /// canvas's scale, premultiplied RGBA — for the surface to sample;
    /// `None` when there are none. Called when the texture is created or
    /// replaced; its contents update in place.
    /// `previous` is the texture before the latest upload when requested by Composite.
    fn children(
        &mut self,
        _current: Option<&wgpu::TextureView>,
        _previous: Option<&wgpu::TextureView>,
    ) {
    }
}

/// Where a surface put a child (LLP 1014 D5): see [`Surface::placement`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Explicitly absent from presentation; `None` still means the kernel frame.
    pub hidden: bool,
    /// Child points to canvas points, row major, projective.
    pub homography: [f32; 9],
    /// Larger nearer the eye.
    pub depth: f32,
    /// Near/far clipping in child coordinates; each row is ax + by + c >= 0.
    /// Native geometry clips in the GPU; software compositors apply these rows.
    pub clip_depth: [[f32; 3]; 2],
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
    instance: Option<wgpu::Instance>,
    device_lost: Arc<AtomicBool>,
    instances: HashMap<u32, Instance>,
    next: u32,
    error: String,
    seekable: bool,
    period_ms: f64,
    /// The tick's commands, recorded by every canvas rendered since the last
    /// flush (LLP 1009 D7).
    open: Option<frame::Open>,
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

mod recovery;

struct Instance {
    surface: Box<dyn Surface>,
    messages: Vec<String>,
    published: Option<String>,
    presentation: Option<Arc<wgpu::Surface<'static>>>,
    config: Option<wgpu::SurfaceConfiguration>,
    /// The next texture, acquired off the presenter's thread (`acquire`).
    #[cfg(not(target_arch = "wasm32"))]
    acquire: acquire::Acquire,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    layer: Option<usize>,
    outstanding: BTreeSet<String>,
    answered: BTreeSet<String>,
    bound: bool,
    dirty: bool,
    /// Its first frame has been handed to the compositor (`Module::seen`);
    /// shared with the callback that says so.
    seen: Arc<AtomicBool>,
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
            instance: None,
            device_lost: Arc::new(AtomicBool::new(false)),
            instances: HashMap::new(),
            next: 0,
            error: String::new(),
            seekable: false,
            period_ms: 0.0,
            open: None,
        }
    }

    /// Adopt a device (the platform-specific loader made it).
    pub fn set_gpu(&mut self, gpu: Gpu) {
        self.check_device();
        // A frame recorded on another device is never submitted on this one.
        self.open = None;
        self.instance = Some(gpu.instance.clone());
        self.device_lost = Arc::new(AtomicBool::new(false));
        let lost = self.device_lost.clone();
        gpu.device.set_device_lost_callback(move |_, _| {
            lost.store(true, Ordering::Release);
            #[cfg(target_arch = "wasm32")]
            crate::web::notify_loss(&lost);
        });
        for inst in self.instances.values_mut() {
            let format = if let (Some(target), Some(config)) = (&inst.presentation, &inst.config) {
                target.configure(&gpu.device, config);
                config.format
            } else {
                if inst.config.is_some() {
                    continue;
                }
                // Web targets are reattached by web::recover; native offscreen
                // surfaces use the same format as readback.
                #[cfg(target_arch = "wasm32")]
                continue;
                #[cfg(not(target_arch = "wasm32"))]
                wgpu::TextureFormat::Rgba8Unorm
            };
            inst.surface.device_ready(gpu.device.features());
            inst.surface.prepare_assets(&gpu.device, &gpu.queue, format);
            inst.dirty = true;
        }
        self.gpu = Some(gpu);
    }

    pub(crate) fn recovery_report(&mut self) -> String {
        let mut ids: Vec<_> = self.instances.keys().copied().collect();
        ids.sort_unstable();
        let rows: Vec<_> = ids
            .into_iter()
            .map(|id| {
                let inst = self.instances.get_mut(&id).unwrap();
                let state = inst
                    .surface
                    .agent("{\"op\":\"state\"}")
                    .unwrap_or("null".into());
                format!("{{\"id\":{id},\"preparation\":{state}}}")
            })
            .collect();
        format!(
            "{{\"status\":\"recovered\",\"instances\":[{}]}}",
            rows.join(",")
        )
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
        self.flush();
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
        surface.clock(self.seekable);
        if let (Some(_), Some(gpu)) = (&presentation, &self.gpu) {
            surface.device_ready(gpu.device.features());
        }
        let (presentation, config) = presentation.map_or((None, None), |(target, config)| {
            (Some(Arc::new(target)), Some(config))
        });
        self.instances.insert(
            id,
            Instance {
                surface,
                messages: Vec::new(),
                published: None,
                presentation,
                config,
                #[cfg(not(target_arch = "wasm32"))]
                acquire: Default::default(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                layer: None,
                outstanding: BTreeSet::new(),
                answered: BTreeSet::new(),
                bound: false,
                dirty: false,
                seen: Default::default(),
                children: None,
                children_generation: 0,
                each: Vec::new(),
            },
        );
        Some(id)
    }

    /// Release presentation resources while preserving every surface's state.
    pub fn lose_device(&mut self) {
        // The recorded frame and its drawables go before their surfaces do.
        self.open = None;
        for inst in self.instances.values_mut() {
            inst.presentation = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                inst.acquire = Default::default();
            }
            inst.surface.device_lost();
            inst.answered.clear();
            inst.outstanding.clear();
            inst.dirty = true;
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

    /// The display's frame period, from the host, for every frame that follows.
    pub fn set_period(&mut self, period_ms: f64) {
        self.period_ms = if period_ms.is_finite() && period_ms > 0.0 {
            period_ms
        } else {
            0.0
        };
    }

    /// Set once by an agent host: every frame honours the seekable clock.
    pub fn set_seekable(&mut self, on: bool) {
        self.flush();
        if self.seekable == on {
            return;
        }
        self.seekable = on;
        for inst in self.instances.values_mut() {
            inst.surface.clock(on);
            inst.drain();
        }
    }

    /// Deliver host lifecycle codes: 0 hidden, 1 visible, 2 interrupted, 3 resumed.
    /// Unknown codes are ignored, including from a newer host.
    pub fn lifecycle(&mut self, id: u32, code: u32) {
        self.settle(id);
        let event = match code {
            0 => Lifecycle::Hidden,
            1 => Lifecycle::Visible,
            2 => Lifecycle::Interrupted,
            3 => Lifecycle::Resumed,
            _ => return,
        };
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.surface.lifecycle(event);
            inst.drain();
        }
    }

    /// Whether this canvas asks for raw device input.
    pub fn wants_input(&self, id: u32) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|i| i.surface.wants_input())
    }

    /// Deliver one device event and mark the canvas dirty.
    pub fn input(&mut self, id: u32, event: &InputEvent) -> bool {
        self.settle(id);
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
        self.settle(id);
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

    /// New inputs for a canvas; a refusal is reported and the surface keeps
    /// its last accepted inputs.
    pub fn bind(&mut self, id: u32, inputs: &[Value], at_ms: Option<f64>) -> bool {
        self.settle(id);
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail::<()>("no such canvas").is_some();
        };
        match inst.surface.bind(inputs, at_ms) {
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
    pub fn take_assets(&mut self, id: u32) -> AssetChanges {
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return AssetChanges::default();
        };
        let mut wanted = Vec::new();
        let AssetChanges {
            requests: requested,
            retired,
        } = inst.surface.assets();
        if !retired.is_empty() {
            inst.dirty = true;
        }
        for name in &retired {
            inst.answered.remove(name);
            inst.outstanding.remove(name);
        }
        for name in requested {
            if !asset_name(&name) {
                self.error =
                    format!("asset `{name}`: expected a relative asset path without .. segments");
            } else if !inst.answered.contains(&name) && !inst.outstanding.contains(&name) {
                if inst.answered.len() + inst.outstanding.len() >= 256 {
                    inst.surface.asset(
                        &name,
                        Err(AssetError::Failed(
                            "surface limit is 256 asset names".into(),
                        )),
                    );
                    inst.dirty = true;
                    continue;
                }
                inst.outstanding.insert(name.clone());
                wanted.push(name);
            }
        }
        AssetChanges {
            requests: wanted,
            retired,
        }
    }

    /// Deliver one requested asset, with None for a missing file; works without a device.
    pub fn asset(&mut self, id: u32, name: &str, bytes: Result<&[u8], AssetError>) -> bool {
        self.settle(id);
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
        inst.answered.insert(name.into());
        inst.surface.asset(name, bytes);
        if let (Some(gpu), Some(config)) = (&self.gpu, &inst.config) {
            inst.surface
                .prepare_assets(&gpu.device, &gpu.queue, config.format);
        }
        inst.drain();
        inst.dirty = true;
        if let Some(SurfaceError(error)) = inst.surface.take_error() {
            self.error = error;
            return false;
        }
        true
    }

    /// Capture state without advancing the surface or consuming its publications.
    pub fn carry(&mut self, id: u32) -> Result<Option<Vec<u8>>, SurfaceError> {
        self.check_device();
        let inst = self
            .instances
            .get_mut(&id)
            .ok_or_else(|| SurfaceError("no such canvas".into()))?;
        inst.surface.carry()
    }

    /// Restore atomically; successful state is published before the next frame.
    pub fn restore(&mut self, id: u32, bytes: &[u8], mode: Restore) -> bool {
        self.settle(id);
        self.check_device();
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return false;
        };
        match inst.surface.restore(bytes, mode) {
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
        self.has_device(id)
            && self
                .instances
                .get(&id)
                .is_some_and(|i| i.dirty || i.surface.preparing())
    }

    /// Whether a canvas's last render went without a drawable (`acquire`):
    /// the presenter renders it again when the module says one arrived.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn starved(&self, id: u32) -> bool {
        self.instances.get(&id).is_some_and(|i| i.acquire.starved())
    }

    /// Whether a render of the canvas has something to do (`acquire`): its
    /// drawable in flight has landed, or none is in flight. A presenter
    /// skips the render of a starved canvas while this is `false`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn landed(&self, id: u32) -> bool {
        self.instances.get(&id).is_some_and(|i| i.acquire.landed())
    }

    /// Every command submitted to the device so far, complete (LLP 1008 §9):
    /// a host that hands the module textures it renders itself waits here
    /// before drawing into one the module may still be reading — sampling
    /// it, or copying it into the previous children.
    pub fn sync(&mut self) -> bool {
        self.flush();
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
        self.settle(id);
        self.instances.remove(&id);
    }

    /// Import the host's retained Metal children texture; reuse an unchanged
    /// pointer, copying previous children before the next hand-over.
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
        self.settle(id);
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
                _ => matches!(
                    inst.surface.children_mode(),
                    ChildrenMode::Composite { previous: true }
                )
                .then(|| {
                    gpu.device.create_texture(&wgpu::TextureDescriptor {
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
                    })
                }),
            };
            let view = texture.create_view(&Default::default());
            let previous_view = previous
                .as_ref()
                .map(|p| p.create_view(&Default::default()));
            inst.surface.children(Some(&view), previous_view.as_ref());
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
        self.flush();
        let gpu = self.gpu.as_ref()?;
        let Some(inst) = self.instances.get_mut(&id) else {
            self.error = "no such canvas".into();
            return None;
        };
        if !inst.bound {
            return None;
        }
        let format = inst
            .config
            .as_ref()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |c| c.format);
        inst.surface.prepare_assets(&gpu.device, &gpu.queue, format);
        let frame = Frame {
            seekable: self.seekable,
            period_ms: self.period_ms,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
            ..*frame
        };
        let (width, height) = frame.pixels();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas readback"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let wants =
            inst.surface
                .render(&frame, &gpu.device, &gpu.queue, &mut encoder, &view, format);
        gpu.queue.submit([encoder.finish()]);
        inst.surface.submitted();
        let result = fixture::read(gpu, &texture).map(|pixels| (pixels, wants));
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

mod device;
pub use device::{load_gpu, requested_features, requested_limits};

#[cfg(not(target_arch = "wasm32"))]
pub mod fixture;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod web;

mod asset_name;
pub use asset_name::asset_name;
