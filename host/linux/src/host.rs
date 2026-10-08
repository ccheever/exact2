//! The runner wrapped for a painter: commits → layout → motion, with the
//! kernel read directly — no batch, no mirror.
//!
//! @ref LLP 1015 §1; LLP 1008 §1 (the same orchestration, whose batch
//! exists because a foreign view tree consumes it — here nothing does)
//!
//! After every commit the host lays the roots out with the kernel's layout
//! under the viewport, feeds the motion engine the commit (LLP 1003 §4),
//! seeks it to the app's clock, and keeps each node's presentation values.
//! The painter then reads frames, styles, and props from the kernel and the
//! presentation values from here. The kernel is the single source of truth
//! and the only copy.

use crate::paint::Presented;
use exact_kernel::motion::{layout_presented, motion_node, node_key, MotionSync, PaintMotion};
use exact_kernel::{Kernel, NodeKey, TextMeasurer, ViewId};
use exact_motion::{Engine, Property};
use exact_plan::Plan;
use exact_runner::{Carried, DataSource, Event, Outcome, RequestOut, Runner, RunnerError, Timed};
use std::collections::BTreeMap;

#[path = "activation.rs"]
mod activation;
#[path = "arrange.rs"]
mod arrange;
#[path = "content_region/host.rs"]
mod content;
#[path = "hatches/host.rs"]
mod hatch;
#[path = "height.rs"]
mod height;
#[path = "height_binding.rs"]
mod height_binding;
#[path = "holds.rs"]
mod holds;
#[path = "lower.rs"]
pub mod lower;
#[path = "paint_motion.rs"]
mod paint_motion;
#[path = "presence.rs"]
mod presence;
#[path = "press.rs"]
mod press;
#[path = "system_support.rs"]
mod system;
#[path = "transform_binding.rs"]
mod transform_binding;
#[path = "value_watch.rs"]
mod value_watch;

use system::{persist_store_writes, physical_memory, store_snapshot};

/// Why the host refused to boot.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    Painter(String),
    Layout(String),
    Asset(String),
    PreparingModule,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Plan bytes to boot: a copy of the caller's, or bytes that live as long as
/// the program (a plan linked into it), whose data pool the decoded plan
/// then keeps in place.
#[derive(Clone, Copy)]
pub(crate) enum PlanBytes<'a> {
    Copied(&'a [u8]),
    Static(&'static [u8]),
}

impl PlanBytes<'_> {
    pub(crate) fn decode(self) -> Result<Plan, exact_plan::PlanError> {
        match self {
            PlanBytes::Copied(bytes) => Plan::decode(bytes),
            PlanBytes::Static(bytes) => Plan::decode_static(bytes),
        }
    }
}

/// One runner, one painter.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    engine: Engine,
    keys: BTreeMap<NodeKey, ViewId>,
    presented: BTreeMap<ViewId, Presented>,
    /// Paint motion's owners and the appearance they resolve by (LLP 1062).
    paint: PaintMotion,
    viewport: (f32, f32),
    now_ms: f64,
    height_owner: Option<NodeKey>,
    pub(crate) flow_damage: crate::paint::damage::Changes,
    /// What changed for the painter's kept rows since its last frame.
    pub(crate) row_dirty: crate::paint::rows::Dirty,
    height_bindings: height_binding::Bindings,
    transform_bindings: transform_binding::Bindings,
    height_projection: Option<exact_kernel::PresentedHeight>,
    height_layout_valid: bool,
    content_region: Option<crate::content_region::ContentRegionState>,
    #[cfg(test)]
    layout_calls: usize,
    data_activated: bool,
    /// The reader plays [`lower::LOWERED`] animations (the Canvas host);
    /// those nodes and what it plays, and a count of changes to them.
    lowering: bool,
    lowered: std::collections::HashMap<u64, u8>,
    /// Whether colour transitions are lowered too (`lower.rs`).
    lower_colors: bool,
    lowered_epoch: u64,
    /// The epoch each lowered node's plays last changed at.
    lowered_changed: std::collections::HashMap<u64, u64>,
    played: std::collections::HashMap<
        u64,
        Vec<(exact_motion::Property, exact_motion::PlayedTransition)>,
    >,
    /// The executor's wake, which a pending activation leaves with the data
    /// source, so the display loop doesn't poll.
    preload_wake: exact_runner::PreloadWake,
    router_op: Option<exact_runner::RouterChange>,
    navigation: crate::navigation::Navigation,
    presence: presence::Presence,
    presses: BTreeMap<NodeKey, press::Feedback>,
    /// The 2D canvases' bitmaps (LLP 1056 D7).
    canvas2d: crate::canvas2d::Canvases,
    /// Views commits renewed (LLP 1078) the presenter has yet to reset.
    renewed: Vec<ViewId>,
    /// The commit each media session claimant mounted in (LLP 1098 D9).
    media_mounts: crate::media_session::Mounts,
    /// The `value`s the presenter keeps typed text against (LLP 1069.001 D4).
    pub(crate) values: value_watch::ValueWatch,
    /// The plan marks nodes `hatch` (`presenter/hatches.rs` says what follows).
    pub(crate) hatched: bool,
}

impl<D: DataSource> Host<D> {
    /// Boot from plan bytes with a text measurer under a viewport (points):
    /// decode, boot the runner, lay out, hear the whole tree in the engine.
    pub fn boot(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        Host::boot_with(plan_bytes, data, measurer, width, height, None, None)
    }

    /// Boot carrying an earlier host's state (the dev reload, LLP 1007 §6).
    pub fn boot_with(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        delivery: Option<exact_runner::Delivery>,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        Self::boot_at(
            plan_bytes, data, measurer, width, height, carried, delivery, "/",
        )
    }

    /// Boot with the native launch location. @ref LLP 1038 D5/D8
    #[allow(clippy::too_many_arguments)]
    pub fn boot_at(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        delivery: Option<exact_runner::Delivery>,
        launch: &str,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        Self::boot_at_with_region(
            plan_bytes, data, measurer, width, height, carried, delivery, launch, None,
        )
    }

    /// Boot one explicitly registered native content region before any layout.
    /// Opt-out is exactly the ordinary `boot_at` path.
    #[allow(clippy::too_many_arguments)]
    pub fn boot_at_with_region(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        let plan = Plan::decode(plan_bytes).map_err(HostError::Plan)?;
        Self::boot_decoded(
            plan, data, measurer, width, height, carried, delivery, launch, region,
        )
    }

    /// [`Host::boot_at_with_region`] of a plan already decoded (the
    /// presenter decodes once, for its fonts and then for this).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn boot_decoded(
        plan: Plan,
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        // @ref LLP 1075.003.000 §3.3 — whether the plan marks nodes for a hatch.
        let hatched = plan.bindings.iter().any(|b| {
            b.kind == exact_plan::BindingKind::Prop
                && exact_kernel::PropId::from_wire(b.id) == Some(exact_kernel::PropId::Hatch)
        });
        // @ref LLP 1100 D10 — this host draws sRGB only.
        exact_kernel::style::wide::set_available(exact_color::Wide::in_srgb);
        // Native hosts link every row's grammar (LLP 1053.000 §2).
        exact_kernel::style::link_backdrop_filter();
        exact_kernel::style::link_segments();
        exact_kernel::style::link_wide_colors();
        exact_kernel::timeline::link();
        let mut kernel = Kernel::new(measurer);
        let env = exact_kernel::Env {
            control_text_styles: Some(crate::paint::control::control_text_styles()),
            ..kernel.env().clone()
        };
        kernel
            .set_env(env)
            .map_err(|e| HostError::Layout(format!("{e:?}")))?;
        // An `app:/data` image shows from the first frame, before storage
        // is configured and whether or not anything was picked (D7).
        crate::picker::know_roots(data.app_id());
        // A named drive's secrets, read before the runner takes the source.
        // A carried reload keeps its memory store and does not touch the files.
        let store_snapshot = store_snapshot(data.app_id(), carried.is_some());
        let mut runner = Runner::boot_with_delivery(
            plan,
            data,
            kernel,
            carried,
            store_snapshot,
            delivery.unwrap_or_default(),
            exact_runner::Viewport::sized(width as f64, height as f64),
            launch,
        )
        .map_err(HostError::Runner)?;
        if let Some(action) = region.and_then(|r| r.activate) {
            runner.act(action, Vec::new()).map_err(HostError::Runner)?;
        }
        let mut host = Host {
            runner,
            engine: Engine::new(),
            keys: BTreeMap::new(),
            presented: BTreeMap::new(),
            paint: Default::default(),
            viewport: (width, height),
            now_ms: 0.0,
            height_owner: None,
            flow_damage: Default::default(),
            row_dirty: Default::default(),
            height_bindings: Default::default(),
            transform_bindings: Default::default(),
            height_projection: None,
            height_layout_valid: false,
            content_region: None,
            #[cfg(test)]
            layout_calls: 0,
            data_activated: false,
            lowering: false,
            lowered: Default::default(),
            lower_colors: false,
            lowered_epoch: 0,
            lowered_changed: Default::default(),
            played: Default::default(),
            preload_wake: Default::default(),
            router_op: None,
            navigation: Default::default(),
            presence: Default::default(),
            presses: Default::default(),
            canvas2d: Default::default(),
            renewed: Vec::new(),
            media_mounts: Default::default(),
            values: Default::default(),
            hatched,
        };
        host.runner
            .set_canvas_limits(exact_runner::Limits::native(physical_memory(), false));
        host.runner.set_row_reuse(crate::app::row_reuse());
        // The engine hears the whole tree once: values, no transitions; an
        // `animation` starts now, as a browser starts one on a new element.
        host.lowering_from_env();
        let mut sync = MotionSync::default();
        host.discover_height_handles();
        host.discover_transform_handles();
        for id in host.preorder() {
            if let Some(node) = host.runner.kernel().node(id) {
                let key = node.key;
                host.keys.insert(key, id);
                host.runner.kernel().motion_sync_node(key, &mut sync);
            }
        }
        let applied = sync.apply(&mut host.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        host.lower_eligibility(&sync);
        host.boot_paint();
        host.presence
            .layout
            .adopt(host.runner.kernel(), host.keys.keys().copied());
        host.project_navigation();
        host.reconcile_height_bindings();
        host.reconcile_transform_bindings();
        if let Some(registration) = region {
            let roots = host.runner.roots();
            host.content_region = Some(
                crate::content_region::ContentRegionState::register(
                    host.runner.kernel_mut(),
                    &roots,
                    registration,
                )
                .map_err(HostError::Layout)?,
            );
        }
        let error = host.layout().err();
        host.observe_layout();
        host.present();
        Ok((host, error))
    }

    /// Tell the runner what this binary knows about its delivery (LLP 1030
    /// D7), from the archive's `compat.json`: the compatibility id, whether
    /// an update store is linked, and the executors. A `delivery` resource
    /// is answered again in that one commit, which the painter picks up
    /// like any other — the kernel is the display list here.
    pub fn set_delivery_from_compat(&mut self, json: &str) -> Option<String> {
        match self.runner.set_delivery_from_compat(json) {
            Ok(None) => None,
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => Some(format!("delivery: {e:?}")),
        }
    }

    /// The delivery facts whole (LLP 1030 D7) — what the update store has to
    /// say after a check or an activation, on top of the binary's own — into
    /// the runner, as one commit when they changed.
    pub fn set_delivery(&mut self, delivery: exact_runner::Delivery) -> Option<String> {
        match self.runner.set_delivery(delivery) {
            Ok(None) => None,
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => Some(format!("delivery: {e:?}")),
        }
    }

    /// The commands the last commits' actions asked for, in order (LLP 1005
    /// §3): `deliveryCheck`, `deliveryActivate`, `setScheme`.
    pub fn take_commands(&mut self) -> Vec<exact_runner::Command> {
        // The voice table's ops are drained with them and play nothing: this
        // host keeps the record and has no output (LLP 1096 D9).
        drop(self.runner.take_sounds());
        self.runner.take_commands()
    }

    /// Canvas 2D after this turn's layout (LLP 1056 D4): the kernel's
    /// content boxes at `scale`, a presented frame for canvases that asked
    /// (`frame`), the due draws, and their lists replayed. Whether any
    /// bitmap changed.
    pub(crate) fn sync_canvases(
        &mut self,
        scale: f64,
        frame: bool,
        assets: &crate::image::Assets,
    ) -> bool {
        if !self.runner.plan().surfaces.is_empty() {
            if self.canvas2d.text.is_none() {
                // Text measured and drawn by one engine (LLP 1056 D8).
                let text = std::sync::Arc::new(crate::canvas2d::text::CanvasText::new(
                    self.runner.plan().clone(),
                    assets.clone(),
                ));
                self.canvas2d.text = Some(text.clone());
                self.runner.set_canvas_text(text);
            }
            self.runner.layout_canvases(scale);
            if frame {
                self.runner.canvas_frame();
            }
            self.runner.draw_canvases(&|_| true);
            // The images the draws asked for, decoded now; each redraws the
            // canvases that asked (LLP 1056 D9).
            let asked = self.runner.take_canvas_image_requests();
            for src in &asked {
                let decoded = assets
                    .read(src)
                    .ok_or_else(|| format!("{src} is not an asset"))
                    .and_then(|bytes| {
                        tiny_skia::Pixmap::decode_png(&bytes)
                            .map_err(|e| format!("{src} does not decode: {e}"))
                    });
                let result = decoded.map(|p| {
                    let size = (p.width(), p.height());
                    self.canvas2d
                        .images
                        .insert(src.clone(), std::sync::Arc::new(p));
                    size
                });
                self.runner.canvas_image(src, result, &[]);
            }
            if !asked.is_empty() {
                self.runner.draw_canvases(&|_| true);
            }
        }
        let lists = self.runner.take_canvas_lists();
        let changed = !lists.is_empty();
        for e in self.canvas2d.apply(lists) {
            self.runner.log(e);
        }
        let live = self.runner.canvas_views();
        self.canvas2d.retain(&live);
        changed
    }

    /// Each 2D canvas's latest bitmap, for the painters.
    pub(crate) fn canvas_snapshots(&self) -> BTreeMap<ViewId, crate::canvas2d::CanvasPaint> {
        self.canvas2d.snapshots()
    }

    /// Whether a 2D canvas asked for another frame (LLP 1056 D5).
    pub fn canvas_wants_frame(&self) -> bool {
        self.runner.canvas_wants_frame()
    }

    /// The runner.
    pub fn runner(&self) -> &Runner<D> {
        &self.runner
    }

    /// The runner, mutably: a capability arm holds its device requests
    /// there (LLP 1069.007 D3).
    pub fn runner_mut(&mut self) -> &mut Runner<D> {
        &mut self.runner
    }

    /// `tap @t` or `type @t` (LLP 1069.007 D4): the agent answers a held
    /// device request, consumed here. The one capability admitted, `share`, has nothing to
    /// deliver but the journal line the runner writes (LLP 1069.003 D6);
    /// `None` for an ordinary `tap` or `type`.
    pub fn answer_hold(&mut self, request: &str) -> Option<String> {
        exact_runner::agent::answer(&mut self.runner, request).map(|(reply, _)| reply)
    }

    pub(crate) fn take_surface_updates(&mut self) -> Vec<exact_runner::SurfaceUpdate> {
        self.runner.take_surface_updates()
    }
    pub(crate) fn surface_record(&mut self, name: &str, json: Option<&str>) -> Option<String> {
        match self.runner.set_surface_record(name, json) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            // A refused record is the surface's, not the operation's that
            // carried it: the runner logs it and `state.surfaceRefusals` names
            // it, and the last accepted record stands (the agent keeps going).
            Err(e) => {
                eprintln!("exact: surface {name} refused: {e:?}");
                None
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn take_store_writes_for_test(&mut self) -> Vec<exact_runner::StoreWrite> {
        self.runner.take_store_writes()
    }

    #[cfg(test)]
    pub(crate) fn apply_test_ops(&mut self, ops: &[exact_kernel::Op]) {
        self.runner.kernel_mut().apply(0, 0, ops).unwrap();
    }

    /// The kernel.
    pub fn kernel(&self) -> &Kernel {
        self.runner.kernel()
    }

    /// Whether the plan gives `platform` the hatch `word` (LLP 1075.003.000.001 §4.3).
    pub(crate) fn plan_gives_hatch(&self, word: &str, platform: &str) -> bool {
        self.runner.plan().handles_hatch(word, platform)
    }

    /// The commit each media session claimant mounted in (LLP 1098 D9).
    pub(crate) fn media_mounts(&self) -> &crate::media_session::Mounts {
        &self.media_mounts
    }

    /// Explicit region selection, including retained provenance while pending.
    pub fn content_region(&self) -> Option<&crate::content_region::ContentRegionState> {
        self.content_region.as_ref()
    }

    /// Mounted collection metadata; no record keys or unmounted rows cross here.
    pub fn collections(&self) -> Vec<exact_runner::CollectionSnapshot> {
        self.runner.collections()
    }

    /// One list's entry of [`Host::collections`].
    pub fn collection(&self, view: ViewId) -> Option<exact_runner::CollectionSnapshot> {
        self.runner.collection(view)
    }

    /// A list's mounted rows as (view, epoch), into `out`.
    pub fn collection_mounted(&self, view: ViewId, out: &mut Vec<(ViewId, u64)>) {
        self.runner.collection_mounted(view, out);
    }

    /// [`Host::collections`] with only each list's first mounted row: views,
    /// sequences and port geometry, not every row's record.
    pub fn collections_shallow(&self) -> Vec<exact_runner::CollectionSnapshot> {
        self.runner.collections_shallow()
    }

    /// Commit viewport geometry and any edge action, retaining commits on refusal.
    /// `false` means stale or unchanged feedback, requiring no layout.
    pub fn collection_feedback(
        &mut self,
        feedback: exact_runner::CollectionFeedback,
    ) -> Result<bool, String> {
        self.collection_feedback_filled(feedback, exact_runner::CollectionFill::default())
    }

    /// [`Host::collection_feedback`] with a fill: the list's velocity and a
    /// slice's limit (LLP 1050.000 §6).
    pub fn collection_feedback_filled(
        &mut self,
        feedback: exact_runner::CollectionFeedback,
        fill: exact_runner::CollectionFill,
    ) -> Result<bool, String> {
        match self.runner.collection_feedback_filled(feedback, fill) {
            Ok(mut result) => {
                let changed = !result.receipts.is_empty();
                if !changed && result.error.is_none() {
                    return Ok(false);
                }
                for timed in &mut result.receipts {
                    timed.at_ms = self.now_ms;
                }
                self.commit(
                    &result.receipts,
                    result.error.map(|e| format!("collection feedback: {e:?}")),
                )
                .map_or(Ok(changed), Err)
            }
            Err(error) => Err(format!("collection feedback: {error:?}")),
        }
    }

    /// The agent's `tap <list> into <key>` (LLP 1070.000 §5).
    pub fn scroll_into_view(
        &mut self,
        view: ViewId,
        key: &str,
        block: &str,
        inline: &str,
    ) -> Result<(), String> {
        let receipt = self
            .runner
            .scroll_into_view_at(view, key, block, inline)
            .map_err(|e| format!("scrollIntoView: {e:?}"))?;
        let timed = Timed {
            at_ms: self.now_ms,
            receipt,
        };
        self.commit(&[timed], None).map_or(Ok(()), Err)
    }

    /// The motion engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// What a reload keeps (`Runner::carry`).
    pub fn carry(&self) -> Carried {
        self.runner.carry()
    }

    /// The viewport, points.
    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    /// The clock's last value, milliseconds.
    pub fn now(&self) -> f64 {
        self.now_ms
    }

    /// Next Contract timer deadline in the runner's clock domain.
    /// @ref LLP 1043.000 §3 D8 — the display loop sleeps until useful work.
    pub fn timer_due_ms(&self) -> Option<f64> {
        self.runner.timer_due_ms()
    }

    /// Whether a frame task wants every display frame (LLP 1073 D5).
    pub fn wants_frames(&self) -> bool {
        self.runner.wants_frames()
    }

    /// Whether motion is running (the presenter runs frames).
    pub fn motion(&self) -> bool {
        !self.engine.quiescent() || self.press_settle().is_some()
    }

    /// The roots, in order.
    pub fn roots(&self) -> Vec<ViewId> {
        self.runner.roots()
    }

    /// A node's presentation values: the engine's, else the committed
    /// style's.
    pub fn presented(&self, id: ViewId) -> Presented {
        let Some(node) = self.runner.kernel().node(id) else {
            return Presented::IDENTITY;
        };
        let mut shown = self
            .presented
            .get(&id)
            .copied()
            .unwrap_or_else(|| Presented::from_style(node.style));
        // The node's own appearance, if it has one: a `color-scheme` it sets
        // (LLP 1034 §8), else a report for its view (LLP 1062 D4). The
        // painter carries it to the subtree, and the session's to the rest.
        shown.dark = match node.style.mask.has(exact_kernel::StyleId::ColorScheme) {
            true => node.color_scheme_dark(),
            false => None,
        }
        .or_else(|| self.paint.view_dark(node.key));
        shown.lowered = self.lowered_mask(node.key);
        shown.press = self
            .presses
            .get(&node.key)
            .map_or(1., |f| f.factor(self.now_ms));
        shown
    }

    /// Whether any node shows press feedback now.
    #[cfg(target_os = "android")]
    pub(crate) fn pressing(&self) -> bool {
        !self.presses.is_empty()
    }

    /// What changed for kept rows since the last take: commits, layouts,
    /// presentation, and every node with press feedback now.
    pub(crate) fn take_row_dirty(&mut self) -> crate::paint::rows::Dirty {
        let mut dirty = std::mem::take(&mut self.row_dirty);
        for key in self.presses.keys() {
            dirty.node(*key);
        }
        dirty
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, `logs`
    /// from the runner; `settle` from the engine.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self
                .engine
                .settle_time()
                .map(|t| t * 1000.)
                .into_iter()
                .chain(self.press_settle())
                .reduce(f64::max)
            {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        exact_runner::agent::handle(&self.runner, request)
    }

    /// A line into the runner's journal (the agent's `logs`): a host fact
    /// worth reading beside the app's own lines.
    pub fn log(&mut self, line: impl Into<String>) {
        self.runner.log(line);
    }

    /// The work behind a continuation, dispatched on this thread after the
    /// commit that handed it out (LLP 1027.002 D3).
    pub fn dispatch_work(&mut self, token: u64) -> exact_runner::Dispatch {
        self.runner.dispatch_work(token)
    }

    /// Whether the source announced a topic not yet applied.
    pub fn has_announced(&self) -> bool {
        self.runner.has_announced()
    }

    /// An executor for the app's grants, which the source's announced topics
    /// also wake (LLP 1016.002).
    pub fn executor(&mut self) -> crate::executor::Executor {
        let executor = crate::executor::Executor::start(&self.grants());
        self.preload_wake.set(executor.waker());
        self.runner.listen(executor.waker());
        executor
    }

    /// A long native call's work: the source's native handler, off this thread.
    pub fn native_work(&mut self, request: &exact_runner::Request) -> exact_runner::Dispatch {
        self.runner.native_work(request)
    }

    /// Work a source held at dispatch that the last commit released.
    pub fn release_work(&mut self) -> Vec<(u64, exact_runner::Dispatch)> {
        self.runner.release_work()
    }

    /// The hosts the app may reach (LLP 1016 D6), as the data crate declares them.
    pub fn grants(&mut self) -> String {
        self.runner.data().grants().to_string()
    }

    /// The requests the runner handed out since the last take (LLP 1016 D2).
    pub fn take_requests(&mut self) -> Vec<RequestOut> {
        self.runner.take_requests()
    }

    /// Admission failures remain on the runner's current tickets, not a queue.
    pub fn refuse_request(&mut self, ticket: u64, reason: &'static str, ordered: bool) {
        self.runner.refuse_request(ticket, reason, ordered);
    }

    /// Take one admission failure through the usual settlement path.
    pub fn take_request_refusal(&mut self, allow_ordered: bool) -> Option<(u64, Outcome)> {
        self.runner.take_request_refusal(allow_ordered)
    }

    /// Ordered refusals hold later ordered dispatch until they settle or are forgotten.
    pub fn has_ordered_request_refusals(&self) -> bool {
        self.runner.has_ordered_request_refusals()
    }

    /// Whether another pump must settle an admission failure.
    pub fn has_request_refusals(&self, allow_ordered: bool) -> bool {
        self.runner.has_request_refusals(allow_ordered)
    }

    /// The executor's replies, oldest first, each a commit at `now_ms` (a
    /// ticket no longer held commits nothing); a reply the source cannot
    /// shape is the error, and the ones before it stand.
    pub fn fulfill_all(
        &mut self,
        outcomes: Vec<(u64, Outcome, Option<u64>)>,
        now_ms: f64,
    ) -> Option<String> {
        self.now_ms = now_ms.max(self.now_ms);
        // Announced topics first: what the device said before these replies.
        let (announced, failed) = self.runner.apply_announced();
        let mut error = failed.map(|e| format!("{e:?}"));
        let mut receipts: Vec<Timed> = announced
            .into_iter()
            .map(|receipt| Timed {
                at_ms: self.now_ms,
                receipt,
            })
            .collect();
        for (ticket, outcome, elapsed_ms) in outcomes {
            match self.runner.fulfill_measured(ticket, outcome, elapsed_ms) {
                Ok(Some(receipt)) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Ok(None) => {}
                Err(e) => {
                    error = Some(format!("{e:?}"));
                    break;
                }
            }
        }
        self.commit(&receipts, error)
    }

    /// A real path arriving at the `open-file` field becomes the document
    /// path, for an app granted `fs.read doc:/` (LLP 1069.010 D1): this
    /// host's one route in besides `EXACT_LAUNCH_URL`.
    fn document_value(&self, view: ViewId, event: Event) -> Event {
        let Event::Change(exact_runner::ControlValue::Text(path)) = &event else {
            return event;
        };
        let open_file = self
            .runner
            .kernel()
            .node(view)
            .is_some_and(|n| n.props.str(exact_kernel::PropId::TestId) == Some("open-file"));
        let granted =
            exact_runner::save_file::covered(self.runner.data_ref().grants(), "fs.read", "doc:/");
        match open_file && granted {
            true => match exact_data::documents::open_route(path, 0) {
                Some(doc) => Event::Change(exact_runner::ControlValue::Text(doc)),
                None => event,
            },
            false => event,
        }
    }

    /// Deliver an event at the app's clock (milliseconds). A refusal is the
    /// error; the tree is untouched (as the kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> Option<String> {
        let event = self.document_value(view, event);
        if matches!(event, Event::Press | Event::PressWith(_))
            && crate::navigation::popover_invoker(self.runner.kernel(), view)
        {
            self.log(crate::navigation::POPOVER_UNSUPPORTED);
            return Some(crate::navigation::POPOVER_UNSUPPORTED.into());
        }
        self.now_ms = now_ms.max(self.now_ms);
        if matches!(event, Event::Press | Event::PressWith(_))
            && self.runner.kernel().node(view).is_some_and(|node| {
                node.props
                    .str(exact_kernel::PropId::Commandfor)
                    .is_some_and(|target| !target.is_empty())
            })
        {
            let refusal = "unsupported: Linux dialog presentation is not implemented";
            self.log(refusal);
            return Some(refusal.into());
        }
        // At the event's time: an action's `performanceNow()` is the host's (LLP 1096 D3).
        let a = crate::traced(c"exact dispatch", || {
            self.runner.dispatch_at(view, event, self.now_ms)
        });
        self.commit(&a.receipts, a.error.map(|e| format!("{e:?}")))
    }

    // Current eligibility only DENIES an old picture's target. It never finds
    // a replacement handler or supplies coordinates/arguments from the live tree.
    pub(crate) fn retained_action_eligible(&self, key: NodeKey) -> bool {
        let Some(node) = self.kernel().node_by_key(key) else {
            return false;
        };
        let mut at = Some(node.id);
        while let Some(id) = at {
            let Some(node) = self.kernel().node(id) else {
                return false;
            };
            let visibility = self.route_visibility(id);
            if visibility.0
                || visibility.1
                || node.style.display == exact_kernel::Display::None
                || node.props.bool(exact_kernel::PropId::Disabled) == Some(true)
                || node
                    .props
                    .str(exact_kernel::PropId::Commandfor)
                    .is_some_and(|target| !target.is_empty())
                || node
                    .props
                    .str(exact_kernel::PropId::Popovertarget)
                    .is_some_and(|target| !target.is_empty())
            {
                return false;
            }
            at = node.parent;
        }
        true
    }

    pub(crate) fn dispatch_retained(
        &mut self,
        key: NodeKey,
        binding: &exact_runner::runner::ActionBinding,
        kind: exact_plan::EventKind,
        now_ms: f64,
    ) -> Result<bool, String> {
        let event = match kind {
            exact_plan::EventKind::Press => Event::Press,
            exact_plan::EventKind::Swiperight => Event::Swiperight,
            _ => return Ok(false),
        };
        // BEFORE host clock/focus/commit. Runner repeats its opaque binding check
        // at dispatch; a refusal takes neither the sample nor an ordinary action.
        if !now_ms.is_finite()
            || now_ms < self.now_ms
            || !self.retained_action_eligible(key)
            || self.runner.validate_action_binding(binding, kind).is_err()
        {
            return Ok(false);
        }
        let result = self.runner.dispatch_bound(binding, event);
        if matches!(
            result,
            Err(exact_runner::runner::ActionBindingError::Refused(_))
        ) {
            return Ok(false);
        }
        self.now_ms = now_ms;
        let error = match result {
            Ok(receipt) => self.commit(
                &[Timed {
                    at_ms: now_ms,
                    receipt,
                }],
                None,
            ),
            Err(error) => self.commit(&[], Some(format!("{error:?}"))),
        };
        error.map_or(Ok(true), Err)
    }

    /// Move the clock: every timer due fires at its own due time (LLP 1012
    /// §2). The clock lands where the runner says — a timer's refusal stops
    /// it at that timer's due time and is the error; the commits before it
    /// are shown.
    pub fn advance(&mut self, now_ms: f64) -> Option<String> {
        self.advance_effects(now_ms).0
    }

    /// [`Host::advance`], stopping after a timer that sends as well: an
    /// agent's jump ([`exact_runner::Runner::advance_until_request`]).
    pub fn advance_until_request(&mut self, now_ms: f64) -> Option<String> {
        let a = self.runner.advance_until_request(now_ms);
        self.advanced(a).0
    }

    /// The `then`s an agent's input settled, the clock unmoved
    /// ([`exact_runner::Runner::land_then`]).
    pub fn land_then(&mut self) -> Option<String> {
        let a = self.runner.land_then();
        self.advanced(a).0
    }

    /// Timer-loop demand, without skipping any runner, layout or effect work.
    pub(crate) fn advance_effects(&mut self, now_ms: f64) -> (Option<String>, bool) {
        let a = self.runner.advance_timed(now_ms);
        self.advanced(a)
    }

    /// A presented frame (LLP 1073 D2): the timers due by `now_ms`, then
    /// every frame task once at it; effects as [`Host::advance_effects`].
    pub(crate) fn frame(&mut self, now_ms: f64) -> (Option<String>, bool) {
        // The frame source started: frame tasks are its, not the timers' (LLP 1073 D4).
        self.runner.present_frames(true);
        let a = self.runner.frame(now_ms);
        self.advanced(a)
    }

    fn advanced(&mut self, a: exact_runner::Advanced) -> (Option<String>, bool) {
        self.now_ms = a.now_ms.max(self.now_ms);
        let error = a.error.map(|e| format!("{e:?}"));
        self.commit_effects(&a.receipts, error)
    }

    /// An image loaded: its intrinsic size in points (`None` when it failed
    /// or was cleared). Lays out again.
    pub fn set_intrinsic(&mut self, view: ViewId, size: Option<(f32, f32)>) -> Option<String> {
        self.set_intrinsics([(view, size)])
    }

    /// Views commits renewed since the last call (LLP 1078).
    pub(crate) fn take_renewed(&mut self) -> Vec<ViewId> {
        std::mem::take(&mut self.renewed)
    }

    /// Several nodes' natural sizes (pictures a sync decoded), then one
    /// layout, when any of them changed: not a layout per picture.
    pub fn set_intrinsics(
        &mut self,
        sizes: impl IntoIterator<Item = (ViewId, Option<(f32, f32)>)>,
    ) -> Option<String> {
        let mut error = None;
        let mut changed = false;
        for (view, size) in sizes {
            let kernel = self.runner.kernel_mut();
            let before = kernel
                .arena()
                .slot_of(view)
                .map(|slot| kernel.arena().intrinsic(slot));
            match kernel.set_intrinsic_size(view, size) {
                Ok(()) => changed |= before != Some(size),
                Err(e) => error = error.or(Some(format!("intrinsic: {e:?}"))),
            }
        }
        if changed {
            error = error.or(self.layout().err());
        }
        error
    }

    /// A symbol picture (`symbol:<role>`) a commit made, renewed or touched
    /// takes its natural size, the em square the image sync reports for it
    /// (LLP 1035.004), before this commit's layout: not in a second layout
    /// once the sync after the commit reports it (every list row built
    /// with an icon paid one).
    fn size_new_symbols(&mut self, receipts: &[Timed]) {
        let kernel = self.runner.kernel_mut();
        if !kernel.has_type(exact_kernel::NodeType::Image) {
            return;
        }
        let mut sizes = Vec::new();
        for t in receipts {
            let r = &t.receipt;
            for key in r.created.iter().chain(&r.renewed).chain(&r.touched) {
                let Some(node) = kernel.node_by_key(*key) else {
                    continue;
                };
                if node.node_type != exact_kernel::NodeType::Image
                    || !node
                        .props
                        .str(exact_kernel::PropId::ImageSource)
                        .is_some_and(|s| s.starts_with("symbol:"))
                {
                    continue;
                }
                let size = node.computed_row(exact_kernel::StyleId::FontSize, |s| s.font_size);
                sizes.push((node.id, (size > 0.).then_some((size, size))));
            }
        }
        for (view, size) in sizes {
            if let Err(e) = kernel.set_intrinsic_size(view, size) {
                self.log(format!("intrinsic: {e:?}"));
                return;
            }
        }
    }

    /// The viewport changed: lay out again.
    pub fn resize(&mut self, width: f32, height: f32) -> Option<String> {
        // @ref LLP 1039 D2 — merge re-answer and relayout, once.
        let receipt = match self.runner.set_viewport(width as f64, height as f64) {
            Ok(receipt) => receipt,
            Err(e) => return Some(format!("viewport: {e:?}")),
        };
        self.viewport = (width, height);
        self.presence.snap = true;
        if let Some(receipt) = receipt {
            return self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            );
        }
        let error = self.layout().err();
        self.observe_layout();
        error
    }

    /// The viewer's place and launch seed, together in one commit.
    pub fn set_place(&mut self, place: &exact_runner::time::Place) -> Option<String> {
        match self
            .runner
            .set_place(&place.locale, &place.time_zone, Some(place.seed))
        {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("place: {e:?}")),
        }
    }

    /// The date (LLP 1027.000.000): re-answer `exactTime` in one commit.
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> Option<String> {
        match self.runner.set_time(epoch_at_zero, utc_offset) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("time: {e:?}")),
        }
    }

    /// The device's posture and the viewport segments (LLP 1078 D4), set
    /// only by the agent here: the kernel's grid and `exactViewport`'s
    /// three fields together — a relayout when a style reads the segments,
    /// one commit when a resource reads the fields.
    pub fn set_segments(
        &mut self,
        posture: exact_runner::Posture,
        cols: u32,
        rows: u32,
        rects: Vec<exact_kernel::Rect>,
    ) -> Option<String> {
        let (Ok(c), Ok(r)) = (u8::try_from(cols), u8::try_from(rows)) else {
            return Some(format!("segments: {cols}x{rows} is past the kernel's grid"));
        };
        let restyled = match self.runner.kernel_mut().set_segments(c, r, rects) {
            Ok(changed) => changed,
            Err(e) => return Some(format!("segments: {e:?}")),
        };
        let fold = exact_runner::Fold {
            posture,
            cols,
            rows,
        };
        match self.runner.set_fold(fold) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) if restyled => {
                let error = self.layout().err();
                self.observe_layout();
                error
            }
            Ok(None) => None,
            Err(e) => Some(format!("segments: {e:?}")),
        }
    }

    /// The display preferences (LLP 1061 D5): re-answer `exactViewport()`
    /// in one commit.
    pub fn set_preferences(&mut self, preferences: exact_runner::Preferences) -> Option<String> {
        match self.runner.set_preferences(preferences) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("preferences: {e:?}")),
        }
    }

    /// The page's visibility, connectivity and share sheet (LLP 1069.000
    /// D2): re-answer `exactPage()` in one commit.
    pub fn set_page(&mut self, page: exact_runner::Page) -> Option<String> {
        match self.runner.set_page(page) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("page: {e:?}")),
        }
    }

    /// The root font size `rem` follows (LLP 1069.000 D3): a relayout in one
    /// commit, no resource asked again.
    pub fn set_root_font_size(&mut self, px: f64) -> Option<String> {
        match self.runner.set_root_font_size(px) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => None,
            Err(e) => Some(format!("root font size: {e:?}")),
        }
    }

    /// Seek presentation. Returns whether the registered Height changed layout;
    /// paint-only properties never trigger layout or text measurement.
    pub fn tick(&mut self, now_ms: f64) -> bool {
        self.now_ms = now_ms.max(self.now_ms);
        self.retire_presses();
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        let changed = match self.layout_motion() {
            Ok(changed) => changed,
            Err(error) => {
                self.log(error);
                false
            }
        };
        if changed {
            self.observe_layout();
        }
        self.present();
        changed
    }

    fn commit(&mut self, receipts: &[Timed], error: Option<String>) -> Option<String> {
        crate::traced(c"exact commit", || self.commit_effects(receipts, error).0)
    }

    fn commit_effects(
        &mut self,
        receipts: &[Timed],
        error: Option<String>,
    ) -> (Option<String>, bool) {
        // Region publication can change without changing its shell geometry.
        let mut paint = error.is_some() || self.content_region.is_some();
        for t in receipts {
            let r = &t.receipt;
            self.flow_damage.commit(self.runner.kernel(), r);
            self.row_dirty.commit(r);
            paint |= r.layout_invalidated
                || !r.created.is_empty()
                || !r.destroyed.is_empty()
                || !r.touched.is_empty();
            for key in &r.destroyed {
                self.forget_height_handle(*key);
                self.forget_transform_handle(*key);
                if let Some(id) = self.keys.remove(key) {
                    self.presented.remove(&id);
                    self.values.committed(id, None);
                }
            }
            for node in r
                .touched
                .iter()
                .filter_map(|k| self.runner.kernel().node_by_key(*k))
            {
                let value = node.props.str(exact_kernel::PropId::Value).unwrap_or("");
                self.values.committed(node.id, Some(value));
            }
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    self.keys.insert(*key, node.id);
                }
            }
            // A renewed node is a new mount (LLP 1078): nothing presented
            // or pressed carries over; the presenter resets the rest.
            for key in &r.renewed {
                self.presses.remove(key);
                if let Some(id) = self.keys.get(key).copied() {
                    self.presented.remove(&id);
                    self.renewed.push(id);
                    self.values.committed(id, None);
                }
            }
            self.media_mounts.commit(self.runner.kernel(), r);
        }
        self.track_presence(receipts);
        self.size_new_symbols(receipts);
        if receipts.iter().any(|t| !t.receipt.created.is_empty()) {
            self.discover_height_handles();
            self.discover_transform_handles();
        }
        // A nameless drive keeps secrets in the runner only (LLP 1018). A
        // named `--storage` drive writes them into that scratch tree, so a
        // reload reads them back (platformer R10). The log is taken either way.
        let writes = self.runner.take_store_writes();
        let app_id = self.runner.data().app_id().to_string();
        for line in persist_store_writes(&app_id, &writes) {
            self.runner.log(line);
        }
        paint |= self.project_navigation();
        self.reconcile_height_bindings();
        self.reconcile_transform_bindings();
        // Motion observes each commit before projected layout: targets are in place
        // before the engine hears them, and a transition a timer started is
        // born at that timer's due time — one seek and sixty give the same
        // bits (LLP 1002 D3; LLP 1012 §2).
        for t in receipts {
            // A pointer sample may advance presentation past an overdue timer.
            // Keep runner due-time order, but never replay the engine backwards.
            let seek = self
                .engine
                .advance((t.at_ms / 1000.0).max(self.engine.now()));
            debug_assert!(seek.is_ok(), "the clock never runs backwards here");
            let sync = self.runner.kernel().motion_sync(&t.receipt);
            let applied = sync.apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
            self.lower_eligibility(&sync);
            self.sync_paint(&t.receipt);
            if let Err(error) = self.sync_height_owner() {
                self.log(error);
                paint = true;
            }
        }
        self.retire_height_binding();
        self.retire_transform_binding();
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        let layout = crate::traced(c"exact layout", || {
            if receipts.is_empty() {
                self.layout_motion()
            } else {
                self.layout()
            }
        });
        self.observe_layout();
        paint |= layout.as_ref().copied().unwrap_or(true);
        // Transitions the reader plays leave the engine before it presents.
        self.play_transitions();
        // Consume the final sample even when the seek has made motion quiescent.
        paint |= self.present();
        (error.or(layout.err()), paint)
    }

    // @ref LLP 1038 D6/D7/D11 — no batch consumer on this host. Keep the
    // last coalesced op for inspection; navigation's agent section stays unavailable.
    fn project_navigation(&mut self) -> bool {
        let mut changed = false;
        if let Some(change) = self.runner.take_router_change() {
            self.router_op = Some(change);
            changed = true;
        }
        // Only stacks and popovers matter to it: the walk keeps those, in
        // preorder, not every node of every mounted row.
        // None at all (most apps, most commits): no walk.
        let kernel = self.runner.kernel();
        let navigation = if kernel.has_prop(exact_kernel::PropId::NavigationBack)
            || kernel.has_prop(exact_kernel::PropId::Popover)
        {
            kernel.preorder_where(&self.runner.roots(), |_, props| {
                props.str(exact_kernel::PropId::NavigationBack).is_some()
                    || props.str(exact_kernel::PropId::Popover).is_some()
            })
        } else {
            Vec::new()
        };
        for line in self.navigation.sync(self.runner.kernel(), &navigation) {
            self.runner.log(line);
        }
        changed
    }

    /// The last router op; this host has no foreign batch consumer.
    pub fn router_op(&self) -> Option<&exact_runner::RouterChange> {
        self.router_op.as_ref()
    }

    /// Hidden/inert through the route and authored inert ancestors.
    /// @ref LLP 1038 D6 — shared by painting, input, and agent layout.
    pub fn route_visibility(&self, id: ViewId) -> (bool, bool) {
        self.navigation.visibility(self.runner.kernel(), id)
    }

    /// Every presentation value the engine changed, kept by node.
    fn present(&mut self) -> bool {
        let mut changed = false;
        for p in self.engine.frame() {
            let key = node_key(p.node);
            let Some(view) = self.keys.get(&key).copied() else {
                continue;
            };
            if p.property == Property::Height {
                continue;
            }
            changed = true;
            self.row_dirty.node(key);
            // A path's `d` is read from the engine where it is painted.
            if p.property == Property::D {
                continue;
            }
            if Property::PAINT.contains(&p.property) {
                self.present_paint(p);
                continue;
            }
            let base = self.presented(view);
            let entry = self.presented.entry(view).or_insert(base);
            match p.property {
                Property::Translate => {
                    entry.translate = (p.value.x as f32, p.value.y as f32);
                    entry.translate_percent = (p.value.z as f32, p.value.w as f32);
                }
                Property::Layout => {
                    entry.layout = layout_presented(&self.engine, p.node, p.value).map(|v| v as f32)
                }
                Property::Scale => entry.scale = p.value.x as f32,
                Property::Rotate => entry.rotate = p.value.x as f32,
                Property::Opacity => entry.opacity = p.value.x as f32,
                Property::R => entry.svg[0] = Some(p.value.x as f32),
                Property::StrokeDashoffset => entry.svg[1] = Some(p.value.x as f32),
                // @ref LLP 1055.000 D15 — geometry, in `Presented::svg`'s order.
                Property::Cx => entry.svg[2] = Some(p.value.x as f32),
                Property::Cy => entry.svg[3] = Some(p.value.x as f32),
                Property::X => entry.svg[4] = Some(p.value.x as f32),
                Property::Y => entry.svg[5] = Some(p.value.x as f32),
                Property::Rx => entry.svg[6] = Some(p.value.x as f32),
                Property::Ry => entry.svg[7] = Some(p.value.x as f32),
                _ => unreachable!("height is projected through layout; paint is above"),
            }
        }
        changed
    }

    /// Every live node in preorder.
    pub fn preorder(&self) -> Vec<ViewId> {
        self.runner
            .kernel()
            .preorder_where(&self.runner.roots(), |_, _| true)
    }
}

/// The kv scope the runner's kept answers live in, beside secrets
/// (LLP 1027 D4). The same scope Apple's store writes.
const KEPT: &str = "exact.kept";

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
