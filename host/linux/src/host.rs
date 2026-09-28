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

#[path = "arrange.rs"]
mod arrange;
#[path = "content_region/host.rs"]
mod content;
#[path = "height.rs"]
mod height;
#[path = "height_binding.rs"]
mod height_binding;
#[path = "holds.rs"]
mod holds;
#[path = "paint_motion.rs"]
mod paint_motion;
#[path = "presence.rs"]
mod presence;
#[path = "press.rs"]
mod press;
#[path = "transform_binding.rs"]
mod transform_binding;

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
    height_bindings: height_binding::Bindings,
    transform_bindings: transform_binding::Bindings,
    height_projection: Option<exact_kernel::PresentedHeight>,
    height_layout_valid: bool,
    content_region: Option<crate::content_region::ContentRegionState>,
    #[cfg(test)]
    layout_calls: usize,
    data_activated: bool,
    router_op: Option<exact_runner::RouterChange>,
    navigation: crate::navigation::Navigation,
    presence: presence::Presence,
    presses: BTreeMap<NodeKey, press::Feedback>,
    /// The 2D canvases' bitmaps (LLP 1056 D7).
    canvas2d: crate::canvas2d::Canvases,
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
        let kernel = Kernel::new(measurer);
        let mut runner = Runner::boot_with_delivery(
            plan,
            data,
            kernel,
            carried,
            Vec::new(),
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
            height_bindings: Default::default(),
            transform_bindings: Default::default(),
            height_projection: None,
            height_layout_valid: false,
            content_region: None,
            #[cfg(test)]
            layout_calls: 0,
            data_activated: false,
            router_op: None,
            navigation: Default::default(),
            presence: Default::default(),
            presses: Default::default(),
            canvas2d: Default::default(),
        };
        host.runner
            .set_canvas_limits(exact_runner::Limits::native(physical_memory(), false));
        // The engine hears the whole tree once: values, no transitions; an
        // `animation` starts now, as a browser starts one on a new element.
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
                // Native hosts parse the wide colour forms (LLP 1056 §8.2).
                exact_runner::exact_canvas::color::link_wide();
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
            Err(e) => Some(format!("surface {name}: {e:?}")),
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

    /// Explicit region selection, including retained provenance while pending.
    pub fn content_region(&self) -> Option<&crate::content_region::ContentRegionState> {
        self.content_region.as_ref()
    }

    /// Mounted collection metadata; no record keys or unmounted rows cross here.
    pub fn collections(&self) -> Vec<exact_runner::CollectionSnapshot> {
        self.runner.collections()
    }

    /// Commit viewport geometry and any edge action, retaining commits on refusal.
    /// `false` means stale or unchanged feedback, requiring no layout.
    pub fn collection_feedback(
        &mut self,
        feedback: exact_runner::CollectionFeedback,
    ) -> Result<bool, String> {
        match self.runner.collection_feedback(feedback) {
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
        shown.dark = Some(self.paint.dark(node.key));
        shown.press = self
            .presses
            .get(&node.key)
            .map_or(1., |f| f.factor(self.now_ms));
        shown
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

    fn configure_storage(&mut self) -> Result<(), exact_runner::DataError> {
        use exact_runner::DataError;
        use std::path::PathBuf;
        // Scripted drives must not read or write the developer's app files;
        // one that names a scratch tree gets storage there instead.
        let scratch = match std::env::var_os("EXACT_AGENT") {
            Some(_) => match agent_scratch()? {
                Some(name) => Some(name),
                None => return Ok(()),
            },
            None => None,
        };
        let app_id = self.runner.data().app_id().to_string();
        if app_id.is_empty() {
            return Ok(());
        }
        if matches!(app_id.as_str(), "." | "..")
            || !app_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
        {
            return Err(DataError::Unavailable("unsafe app storage identity".into()));
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| DataError::Unavailable("app storage needs an absolute HOME".into()))?;
        let base = |variable: &str, fallback: &str| {
            std::env::var_os(variable)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
                .join("exact")
                .join(&app_id)
        };
        let mut data = base("XDG_DATA_HOME", ".local/share").join("data");
        let mut cache = base("XDG_CACHE_HOME", ".cache");
        if let Some(name) = scratch {
            cache = cache.join("agent").join(name);
            data = cache.join("data");
        }
        // Sibling roots keep app:/cache grants from implicitly reaching tmp.
        // The user's cache base avoids a predictable shared /tmp directory.
        let temporary = cache.join("temporary");
        let cache = cache.join("cache");
        // What `app:/` names for the picker and an image's source (LLP
        // 1069.002 D4, D7); the last launch's picks go.
        crate::picker::set_roots(data.clone(), cache.clone(), temporary.clone());
        self.runner.data().configure_storage(data, cache, temporary)
    }

    /// Activate deferred data only after the presenter has produced first pixel.
    /// Returns whether a data-ready commit needs presenting and dispatching.
    pub fn activate_data(&mut self) -> Result<bool, String> {
        if self.data_activated {
            return Ok(false);
        }
        if !self
            .runner
            .data_ref()
            .preload()
            .map_err(|e| format!("prepare data: {e:?}"))?
        {
            return Ok(false);
        }
        if let Err(error) = self
            .configure_storage()
            .and_then(|()| self.runner.data().activate())
        {
            return Err(format!("activate data: {error:?}"));
        }
        self.data_activated = true;
        match self.runner.data_ready() {
            Ok(Some(receipt)) => match self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ) {
                Some(error) => Err(error),
                None => Ok(true),
            },
            Ok(None) => Ok(false),
            Err(error) => Err(format!("data ready: {error:?}")),
        }
    }

    /// Deferred image preparation needs another turn after first pixel.
    pub fn data_pending(&self) -> bool {
        !self.data_activated
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
    pub fn fulfill_all(&mut self, outcomes: Vec<(u64, Outcome)>, now_ms: f64) -> Option<String> {
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
        for (ticket, outcome) in outcomes {
            match self.runner.fulfill(ticket, outcome) {
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

    /// Deliver an event at the app's clock (milliseconds). A refusal is the
    /// error; the tree is untouched (as the kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> Option<String> {
        if matches!(event, Event::Press)
            && crate::navigation::popover_invoker(self.runner.kernel(), view)
        {
            self.log(crate::navigation::POPOVER_UNSUPPORTED);
            return Some(crate::navigation::POPOVER_UNSUPPORTED.into());
        }
        self.now_ms = now_ms.max(self.now_ms);
        if matches!(event, Event::Press)
            && self
                .runner
                .kernel()
                .node(view)
                .is_some_and(|node| node.props.str(exact_kernel::PropId::Commandfor).is_some())
        {
            let refusal = "unsupported: Linux dialog presentation is not implemented";
            self.log(refusal);
            return Some(refusal.into());
        }
        match self.runner.dispatch(view, event) {
            Ok(receipt) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Err(e) => self.commit(&[], Some(format!("{e:?}"))),
        }
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
                || node.props.str(exact_kernel::PropId::Commandfor).is_some()
                || node
                    .props
                    .str(exact_kernel::PropId::Popovertarget)
                    .is_some()
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

    /// Timer-loop demand, without skipping any runner, layout or effect work.
    pub(crate) fn advance_effects(&mut self, now_ms: f64) -> (Option<String>, bool) {
        let a = self.runner.advance_timed(now_ms);
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
        match self.runner.kernel_mut().set_intrinsic_size(view, size) {
            Ok(()) => self.layout().err(),
            Err(e) => Some(format!("intrinsic: {e:?}")),
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
        self.commit_effects(receipts, error).0
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
            paint |= r.layout_invalidated
                || !r.created.is_empty()
                || !r.destroyed.is_empty()
                || !r.touched.is_empty();
            for key in &r.destroyed {
                self.forget_height_handle(*key);
                self.forget_transform_handle(*key);
                if let Some(id) = self.keys.remove(key) {
                    self.presented.remove(&id);
                }
            }
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    self.keys.insert(*key, node.id);
                }
            }
        }
        self.track_presence(receipts);
        if receipts.iter().any(|t| !t.receipt.created.is_empty()) {
            self.discover_height_handles();
            self.discover_transform_handles();
        }
        // LLP 1018: a memory store here — the runner holds the values and
        // nothing persists them until this host links a store, so the write
        // log (secrets included) is dropped each commit, never accumulated.
        drop(self.runner.take_store_writes());
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
            let applied = self
                .runner
                .kernel()
                .motion_sync(&t.receipt)
                .apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
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
        let layout = if receipts.is_empty() {
            self.layout_motion()
        } else {
            self.layout()
        };
        self.observe_layout();
        paint |= layout.as_ref().copied().unwrap_or(true);
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
        for line in self.navigation.sync(self.runner.kernel(), &self.preorder()) {
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
            if Property::PAINT.contains(&p.property) {
                self.present_paint(p);
                continue;
            }
            let base = self.presented(view);
            let entry = self.presented.entry(view).or_insert(base);
            match p.property {
                Property::Translate => entry.translate = (p.value.x as f32, p.value.y as f32),
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
        let kernel = self.runner.kernel();
        let mut stack: Vec<ViewId> = self.runner.roots().into_iter().rev().collect();
        let mut order = Vec::new();
        while let Some(id) = stack.pop() {
            order.push(id);
            if let Some(node) = kernel.node(id) {
                let mut children = node.children();
                children.reverse();
                stack.extend(children);
            }
        }
        order
    }
}

/// A scripted drive's scratch storage (`EXACT_AGENT_STORAGE=<name>`): a tree
/// of its own under the cache base, so a drive can exercise storage without
/// touching the app's real files. Absent, a drive has no storage.
fn agent_scratch() -> Result<Option<String>, exact_runner::DataError> {
    let Some(name) = std::env::var_os("EXACT_AGENT_STORAGE") else {
        return Ok(None);
    };
    match name.to_str() {
        Some(name)
            if !matches!(name, "" | "." | "..")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)) =>
        {
            Ok(Some(name.to_owned()))
        }
        _ => Err(exact_runner::DataError::Unavailable(
            "EXACT_AGENT_STORAGE: one name of letters, digits, '.', '-' or '_'".into(),
        )),
    }
}

/// Physical memory in bytes, for the canvas budget (LLP 1056 D4): a quarter
/// of it, as WebKit charged on iOS. Linux reports it in `/proc/meminfo`;
/// elsewhere (the host run on a Mac) 8 GiB is assumed.
fn physical_memory() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|m| {
            let line = m.lines().find(|l| l.starts_with("MemTotal:"))?;
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            Some(kb * 1024)
        })
        .unwrap_or(8 << 30)
}
