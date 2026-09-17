//! The runner wrapped for a presenter: receipts → batches, with layout and
//! motion.
//!
//! @ref LLP 1008 §1
//!
//! After every commit the host walks the receipt (destroy, create, props,
//! style, children — the web host's rule: the view tree mirrors the kernel
//! tree), lays the roots out with the kernel's layout under the viewport,
//! emits every parent-relative frame that changed and every scroll
//! container's content size that changed, then feeds the motion engine the
//! commit (LLP 1003 §4), seeks it to the app's clock, and emits each
//! presentation value that changed. The kernel is the single source of
//! truth; the mirror is a memo of what the presenter has been told.

use crate::batch::Batch;
use crate::style;
use exact_kernel::motion::{motion_node, targets, MotionSync};
use exact_kernel::{
    Env, Frame, Kernel, NodeKey, NodeRef, NodeType, Offer, Overflow, PropId, PropValue,
    TextMeasurer, ViewId,
};
use exact_motion::{Change, Engine, Property};
use exact_plan::{EventKind, Plan};
use exact_runner::{Carried, DataSource, Event, Outcome, RequestOut, Runner, RunnerError, Timed};
use ibex2::host::Secrets;
use std::collections::BTreeMap;

/// Why the host refused.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    Layout(String),
    Delivery(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Mirror {
    props: BTreeMap<String, String>,
    style: String,
    children: Vec<ViewId>,
    frame: Option<(f32, f32, f32, f32)>,
    content: Option<(f32, f32)>,
}

/// One runner, one presenter.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    mirror: BTreeMap<ViewId, Mirror>,
    keys: BTreeMap<NodeKey, ViewId>,
    roots: Vec<ViewId>,
    /// Last published common collection snapshot; refreshed only after layout.
    collections_json: String,
    engine: Engine,
    viewport: (f32, f32),
    now_ms: f64,
    /// Where the app's kept secrets go after a commit (LLP 1018 D6); `None`
    /// keeps them in the runner only (a test, or no grants).
    secrets: Option<Secrets>,
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
        secrets: Option<Secrets>,
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
        secrets: Option<Secrets>,
        compat: Option<&str>,
        delivery: Option<&'static crate::delivery::Hooks>,
        candidate_delivery: Option<exact_runner::Delivery>,
        launch: &str,
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
        let runner = Runner::boot_with_delivery(
            plan,
            data,
            kernel,
            carried,
            snapshot,
            facts,
            exact_runner::Viewport {
                width: width as f64,
                height: height as f64,
            },
            launch,
        )
        .map_err(HostError::Runner)?;
        // The candidate catalog is installed before first text measurement.
        // Platform registration is deferred until the app accepts it.
        prepare(runner.plan());
        let mut host = Host {
            runner,
            mirror: BTreeMap::new(),
            keys: BTreeMap::new(),
            roots: Vec::new(),
            collections_json: "[]".into(),
            engine: Engine::new(),
            viewport: (width, height),
            now_ms: 0.0,
            data_activated: false,
            secrets,
            update_line: None,
            delivery,
        };
        let mut batch = Batch::new();
        let order = host.preorder();
        for id in &order {
            host.create(*id, &mut batch);
        }
        for id in &order {
            host.emit_children(*id, &mut batch);
        }
        host.roots = host.runner.roots();
        batch.roots(&host.roots.clone());
        for s in host.runner.take_surface_updates() {
            batch.surface(s.view, &s.name, &s.values);
        }
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = host.runner.take_router_change() {
            batch.router(&change);
        }
        for c in host.runner.take_commands() {
            batch.command(&c.name, &c.args);
        }
        // The engine hears the whole tree once: values, no transitions.
        let mut sync = MotionSync::default();
        for id in &order {
            if let Some(node) = host.runner.kernel().node(*id) {
                let n = motion_node(node.key);
                sync.transitions.push((n, node.style.transition.clone()));
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
        host.layout(&mut batch).map_err(HostError::Layout)?;
        // A failed first layout is a refused boot, not a partially committed
        // host. In particular, no candidate secret writes escape before this
        // point on a dev reload.
        host.present(&mut batch, true);
        let timers = host.runner.has_timers();
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
        // Scripted drives must not read or write the developer's app files.
        if std::env::var_os("EXACT_AGENT").is_some() {
            return Ok(());
        }
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
        let data = home
            .join("Library/Application Support/exact")
            .join(&app_id)
            .join("data");
        let cache = home.join("Library/Caches/exact").join(&app_id);
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

    /// Transfer a source-owned operation to the native executor.
    pub fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.runner.data().continuation(token)
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
            let result = match &w.value {
                Some(v) => secrets.set(&w.name, v),
                None => secrets.forget(&w.name),
            };
            if let Err(e) = result {
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
        let mut receipts = Vec::new();
        let mut error = None;
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

    /// Strict common LE viewport feedback, with no event/resource/timer dispatch.
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
            Ok(Some(receipt)) => {
                self.now_ms = self.now_ms.max(now_ms);
                self.commit(
                    &[Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }],
                    None,
                )
            }
            Ok(None) => self.finish(Batch::new(), None),
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
        self.now_ms = a.now_ms.max(self.now_ms);
        let error = a.error.map(|e| format!("{e:?}"));
        self.commit(&a.receipts, error)
    }

    /// An image loaded: its intrinsic size, in points (`None` when it failed
    /// or was cleared). Lays out again; the batch carries the frames that
    /// moved — the image's, and everything its size pushed.
    pub fn set_intrinsic(&mut self, view: ViewId, size: Option<(f32, f32)>) -> String {
        let mut batch = Batch::new();
        let error = match self.runner.kernel_mut().set_intrinsic_size(view, size) {
            Ok(()) => self.layout(&mut batch).err(),
            Err(e) => Some(format!("intrinsic: {e:?}")),
        };
        self.finish(batch, error)
    }

    /// The viewport changed: lay out again; the batch carries the frames
    /// that moved.
    pub fn resize(&mut self, width: f32, height: f32) -> String {
        // @ref LLP 1039 D2 — merge re-answer and relayout, once.
        let receipt = match self.runner.set_viewport(width as f64, height as f64) {
            Ok(receipt) => receipt,
            Err(e) => return self.finish(Batch::new(), Some(format!("viewport: {e:?}"))),
        };
        self.viewport = (width, height);
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
                for id in self.preorder() {
                    self.update(id, &mut batch);
                }
                self.layout(&mut batch).err()
            }
            Err(e) => Some(format!("insets: {e:?}")),
        };
        self.finish(batch, error)
    }

    /// A horizontal platform drag holds translate; release returns to its
    /// authored target under the authored transition, carrying velocity.
    pub fn drag_x(
        &mut self,
        view: ViewId,
        delta: f64,
        velocity: f64,
        release: bool,
        now_ms: f64,
    ) -> String {
        let mut batch = Batch::new();
        if ![delta, velocity, now_ms].iter().all(|v| v.is_finite()) {
            return self.finish(batch, Some("drag requires finite values".into()));
        }
        let Some(node) = self.runner.kernel().node(view) else {
            return self.finish(batch, Some("drag target is gone".into()));
        };
        let key = motion_node(node.key);
        let target = targets(node.style)
            .into_iter()
            .find(|(p, _)| *p == Property::Translate)
            .unwrap()
            .1;
        // The app authors the indicator as a direct child. The gesture holds
        // its existing style rows; it owns no additional visual/state graph.
        let indicators: Vec<_> = node
            .children()
            .into_iter()
            .filter_map(|id| {
                let child = self.runner.kernel().node(id)?;
                (child.props.bool(PropId::SwipeIndicator) == Some(true))
                    .then(|| (motion_node(child.key), targets(child.style)))
            })
            .collect();
        self.now_ms = now_ms.max(self.now_ms);
        let result = self.engine.advance(self.now_ms / 1000.0).and_then(|()| {
            if release {
                self.engine.observe(Change {
                    node: key,
                    property: Property::Translate,
                    value: target,
                    velocity: Some(exact_motion::Value::new(velocity, 0.0)),
                })
            } else {
                self.engine.hold(
                    key,
                    Property::Translate,
                    exact_motion::Value::new(target.x + delta, target.y),
                )
            }?;
            let progress = (delta / 64.0).clamp(0.0, 1.0);
            for (indicator, values) in &indicators {
                for &(property, value) in values {
                    if !matches!(property, Property::Opacity | Property::Scale) {
                        continue;
                    }
                    if release {
                        self.engine.observe(Change {
                            node: *indicator,
                            property,
                            value,
                            velocity: None,
                        })?;
                    } else {
                        self.engine.hold(
                            *indicator,
                            property,
                            exact_motion::Value::scalar(value.x + (1.0 - value.x) * progress),
                        )?;
                    }
                }
            }
            Ok(())
        });
        self.present(&mut batch, false);
        self.finish(batch, result.err().map(|e| format!("drag: {e:?}")))
    }

    /// A motion frame: seek the engine to `now_ms` and report every
    /// presentation value that changed. Nothing else moves.
    pub fn tick(&mut self, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        let mut batch = Batch::new();
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        self.present(&mut batch, false);
        self.finish(batch, None)
    }

    fn finish(&self, batch: Batch, error: Option<String>) -> String {
        batch.finish(
            self.runner.has_timers(),
            !self.engine.quiescent(),
            self.runner.now_ms(),
            error.as_deref(),
        )
    }

    fn commit(&mut self, receipts: &[Timed], error: Option<String>) -> String {
        let mut batch = Batch::new();
        for t in receipts {
            let r = &t.receipt;
            for key in &r.destroyed {
                if let Some(id) = self.keys.remove(key) {
                    self.mirror.remove(&id);
                    batch.destroy(id);
                }
            }
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    self.create(id, &mut batch);
                }
            }
            for key in r.created.iter().chain(r.touched.iter()) {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    if r.touched.contains(key) {
                        self.update(id, &mut batch);
                    }
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
        let roots = self.runner.roots();
        if roots != self.roots {
            self.roots = roots.clone();
            batch.roots(&roots);
        }
        let layout_error = if receipts.is_empty() {
            None
        } else {
            self.layout(&mut batch).err()
        };
        for s in self.runner.take_surface_updates() {
            batch.surface(s.view, &s.name, &s.values);
        }
        // The capabilities the actions called, after their commits, in order.
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = self.runner.take_router_change() {
            batch.router(&change);
        }
        for c in self.runner.take_commands() {
            batch.command(&c.name, &c.args);
        }
        self.persist();
        // Motion last, each commit at its own time: targets are in place
        // before the engine hears them, and a transition a timer started is
        // born at that timer's due time — so one seek and sixty give the same
        // bits (LLP 1002 D3; LLP 1012).
        for t in receipts {
            let seek = self.engine.advance(t.at_ms / 1000.0);
            debug_assert!(seek.is_ok(), "the clock never runs backwards here");
            let applied = self
                .runner
                .kernel()
                .motion_sync(&t.receipt)
                .apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        }
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        self.present(&mut batch, false);
        self.finish(batch, error.or(layout_error))
    }

    /// Lay every root out under the viewport and emit the parent-relative
    /// frames and scroll content sizes that changed.
    fn layout(&mut self, batch: &mut Batch) -> Result<(), String> {
        let (w, h) = self.viewport;
        for root in self.runner.roots() {
            self.runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(w, h))
                .map_err(|e| format!("layout: {e:?}"))?;
        }
        for id in self.preorder() {
            let kernel = self.runner.kernel();
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let parent = node.parent.and_then(|p| kernel.node(p)).map(|p| p.frame);
            let rel = relative(node.frame, parent);
            let content = (style::effective_overflow(&node)
                != (Overflow::Visible, Overflow::Visible))
                .then(|| content_size(&node, kernel));
            let m = self.mirror.entry(id).or_default();
            // An ancestor hint may change without touching the editor. Pass
            // its effective value through native containment, or clear it to
            // restore the platform default when the last declaration disappears.
            if node.node_type == NodeType::TextInput {
                let spelling = node.spellcheck().map(|value| value.to_string());
                if m.props.get("spellcheck") != spelling.as_ref() {
                    if let Some(value) = spelling {
                        batch.props(id, &[("spellcheck", value.clone())], &[]);
                        m.props.insert("spellcheck".into(), value);
                    } else {
                        batch.props(id, &[], &["spellcheck"]);
                        m.props.remove("spellcheck");
                    }
                }
            }
            if m.frame != Some(rel) {
                m.frame = Some(rel);
                batch.frame(id, rel.0, rel.1, rel.2, rel.3);
            }
            if let Some(c) = content {
                if m.content != Some(c) {
                    m.content = Some(c);
                    batch.content(id, c.0, c.1);
                }
            }
        }
        // Layout/receipt work may change the live window. Motion-only ticks and
        // stale feedback never traverse the tree to collect this metadata.
        let collections = self.runner.collections_json();
        if collections != self.collections_json {
            batch.collections(&collections);
            self.collections_json = collections;
        }
        Ok(())
    }

    /// Every presentation value the engine changed, as `present` ops. At
    /// boot only values that are not the property's identity: the presenter
    /// starts every view at identity, and the four motion rows are never in
    /// the style dictionary.
    fn present(&mut self, batch: &mut Batch, boot: bool) {
        for p in self.engine.frame() {
            if boot && p.value == p.property.identity() {
                continue;
            }
            let key = NodeKey {
                index: p.node as u32,
                generation: (p.node >> 32) as u32,
            };
            let Some(view) = self.keys.get(&key).copied() else {
                continue;
            };
            let (x, y) = match p.property {
                Property::Translate => (p.value.x, p.value.y),
                _ => (p.value.x, 0.0),
            };
            batch.present(view, p.property.name(), x, y);
        }
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

    fn create(&mut self, id: ViewId, batch: &mut Batch) {
        let node = self.runner.kernel().node(id).expect("live");
        let key = node.key;
        let kind = kind_for(&node);
        let props = props_for(&node);
        let env = self.runner.kernel().env();
        let (style, _skipped) = style::style_json_for(&node, &env);
        let handlers: Vec<&str> = self
            .runner
            .handlers_of(id)
            .into_iter()
            .map(|e| match e {
                EventKind::Press => "press",
                EventKind::Change => "change",
                EventKind::Hover => "hover",
                EventKind::Focus => "focus",
                EventKind::Blur => "blur",
                EventKind::Key => "key",
                EventKind::Submit => "submit",
                EventKind::Load => "load",
                EventKind::Message => "message",
                EventKind::Contextmenu => "contextmenu",
                EventKind::Dblclick => "dblclick",
                EventKind::Swiperight => "swiperight",
                EventKind::Scroll => "scroll",
                EventKind::Navigate => "navigate",
            })
            .collect();
        let pairs: Vec<(&str, String)> =
            props.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        batch.create(id, kind, &pairs, &style, &handlers);
        self.mirror.insert(
            id,
            Mirror {
                props,
                style,
                ..Mirror::default()
            },
        );
        self.keys.insert(key, id);
    }

    fn update(&mut self, id: ViewId, batch: &mut Batch) {
        let node = self.runner.kernel().node(id).expect("live");
        let props = props_for(&node);
        let env = self.runner.kernel().env();
        let (style, _skipped) = style::style_json_for(&node, &env);
        let m = self.mirror.entry(id).or_default();
        if props != m.props {
            let set: Vec<(&str, String)> = props
                .iter()
                .filter(|(k, v)| m.props.get(*k) != Some(*v))
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            let clear: Vec<&str> = m
                .props
                .keys()
                .filter(|k| !props.contains_key(*k))
                .map(String::as_str)
                .collect();
            batch.props(id, &set, &clear);
            m.props = props;
        }
        if style != m.style {
            batch.style(id, &style);
            m.style = style;
        }
    }

    fn emit_children(&mut self, id: ViewId, batch: &mut Batch) {
        let children = self.runner.kernel().node(id).expect("live").children();
        let m = self.mirror.entry(id).or_default();
        if children != m.children {
            batch.children(id, &children);
            m.children = children;
        }
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
