//! The runner wrapped for a presenter: receipts → batches, with layout and
//! motion.
//!
//! @ref LLP 1008 §1
//!
//! After every commit the host walks the receipt (destroy, create, props,
//! style, children — the web host's rule: the view tree mirrors the kernel
//! tree), feeds the motion engine the commit (LLP 1003 §4), and seeks it to
//! the app's clock before laying out the roots under the viewport. Layout
//! projects the explicitly registered Height, then emits changed frames and
//! scroll content sizes; other presentation values follow. The kernel is the single source of
//! truth; the mirror is a memo of what the presenter has been told.

use crate::batch::Batch;
use crate::style;
use exact_kernel::motion::{motion_node, targets, MotionSync};
use exact_kernel::{
    Env, Frame, Kernel, NodeKey, NodeRef, NodeType, Offer, Overflow, PropId, PropValue,
    TextMeasurer, ViewId,
};
use exact_motion::{Change, Engine, HoldToken, Property};

#[path = "arrange.rs"]
mod arrange;
#[cfg(test)]
#[path = "arrange_tests.rs"]
mod arrange_tests;
#[path = "content_region/host.rs"]
mod content_region_host;
#[path = "height.rs"]
mod height;
#[path = "height_drag.rs"]
mod height_drag;
#[cfg(test)]
#[path = "height_tests.rs"]
mod height_tests;
#[path = "holds.rs"]
mod holds;
#[path = "paragraph.rs"]
mod paragraph;
#[path = "presence.rs"]
mod presence;
#[cfg(test)]
#[path = "transform_drag_tests.rs"]
mod transform_drag_tests;
use exact_plan::{EventKind, Plan};
use exact_runner::{Carried, DataSource, Event, Outcome, RequestOut, Runner, RunnerError, Timed};
pub use height::{HeightOwnerChange, HeightOwnerDisposition, HeightOwnerError};
use height_drag::{HeightDrag, HeightHandle};
#[path = "layout.rs"]
mod layout;
#[path = "transform_drag.rs"]
mod transform_drag;
#[path = "transform_drag_wire.rs"]
mod transform_drag_wire;
use crate::store::Platform;
use exact_kernel::id::{IdMap, IdSet};
use std::collections::{BTreeMap, BTreeSet};
use transform_drag::TransformDrags;

/// How many times one list report may measure, re-render and lay out before
/// the batch goes out. A window settles in two; rows of zero height can ask
/// for more, and the presenter's next report continues where this stopped.
const LIST_SETTLE_PASSES: usize = 4;

/// Why the host refused.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    Layout(String),
    Delivery(String),
    RuntimeIdExhausted,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Mirror {
    props: BTreeMap<String, String>,
    style: String,
    children: Vec<ViewId>,
    frame: Option<(f32, f32, f32, f32)>,
    content: Option<(f32, f32)>,
    flow: Vec<exact_kernel::FlowShape>,
}

/// One runner, one presenter.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    /// Whether the plan declares a `head`, and the title the presenter was
    /// last told (LLP 1048.003 D1): the window or scene title.
    has_heads: bool,
    head_title: Option<String>,
    mirror: IdMap<ViewId, Mirror>,
    keys: IdMap<NodeKey, ViewId>,
    inline_runs: IdMap<ViewId, (ViewId, Vec<EventKind>)>,
    dirty_paragraphs: BTreeSet<ViewId>,
    pending_layout: IdSet<NodeKey>,
    roots: Vec<ViewId>,
    /// Last published common collection snapshot; refreshed only after layout.
    collections_json: String,
    engine: Engine,
    holds: BTreeMap<u64, HoldToken>,
    height_owner: Option<NodeKey>,
    height_handles: BTreeMap<NodeKey, HeightHandle>,
    height_auto_owned: bool,
    height_drag: Option<HeightDrag>,
    transform_drags: TransformDrags,
    /// The one Arrange contact, from its catch until its source settles.
    arrange: Option<arrange::Arrange>,
    presence: presence::Presence,
    content_region: Option<crate::content_region::RegionState>,
    /// Lists whose last report stopped before their rows' heights were read back
    /// (a registered content region publishes as it lays out, so a report is
    /// one round there): the window wants another report.
    list_unsettled: BTreeSet<ViewId>,
    height_projection: Vec<(NodeKey, f32)>,
    height_sampling: Vec<(NodeKey, f32)>,
    height_presented: Vec<exact_kernel::PresentedHeight>,
    height_transitions: BTreeMap<NodeKey, Option<exact_kernel::Dimension>>,
    height_transition_epoch: Option<u64>,
    height_targets_dirty: bool,
    #[cfg(test)]
    height_target_passes: usize,
    #[cfg(test)]
    layout_calls: usize,
    viewport: (f32, f32),
    now_ms: f64,
    /// Where the app's kept secrets, and the runner's kept answers, go after
    /// a commit (LLP 1018 D6); `None` keeps them in the runner only (a test,
    /// or grants that do not parse).
    secrets: Option<Platform>,
    data_activated: bool,
    /// The update store's last line this host journaled, so a sync after
    /// a check writes it once.
    update_line: Option<String>,
    delivery: Option<&'static crate::delivery::Hooks>,
}

impl<D: DataSource> Host<D> {
    /// Boot from plan bytes with the app's text measurer and viewport (points):
    /// decode (a validation pass), boot the runner, lay out, and produce the
    /// first batch, which creates and places the whole tree.
    pub fn boot(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_stored(
            plan_bytes,
            data,
            measurer,
            width,
            height,
            None,
            Vec::new(),
            None,
        )
    }

    /// Boot carrying an earlier host's state (the dev reload, LLP 1007 §6):
    /// slots by name where their types still fit, settled resources where
    /// their arguments still match, the clock.
    pub fn boot_with(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_stored(
            plan_bytes,
            data,
            measurer,
            width,
            height,
            carried,
            Vec::new(),
            None,
        )
    }

    /// Boot with the app's kept secrets (LLP 1018 D6): `snapshot` is what
    /// the platform's store holds under the granted names, read before this
    /// call (a carried boot takes the carried store instead); `secrets` is
    /// where the commits' writes go, after each commit, on this thread.
    #[allow(clippy::too_many_arguments)]
    pub fn boot_stored(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        secrets: Option<Platform>,
    ) -> Result<(Host<D>, String), HostError> {
        let (mut host, batch) = Host::boot_stored_after_decode(
            plan_bytes,
            data,
            measurer,
            width,
            height,
            carried,
            snapshot,
            secrets,
            None,
            None,
            None,
            "/",
            None,
            |_| {},
        )?;
        host.commit_boot();
        Ok((host, batch))
    }

    /// Boot with one action over the accepted plan before its first layout.
    /// The Apple ABI uses this for synchronous font registration without
    /// decoding the plan twice.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn boot_stored_after_decode(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        secrets: Option<Platform>,
        compat: Option<&str>,
        delivery: Option<&'static crate::delivery::Hooks>,
        candidate_delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
        prepare: impl FnOnce(&Plan),
    ) -> Result<(Host<D>, String), HostError> {
        Self::boot_stored_after_decode_mode(
            plan_bytes,
            data,
            measurer,
            width,
            height,
            carried,
            snapshot,
            secrets,
            compat,
            delivery,
            candidate_delivery,
            launch,
            region,
            None,
            prepare,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn boot_stored_after_decode_mode(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        secrets: Option<Platform>,
        compat: Option<&str>,
        delivery: Option<&'static crate::delivery::Hooks>,
        candidate_delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
        native: Option<crate::content_region::NativeProjectionLimits>,
        prepare: impl FnOnce(&Plan),
    ) -> Result<(Host<D>, String), HostError> {
        if let Some(json) = compat {
            exact_runner::delivery::refuse_analysis(json)
                .map_err(|why| HostError::Delivery(why.into()))?;
            let expected = exact_runner::Delivery::default().with_compat(json).store != '0';
            if expected != delivery.is_some() {
                return Err(HostError::Delivery("the baked store level does not match the linked delivery adapter; regenerate the app entry".into()));
            }
        }
        let plan = Plan::decode(plan_bytes).map_err(HostError::Plan)?;
        let kernel = Kernel::new(measurer);
        let facts = candidate_delivery.unwrap_or_else(|| {
            let mut facts = exact_runner::Delivery::default();
            if let Some(json) = compat {
                facts = facts.with_compat(json);
                if let Some(hooks) = delivery {
                    (hooks.status_into)(&mut facts);
                }
            }
            facts
        });
        let mut runner = Runner::boot_with_delivery(
            plan,
            data,
            kernel,
            carried,
            snapshot,
            facts,
            exact_runner::Viewport::sized(width as f64, height as f64),
            launch,
        )
        .map_err(HostError::Runner)?;
        if let Some(action) = region.and_then(|r| r.activate) {
            runner.act(action, Vec::new()).map_err(HostError::Runner)?;
        }
        let mut content_region = region
            .map(|r| crate::content_region::RegionState::new(runner.kernel_mut(), r))
            .transpose()
            .map_err(HostError::Layout)?;
        if let Some(limits) = native {
            content_region
                .as_mut()
                .ok_or_else(|| HostError::Layout("native registration missing".into()))?
                .native = Some(
                crate::content_region::NativeProjection::new(limits).map_err(HostError::Layout)?,
            );
        }
        // The candidate catalog is installed before first text measurement.
        // Platform registration is deferred until the app accepts it.
        prepare(runner.plan());
        let has_heads = runner
            .plan()
            .nodes
            .iter()
            .any(|n| n.node_type == NodeType::Head as u8);
        let mut host = Host {
            has_heads,
            head_title: None,
            runner,
            mirror: IdMap::default(),
            keys: IdMap::default(),
            inline_runs: IdMap::default(),
            dirty_paragraphs: BTreeSet::new(),
            pending_layout: IdSet::default(),
            roots: Vec::new(),
            collections_json: "[]".into(),
            engine: Engine::new(),
            holds: BTreeMap::new(),
            height_owner: None,
            height_handles: BTreeMap::new(),
            height_auto_owned: false,
            height_drag: None,
            transform_drags: TransformDrags::new()?,
            arrange: None,
            presence: presence::Presence::default(),
            content_region,
            list_unsettled: BTreeSet::new(),
            height_projection: Vec::new(),
            height_sampling: Vec::new(),
            height_presented: Vec::new(),
            height_transitions: BTreeMap::new(),
            height_transition_epoch: None,
            height_targets_dirty: true,
            #[cfg(test)]
            height_target_passes: 0,
            #[cfg(test)]
            layout_calls: 0,
            viewport: (width, height),
            now_ms: 0.0,
            data_activated: false,
            secrets,
            update_line: None,
            delivery,
        };
        let mut batch = Batch::new();
        host.native_prepare_candidate().map_err(HostError::Layout)?;
        let order = host.preorder();
        let handlers = host.runner.handlers();
        for id in &order {
            host.create(*id, handlers.get(id).map_or(&[], Vec::as_slice), &mut batch);
        }
        for id in &order {
            host.emit_children(*id, &mut batch);
        }
        host.emit_paragraphs(&mut batch);
        host.roots = host.runner.roots();
        batch.roots(&host.roots.clone());
        host.emit_title(&mut batch);
        for s in host.runner.take_surface_updates() {
            batch.surface(&s);
        }
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = host.runner.take_router_change() {
            batch.router(&change);
        }
        for c in host.runner.take_commands() {
            batch.command(&c.name, &c.args);
        }
        // The engine hears the whole tree once: values, no transitions; an
        // `animation` starts now, as a browser starts one on a new element.
        let mut sync = MotionSync::default();
        for id in &order {
            if let Some(node) = host.runner.kernel().node(*id) {
                let n = motion_node(node.key);
                sync.transitions.push((n, node.style.transition.clone()));
                sync.layout.push((n, node.style.layout_transition.clone()));
                sync.animations.push((n, node.style.animation.clone()));
                for (property, value) in targets(node.style) {
                    sync.changes.push(Change {
                        node: n,
                        property,
                        value,
                        velocity: None,
                    });
                }
            }
        }
        let applied = sync.apply(&mut host.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        host.reconcile_height_handles(&mut batch, true);
        host.layout(&mut batch).map_err(HostError::Layout)?;
        // A failed first layout is a refused boot, not a partially committed
        // host. In particular, no candidate secret writes escape before this
        // point on a dev reload.
        host.emit_transform_drags(&mut batch);
        host.present(&mut batch, true);
        let timers = host.runner.timer_due_ms();
        let motion = !host.engine.quiescent();
        let clock = host.runner.now_ms();
        Ok((host, batch.finish(timers, motion, clock, None)))
    }

    /// Release effects only after the containing app accepted every candidate.
    pub(crate) fn commit_boot(&mut self) {
        if let Some(note) = self.delivery.and_then(|hooks| (hooks.take_note)()) {
            self.runner.log(note);
        }
        self.persist();
    }

    /// The runner.
    pub fn runner(&self) -> &Runner<D> {
        &self.runner
    }

    /// What a reload keeps (`Runner::carry`).
    pub fn carry(&self) -> Carried {
        self.runner.carry()
    }

    /// The motion engine: presentation values as the presenter shows them.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// What the update store has to say, into the runner (LLP 1030 D7) —
    /// after a check, after an activation: the `delivery` resource is
    /// answered again and the batch carries the commit; the check's line
    /// goes to the journal once, so the agent's `logs` reads it beside the
    /// app's own.
    pub fn sync_delivery(&mut self) -> String {
        let mut delivery = self.runner.delivery().clone();
        if let Some(hooks) = self.delivery {
            (hooks.status_into)(&mut delivery);
        }
        let line = self.delivery.and_then(|h| (h.last_line)());
        if line.is_some() && line != self.update_line {
            self.update_line = line.clone();
            self.runner.log(line.unwrap_or_default());
        }
        match self.runner.set_delivery(delivery) {
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Ok(None) => self.commit(&[], None),
            Err(e) => self.commit(&[], Some(format!("delivery: {e:?}"))),
        }
    }

    fn configure_storage(source: &mut D) -> Result<(), exact_runner::DataError> {
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
        let app_id = source.app_id().to_string();
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
        let mut data = home
            .join("Library/Application Support/exact")
            .join(&app_id)
            .join("data");
        let mut cache = home.join("Library/Caches/exact").join(&app_id);
        if let Some(name) = scratch {
            cache = cache.join("agent").join(name);
            data = cache.join("data");
        }
        // Sibling roots keep app:/cache grants from implicitly reaching tmp.
        // The user's cache base avoids a predictable shared /tmp directory.
        let temporary = cache.join("temporary");
        let cache = cache.join("cache");
        source.configure_storage(data, cache, temporary)
    }

    /// The common post-pixel activation order for committed sources.
    pub(crate) fn activate_source(source: &mut D) -> Result<(), exact_runner::DataError> {
        Self::configure_storage(source)?;
        source.activate()
    }

    /// The work behind a continuation, dispatched on this thread after the
    /// commit that handed it out (LLP 1027.002 D3).
    pub fn dispatch_work(&mut self, token: u64) -> exact_runner::Dispatch {
        self.runner.dispatch_work(token)
    }

    /// Take the source's announced topics, waking the host (LLP 1016.002).
    pub fn listen(&mut self, wake: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.runner.listen(wake);
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

    /// Load deferred app logic only after the presenter reports first pixel.
    pub fn activate_data(&mut self) -> String {
        if self.data_activated {
            return self.commit(&[], None);
        }
        match self.runner.data_ref().preload() {
            Ok(false) => return "{\"ops\":[],\"pending\":true}".into(),
            Err(error) => return self.commit(&[], Some(format!("prepare data: {error:?}"))),
            Ok(true) => {}
        }
        if let Err(error) = Self::activate_source(self.runner.data()) {
            return self.commit(&[], Some(format!("activate data: {error:?}")));
        }
        self.data_activated = true;
        match self.runner.data_ready() {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.commit(&[], None),
            Err(error) => self.commit(&[], Some(format!("data ready: {error:?}"))),
        }
    }

    /// What the last commit kept or forgot, into the platform's store (LLP
    /// 1018 D6) — synchronous, on this thread, milliseconds once per login.
    /// A write that fails is journaled; the app is otherwise unaffected, as
    /// a web app is when `setItem` throws: the next launch will not remember.
    fn persist(&mut self) {
        for w in self.runner.take_store_writes() {
            let Some(secrets) = &self.secrets else {
                continue;
            };
            if let Err(e) = secrets.write(&w) {
                self.runner.log(format!("store {} failed: {e}", w.name));
            }
        }
    }

    /// The requests the runner handed out since the last take (LLP 1016 D2):
    /// the bridge gives them to the executor; nothing reaches the presenter.
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

    /// The outcomes the executor brought back, oldest first, as one batch:
    /// each reply is a commit at `now_ms` (a ticket no longer held commits
    /// nothing); a reply the source cannot shape is the batch's error and
    /// the ones before it stand.
    pub fn fulfill_all(&mut self, outcomes: Vec<(u64, Outcome)>, now_ms: f64) -> String {
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

    /// Strict common LE viewport feedback followed by any runner edge action.
    /// Stale or malformed facts leave layout and the motion clock untouched.
    /// Only a runner receipt enters the ordinary native view commit path.
    pub fn collection_feedback(&mut self, bytes: &[u8], now_ms: f64) -> String {
        if !(0.0..=exact_runner::MAX_CLOCK_MS).contains(&now_ms) {
            return self.finish(
                Batch::new(),
                Some("invalid collection feedback time".into()),
            );
        }
        match self.runner.collection_feedback_bytes(bytes) {
            Ok(mut result) => {
                if result.receipts.is_empty() && result.error.is_none() {
                    return self.finish(Batch::new(), None);
                }
                self.now_ms = self.now_ms.max(now_ms);
                for timed in &mut result.receipts {
                    timed.at_ms = self.now_ms;
                }
                self.commit(&result.receipts, result.error.map(|e| format!("{e:?}")))
            }
            Err(error) => self.finish(Batch::new(), Some(format!("{error:?}"))),
        }
    }

    /// A presenter's line for the runner's journal (LLP 1012 §3): a refused
    /// intent — a focus that could not be delivered, a route key that names
    /// no route, a presentation the owner refused — with its reason.
    pub fn log(&mut self, line: &str) {
        self.runner.log(line);
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, and
    /// `logs` from the runner; `settle` — the clock at which the last
    /// transition in flight ends, milliseconds, `null` when quiescent — from
    /// the engine, which is what the presenter's `clock` advances to.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self.engine.settle_time() {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t * 1000.0)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        exact_runner::agent::handle(&self.runner, request)
    }

    /// Deliver an event at the app's clock (milliseconds); the batch makes
    /// the presenter equal to the tree after the commit, laid out, with any
    /// motion the change started. A refusal is reported in the batch's
    /// `error`, and the presenter is untouched (as the kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.dispatch(view, event) {
            Ok(receipt) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => self.commit(&[], Some(format!("{e:?}"))),
        }
    }

    /// [`Host::dispatch_at`] at the clock's last value.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> String {
        self.dispatch_at(view, event, self.now_ms)
    }

    /// Move the clock; every timer due fires at its own time; one batch for
    /// all of them, the engine hearing each commit at the time it was made.
    /// A timer's refusal stops the clock there: the commits before it are in
    /// the batch, the refusal in `error`, and `clock` says where the runner
    /// stands.
    pub fn advance(&mut self, now_ms: f64) -> String {
        let a = self.runner.advance_timed(now_ms);
        self.advanced(a)
    }

    /// [`Host::advance`], stopping after a timer that sends as well: an
    /// agent's jump ([`exact_runner::Runner::advance_until_request`]).
    pub fn advance_until_request(&mut self, now_ms: f64) -> String {
        let a = self.runner.advance_until_request(now_ms);
        self.advanced(a)
    }

    fn advanced(&mut self, a: exact_runner::Advanced) -> String {
        self.now_ms = a.now_ms.max(self.now_ms);
        let error = a.error.map(|e| format!("{e:?}"));
        self.commit(&a.receipts, error)
    }

    /// An image loaded: its intrinsic size, in points (`None` when it failed
    /// or was cleared). Lays out again; the batch carries the frames that
    /// moved — the image's, and everything its size pushed.
    pub fn set_intrinsic(&mut self, view: ViewId, size: Option<(f32, f32)>) -> String {
        self.height_targets_dirty = true;
        let mut batch = Batch::new();
        let error = match self.runner.kernel_mut().set_intrinsic_size(view, size) {
            Ok(()) => self.layout(&mut batch).err(),
            Err(e) => Some(format!("intrinsic: {e:?}")),
        };
        self.finish(batch, error)
    }

    /// Several images' intrinsic sizes at once — the symbols a batch just
    /// created — under one layout. A refusal for one view (it is gone, or
    /// not an image) leaves the others set and is the batch's error.
    pub fn set_intrinsics(&mut self, sizes: &[(ViewId, Option<(f32, f32)>)]) -> String {
        self.height_targets_dirty = true;
        let mut batch = Batch::new();
        let mut error = None;
        for &(view, size) in sizes {
            if let Err(e) = self.runner.kernel_mut().set_intrinsic_size(view, size) {
                error.get_or_insert(format!("intrinsic: {e:?}"));
            }
        }
        if let Err(e) = self.layout(&mut batch) {
            error.get_or_insert(e);
        }
        self.finish(batch, error)
    }

    /// Publish newly prepared native paragraph metrics through ordinary layout.
    pub fn text_ready(&mut self, key: exact_kernel::NodeKey, revision: u64) -> String {
        let mut batch = Batch::new();
        let error = if self
            .runner
            .kernel_mut()
            .invalidate_text_metrics(key, revision)
        {
            self.layout(&mut batch).err()
        } else {
            None
        };
        self.finish(batch, error)
    }

    /// The viewport changed: lay out again; the batch carries the frames
    /// A surface changed its current public record, or was disposed.
    pub fn surface_record(&mut self, name: &str, json: Option<&str>) -> String {
        let (receipts, error) = match self.runner.set_surface_record(name, json) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(error) => (vec![], Some(format!("surface {name}: {error:?}"))),
        };
        self.commit(&receipts, error)
    }

    /// that moved.
    pub fn resize(&mut self, width: f32, height: f32) -> String {
        self.resize_inner(width, height)
    }

    /// The date (LLP 1027.000.000): re-answer `exactTime` in one commit.
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> String {
        match self.runner.set_time(epoch_at_zero, utc_offset) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("time: {e:?}"))),
        }
    }

    /// The user's display preferences (LLP 1061 D4): re-answer
    /// `exactViewport` in one commit; the same preferences commit nothing.
    pub fn set_preferences(&mut self, preferences: exact_runner::Preferences) -> String {
        match self.runner.set_preferences(preferences) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("preferences: {e:?}"))),
        }
    }

    /// The viewer's locale and zone, beside the date: one commit when it changes.
    pub fn set_place(&mut self, locale: &str, time_zone: &str, seed: Option<f64>) -> String {
        let mut receipts = Vec::new();
        let mut error = None;
        let place = self.runner.set_place(locale, time_zone);
        let seeded = seed
            .map(|seed| self.runner.set_seed(seed))
            .unwrap_or(Ok(None));
        for result in [place, seeded] {
            match result {
                Ok(Some(receipt)) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Ok(None) => {}
                Err(e) => error = Some(format!("place: {e:?}")),
            }
        }
        if receipts.is_empty() {
            return self.finish(Batch::new(), error);
        }
        self.commit(&receipts, error)
    }

    fn resize_inner(&mut self, width: f32, height: f32) -> String {
        // @ref LLP 1039 D2 — merge re-answer and relayout, once.
        let receipt = match self.runner.set_viewport(width as f64, height as f64) {
            Ok(receipt) => receipt,
            Err(e) => return self.finish(Batch::new(), Some(format!("viewport: {e:?}"))),
        };
        self.viewport = (width, height);
        self.height_targets_dirty = true;
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
        let mut batch = Batch::new();
        let error = self.layout(&mut batch).err();
        self.finish(batch, error)
    }

    /// Native list geometry; row heights come from the same kernel layout
    /// that supplied the presenter's frames, never a second text measurer.
    ///
    /// A created row is an estimate until it is laid out, and the kernel is
    /// what lays it out — so the window settles here, in one call: report,
    /// lay out, read the rows back, report again until nothing changes. The
    /// presenter gets one batch carrying final frames. It used to take a
    /// second report from AppKit's frames, and with it a second JSON batch,
    /// a second `Presenter.apply` and its whole-tree passes, for every
    /// change that created a row (LLP 1044 F7).
    pub fn list_viewport(
        &mut self,
        view: ViewId,
        geometry: exact_runner::ListViewport<'_>,
    ) -> String {
        self.list_viewport_within(view, geometry, None)
    }

    /// [`Host::list_viewport`] creating at most `create_limit` rows beyond
    /// those the scrollport shows (`Runner::list_viewport_within`); the budget
    /// is the call's, across its settling rounds. [`Host::list_pending`] then
    /// says whether the window wants another report.
    pub fn list_viewport_within(
        &mut self,
        view: ViewId,
        geometry: exact_runner::ListViewport<'_>,
        create_limit: Option<usize>,
    ) -> String {
        let mut receipts: Vec<Timed> = Vec::new();
        let mut error = None;
        let mut top = geometry.top;
        // The report's budget is for the call, not for each round of it: what
        // a round creates comes off what the next may. Rows the scrollport
        // shows are outside any budget, so they still settle here.
        let mut budget = create_limit;
        self.list_unsettled.remove(&view);
        for pass in 0..LIST_SETTLE_PASSES {
            let rows = self.list_rows(view);
            let round = exact_runner::ListViewport {
                rows: &rows,
                top,
                ..geometry
            };
            match self.runner.list_viewport_within(view, round, budget) {
                Ok(receipt)
                    if receipt.created.is_empty()
                        && receipt.destroyed.is_empty()
                        && receipt.touched.is_empty() =>
                {
                    break;
                }
                Ok(receipt) => {
                    receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    });
                    // A registered content region publishes as it computes:
                    // one round here, and `list_pending` asks for the next.
                    if self.content_region.is_some() {
                        self.list_unsettled.insert(view);
                        break;
                    }
                    if let Err(e) = self.compute_layout() {
                        error = Some(e);
                        break;
                    }
                    if let Some(status) = self.runner.list_status(view) {
                        // Measuring can move the offset to hold the reading key.
                        top = status.top;
                        budget = budget.map(|left| left.saturating_sub(status.created));
                    }
                    if pass + 1 == LIST_SETTLE_PASSES {
                        self.list_unsettled.insert(view);
                    }
                }
                Err(e) => {
                    error = Some(format!("{e:?}"));
                    break;
                }
            }
        }
        if receipts.is_empty() && error.is_none() {
            return self.finish(Batch::new(), None);
        }
        self.commit(&receipts, error)
    }

    /// Whether a list's last report left rows to create or retire under its budget.
    pub fn list_pending(&self, view: ViewId) -> bool {
        self.list_unsettled.contains(&view)
            || self
                .runner
                .list_status(view)
                .is_some_and(|status| status.pending)
    }

    /// A windowed list's mounted row wrappers and their laid-out heights.
    fn list_rows(&self, view: ViewId) -> Vec<(ViewId, f64)> {
        let kernel = self.runner.kernel();
        kernel
            .node(view)
            .and_then(|list| list.children().first().copied())
            .and_then(|content| kernel.node(content))
            .map(|content| {
                content
                    .children()
                    .into_iter()
                    .filter_map(|id| kernel.node(id).map(|row| (id, row.frame.height as f64)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The safe-area insets changed (a boot under `viewport-fit=cover`, a
    /// rotation): the kernel's environment is set, every node whose style
    /// holds an `env()` length gets its dictionary re-sent with the new
    /// points and is laid out again; the batch carries what moved. Empty
    /// when nothing reads the insets, or they did not change.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) -> String {
        let mut batch = Batch::new();
        let error = match self
            .runner
            .kernel_mut()
            .set_env(Env::new(top, right, bottom, left))
        {
            Ok(false) => None,
            Ok(true) => {
                self.height_targets_dirty = true;
                for id in self.preorder() {
                    self.update(id, &mut batch);
                }
                self.emit_paragraphs(&mut batch);
                self.layout(&mut batch).err()
            }
            Err(e) => Some(format!("insets: {e:?}")),
        };
        self.finish(batch, error)
    }

    /// A motion frame: seek the engine to `now_ms` and report every
    /// presentation value that changed. Nothing else moves.
    pub fn tick(&mut self, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        let mut batch = Batch::new();
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        if self.arrange_settled() {
            return self.arrange_settle();
        }
        let error = self.height_layout_if_needed(&mut batch).err();
        // Only suspended ancestor mappings need a settle recheck. Normal
        // photo Translate/Scale frames keep the existing cheap tick path.
        if self.transform_drags.mapping_pending {
            self.emit_transform_drags(&mut batch);
        }
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    /// The active head's title, when a plan with a head may have moved it
    /// (LLP 1048.003 D1). The app owning the window or scene shows it.
    fn emit_title(&mut self, batch: &mut Batch) {
        if !self.has_heads {
            return;
        }
        let title = self.runner.head().title;
        if title != self.head_title {
            batch.title(title.as_deref());
            self.head_title = title;
        }
    }

    fn finish(&self, batch: Batch, error: Option<String>) -> String {
        batch.finish(
            self.runner.timer_due_ms(),
            !self.engine.quiescent(),
            self.runner.now_ms(),
            error.as_deref(),
        )
    }

    fn commit(&mut self, receipts: &[Timed], error: Option<String>) -> String {
        self.commit_into(receipts, error, Batch::new())
    }

    fn commit_into(&mut self, receipts: &[Timed], error: Option<String>, batch: Batch) -> String {
        let (mut batch, error) = self.commit_tree(receipts, error, batch);
        // A receipt can end an Arrange contact; its terminal runs at receipt time.
        let arrange = self.arrange_after_commit(&mut batch);
        self.commit_finish(batch, error.or(arrange))
    }

    /// The receipts into the mirror, the engine and layout; not yet presented.
    fn commit_tree(
        &mut self,
        receipts: &[Timed],
        error: Option<String>,
        mut batch: Batch,
    ) -> (Batch, Option<String>) {
        self.list_unsettled
            .retain(|view| self.runner.kernel().node(*view).is_some());
        self.native_retire_removed_owner(&mut batch);
        self.native_note_receipts(receipts);
        let bulk_handlers = (receipts
            .iter()
            .map(|t| t.receipt.created.len())
            .sum::<usize>()
            > 1)
        .then(|| self.runner.handlers());
        for t in receipts {
            let r = &t.receipt;
            self.begin_exits(r, &mut batch);
            for key in &r.destroyed {
                if let Some(id) = self.keys.remove(key) {
                    if let Some((owner, _)) = self.inline_runs.remove(&id) {
                        self.dirty_paragraphs.insert(owner);
                    } else if !self.native_selected_id(id) {
                        self.mirror.remove(&id);
                        if !self.exit_holds(id, &mut batch) {
                            batch.destroy(id);
                        }
                    }
                    self.transform_drags.remove(id);
                }
            }
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    let handlers = bulk_handlers.as_ref().map_or_else(
                        || self.runner.handlers_of(id),
                        |all| all.get(&id).cloned().unwrap_or_default(),
                    );
                    self.create(id, &handlers, &mut batch);
                }
            }
            for key in &r.touched {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    self.update(id, &mut batch);
                }
            }
        }
        // Receipts share the final kernel tree. A parent touched by an early
        // timer can already name a child created by a later timer in this seek.
        // All surviving views must exist before any final child list is attached.
        for t in receipts {
            for key in t.receipt.created.iter().chain(t.receipt.touched.iter()) {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    self.emit_children(node.id, &mut batch);
                }
            }
        }
        self.emit_paragraphs(&mut batch);
        let roots = self.runner.roots();
        if roots != self.roots {
            self.roots = roots.clone();
            batch.roots(&roots);
        }
        if !receipts.is_empty() {
            self.emit_title(&mut batch);
        }
        let mut height_target_error = None;
        // Runner receipts retain due-time order. Unobserved motion starts at
        // that due time; a late receipt cannot rewind an already presented
        // frame/hold. Match Web's floor at the engine's current presentation
        // time, not this batch's final time. No timer work enters pointer moves.
        for t in receipts {
            let seek = self
                .engine
                .advance((t.at_ms / 1000.0).max(self.engine.now()));
            debug_assert!(seek.is_ok(), "the clock never runs backwards here");
            let mut sync = self.runner.kernel().motion_sync(&t.receipt);
            self.spare_exits(&mut sync);
            let applied = sync.apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
            self.play_exits();
            self.reconcile_height_handles(&mut batch, true);
            let synced = self.sync_height_owner();
            debug_assert!(synced.is_ok(), "validated height sync");
            if let Err(error) = self.sync_height_transitions() {
                height_target_error.get_or_insert(error);
            }
            // Latest target/declaration must reach the held slot before an
            // invalidated header cancels it (negative delays sample at once).
            self.cancel_invalid_height_drag();
            self.reconcile_transform_drags(&mut batch);
        }
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        let layout_error = if height_target_error.is_some() {
            height_target_error
        } else if receipts.is_empty() {
            self.height_layout_if_needed(&mut batch).err()
        } else {
            self.layout(&mut batch).err()
        };
        for s in self.runner.take_surface_updates() {
            batch.surface(&s);
        }
        // The capabilities the actions called, after their commits, in order.
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = self.runner.take_router_change() {
            batch.router(&change);
        }
        for c in self.runner.take_commands() {
            batch.command(&c.name, &c.args);
        }
        (batch, error.or(layout_error))
    }

    /// Persist, then the presentation after every commit in the batch.
    fn commit_finish(&mut self, mut batch: Batch, error: Option<String>) -> String {
        self.persist();
        self.emit_transform_drags(&mut batch);
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    fn preorder(&self) -> Vec<ViewId> {
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

/// A frame in its parent's space (a root's is absolute).
fn relative(frame: Frame, parent: Option<Frame>) -> (f32, f32, f32, f32) {
    match parent {
        Some(p) => (frame.x - p.x, frame.y - p.y, frame.width, frame.height),
        None => (frame.x, frame.y, frame.width, frame.height),
    }
}

/// Natural scrollable overflow, including padding and descendants. The
/// presenter applies the CSS client-size minimum against its actual viewport;
/// flooring here loses the extent a native container needs under its own insets.
fn content_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    // Taffy's block containers do not always count end-edge padding in
    // `content_size` (its flex containers do); CSS's `scrollHeight` does.
    // Floor with the direct children's extent plus the end padding.
    let env = kernel.env();
    let pad = |d: exact_kernel::Dimension, against: f32| match d.resolve(&env) {
        exact_kernel::Dimension::Points(p) => p,
        exact_kernel::Dimension::Percent(p) => against * p / 100.0,
        exact_kernel::Dimension::Calc(p, x) => against * p / 100.0 + x,
        exact_kernel::Dimension::Auto | exact_kernel::Dimension::Env(..) => 0.0,
    };
    let pad_right = pad(node.style.padding_right, node.frame.width);
    let pad_bottom = pad(node.style.padding_bottom, node.frame.width);
    let mut w = node.content.0;
    let mut h = node.content.1;
    for child in node.children() {
        if let Some(c) = kernel.node(child) {
            w = w.max(c.frame.x - node.frame.x + c.frame.width + pad_right);
            h = h.max(c.frame.y - node.frame.y + c.frame.height + pad_bottom);
        }
    }
    (w, h)
}

/// The presenter's kind for a node: its type, in the schema's names.
fn kind_for(node: &NodeRef<'_>) -> &'static str {
    if node.node_type == NodeType::TextInput
        && node.props.str(PropId::SemanticTag) == Some("textarea")
    {
        return "textarea";
    }
    match node.node_type {
        NodeType::View => "view",
        NodeType::List => "list",
        NodeType::NativeView => "native",
        NodeType::Svg => "svg",
        NodeType::ScrollView => "scroll",
        NodeType::Text => "text",
        NodeType::Image => "image",
        NodeType::TextInput => "input",
        NodeType::Pressable => "button",
        NodeType::Toggle => "toggle",
        NodeType::Canvas => "canvas",
        NodeType::WebView => "iframe",
        NodeType::Video => "video",
        // A head takes no space; its title is the window's (LLP 1048.003 D1).
        NodeType::Head => "view",
    }
}

/// Props by their own names, as strings.
fn props_for(node: &NodeRef<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (id, value) in node.props.iter() {
        let text = match value {
            PropValue::Str(s) => s.clone(),
            PropValue::Bool(b) => b.to_string(),
            PropValue::Int(i) => i.to_string(),
            PropValue::Float(f) => style::num(*f as f32),
        };
        let _: PropId = id;
        out.insert(id.name().to_string(), text);
    }
    // Swift paints a paragraph from its runs' `text`: the string the kernel
    // measured, `text-transform` applied (LLP 1055 D5).
    if let Some(std::borrow::Cow::Owned(shown)) = node.shown_text() {
        out.insert(PropId::Text.name().to_string(), shown);
    }
    if node.node_type == NodeType::List && node.props.bool(PropId::Virtualized) == Some(true) {
        // The runner preserves collection anchors and follows the end using
        // sequence-checked corrections. Eager native autoscroll would compete.
        out.remove(PropId::ScrollFollowEnd.name());
    }
    if node.node_type == NodeType::TextInput {
        out.remove("spellcheck");
        if let Some(value) = node.spellcheck() {
            out.insert("spellcheck".into(), value.to_string());
        }
    }
    if node.node_type == NodeType::Image {
        if let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        {
            out.insert(
                "symbolName".into(),
                exact_kernel::generated::symbol(role)
                    .map(|s| s.0)
                    .unwrap_or("")
                    .into(),
            );
        }
    }
    out
}

fn handler_name(e: EventKind) -> Option<&'static str> {
    match e {
        EventKind::Reachstart | EventKind::Reachend => None,
        _ => Some(e.name()),
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

#[cfg(test)]
mod storage_tests {
    use super::*;
    use exact_kernel::MonospaceMeasurer;
    use exact_plan::{builder::PlanBuilder, Value};
    use exact_runner::DataError;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Seen {
        paths: Option<[PathBuf; 3]>,
        activations: usize,
    }

    struct Source(&'static str, Arc<Mutex<Seen>>);
    impl DataSource for Source {
        fn app_id(&self) -> &str {
            self.0
        }
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
        fn configure_storage(
            &mut self,
            data: PathBuf,
            cache: PathBuf,
            temporary: PathBuf,
        ) -> Result<(), DataError> {
            let mut seen = self.1.lock().unwrap();
            assert_eq!(seen.activations, 0, "configuration precedes app activation");
            seen.paths = Some([data, cache, temporary]);
            Ok(())
        }
        fn activate(&mut self) -> Result<(), DataError> {
            self.1.lock().unwrap().activations += 1;
            Ok(())
        }
    }

    #[test]
    fn storage_configuration_is_post_pixel_app_scoped_and_absent_in_agent_mode() {
        const CHILD: &str = "EXACT_STORAGE_CONFIGURATION_TEST";
        if std::env::var_os(CHILD).is_none() {
            for agent in [false, true] {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                command.args(["--exact", "host::storage_tests::storage_configuration_is_post_pixel_app_scoped_and_absent_in_agent_mode"])
                    .env(CHILD, "1").env_remove("EXACT_AGENT");
                if agent {
                    command.env("EXACT_AGENT", "1");
                }
                let output = command.output().unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
        let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
        builder.node(NodeType::View as u8, None, None, 0, &[], &[], None);
        let plan = builder.finish().unwrap().encode();
        let mut paths = Vec::new();
        for app_id in ["test.exact.storage.a", "test.exact.storage.b"] {
            let seen = Arc::new(Mutex::new(Seen::default()));
            let (mut host, _) = Host::boot(
                &plan,
                Source(app_id, seen.clone()),
                Box::new(MonospaceMeasurer::default()),
                10.0,
                10.0,
            )
            .unwrap();
            assert_eq!(seen.lock().unwrap().activations, 0);
            assert!(
                seen.lock().unwrap().paths.is_none(),
                "boot cannot configure storage"
            );
            host.activate_data();
            host.activate_data();
            let seen = seen.lock().unwrap();
            assert_eq!(seen.activations, 1);
            if std::env::var_os("EXACT_AGENT").is_some() {
                assert!(seen.paths.is_none());
            } else {
                let app_paths = seen.paths.as_ref().unwrap();
                assert!(app_paths.iter().all(|p| p.is_absolute()));
                for (index, path) in app_paths.iter().enumerate() {
                    assert!(path.components().any(|part| part.as_os_str() == app_id));
                    assert!(app_paths
                        .iter()
                        .enumerate()
                        .all(|(other, p)| other == index || !p.starts_with(path)));
                }
                paths.push(app_paths.clone());
            }
        }
        if paths.len() == 2 {
            assert!(paths[0].iter().zip(&paths[1]).all(|(a, b)| a != b));
        }
    }
}
