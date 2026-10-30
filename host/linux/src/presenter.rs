//! The presenter: painter state beyond the kernel — scrolling, images, focus
//! and the pointer — and operations from frames through input to screenshots.
//!
//! @ref LLP 1015 §4; LLP 1010 §3 (scroll chaining: the web's
//! `overscroll-behavior: auto`); LLP 1012 (the five host-side operations)
//!
//! Scroll offsets are host state, never plan state (LLP 1010). The window
//! is a viewport over a document: the page scrolls when the roots' extent
//! exceeds it. A press is a hit at a point — the deepest painted box under
//! it, then up to the nearest node with a `press` handler, the path a click
//! takes in a browser. A wheel goes to the innermost scroll container under
//! the point that can take its dominant axis, else to the page.
use crate::gpu::Gpu;
use crate::host::{Host, HostError, PlanBytes};
use crate::image::AssetResolver;
use crate::image::{Assets, Images};
use crate::paint::{
    content_size, effective_overflow, Backend, Frame, PaintedBox, Painter, Rect4, Scene,
};
use crate::raster::Raster;
use crate::text::{Measurer, Shared, TextEngine};
use exact_kernel::{NodeType, Overflow, PropId, ViewId};
use exact_plan::{EventKind, Plan};
use exact_runner::agent::{num, quote};
use exact_runner::{DataSource, Event};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tiny_skia::Pixmap;

pub mod access;
mod addressed_tap;
mod ahead;
mod alert;
mod arrange;
mod arrange_geometry;
mod arrive;
mod back;
pub mod bars;
mod build_channel;
pub mod clipboard;
mod clock;
mod collection;
mod commands;
mod contact;
mod control;
mod delivery;
#[cfg(windows)]
pub mod desktop_access;
mod device;
mod display_frame;
pub mod edit_menu;
mod events;
mod feedback;
mod field;
mod field_pointer;
mod field_scroll;
#[cfg(test)]
mod field_tests;
#[cfg(test)]
mod frame_target_tests;
mod group;
mod hatch;
mod history;
pub mod ime;
mod painter;
mod pan_release;
mod picker;
mod pointer;
mod pump;
mod radio;
mod range_frame;
#[cfg(test)]
mod save_tests;
mod svg_hit;
#[cfg(target_os = "linux")]
pub(crate) use display_frame::SubmittedFrame;
#[path = "content_region/presenter.rs"]
mod content_region;
#[cfg(test)]
#[path = "content_region/presenter_tests.rs"]
mod content_region_tests;
#[cfg(test)]
#[path = "content_region/wide_tests.rs"]
mod content_region_wide_tests;
pub(crate) mod contrast;
#[cfg(any(target_os = "android", test))]
mod feed;
mod height;
mod height_drag;
#[cfg(test)]
mod height_drag_tests;
mod hosted;
mod images;
mod links;
mod palette;
pub mod preferences;
pub(crate) mod remote;
mod requests;
mod retained_action;
mod reveal;
#[cfg(test)]
mod scroll_anchor_tests;
mod scroll_end;
mod scrollbar;
mod scrollers;
pub(crate) mod split;
mod wheel;
use scroll_end::Completion;
#[cfg(all(test, not(windows)))]
mod scroll_end_tests;
mod shortcuts;
#[cfg(target_os = "android")]
pub(crate) mod still;
mod swipe;
pub mod top;
mod transform;
mod transform_geometry;
mod typing;
mod window;
#[cfg(windows)]
mod word;

#[cfg(test)]
mod collection_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod kept_rows_tests;
#[cfg(test)]
mod longpoll_tests;
#[cfg(test)]
mod spacing_tests;
#[cfg(test)]
mod swipe_tests;
#[cfg(test)]
mod visibility_tests;

pub use clock::{wall_since, Wall};
pub use device::{touch_only, Device, Pointing};
pub use hosted::action as field_action;
use painter::cpu_info;
pub(crate) use painter::PainterBoot;
pub use painter::{set_custom_painter, PainterChoice, PainterFactory, PainterInfo};
pub(crate) use palette::reported_accent;
pub use palette::{material, ContrastPalette, Material, Palette, MATERIAL_ROLES};
pub use pointer::PointerKind;
pub use requests::{Haptic, Pick, Request};
pub use scrollers::{Scroller, Under, PAGE};
pub use window::{Chrome, Window};

/// The presenter: one host, its painter, and the host state.
pub struct Presenter<D: DataSource> {
    /// Scrolled lists whose collection turn waits for the frame (see
    /// [`Presenter::set_deferred_collections`]), with whether input moved the
    /// acknowledged offset; `None` when turns run at once.
    deferred_collections: Option<Vec<(ViewId, bool)>>,
    pub(crate) host: Host<D>,
    /// The scroller the last wheel moved.
    last_wheel: Option<ViewId>,
    /// The display's clock while it owns time, given to every host (`clock.rs`)
    live_clock: Option<clock::Wall>,
    native_timers: bool,
    timers_hidden: bool,
    clock_generation: u64,
    timer_observations: Option<(bool, bool, bool)>,
    idle_timer_retry: bool,
    pub(crate) surfaces: crate::surfaces::Surfaces,
    module: Option<crate::delivery::Module>,
    painted: bool,
    activation_failed: bool,
    dev: Option<crate::fetch::Poller>,
    pending_dev: Option<crate::fetch::Generation>,
    pending_update: bool,
    text: Shared,
    /// A touch down's press, to lay out ahead (`ahead.rs`).
    ahead: Option<ahead::Ahead>,
    pub(crate) brush: Painter,
    viewport: (f32, f32),
    scroll: BTreeMap<ViewId, (f32, f32)>,
    scroll_end: scroll_end::State,
    page: (f32, f32),
    images: Images,
    pub(crate) assets: Assets,
    /// The binary's `compat.json`, also given to a fresh runner on reload (LLP 1030 D3a).
    pub(crate) compat: String,
    /// How many hosts this presenter has had: a frame sampler starts over
    /// when another replaces the one it measured (LLP 1079 D3).
    pub(crate) hosts: u64,
    pub(crate) focus: Option<ViewId>,
    /// Last connected path from the focused node, for a removal commit.
    focus_out: Vec<(ViewId, Event)>,
    /// The current pointer press may yield its focus to a newly mounted field.
    pub(crate) autofocus_from: Option<ViewId>,
    /// The controls commits disabled and blurred (`blur_disabled_focus`) since
    /// a press began, so its fallback does not give its own the focus back.
    pub(crate) disabled_blur: Vec<ViewId>,
    /// The text field typed into since it took the focus: its `change`
    /// fires on blur or Enter, HTML's commit (LLP 1069.001 D4).
    pub(crate) edited: Option<ViewId>,
    /// Each text field's selection (x2apps codeedit #2), kept for the value
    /// it indexes, focused or not: the next key there edits at it.
    pub(crate) fields: BTreeMap<ViewId, field::FieldMark>,
    field_history: history::Histories,
    field_drag: Option<field_pointer::Drag>,
    ime: Option<ime::Composition>,
    ime_epoch: u64,
    field_reveal: bool,
    window_active: bool,
    window_focus: Option<exact_kernel::NodeKey>,
    clipboard: Option<Box<dyn clipboard::Clipboard>>,
    url_opener: Option<links::UrlOpener>,
    /// The modifier keys held, each side a bit (Shift, Control, Alt, Meta,
    /// left then right): a `key` event's flags (`KeyboardEvent.shiftKey`…).
    pub(crate) held: u8,
    /// The keys whose down a shortcut took at a world's canvas: their up
    /// belongs to no handler and no world (b6 review B1).
    pub(crate) shortcut_keys: std::collections::BTreeSet<String>,
    /// Unbound checkboxes' own states, as a browser keeps an uncontrolled
    /// control's (LLP 1069.001 D4); a bound one draws its `checked`.
    pub(crate) controls: BTreeMap<ViewId, bool>,
    /// A date's, range's or select's choice, or a field's typed text, since
    /// its bound value last changed, beside that bound value
    /// (`paint::control::choice`, `field_text`).
    pub(crate) chosen: BTreeMap<ViewId, (String, String)>,
    /// The select whose menu is open (LLP 1069.001 D7).
    pub(crate) menu: Option<ViewId>,
    autofocus_processed: std::collections::BTreeSet<ViewId>,
    /// What the focus's following of the route keeps (`arrive.rs`).
    route_focus: arrive::State,
    pointer: Option<(f32, f32)>,
    /// The nodes with a `hover` handler under the pointer, innermost first.
    hovered: Vec<ViewId>,
    /// Where the pointer last moved for hover (`hover_at`): a frame that
    /// moves content under it hovers again there (`follow_pointer`).
    hover_point: Option<(f32, f32)>,
    /// The node holding the pointer's `pointerdown` until it lifts: the innermost hearing a pointer event, which hears the moves.
    pointer_held: Option<exact_kernel::NodeKey>,
    /// Where the last pointer press was, its `clientX`/`clientY` (LLP 1021 D2).
    pub(crate) press_point: Option<(f32, f32)>,
    /// The nodes from the held one up that hear `pointerdown` or
    /// `pointerup`, innermost first: each hears the up.
    pointer_path: Vec<exact_kernel::NodeKey>,
    /// During a pointer or wheel walk, a retired contact is cancelled when it
    /// ends so no other walk runs inside it ([`Presenter::retire_pointer`]).
    pointer_walking: bool,
    /// The held pointer's buttons as DOM counts them: 1 primary, 2 secondary, 4 middle (review b5-b 1).
    pointer_buttons: u8,
    pub(crate) pads: crate::surfaces::controls::PadRoute,
    pub(crate) control_bindings: BTreeMap<(u32, u32), crate::surfaces::ControlBinding>,
    pub(crate) control_contact: Option<(ViewId, f32, f32)>,
    boxes: Vec<PaintedBox>,
    /// Bumped whenever `boxes` is replaced.
    boxes_serial: u64,
    /// The fields `rangeFrame` reads, as painted beside `boxes`.
    ranges: std::rc::Rc<range_frame::Painted>,
    pub(crate) dirty: bool,
    /// The app's `setScheme` (`None`: follow the system) and the appearance
    /// its shell or an agent says (LLP 1061 D5); light until either does.
    pub(crate) scheme: (Option<bool>, bool),
    /// The device's settings as its shell last told them (LLP 1115.001 D3.1).
    device: Option<Device>,
    text_scale: f32,
    /// Pointer kind and drag slop: mouse and shared slop until a shell says (D7.4).
    pointer_kind: PointerKind,
    slop: f64,
    /// The shell's window, its insets, and the page's place in it (D3.3).
    window: window::State,
    /// What a shell that scrolls by itself was told of its scrollers (D7.3).
    reader: scrollers::Reader,
    /// The colours last painted (D4), and what this asks of its shell (D5).
    colors: palette::State,
    requests: requests::State,
    /// The text fields the shell hosts, as it was told them (D8).
    hosted: hosted::State,
    /// The top layer's open surface, where a shell makes windows (D10).
    top: top::State,
    alert: Option<alert::Open>,
    /// The shell's top app bar and navigation bar, where it has them (D11).
    bars: bars::State,
    /// The accessibility tree its shell was last told, where one asks (D13).
    access: access::State,
    /// The viewport segments the agent set (LLP 1078 D7), for `layout.env`;
    /// the posture and counts are the runner's.
    pub(crate) segments: Vec<exact_kernel::Rect>,
    /// A failed painter's blank fallback cannot bless an update generation.
    pub(crate) last_frame_succeeded: bool,
    pub(crate) display: display_frame::State,
    /// Requested painter (`Auto` may change its mind after a failed frame).
    choice: PainterChoice,
    /// Boot font-scan milliseconds: the machine's cost, not the app's.
    pub fonts_ms: f64,
    /// The painter, for the report.
    pub painter: PainterInfo,
    /// The executor for a request that leaves the process (LLP 1016 D2).
    executor: crate::executor::Executor,
    /// Requests held at dispatch, released after a later commit (LLP 1027.002 D3).
    parked: BTreeMap<u64, exact_runner::RequestOut>,
    refusal_turn: bool,
    collection: collection::State,
    contact: Option<contact::Contact>,
    retained_motion: Option<retained_action::MotionPermit>,
    arrange: Option<arrange::State>,
    group: Option<group::State>,
    transform_geometry: transform_geometry::State,
    /// The update store, once the app opened one (LLP 1026 D9; `app.rs`).
    updates: Option<Box<dyn crate::delivery::Store>>,
    /// Commands from the last commits, consumed by `run_commands`.
    commands: Vec<exact_runner::Command>,
    /// Range replacement selects wait until the action's commands finish.
    field_select_tasks: Vec<ViewId>,
    field_change_tasks: std::collections::BTreeSet<ViewId>,
    /// Driven by the agent (`agent.rs`): a `share` is held, never refused
    /// (LLP 1069.003 D6).
    pub(crate) agent: bool,
    /// Boot resolves every initially referenced asset before first pixel.
    /// During that transaction its integrity refusal is returned as a boot
    /// error; later refusals are journaled without retitling a live session.
    booting: bool,
    content_registration: Option<crate::content_region::ContentRegionRegistration>,
    last_region_frame: Option<Arc<Pixmap>>,
    last_region_scale: Option<u32>,
    /// The app's hatches (LLP 1075.003.000.001; `presenter/hatch.rs`).
    pub(crate) hatches: crate::hatches::Session,
}
/// Two decimals, the agent API's precision.
fn r2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}
impl<D: DataSource> Presenter<D> {
    /// Boot from entry zero or one selected generation. The selected asset
    /// roster is complete: absent names cannot fall through to `root`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn boot_selected(
        plan: PlanBytes<'_>,
        data: D,
        viewport: (f32, f32),
        scale: f32,
        root: PathBuf,
        selected: Option<AssetResolver>,
        (compat, delivery): (&str, exact_runner::Delivery),
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
        painter: PainterBoot,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        let assets = match selected {
            Some(set) => Assets::selected(root, set),
            None => Assets::embedded(root),
        };
        let (mut presenter, error) = Self::boot_with_assets(
            plan,
            data,
            viewport,
            scale,
            assets,
            painter,
            Some(delivery),
            launch,
            region,
        )?;
        presenter.compat = compat.to_string();
        presenter.measure();
        Ok((presenter, error))
    }

    #[allow(clippy::too_many_arguments)]
    fn boot_with_assets(
        plan: PlanBytes<'_>,
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: Assets,
        painter: PainterBoot,
        delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        let choice = painter.choice;
        if region.is_some() && choice != PainterChoice::Cpu {
            return Err(HostError::Painter(
                "content-region trial requires explicit CPU painting (EXACT_PAINTER=cpu)".into(),
            ));
        }
        if region.is_some() {
            crate::text::transfer::PaintContext::new(scale)
                .map_err(|e| HostError::Painter(format!("content raster context: {e:?}")))?;
        }
        let t = std::time::Instant::now();
        let decoded = plan.decode().map_err(HostError::Plan)?;
        #[cfg(windows)]
        crate::app::boot_phase("plan-ready");
        #[cfg(windows)]
        let preparing = painter.prepare_direct();
        let text = TextEngine::shared_for_assets(&decoded, &assets);
        if let Some(reason) = assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        let fonts_ms = t.elapsed().as_secs_f64() * 1000.0;
        #[cfg(windows)]
        crate::app::boot_phase("fonts-ready");
        #[cfg(windows)]
        let opened = preparing
            .is_none()
            .then(|| painter.open())
            .transpose()
            .map_err(HostError::Painter)?;
        #[cfg(not(windows))]
        let (backend, painter) = painter.open().map_err(HostError::Painter)?;
        let (mut host, error) = Host::boot_decoded(
            decoded,
            data,
            Box::new(Measurer(text.clone())),
            viewport.0,
            viewport.1,
            None,
            delivery,
            launch,
            region,
            Some(text.clone()),
        )?;
        // @ref LLP 1115.000 D10.3 — a hairline is one device pixel of this.
        let error = error.or(host.set_display_scale(f64::from(scale)));
        #[cfg(windows)]
        let (backend, painter) = match preparing {
            Some(preparing) => PainterBoot::finish_direct(preparing.map_err(HostError::Painter)?)
                .map_err(HostError::Painter)?,
            None => opened.expect("synchronous painter"),
        };
        let mut images = Images::with_assets(assets.clone());
        images.fit(viewport, scale);
        if assets.is_selected() {
            if let Some(error) = error {
                return Err(HostError::Layout(error));
            }
            let metadata_ready =
                images.prepare_metadata(host.kernel(), &host.preorder(), Duration::from_secs(1));
            if let Some(reason) = assets.take_refusal() {
                return Err(HostError::Asset(reason));
            }
            if !metadata_ready {
                return Err(HostError::Layout(
                    "selected image metadata did not finish preparing".into(),
                ));
            }
        }
        images.enable_decode();
        // Initial selected layout and asset metadata/integrity accepted.
        // Decoded pixels and their natural dimensions arrive together later.
        // Only now may the app's queued requests reach its executor.
        let executor = host.executor();
        if let Some(note) = executor.note() {
            host.log(note.to_string());
        }
        let mut p = Presenter {
            last_wheel: None,
            host,
            executor,
            parked: BTreeMap::new(),
            refusal_turn: false,
            collection: collection::State::default(),
            contact: None,
            retained_motion: None,
            transform_geometry: Default::default(),
            arrange: None,
            group: None,
            brush: Painter::new(text.clone(), scale, backend),
            text,
            ahead: None,
            viewport,
            scroll: BTreeMap::new(),
            scroll_end: Default::default(),
            page: (0.0, 0.0),
            images,
            assets,
            compat: String::new(),
            hosts: 0,
            focus: None,
            focus_out: Vec::new(),
            autofocus_from: None,
            disabled_blur: Vec::new(),
            edited: None,
            fields: BTreeMap::new(),
            field_history: Default::default(),
            field_drag: None,
            ime: None,
            ime_epoch: 0,
            field_reveal: false,
            window_active: true,
            window_focus: None,
            clipboard: None,
            url_opener: None,
            held: 0,
            shortcut_keys: Default::default(),
            controls: BTreeMap::new(),
            chosen: BTreeMap::new(),
            menu: None,
            autofocus_processed: Default::default(),
            route_focus: Default::default(),
            pointer: None,
            hovered: Vec::new(),
            hover_point: None,
            pointer_held: None,
            press_point: None,
            pointer_path: Vec::new(),
            pointer_walking: false,
            pointer_buttons: 0,
            control_contact: None,
            pads: Default::default(),
            control_bindings: BTreeMap::new(),
            boxes: Vec::new(),
            boxes_serial: 0,
            ranges: Default::default(),
            dirty: true,
            deferred_collections: None,
            live_clock: None,
            native_timers: false,
            timers_hidden: false,
            clock_generation: 0,
            idle_timer_retry: false,
            timer_observations: cfg!(target_os = "android").then_some((false, false, true)),
            scheme: (None, false),
            device: None,
            text_scale: 1.0,
            pointer_kind: PointerKind::Mouse,
            slop: exact_motion::gesture::SLOP,
            window: Default::default(),
            reader: Default::default(),
            colors: Default::default(),
            requests: Default::default(),
            hosted: Default::default(),
            top: Default::default(),
            alert: None,
            bars: Default::default(),
            access: Default::default(),
            segments: Vec::new(),
            surfaces: Default::default(),
            module: None,
            painted: false,
            activation_failed: false,
            dev: None,
            pending_dev: None,
            pending_update: false,
            last_frame_succeeded: false,
            display: display_frame::State::default(),
            choice,
            fonts_ms,
            painter,
            updates: None,
            commands: Vec::new(),
            field_select_tasks: Vec::new(),
            field_change_tasks: Default::default(),
            agent: crate::app::AGENT_BOOT.get(),
            booting: true,
            content_registration: region,
            last_region_frame: None,
            last_region_scale: None,
            hatches: Default::default(),
        };
        p.read_ranges();
        p.set_system_scheme(false);
        let e = p.after_commit();
        p.booting = false;
        if let Some(reason) = p.assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        Ok((p, error.or(e)))
    }

    /// The dev loop's restart, state carried; pictures and offsets go, focus stays (LLP 1007 §6).
    pub fn reload(&mut self, plan: &[u8], data: D) -> Result<Option<String>, HostError> {
        self.reload_module(plan, data, self.module.clone(), false)
    }

    /// Replace a local development pair transactionally, carrying compatible state.
    pub fn reload_module(
        &mut self,
        plan: &[u8],
        data: D,
        module: Option<crate::delivery::Module>,
        let_go: bool,
    ) -> Result<Option<String>, HostError> {
        if self.content_registration.is_some() {
            return Err(HostError::Layout(
                "content-region trial requires session retirement before reload".into(),
            ));
        }
        let module = module.or_else(|| self.module.clone());
        let decoded = Plan::decode(plan).map_err(HostError::Plan)?;
        // Preserve live font state until the candidate runner boots successfully.
        let candidate_text = TextEngine::shared_for_assets(&decoded, &self.assets);
        if let Some(reason) = self.assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        let kept = self.focus_place();
        let mut carried = self.host.carry();
        let data = self.prepare_logic(
            plan,
            data,
            module.as_ref(),
            &candidate_text,
            &mut carried,
            self.host.runner().delivery(),
            let_go,
        )?;
        let (mut host, error) = exact_runner::time::launching_with(
            exact_runner::time::Launch {
                time: Some(self.host.runner().wall_time()),
                place: Some(self.host.runner().place().clone()),
                ..exact_runner::time::Launch::NONE
            },
            || {
                Host::boot_with(
                    plan,
                    data,
                    Box::new(Measurer(candidate_text.clone())),
                    self.viewport.0,
                    self.viewport.1,
                    Some(&carried),
                    Some(self.host.runner().delivery().clone()),
                )
            },
        )?;
        if let Some(error) = error {
            return Err(HostError::Layout(error));
        }
        self.restore_time(&mut host)?;
        let shaders = self
            .surfaces
            .prepare_shaders(&self.compat, &self.assets)
            .map_err(HostError::Asset)?;
        self.surfaces
            .commit_shaders(shaders)
            .map_err(HostError::Asset)?;
        self.host = host;
        self.replaced();
        if self.display.new_session() {
            self.painted = false;
        }
        self.activation_failed = false;
        self.module = module;
        self.text = candidate_text.clone();
        self.brush.text = candidate_text;
        // The old executor lets go of everything, then says which uploads had
        // begun to go out; the rest never left (LLP 1016.000 D10).
        self.executor.forget(|_| false);
        let started = self.executor.let_go_started(|_| false);
        self.executor = self.host.executor();
        self.host.runner_mut().uploads_started(&started);
        self.parked.clear();
        self.scroll.clear();
        self.collection = collection::State::default();
        self.contact = None;
        self.retained_motion = None;
        self.transform_geometry = Default::default();
        self.arrange = None;
        self.group = None;
        self.brush.lift = Default::default();
        self.page = (0.0, 0.0);
        self.images.reset();
        self.restore_focus(kept);
        let e = self.after_commit();
        Ok(e)
    }

    /// The host.
    pub fn host(&self) -> &Host<D> {
        &self.host
    }

    /// The host, mutably: the agent answers a held device request there.
    pub fn host_mut(&mut self) -> &mut Host<D> {
        &mut self.host
    }

    /// The text engine.
    pub fn text(&self) -> &Shared {
        &self.text
    }

    /// The currently painted paragraph, with the same fragment geometry as ink.
    pub fn paragraph(&self, id: ViewId) -> Option<&crate::text::Paragraph> {
        let key = self.host.kernel().node(id)?.key;
        self.brush.paragraph(key).map(|p| &**p)
    }
    /// Last CPU flow repaint regions; an empty slice means a full repaint.
    pub fn damage_rects(&self) -> &[crate::paint::Rect4] {
        self.brush.damage_rects()
    }

    /// The images.
    pub fn images(&self) -> &Images {
        &self.images
    }

    /// The viewport, points.
    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    /// The page's scroll offset.
    pub fn page(&self) -> (f32, f32) {
        self.page
    }

    /// The focused input.
    pub fn focus(&self) -> Option<ViewId> {
        self.focus
    }

    /// Paint again at the next frame (a host whose surface came back).
    pub fn repaint(&mut self) {
        self.dirty = true;
    }

    /// Whether the picture is stale.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Where the pointer is drawn (`None` draws none).
    pub fn set_pointer(&mut self, pointer: Option<(f32, f32)>) {
        if self.pointer != pointer {
            self.pointer = pointer;
            self.dirty = true;
        }
    }

    /// The viewport changed.
    pub fn resize_scaled(&mut self, width: f32, height: f32, scale: f32) -> Option<String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Some("display scale must be finite and positive".into());
        }
        self.brush.scale = scale;
        self.dirty = true;
        let told = self.host.set_display_scale(f64::from(scale));
        told.or(self.resize(width, height))
    }

    /// A display needs another animation or GPU canvas frame.
    pub fn wants_display_frames(&self) -> bool {
        self.wants_frames() || self.surfaces.has_rendered_canvas()
    }

    /// The viewport changed.
    pub fn resize(&mut self, width: f32, height: f32) -> Option<String> {
        let error = self.host.resize(width, height);
        if error.is_some() {
            return error;
        }
        let geometry_changed = self.viewport != (width, height);
        self.field_reveal |= geometry_changed;
        self.dirty |= geometry_changed;
        self.viewport = (width, height);
        self.images.fit(self.viewport, self.brush.scale);
        if geometry_changed {
            self.collection.advance_all();
        }
        self.after_commit()
    }

    /// After anything that may have committed: images follow the tree,
    /// offsets stay in range, focus stays on a live input, the picture is
    /// stale.
    pub(crate) fn after_commit(&mut self) -> Option<String> {
        self.dirty |= self.brush.paint_epoch != Some(self.host.kernel().epoch());
        self.finish_commit()
    }

    fn finish_commit(&mut self) -> Option<String> {
        let error = self.service_commit();
        self.size_controls();
        // This turn consumes the reports now. Only refinement left over may
        // wake another turn; waking here makes an idle list redraw forever.
        self.schedule_collections();
        let refined = crate::traced(c"exact refine collections", || self.refine_collections());
        let geometry = self.refresh_transform_geometry();
        self.sync_surface();
        error.or(refined).or(geometry)
    }
    // Collection feedback calls this directly: never recurse through refinement.
    fn sync_commit(&mut self) -> Option<String> {
        self.dirty = true;
        self.service_commit()
    }
    fn service_commit(&mut self) -> Option<String> {
        self.sync_split();
        // A resource refused, a request or a send failed (LLP 1142 D4): on
        // stderr as well as in the journal. The runner queues none unless it
        // measures, so a production binary prints none.
        for line in self.host.runner().take_alerts() {
            eprintln!("exact: {line}");
        }
        self.restore_scroll_anchors();
        self.alert_sync();
        // What the commit asked the host to run goes to the executor (LLP
        // 1016 D2); the reply comes back through `pump`. Its commands wait
        // for the loop (`run_commands`).
        self.hatches.stale = true;
        // A continuation is dispatched here, on this thread, after the
        // commit that handed it out (LLP 1027.002 D3); one a source holds
        // is parked and released after a later commit.
        // An app's aborted fetches first (any method), then what the runner let go.
        self.executor
            .cancel(&self.host.runner_mut().take_explicit_aborts());
        self.executor
            .forget(|ticket| self.host.runner().holds(ticket));
        // Of the uploads it let go, which had begun to go out (LLP 1016.000 D10).
        let started = (self.executor).let_go_started(|ticket| self.host.runner().holds(ticket));
        self.host.runner_mut().uploads_started(&started);
        if !self.host.has_ordered_request_refusals() {
            for (ticket, reason) in self.executor.resume_ordered() {
                self.host.refuse_request(ticket, reason, true);
                self.executor.notify();
            }
        }
        self.cancel_removed_controls();
        self.forget_replaced_choices();
        self.sync_field_values();
        let admitted = self.host.grants();
        for r in self.host.take_requests() {
            // A deadline the surface and auth paths can't keep, or a long poll
            // on work that is no HTTP request (LLP 1016.000 D8): the executor
            // refuses it at admission, fencing later ordered work.
            if r.request
                .timeout_refusal()
                .or(r.request.long_poll_refusal())
                .is_some()
            {
                self.run_dispatch(r, exact_runner::Dispatch::Missing);
                continue;
            }
            if r.request.surface.is_some() {
                self.surfaces.enqueue(r, &admitted);
                continue;
            }
            // No system browser here (LLP 1069.006 D5): 501, delivered by the next pump as an
            // admission answer; under the agent it is held, as on every host (LLP 1146 D4).
            if r.request.is_auth() {
                let (agent, none) = (self.agent, exact_runner::auth::Browser::None);
                exact_runner::auth::arm(self.host.runner_mut(), &r, agent, none, None);
                self.executor.notify();
                continue;
            }
            let dispatch = match r.request.continuation {
                Some(token) => self.host.dispatch_work(token),
                None if r.request.is_native() => self.host.native_work(&r.request),
                // @ref LLP 1103 D1 — a driver fault fails it before transport.
                None => (self.host.runner_mut().fault_dispatch(&r))
                    .unwrap_or(exact_runner::Dispatch::Missing),
            };
            self.run_dispatch(r, dispatch);
        }
        for (token, dispatch) in self.host.release_work() {
            if let Some(r) = self.parked.remove(&token) {
                self.run_dispatch(r, dispatch);
            }
        }
        // An abort made in this batch's turns is answered at the next pump: wake it.
        if self.host.runner_mut().collected_aborts() {
            self.executor.notify();
        }
        // The platform's checks for presence calls: the shell's (LLP 1133.010 D8).
        for ask in self.host.runner_mut().take_presence_asks() {
            match self.has_shell() {
                true => _ = self.request(requests::Request::Presence(ask.json())),
                false => self.host.runner_mut().presence_answer(
                    ask.id,
                    exact_runner::presence::PresenceAnswer::Failed(
                        "NotSupportedError".into(),
                        "this host has no shell to check the person".into(),
                    ),
                ),
            }
        }
        // Socket frames of the commits that stood (LLP 1016.000 D7).
        let executor = &self.executor;
        self.host
            .runner_mut()
            .publish_sockets(|batch| executor.socket(batch));
        let commands = self.host.take_commands();
        if !commands.is_empty() {
            self.commands.extend(commands);
            self.executor.notify();
        }
        let renewed = self.renew();
        let mut error = renewed.or(self.sync_images());
        if !self.booting {
            error = error.or_else(|| {
                self.assets
                    .take_refusal()
                    .map(|reason| format!("selected asset refused: {reason}"))
            });
        }
        // A focusout can synchronously reveal the route this commit hid.
        self.retire_scroll_ends();
        let (focus_sent, focus_error) = self.clear_removed_focus();
        error = error.or(focus_error);
        if focus_sent {
            return error;
        }
        self.sync_authored_scroll();
        self.dirty |= self.clamp_scroll();
        self.retire_pointer();
        self.arrange_settled();
        self.dirty |= error.is_some();
        // The focus a commit leaves: its `autofocus`, then the route's.
        error = error.or(self.autofocus());
        self.arrive();
        error
    }

    fn run_dispatch(&mut self, r: exact_runner::RequestOut, dispatch: exact_runner::Dispatch) {
        let ticket = r.ticket;
        let ordered = r.request.is_ordered();
        let result = match dispatch {
            exact_runner::Dispatch::Run(work) => self.executor.run(r, Some(work)),
            exact_runner::Dispatch::Held => {
                if let Some(token) = r.request.continuation {
                    self.parked.insert(token, r);
                }
                Ok(())
            }
            // A re-ask: settled in its ordered place, no work (LLP 1041 §8.4).
            exact_runner::Dispatch::Again => self.executor.again(&r),
            exact_runner::Dispatch::Host(_) | exact_runner::Dispatch::Missing => {
                self.executor.run(r, None)
            }
        };
        if let Err(reason) = result {
            self.host.refuse_request(ticket, reason, ordered);
            self.executor.notify();
        }
    }

    /// The document's extent: the roots' frames, never smaller than the
    /// viewport (`fitDocument`, LLP 1010 §3).
    fn document(&self) -> (f32, f32) {
        self.display
            .document()
            .unwrap_or_else(|| self.live_document())
    }

    fn live_document(&self) -> (f32, f32) {
        let kernel = self.host.kernel();
        let mut size = self.viewport;
        for root in self.host.roots() {
            if let Some(n) = kernel.node(root) {
                size.0 = size.0.max(n.frame.x + n.frame.width);
                size.1 = size.1.max(n.frame.y + n.frame.height);
            }
        }
        size
    }

    /// Views commits renewed (LLP 1078) start as new ones: a scroller at its
    /// start, a picture only of the source its node names.
    fn renew(&mut self) -> Option<String> {
        let renewed = self.host.take_renewed();
        if renewed.is_empty() {
            return None;
        }
        for id in &renewed {
            self.ime_forget(*id);
            self.field_history.invalidate(*id);
            if self.window_focus.is_some_and(|key| {
                self.host
                    .kernel()
                    .node_by_key(key)
                    .is_some_and(|n| n.id == *id)
            }) {
                self.window_focus = None;
            }
            self.scroll_end.forget(*id);
            self.dirty |= self.scroll.remove(id).is_some();
        }
        self.renew_scroll_anchors(&renewed);
        self.hosted_renewed(&renewed);
        self.hatches.renewed.extend(&renewed);
        let reports = self.images.renew(self.host.kernel(), &renewed);
        if reports.is_empty() {
            return None;
        }
        self.dirty = true;
        self.host.set_intrinsics(reports)
    }

    fn clamp_scroll(&mut self) -> bool {
        self.restore_scroll_anchors();
        let (collection_limits, held) = (self.collection_scroll_limits(), self.hidden_scrollers());
        let kernel = self.host.kernel();
        let region = self.host.content_region();
        let mut gone = Vec::new();
        let mut clamped = Vec::new();
        let mut changed = false;
        for (id, off) in self.scroll.iter_mut().filter(|(id, _)| !held.contains(*id)) {
            match kernel.node(*id) {
                Some(n) => {
                    let next = self
                        .display
                        .bounds(kernel, *id)
                        .unwrap_or_else(|| {
                            self.brush.scroll_bounds(
                                kernel,
                                region,
                                &n,
                                collection_limits.get(id).copied(),
                            )
                        })
                        .clamp(*off);
                    if *off != next {
                        clamped.push((*id, *off, next));
                        changed = true;
                    }
                    *off = next;
                }
                None => gone.push(*id),
            }
        }
        for (id, before, after) in clamped {
            self.rebase_clamped_anchor(id, before, after);
        }
        changed |= !gone.is_empty();
        for id in gone {
            self.scroll.remove(&id);
        }
        let doc = self.document();
        let viewport = self.display.viewport().unwrap_or(self.viewport);
        let page = self.page;
        self.page.0 = self.page.0.clamp(0.0, (doc.0 - viewport.0).max(0.0));
        self.page.1 = self.page.1.clamp(0.0, (doc.1 - viewport.1).max(0.0));
        self.rebase_clamped_anchor(PAGE, page, self.page);
        self.publish_scroll();
        changed || self.page != page
    }

    /// Every node's painted box, in paint order (a fresh frame when stale).
    pub fn boxes(&mut self) -> &[PaintedBox] {
        if self.dirty && !self.display.attached() {
            let _ = self.frame_for(crate::paint::FrameTarget::Geometry);
        }
        &self.boxes
    }

    fn box_of(&mut self, id: ViewId) -> Option<PaintedBox> {
        self.boxes();
        self.boxes
            .iter()
            .find(|b| b.id == id && self.display.allows(self.host.kernel(), id))
            .copied()
            .or_else(|| {
                self.inline_box(id)
                    .filter(|_| self.display.allows(self.host.kernel(), id))
            })
    }

    /// How far scroll container `id`'s offset travels on each axis (LLP 1012 `rx`/`ry`): the bounds its offset is
    /// clamped to, as `clamp_scroll` reads them.
    fn scroll_range(&self, id: ViewId, collection_limits: &BTreeMap<ViewId, f32>) -> (f32, f32) {
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(id) else {
            return (0., 0.);
        };
        let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
            self.brush.scroll_bounds(
                kernel,
                self.host.content_region(),
                &node,
                collection_limits.get(&id).copied(),
            )
        });
        (bounds.max.0.max(0.), bounds.max.1.max(0.))
    }

    /// The agent's `layout`: every node's box in the viewport (scroll
    /// folded in), scroll containers with their offsets, by id. With a
    /// `node`, the runner's explanation of that node (LLP 1035.002 D1) plus
    /// what a painter knows — its painted box and a 1:1 capture; no window,
    /// no screen, nothing mounted, and the reply says so rather than
    /// guessing.
    pub fn layout_json(&mut self, node: Option<u32>, include_plan: bool) -> String {
        let clock = self.host.now();
        let (vw, vh) = self.viewport;
        let mut boxes: Vec<PaintedBox> = self.boxes().to_vec();
        boxes.sort_by_key(|b| b.id);
        let mut s = String::new();
        // LLP 1012 §1: the insets a device's shell told (`window.rs`; zeros
        // on a host with none, which has no safe area or software keyboard);
        // the fold is whatever the agent preferred (LLP 1078 D7).
        let fold = self.host.runner().viewport().fold;
        let [top, right, bottom, left, keyboard] = self.env_insets().map(|v| num(r2(v)));
        let _ = write!(
            s,
            "{{\"clock\":{},\"viewport\":{{\"w\":{},\"h\":{}}},\"env\":{{\"safe-area-inset-top\":{top},\"safe-area-inset-right\":{right},\"safe-area-inset-bottom\":{bottom},\"safe-area-inset-left\":{left},\"keyboard-inset-height\":{keyboard},\"device-posture\":\"{}\",\"horizontal-viewport-segments\":{},\"vertical-viewport-segments\":{},\"viewport-segments\":[",
            num(clock),
            num(r2(vw)),
            num(r2(vh)),
            fold.posture.keyword(),
            fold.cols,
            fold.rows
        );
        for (i, r) in self.segments.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "[{},{},{},{}]",
                num(r2(r.x)),
                num(r2(r.y)),
                num(r2(r.width)),
                num(r2(r.height))
            );
        }
        s.push_str("]},\"nodes\":[");
        let limits = self.collection_scroll_limits();
        for (i, b) in boxes.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "{{\"id\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{}",
                b.id,
                num(r2(b.rect.0)),
                num(r2(b.rect.1)),
                num(r2(b.rect.2)),
                num(r2(b.rect.3))
            );
            if let Some((sx, sy)) = b.scroll {
                let (rx, ry) = self.scroll_range(b.id, &limits);
                let _ = write!(
                    s,
                    ",\"sx\":{},\"sy\":{},\"rx\":{},\"ry\":{}",
                    num(r2(sx)),
                    num(r2(sy)),
                    num(r2(rx)),
                    num(r2(ry))
                );
            }
            s.push('}');
        }
        s.push(']');
        if let Some(id) = node {
            let detail = self.host.agent(&format!(
                "{{\"op\":\"node\",\"id\":{id},\"plan\":{include_plan}}}"
            ));
            if detail.starts_with("{\"error\"") {
                return detail;
            }
            // Reopen the runner's object: exactly its last brace, never the
            // inner object's before it.
            let mut detail = detail;
            if detail.ends_with('}') {
                detail.pop();
            }
            // @ref LLP 1038 D6; LLP 1035.002 D1 — hidden rows remain in
            // the kernel, but have no painted box and refuse input.
            let (route_hidden, inert) = self.host.route_visibility(id);
            let hidden = route_hidden || self.placement_hidden(id);
            let _ = write!(
                detail,
                ",\"visible\":{{\"hidden\":{hidden},\"inert\":{inert}}}"
            );
            match boxes.iter().find(|b| b.id == id) {
                Some(b) => {
                    let _ = write!(
                        detail,
                        ",\"space\":{{\"viewport\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"capture\":{{\"scale\":1}}}}",
                        num(r2(b.rect.0)),
                        num(r2(b.rect.1)),
                        num(r2(b.rect.2)),
                        num(r2(b.rect.3))
                    );
                }
                None if self.placement_hidden(id) => detail.push_str(",\"space\":{\"viewport\":{\"x\":0,\"y\":0,\"w\":0,\"h\":0},\"capture\":{\"scale\":1}}"),
                None => detail.push_str(",\"space\":{\"capture\":{\"scale\":1}}"),
            }
            // @ref LLP 1043.000 §3 D7 — ordinary paragraphs keep the existing
            // reply shape, while a flowed empty paragraph still reports its facts.
            if let Some(p) = self.paragraph(id).filter(|p| p.flow_line_height() > 0.0) {
                detail.push_str(",\"fragments\":[");
                for (i, f) in p.fragments().iter().enumerate() {
                    if i > 0 {
                        detail.push(',');
                    }
                    let _ = write!(detail, "{{\"start\":{},\"end\":{},\"x\":{},\"y\":{},\"width\":{},\"band\":{},\"height\":{}}}", f.start, f.end, num(f.x as f64), num(f.y as f64), num(f.width as f64), f.line, num(p.flow_line_height() as f64));
                }
                detail.push(']');
            }
            detail.push_str(",\"native\":{\"unavailable\":true");
            if let Some(source) = self
                .host
                .runner()
                .kernel()
                .node(id)
                .filter(|n| n.node_type == NodeType::Image)
                .and_then(|n| n.props.str(PropId::ImageSource))
                .filter(|s| s.starts_with("symbol:"))
            {
                let name = &source[7..];
                let reason = if name == "sf/" || name.is_empty() {
                    "empty"
                } else if !name.starts_with("sf/") && exact_kernel::symbol_path(name).is_none() {
                    "role"
                } else {
                    "platform"
                };
                detail.push_str(",\"symbol\":{\"found\":false,\"source\":");
                quote(source, &mut detail);
                detail.push_str(",\"name\":");
                quote(name.strip_prefix("sf/").unwrap_or(name), &mut detail);
                let _ = write!(detail, ",\"reason\":\"{reason}\"}}");
            }
            let _ = write!(detail, "}},\"observed\":{{\"clock\":{}}}}}", num(clock));
            s.push_str(",\"node\":");
            s.push_str(&detail);
        }
        s.push('}');
        s
    }

    pub(crate) fn placement_hidden(&self, id: ViewId) -> bool {
        let mut at = Some(id);
        while let Some(id) = at {
            if matches!(
                self.brush.placements.get(&id),
                Some(crate::placement::Placement::Hidden)
            ) {
                return true;
            }
            at = self.host.kernel().node(id).and_then(|n| n.parent);
        }
        false
    }

    /// The deepest painted box under a point (viewport points), through every clip.
    pub fn hit(&mut self, x: f32, y: f32) -> Option<ViewId> {
        self.hit_with_visibility(x, y, false)
    }

    /// Ordinary mouse releases use current CSS eligibility even before the
    /// next display flip. Retained regions keep their picture's hit policy.
    pub(super) fn hit_after_pointerdown(&mut self, x: f32, y: f32) -> Option<ViewId> {
        self.hit_with_visibility(x, y, true)
    }

    fn hit_with_visibility(&mut self, x: f32, y: f32, current: bool) -> Option<ViewId> {
        // Under an open surface of the top layer the page is inert.
        if !self.display.contains(x, y) || (self.top_open() && self.alert.is_none()) {
            return None;
        }
        self.boxes();
        self.painted_hit(x, y, current)
            .map(|id| self.inline_hit(id, x, y).unwrap_or(id))
    }

    /// The deepest box of the latest paint under a point, as [`Self::hit`]
    /// finds it once the boxes are fresh and the page takes a press.
    pub(super) fn painted_hit(&self, x: f32, y: f32, current: bool) -> Option<ViewId> {
        self.boxes
            .iter()
            .rev()
            .filter(|b| {
                self.alert_allows(b.id)
                    && b.contains(x, y)
                    && crate::paint::hits(self.host.kernel(), b, x, y)
                    && !self.host.route_visibility(b.id).1
                    && self.display.allows(self.host.kernel(), b.id)
            })
            .find_map(|b| {
                let mut hit = *b;
                if current && !self.brush.region_blocks_action(b.id) {
                    let node = self.host.kernel().node(b.id)?;
                    hit.pointer_hit = node
                        .computed_row(exact_kernel::StyleId::PointerEvents, |s| s.pointer_events)
                        != exact_kernel::PointerEvents::None
                        && crate::paint::paints(self.host.kernel(), b.id, None);
                }
                self.svg_hit(&hit, x, y)
            })
    }

    /// The agent's wheel: over the node's center.
    pub fn wheel(&mut self, id: ViewId, dx: f32, dy: f32) -> Result<String, String> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err("wheel needs finite deltas".into());
        }
        if !self.alert_allows(id) || self.host.route_visibility(id).1 {
            return Err(format!("view {id} is hidden or inert"));
        }
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let (x, y) = (b.rect.0 + b.rect.2 / 2.0, b.rect.1 + b.rect.3 / 2.0);
        self.wheel_at(x, y, dx, dy);
        Ok(format!(
            "{{\"tapped\":{id},\"wheel\":[{},{}],\"at\":[{},{}]}}",
            num(dx as f64),
            num(dy as f64),
            num(r2(x)),
            num(r2(y))
        ))
    }

    /// The agent's `tap <list> into <key>` (LLP 1070.000 §5): the runner's
    /// request, committed, then every report it asks for.
    pub fn into_view(
        &mut self,
        id: ViewId,
        key: &str,
        block: &str,
        inline: &str,
    ) -> Result<String, String> {
        self.host.scroll_into_view(id, key, block, inline)?;
        if let Some(e) = self.after_commit() {
            return Err(e);
        }
        if let Some(e) = self.settle_collections() {
            return Err(e);
        }
        Ok(format!("{{\"tapped\":{id},\"into\":true}}"))
    }

    /// The executor's wake: readable when a reply is queued (for `poll`).
    #[cfg(unix)]
    pub fn executor_fd(&self) -> std::os::unix::io::RawFd {
        self.executor.fd()
    }

    /// Metadata, decode completion or changed budget demand wakes an idle display.
    #[cfg(unix)]
    pub fn image_fd(&self) -> std::os::unix::io::RawFd {
        self.images.wake_fd()
    }

    /// Whether a request is in flight.
    pub fn pending(&self) -> bool {
        self.host.runner().has_pending()
    }

    /// Another host took over: it is measured as the last was, and counted.
    pub(crate) fn replaced(&mut self) {
        self.read_ranges();
        self.scroll_end = Default::default();
        self.give_clock();
        // A new kernel may have the same epoch.
        self.brush.paint_epoch = None;
        // A new runner reuses view ids and node keys: an unbound control's
        // own state, a choice or typed text (LLP 1069.001 D4), an edit to
        // commit, a caret, an open menu, a hover or a held pointer is not
        // its tree's, as Apple's reset and the web's rebuilt page have it.
        self.chosen.clear();
        self.controls.clear();
        self.edited = None;
        self.fields.clear();
        self.field_drag = None;
        self.field_history = Default::default();
        self.ime = None;
        self.window_focus = None;
        self.menu = None;
        self.hovered.clear();
        self.pointer_held = None;
        self.pointer_path.clear();
        self.hatch_reset();
        self.top_replaced();
        self.holds_changed();
        self.hosts += 1;
        self.measure();
    }

    /// Measure the work of every binary but a production one (LLP 1079
    /// D1), once its compat is known: the runner boots measuring nothing.
    pub(crate) fn measure(&mut self) {
        let on = !exact_runner::delivery::production(&self.compat);
        self.host.runner_mut().measure(on, true);
        // The live table is development's too (LLP 1121 stage 4).
        self.host.runner_mut().admit_live(Some(&self.compat));
    }

    /// The binary's delivery facts (LLP 1030 D7), from its `compat.json`:
    /// a `delivery` resource is answered again, and the picture follows.
    pub fn set_delivery_from_compat(&mut self, json: &str) -> Option<String> {
        self.compat = json.to_string();
        self.measure();
        let e = self.host.set_delivery_from_compat(json);
        let after = self.after_commit();
        e.or(after)
    }

    /// A motion frame.
    pub fn tick(&mut self, now_ms: f64) {
        let animating = self.host.motion()
            || self.arrange.is_some()
            || self.group.is_some()
            || self.focus.is_some()
            || self.brush.moving();
        if self.host.tick(now_ms) {
            self.hatches.stale = true;
            self.dirty = true;
            self.clamp_scroll();
            self.queue_collections();
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
        self.tick_arrange(self.host.now());
        if let Some(error) = self.deliver_scroll_ends() {
            self.host.log(error);
        }
        self.dirty |= animating;
    }

    /// The box of a node, if painted.
    pub fn rect_of(&mut self, id: ViewId) -> Option<Rect4> {
        self.box_of(id).map(|b| b.rect)
    }
    /// A scroll container's offset.
    pub fn scroll_of(&self, id: ViewId) -> (f32, f32) {
        self.scroll.get(&id).copied().unwrap_or((0.0, 0.0))
    }
    /// How many nodes are live.
    pub fn node_count(&self) -> usize {
        self.host.kernel().live_count()
    }
}

/// Whether `id` is live and reachable from a root (in [`Kernel::rows`]).
fn attached(kernel: &exact_kernel::Kernel, id: ViewId) -> bool {
    let mut at = kernel.node(id);
    while let Some(node) = at {
        if node.is_root {
            return true;
        }
        at = node.parent.and_then(|p| kernel.node(p));
    }
    false
}
