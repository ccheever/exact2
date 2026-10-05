//! The presenter: what a painter holds beyond the kernel — scroll offsets,
//! images, focus, the pointer — and the operations that touch it: frames,
//! hit-testing, presses, wheels, typing, the clock, screenshots.
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

mod arrange;
mod arrange_geometry;
mod clock;
mod collection;
mod commands;
mod contact;
mod control;
mod delivery;
mod display_frame;
mod events;
mod field;
#[cfg(test)]
mod field_tests;
mod group;
mod painter;
mod pan_release;
mod picker;
mod pointer;
mod radio;
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
#[cfg(target_os = "android")]
mod feed;
mod height;
mod height_drag;
#[cfg(test)]
mod height_drag_tests;
mod images;
mod links;
mod preferences;
mod retained_action;
mod reveal;
mod shortcuts;
#[cfg(target_os = "android")]
pub(crate) mod still;
mod swipe;
mod transform;
mod transform_geometry;
mod typing;

#[cfg(test)]
#[path = "presenter/collection_tests.rs"]
mod collection_tests;

#[cfg(test)]
#[path = "presenter/swipe_tests.rs"]
mod swipe_tests;

#[cfg(test)]
#[path = "presenter/events_tests.rs"]
mod events_tests;

use painter::{cpu_info, open_backend};
pub use painter::{set_custom_painter, PainterChoice, PainterFactory, PainterInfo};

/// The presenter: one host, its painter, and the host state.
pub struct Presenter<D: DataSource> {
    pub(crate) host: Host<D>,
    /// The scroller the last wheel moved.
    last_wheel: Option<ViewId>,
    pub(crate) surfaces: crate::surfaces::Surfaces,
    module: Option<crate::delivery::Module>,
    painted: bool,
    activation_failed: bool,
    dev: Option<crate::fetch::Poller>,
    pending_dev: Option<crate::fetch::Generation>,
    pending_update: bool,
    text: Shared,
    pub(crate) brush: Painter,
    viewport: (f32, f32),
    scroll: BTreeMap<ViewId, (f32, f32)>,
    page: (f32, f32),
    images: Images,
    pub(crate) assets: Assets,
    /// The binary's `compat.json` (LLP 1030 D3a), once handed over: a
    /// reload boots a fresh runner, which is told again.
    pub(crate) compat: String,
    /// How many hosts this presenter has had: a frame sampler starts over
    /// when another replaces the one it measured (LLP 1079 D3).
    pub(crate) hosts: u64,
    pub(crate) focus: Option<ViewId>,
    /// The text field typed into since it took the focus: its `change`
    /// fires on blur or Enter, HTML's commit (LLP 1069.001 D4).
    pub(crate) edited: Option<ViewId>,
    /// Each text field's selection (x2apps codeedit #2), kept for the value
    /// it indexes, focused or not: the next key there edits at it.
    pub(crate) fields: BTreeMap<ViewId, field::FieldMark>,
    /// The modifier keys held, each side a bit (Shift, Control, Alt, Meta,
    /// left then right): a `key` event's flags (`KeyboardEvent.shiftKey`…).
    pub(crate) held: u8,
    /// The keys whose down a shortcut took at a world's canvas: their up
    /// belongs to no handler and no world (b6 review B1).
    pub(crate) shortcut_keys: std::collections::BTreeSet<String>,
    /// Unbound checkboxes' own states, as a browser keeps an uncontrolled
    /// control's (LLP 1069.001 D4); a bound one draws its `checked`.
    pub(crate) controls: BTreeMap<ViewId, bool>,
    /// A date's, range's or select's choice since its bound value last
    /// changed, beside that bound value (`paint::control::choice`).
    pub(crate) chosen: BTreeMap<ViewId, (String, String)>,
    /// The select whose menu is open (LLP 1069.001 D7).
    pub(crate) menu: Option<ViewId>,
    autofocus_processed: std::collections::BTreeSet<ViewId>,
    pointer: Option<(f32, f32)>,
    /// The nodes with a `hover` handler under the pointer, innermost first.
    hovered: Vec<ViewId>,
    /// The node holding the pointer's `pointerdown` until it lifts.
    pointer_held: Option<exact_kernel::NodeKey>,
    /// The held pointer's buttons as DOM counts them: 1 primary, 2
    /// secondary, 4 middle (review b5-b 1).
    pointer_buttons: u8,
    pub(crate) control_bindings: BTreeMap<(u32, u32), crate::surfaces::ControlBinding>,
    pub(crate) control_contact: Option<(ViewId, f32, f32)>,
    boxes: Vec<PaintedBox>,
    pub(crate) dirty: bool,
    /// The app's `setScheme` (`None`: follow the system) and the system's
    /// appearance, which only an agent sets here (LLP 1061 D5).
    pub(crate) scheme: (Option<bool>, bool),
    /// The viewport segments the agent set (LLP 1078 D7), for `layout.env`;
    /// the posture and counts are the runner's.
    pub(crate) segments: Vec<exact_kernel::Rect>,
    /// A failed painter's blank fallback cannot bless an update generation.
    last_frame_succeeded: bool,
    pub(crate) display: display_frame::State,
    /// Which painter was asked for (`Auto` may change its mind after a
    /// failed frame).
    choice: PainterChoice,
    /// How long the font scan took at boot, milliseconds (the one cost that
    /// is the machine's, not the app's).
    pub fonts_ms: f64,
    /// The painter, for the report.
    pub painter: PainterInfo,
    /// The executor for a request that leaves the process (LLP 1016 D2).
    executor: crate::executor::Executor,
    /// Requests whose continuation a source held at dispatch (LLP 1027.002
    /// D3): released after a later commit, by token.
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
    /// The commands the last commits' actions asked for, for the loop that
    /// runs them (`run_commands`).
    commands: Vec<exact_runner::Command>,
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
}
/// Two decimals, the agent API's precision.
fn r2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}
impl<D: DataSource> Presenter<D> {
    /// Boot the app under a viewport (points) at a device scale, with its
    /// asset root. The boot error, if any, is reported beside the presenter
    /// (the tree is what booted).
    pub fn boot(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        Presenter::boot_with(
            plan,
            data,
            viewport,
            scale,
            assets,
            PainterChoice::from_env(),
        )
    }

    /// Boot with a chosen painter (`boot` reads `EXACT_PAINTER`).
    pub fn boot_with(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
        choice: PainterChoice,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        let (mut presenter, error) = Self::boot_with_assets(
            PlanBytes::Copied(plan),
            data,
            viewport,
            scale,
            Assets::embedded(assets),
            choice,
            None,
            "/",
            None,
        )?;
        presenter.measure();
        Ok((presenter, error))
    }

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
            PainterChoice::from_env(),
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
        choice: PainterChoice,
        delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
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
        let text = TextEngine::shared_for_assets(&decoded, &assets);
        if let Some(reason) = assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        let fonts_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (backend, painter) = open_backend(choice).map_err(HostError::Painter)?;
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
        )?;
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
            viewport,
            scroll: BTreeMap::new(),
            page: (0.0, 0.0),
            images,
            assets,
            compat: String::new(),
            hosts: 0,
            focus: None,
            edited: None,
            fields: BTreeMap::new(),
            held: 0,
            shortcut_keys: Default::default(),
            controls: BTreeMap::new(),
            chosen: BTreeMap::new(),
            menu: None,
            autofocus_processed: Default::default(),
            pointer: None,
            hovered: Vec::new(),
            pointer_held: None,
            pointer_buttons: 0,
            control_contact: None,
            control_bindings: BTreeMap::new(),
            boxes: Vec::new(),
            dirty: true,
            scheme: (None, false),
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
            agent: false,
            booting: true,
            content_registration: region,
            last_region_frame: None,
            last_region_scale: None,
        };
        p.set_system_scheme(false);
        let e = p.after_commit();
        p.booting = false;
        if let Some(reason) = p.assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        Ok((p, error.or(e)))
    }

    /// The dev loop's restart: boot the new plan with state carried; every
    /// picture and offset goes, and focus stays at its place (`restore_focus`)
    /// (LLP 1007 §6).
    pub fn reload(&mut self, plan: &[u8], data: D) -> Result<Option<String>, HostError> {
        self.reload_module(plan, data, self.module.clone())
    }

    /// Replace a local development pair transactionally, carrying compatible state.
    pub fn reload_module(
        &mut self,
        plan: &[u8],
        data: D,
        module: Option<crate::delivery::Module>,
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
        )?;
        let (mut host, error) = Host::boot_with(
            plan,
            data,
            Box::new(Measurer(candidate_text.clone())),
            self.viewport.0,
            self.viewport.1,
            Some(&carried),
            Some(self.host.runner().delivery().clone()),
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
        self.executor = self.host.executor();
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

    /// A restart with carried state (a dev reload, a delivered update)
    /// replaces the tree. Focus follows the focused node's place in the
    /// runner's tree — its index among its siblings at each level, and its
    /// type — and the restarted tree autofocuses nothing; a node mounted
    /// later still may.
    fn focus_place(&self) -> Option<(Vec<usize>, NodeType)> {
        let (kernel, roots) = (self.host.kernel(), self.host.runner().roots());
        let node_type = kernel.node(self.focus?)?.node_type;
        let (mut path, mut at) = (Vec::new(), self.focus?);
        loop {
            let parent = kernel.node(at)?.parent;
            let siblings = parent.map_or(Some(roots.clone()), |p| {
                kernel.node(p).map(|n| n.children())
            })?;
            path.push(siblings.iter().position(|&id| id == at)?);
            let Some(parent) = parent else { break };
            at = parent;
        }
        path.reverse();
        Some((path, node_type))
    }
    fn restore_focus(&mut self, kept: Option<(Vec<usize>, NodeType)>) {
        let kernel = self.host.kernel();
        let rows = kernel.rows(None).unwrap_or_default();
        self.autofocus_processed = rows.iter().map(|r| r.id).collect();
        let target = kept.and_then(|(path, node_type)| {
            let (mut ids, mut id) = (self.host.runner().roots(), None);
            for index in path {
                let at = *ids.get(index)?;
                ids = kernel.node(at)?.children();
                id = Some(at);
            }
            id.filter(|&id| kernel.node(id).is_some_and(|n| n.node_type == node_type))
        });
        self.focus = target.filter(|&id| self.focusable(id) && !self.host.route_visibility(id).1);
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

    /// Whether the picture is stale.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// The last frame's (paint, readback) milliseconds; CPU readback is zero.
    pub fn last_frame_ms(&self) -> Option<(f64, f64)> {
        self.brush.last_frame_ms()
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
        self.resize(width, height)
    }

    /// A display needs another animation or GPU canvas frame.
    pub fn wants_display_frames(&self) -> bool {
        self.host.wants_frames() || self.surfaces.has_rendered_canvas()
    }

    /// The viewport changed.
    pub fn resize(&mut self, width: f32, height: f32) -> Option<String> {
        let error = self.host.resize(width, height);
        if error.is_some() {
            return error;
        }
        let geometry_changed = self.viewport != (width, height);
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
        self.dirty = true;
        self.finish_commit()
    }

    fn finish_commit(&mut self) -> Option<String> {
        let error = self.service_commit();
        self.size_controls();
        self.queue_collections();
        let refined = crate::traced(c"exact refine collections", || self.refine_collections());
        let geometry = self.refresh_transform_geometry();
        let resized = self.deliver_resizes();
        error.or(refined).or(geometry).or(resized)
    }

    // Collection feedback calls this directly: never recurse through refinement.
    fn sync_commit(&mut self) -> Option<String> {
        self.dirty = true;
        self.service_commit()
    }

    fn service_commit(&mut self) -> Option<String> {
        // What the commit asked the host to run goes to the executor (LLP
        // 1016 D2); the reply comes back through `pump`. Its commands wait
        // for the loop (`run_commands`).
        // A continuation is dispatched here, on this thread, after the
        // commit that handed it out (LLP 1027.002 D3); one a source holds
        // is parked and released after a later commit.
        self.executor
            .forget(|ticket| self.host.runner().holds(ticket));
        if !self.host.has_ordered_request_refusals() {
            self.executor.resume_ordered();
        }
        self.cancel_removed_controls();
        let admitted = self.host.grants();
        for r in self.host.take_requests() {
            if r.request.surface.is_some() {
                self.surfaces.enqueue(r, &admitted);
                continue;
            }
            // No system browser here (LLP 1069.006 D5): 501, delivered by
            // the next pump as an admission answer.
            if r.request.is_auth() {
                let runner = self.host.runner_mut();
                exact_runner::auth::arm(runner, &r, false, exact_runner::auth::Browser::None, None);
                self.executor.notify();
                continue;
            }
            let dispatch = match r.request.continuation {
                Some(token) => self.host.dispatch_work(token),
                None if r.request.is_native() => self.host.native_work(&r.request),
                None => {
                    self.run_dispatch(r, exact_runner::Dispatch::Missing);
                    continue;
                }
            };
            self.run_dispatch(r, dispatch);
        }
        for (token, dispatch) in self.host.release_work() {
            if let Some(r) = self.parked.remove(&token) {
                self.run_dispatch(r, dispatch);
            }
        }
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
        if let Some(f) = self.focus {
            if self.host.kernel().node(f).is_none() || self.host.route_visibility(f).1 {
                self.focus = None;
                self.dirty = true;
            }
        }
        self.sync_authored_scroll();
        self.dirty |= self.clamp_scroll();
        self.retire_pointer();
        self.arrange_settled();
        self.dirty |= error.is_some();
        // Only autofocus nodes matter: a walk that keeps those, in
        // preorder, not an export row per node of every mounted row.
        let kernel = self.host.kernel();
        if !self.autofocus_processed.is_empty() {
            self.autofocus_processed.retain(|id| attached(kernel, *id));
        }
        let candidates = if kernel.has_prop(PropId::Autofocus) {
            kernel.preorder_where(&kernel.roots(), |_, props| {
                props.bool(PropId::Autofocus) == Some(true)
            })
        } else {
            Vec::new()
        };
        {
            for id in candidates {
                if self.autofocus_processed.contains(&id)
                    || !self.focusable(id)
                    || self.host.route_visibility(id).1
                {
                    continue;
                }
                if !self
                    .box_of(id)
                    .is_some_and(|b| b.rect.2 > 0.0 && b.rect.3 > 0.0)
                {
                    continue;
                }
                self.autofocus_processed.insert(id);
                if self.focus.is_none() {
                    self.focus = Some(id);
                    self.dirty = true;
                }
                break;
            }
        }
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
            self.dirty |= self.scroll.remove(id).is_some();
        }
        let reports = self.images.renew(self.host.kernel(), &renewed);
        if reports.is_empty() {
            return None;
        }
        self.dirty = true;
        self.host.set_intrinsics(reports)
    }

    fn clamp_scroll(&mut self) -> bool {
        let collection_limits = self.collection_scroll_limits();
        let kernel = self.host.kernel();
        let region = self.host.content_region();
        let mut gone = Vec::new();
        let mut changed = false;
        for (id, off) in self.scroll.iter_mut() {
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
                    changed |= *off != next;
                    *off = next;
                }
                None => gone.push(*id),
            }
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
        self.publish_scroll();
        changed || self.page != page
    }

    /// Tell the runner where every scroller and the page stand, for
    /// `frame()` (LLP 1051.000 D1): after a clamp, and wherever a scroll
    /// moves geometry (`refresh_transform_geometry`).
    pub(crate) fn publish_scroll(&mut self) {
        let runner = self.host.runner_mut();
        runner.scrolled(None, f64::from(self.page.0), f64::from(self.page.1));
        for (id, (left, top)) in &self.scroll {
            runner.scrolled(Some(*id), f64::from(*left), f64::from(*top));
        }
    }

    /// Every node's painted box, in paint order (a fresh frame when stale).
    pub fn boxes(&mut self) -> &[PaintedBox] {
        if self.dirty && !self.display.attached() {
            let _ = self.frame();
        }
        &self.boxes
    }

    fn box_of(&mut self, id: ViewId) -> Option<PaintedBox> {
        self.boxes();
        self.boxes
            .iter()
            .find(|b| b.id == id && self.display.allows(self.host.kernel(), id))
            .copied()
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
        // LLP 1012 §1: this host has no safe area or software keyboard; its
        // fold is whatever the agent preferred (LLP 1078 D7).
        let fold = self.host.runner().viewport().fold;
        let _ = write!(
            s,
            "{{\"clock\":{},\"viewport\":{{\"w\":{},\"h\":{}}},\"env\":{{\"safe-area-inset-top\":0,\"safe-area-inset-right\":0,\"safe-area-inset-bottom\":0,\"safe-area-inset-left\":0,\"keyboard-inset-height\":0,\"device-posture\":\"{}\",\"horizontal-viewport-segments\":{},\"vertical-viewport-segments\":{},\"viewport-segments\":[",
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
                let _ = write!(s, ",\"sx\":{},\"sy\":{}", num(r2(sx)), num(r2(sy)));
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
                let reason = if name == "sf/" {
                    "empty"
                } else if !name.starts_with("sf/") && exact_kernel::symbol(name).is_none() {
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

    /// The deepest painted box under a point (viewport points), through
    /// every clip.
    pub fn hit(&mut self, x: f32, y: f32) -> Option<ViewId> {
        if !self.display.contains(x, y) {
            return None;
        }
        self.boxes();
        self.boxes
            .iter()
            .rev()
            .filter(|b| {
                b.contains(x, y)
                    && crate::paint::hits(self.host.kernel(), b, x, y)
                    && !self.host.route_visibility(b.id).1
                    && self.display.allows(self.host.kernel(), b.id)
            })
            .find_map(|b| self.svg_hit(b, x, y))
    }

    /// The agent's `tap`: a press at the node's center through the same
    /// path a pointer takes.
    pub fn tap(&mut self, id: ViewId) -> Result<String, String> {
        self.boxes();
        if self.host.route_visibility(id).1 || self.placement_hidden(id) {
            return Err(format!("view {id} is hidden or inert"));
        }
        if crate::navigation::popover_invoker(self.host.kernel(), id) {
            return Err(crate::navigation::POPOVER_UNSUPPORTED.into());
        }
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let (x, y) = crate::paint::tap_point(self.host.kernel(), &b).unwrap_or_else(|| b.center());
        let mut hit = self.hit(x, y);
        while hit.is_some() && hit != Some(id) {
            hit = hit.and_then(|n| self.host.kernel().node(n).and_then(|n| n.parent));
        }
        if hit != Some(id) {
            return Err(format!(
                "view {id} is covered or not hit at its projected center"
            ));
        }
        let now = self.host.now();
        let actual = self.hit(x, y).and_then(|hit| {
            self.control_target(hit)
                .or_else(|| self.handler_target(hit, EventKind::Press))
        });
        if let Some(actual) = actual.filter(|actual| {
            *actual != id
                && !self.drawn_in(*actual, id)
                && self
                    .control_target(id)
                    .or_else(|| self.handler_target(id, EventKind::Press))
                    != Some(*actual)
        }) {
            return Err(format!(
                "view {id} activates view {actual} at its projected center"
            ));
        }
        let activated = self.press_at(x, y, now);
        if actual.is_some() && activated.is_none() {
            return Err(format!("view {id} did not accept activation"));
        }
        let id = activated.unwrap_or(id);
        Ok(format!(
            "{{\"tapped\":{id},\"at\":[{},{}]}}",
            num(r2(x)),
            num(r2(y))
        ))
    }

    /// A wheel at a point (the web's sign: a positive `dy` scrolls down).
    /// A phase-less tick is its own gesture and is not split (LLP 1070 G2,
    /// Chrome's measured tick): the innermost scroll container under the point
    /// that can take ANY of its components takes those it can, clamped, and
    /// the rest is dropped. One that can take none chains to the next scroller
    /// up, then the page, unless its `overscroll-behavior` is `contain` or
    /// `none` on an axis the tick moves along: it keeps (drops) the tick.
    pub fn wheel_at(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        if ![x, y, dx, dy].iter().all(|v| v.is_finite())
            || (dx == 0.0 && dy == 0.0)
            || !self.display.contains(x, y)
        {
            return;
        }
        // A canvas that wants input takes the wheel, and the nodes' own
        // `wheel` is still heard (review b5-b 2), as the web's element hears it.
        let canvas = self.surface_wheel(x, y, dx, dy, self.pointer_now());
        if self.wheel_event(x, y, dx, dy) || canvas {
            return;
        }
        // A UI wheel can take over a UI gesture, but does not release a game's
        // captured pointer. Browser wheel events do not end pointer capture.
        if self.contact_canvas().is_none() {
            if let Err(error) = self.pointer_cancel(self.pointer_now()) {
                self.host.log(error);
            }
        }
        let mut at = self.hit(x, y);
        // Only a node the display has no bounds for needs the collections'.
        let mut collection_limits = None;
        let kernel = self.host.kernel();
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else { break };
            let bounds = self.display.bounds(kernel, id).unwrap_or_else(|| {
                let limits =
                    collection_limits.get_or_insert_with(|| self.collection_scroll_limits());
                self.brush.scroll_bounds(
                    kernel,
                    self.host.content_region(),
                    &node,
                    limits.get(&id).copied(),
                )
            });
            let (ox, oy) = bounds.axes;
            if matches!(ox, Overflow::Scroll | Overflow::Auto)
                || matches!(oy, Overflow::Scroll | Overflow::Auto)
            {
                let max = bounds.max;
                let off = self.scroll.get(&id).copied().unwrap_or((0.0, 0.0));
                let takes = |scrolls: bool, d: f32, off: f32, max: f32| {
                    scrolls
                        && d != 0.0
                        && max > 0.0
                        && ((d > 0.0 && off < max) || (d < 0.0 && off > 0.0))
                };
                let take_x = takes(
                    matches!(ox, Overflow::Scroll | Overflow::Auto),
                    dx,
                    off.0,
                    max.0,
                );
                let take_y = takes(
                    matches!(oy, Overflow::Scroll | Overflow::Auto),
                    dy,
                    off.1,
                    max.1,
                );
                if take_x || take_y {
                    let nx = if take_x {
                        (off.0 + dx).clamp(0.0, max.0)
                    } else {
                        off.0
                    };
                    let ny = if take_y {
                        (off.1 + dy).clamp(0.0, max.1)
                    } else {
                        off.1
                    };
                    self.scroll.insert(id, (nx, ny));
                    self.last_wheel = Some(id);
                    self.dirty = true;
                    self.collection_scrolled(id);
                    if let Some(error) = self.refresh_transform_geometry() {
                        self.host.log(error);
                    }
                    return;
                }
                // At its edge (or with no travel) along every component the
                // tick has: a contained axis ends the chain here.
                let contained = |b: exact_kernel::OverscrollBehavior| {
                    b != exact_kernel::OverscrollBehavior::Auto
                };
                if (dx != 0.0 && contained(node.style.overscroll_behavior_x))
                    || (dy != 0.0 && contained(node.style.overscroll_behavior_y))
                {
                    return;
                }
            }
            // An SVG element (what a hit on a shape names, LLP 1055.000 D8)
            // has no display box: it scrolls with its `svg`'s ancestors.
            at = self.display.parent(kernel, id).or_else(|| {
                node.node_type
                    .is_svg_element()
                    .then_some(node.parent)
                    .flatten()
            });
        }
        let doc = self.document();
        let viewport = self.display.viewport().unwrap_or(self.viewport);
        let max = ((doc.0 - viewport.0).max(0.0), (doc.1 - viewport.1).max(0.0));
        let next = (
            (self.page.0 + dx).clamp(0.0, max.0),
            (self.page.1 + dy).clamp(0.0, max.1),
        );
        if next != self.page {
            self.page = next;
            self.dirty = true;
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
    }

    /// The agent's wheel: over the node's center.
    pub fn wheel(&mut self, id: ViewId, dx: f32, dy: f32) -> Result<String, String> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err("wheel needs finite deltas".into());
        }
        if self.host.route_visibility(id).1 {
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

    /// The executor's replies into the runner (LLP 1016 D2), each a
    /// commit: the display loop calls this when the executor's fd is
    /// readable, the agent when it waits. `None` when nothing was queued.
    pub fn pump(&mut self, now_ms: f64) -> Option<String> {
        let region_error = self.poll_content_region();
        self.executor.begin_pump();
        let region_error = region_error.or(self.dispatch_authored_scroll());
        self.refusal_turn = !self.refusal_turn;
        let mut outcomes = if self.refusal_turn {
            self.host
                .take_request_refusal(self.executor.ordered_idle())
                .into_iter()
                .map(|(ticket, outcome)| (ticket, outcome, None))
                .collect()
        } else {
            self.executor.drain()
        };
        if outcomes.is_empty() {
            outcomes = if self.refusal_turn {
                self.executor.drain()
            } else {
                self.host
                    .take_request_refusal(self.executor.ordered_idle())
                    .into_iter()
                    .map(|(ticket, outcome)| (ticket, outcome, None))
                    .collect()
            };
        }
        if self.host.has_request_refusals(self.executor.ordered_idle()) {
            self.executor.notify();
        }
        if outcomes.is_empty() && !self.host.has_announced() {
            let refined = self.refine_collections();
            return region_error
                .or(refined)
                .or(self.refresh_transform_geometry());
        }
        let e = self.host.fulfill_all(outcomes, now_ms);
        let after = self.after_commit();
        region_error.or(e).or(after)
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
        self.brush.paint_epoch = None; // A new kernel may have the same epoch.
        self.hosts += 1;
        self.measure();
    }

    /// Measure the work of every binary but a production one (LLP 1079
    /// D1), once its compat is known: the runner boots measuring nothing.
    pub(crate) fn measure(&mut self) {
        let on = !exact_runner::delivery::production(&self.compat);
        self.host.runner_mut().measure(on, true);
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

    /// The runner's clock (timers), from the presenter's loop.
    pub fn advance(&mut self, now_ms: f64) -> Option<String> {
        let (e, paint) = self.host.advance_effects(now_ms);
        self.dirty |= paint;
        let after = self.finish_commit();
        e.or(after)
    }

    /// A display frame for the frame tasks (LLP 1073 D5): timers due by
    /// `now_ms`, then every frame task once at it.
    pub fn animation_frame(&mut self, now_ms: f64) -> Option<String> {
        let (e, paint) = self.host.frame(now_ms);
        self.dirty |= paint;
        let after = self.finish_commit();
        e.or(after)
    }

    /// A motion frame.
    pub fn tick(&mut self, now_ms: f64) {
        if self.host.tick(now_ms) {
            self.clamp_scroll();
            self.queue_collections();
            if let Some(error) = self.refresh_transform_geometry() {
                self.host.log(error);
            }
        }
        self.tick_arrange(self.host.now());
        self.dirty = true;
    }

    /// The pixels, as a PNG at `path`.
    pub fn screenshot(&mut self, path: &str) -> Result<String, String> {
        let frame = self.frame();
        let png = frame.encode_png().map_err(|e| format!("png: {e}"))?;
        std::fs::write(path, png).map_err(|e| format!("write {path}: {e}"))?;
        let mut s = String::from("{\"screenshot\":");
        quote(path, &mut s);
        let _ = write!(
            s,
            ",\"w\":{},\"h\":{}}}",
            num(r2(self.viewport.0)),
            num(r2(self.viewport.1))
        );
        Ok(s)
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
