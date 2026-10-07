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
    Frame, Kernel, NodeKey, NodeRef, NodeType, Offer, Overflow, PropId, PropValue, TextMeasurer,
    ViewId,
};
use exact_motion::{Change, Engine, HoldToken, Property};

#[path = "agent.rs"]
mod agent;

#[path = "arrange.rs"]
mod arrange;
#[path = "arrange_group.rs"]
mod arrange_group;
#[cfg(test)]
#[path = "arrange_group_tests.rs"]
mod arrange_group_tests;
#[cfg(test)]
#[path = "arrange_tests.rs"]
mod arrange_tests;
#[path = "canvas2d.rs"]
pub(crate) mod canvas2d;
#[path = "colors.rs"]
mod colors;
#[path = "content_region/host.rs"]
mod content_region_host;
#[cfg(test)]
#[path = "control_text_tests.rs"]
mod control_text_tests;
#[path = "covers.rs"]
mod covers;
#[path = "flights.rs"]
mod flights;
#[path = "fold.rs"]
mod fold;
#[path = "height.rs"]
mod height;
#[path = "height_drag.rs"]
mod height_drag;
#[cfg(test)]
#[path = "height_tests.rs"]
mod height_tests;
#[path = "holds.rs"]
mod holds;
#[path = "paint.rs"]
mod paint;
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
#[path = "activation.rs"]
mod activation;
#[cfg(test)]
#[path = "activation_tests.rs"]
mod activation_tests;
#[cfg(test)]
#[path = "box_motion_tests.rs"]
mod box_motion_tests;
#[path = "layout.rs"]
mod layout;
#[path = "resize.rs"]
mod resize;
#[cfg(test)]
#[path = "storage_tests.rs"]
mod storage_tests;
#[path = "svg.rs"]
mod svg;
#[path = "svg_lower.rs"]
mod svg_lower;
#[cfg(test)]
#[path = "svg_tests.rs"]
mod svg_tests;
#[path = "transform_drag.rs"]
mod transform_drag;
#[path = "transform_drag_wire.rs"]
mod transform_drag_wire;
use crate::store::Platform;
use exact_kernel::id::{IdMap, IdSet};
use std::collections::{BTreeMap, BTreeSet};
use transform_drag::TransformDrags;

/// Why the host refused.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    Layout(String),
    Delivery(String),
    /// The plan uses capabilities this archive doesn't link, by name
    /// (LLP 1047.001 D5): `Unlinked("grouped_lists")`.
    Unlinked(String),
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
    field_content: Option<exact_kernel::Frame>,
}

/// One runner, one presenter.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    /// Whether the plan declares a `head`, and the title the presenter was
    /// last told (LLP 1048.003 D1): the window or scene title.
    has_heads: bool,
    head_title: Option<String>,
    /// The active head's `edited` (LLP 1069.010 D6), sent with the title.
    head_edited: bool,
    language: Option<String>,
    mirror: IdMap<ViewId, Mirror>,
    keys: IdMap<NodeKey, ViewId>,
    inline_runs: IdMap<ViewId, (ViewId, Vec<EventKind>)>,
    /// Hatch words: those journaled as viewless here, and those the plan does not give this platform.
    hatches: paragraph::HatchWords,
    /// SVG scenes and lowered CSS animations (LLP 1055 D4, D7).
    svg: svg::SvgState,
    /// 2D canvases whose replays the presenter has not caught up with: a
    /// frame request waits for them (`exact_canvas_held`, LLP 1056 D5).
    canvas_held: IdSet<ViewId>,
    canvas_kept: [Vec<exact_runner::CanvasList>; 2],
    /// Canvas draws run in a turn of their own (`exact_canvas_draw`), not in
    /// the turns main waits on (LLP 1072 §8.5).
    canvas_deferred: bool,
    dirty_paragraphs: BTreeSet<ViewId>,
    pending_layout: IdSet<NodeKey>,
    layout_withheld: bool,
    /// Each sticky node's constraint as the presenter last heard it (LLP 1083).
    stickies: IdMap<ViewId, exact_kernel::StickyConstraint>,
    /// Each multi-column record as the presenter last heard it (LLP 1093 D7).
    fragments: IdMap<ViewId, String>,
    ranks: IdMap<ViewId, i64>,
    roots: Vec<ViewId>,
    /// Last published common collection snapshot; refreshed only after layout.
    collections_json: String,
    engine: Engine,
    paint: paint::Paint,
    holds: BTreeMap<u64, HoldToken>,
    height_owner: Option<NodeKey>,
    height_handles: BTreeMap<NodeKey, HeightHandle>,
    height_auto_owned: bool,
    height_drag: Option<HeightDrag>,
    transform_drags: TransformDrags,
    /// The one Arrange contact, from its catch until its source settles.
    arrange: Option<arrange::Arrange>,
    /// A grouped session (LLP 1094), from its lift until its ghost lands.
    group: Option<arrange_group::GroupArrange>,
    presence: presence::Presence,
    flights: flights::Flights,
    content_region: Option<crate::content_region::RegionState>,
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
    secrets: Option<Box<dyn crate::store::KeptStore>>,
    data_activated: bool,
    /// The session's wake (`listen`), which a pending activation leaves
    /// with the data source.
    preload_wake: exact_runner::PreloadWake,
    /// The update store's last line this host journaled, so a sync after
    /// a check writes it once.
    update_line: Option<String>,
    delivery: Option<&'static crate::delivery::Hooks>,
    /// The reported colours' generation this session last presented; `None`
    /// before its first report, which corrects without motion (LLP 1095 D9).
    colors_seen: Option<u64>,
}

/// A plan's bytes at boot: copied from while decoding, or linked into the
/// program, whose data pool the decoded plan then keeps in place.
#[derive(Clone, Copy)]
pub(crate) enum PlanBytes<'a> {
    Copied(&'a [u8]),
    Static(&'static [u8]),
}

impl PlanBytes<'_> {
    fn decode(self) -> Result<Plan, exact_plan::PlanError> {
        match self {
            PlanBytes::Copied(bytes) => Plan::decode(bytes),
            PlanBytes::Static(bytes) => Plan::decode_static(bytes),
        }
    }
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
    /// where the commits' writes go after each commit: secrets on this
    /// thread, kept answers on their writer's (`store::flush_kept`).
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
            PlanBytes::Copied(plan_bytes),
            data,
            measurer,
            exact_runner::Viewport::sized(width as f64, height as f64),
            carried,
            snapshot,
            secrets.map(|p| Box::new(p) as Box<dyn crate::store::KeptStore>),
            None,
            None,
            None,
            "/",
            None,
            crate::link::Links::ALL,
            |_| Ok(()),
        )?;
        host.commit_boot();
        Ok((host, batch))
    }

    /// Boot with one action over the accepted plan before its first layout.
    /// The Apple ABI uses this for synchronous font registration without
    /// decoding the plan twice.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn boot_stored_after_decode(
        plan_bytes: PlanBytes<'_>,
        data: D,
        measurer: Box<dyn TextMeasurer>,
        viewport: exact_runner::Viewport,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        secrets: Option<Box<dyn crate::store::KeptStore>>,
        compat: Option<&str>,
        delivery: Option<&'static crate::delivery::Hooks>,
        candidate_delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
        links: crate::link::Links<D>,
        prepare: impl FnOnce(&mut Runner<D>) -> Result<(), HostError>,
    ) -> Result<(Host<D>, String), HostError> {
        Self::boot_stored_after_decode_mode(
            plan_bytes,
            data,
            measurer,
            viewport,
            carried,
            snapshot,
            secrets,
            compat,
            delivery,
            candidate_delivery,
            launch,
            region,
            None,
            links,
            prepare,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn boot_stored_after_decode_mode(
        plan_bytes: PlanBytes<'_>,
        data: D,
        measurer: Box<dyn TextMeasurer>,
        viewport: exact_runner::Viewport,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        secrets: Option<Box<dyn crate::store::KeptStore>>,
        compat: Option<&str>,
        delivery: Option<&'static crate::delivery::Hooks>,
        candidate_delivery: Option<exact_runner::Delivery>,
        launch: &str,
        region: Option<crate::content_region::ContentRegionRegistration>,
        native: Option<crate::content_region::NativeProjectionLimits>,
        links: crate::link::Links<D>,
        prepare: impl FnOnce(&mut Runner<D>) -> Result<(), HostError>,
    ) -> Result<(Host<D>, String), HostError> {
        if let Some(json) = compat {
            exact_runner::delivery::refuse_analysis(json)
                .map_err(|why| HostError::Delivery(why.into()))?;
            let expected = exact_runner::Delivery::default().with_compat(json).store != '0';
            if expected != delivery.is_some() {
                return Err(HostError::Delivery("the baked store level does not match the linked delivery adapter; regenerate the app entry".into()));
            }
        }
        let plan = plan_bytes.decode().map_err(HostError::Plan)?;
        // Before anything is built from it (LLP 1047.001 D5).
        if let Some(names) = crate::link::missing(&plan, crate::link::linked()) {
            return Err(HostError::Unlinked(names));
        }
        // The grammars this archive links (LLP 1047.001 D3; every one for a
        // public boot, LLP 1053.000 §2).
        links.grammars.link();
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
        // An `app:/data` image shows from the first frame, before storage
        // is configured and whether or not anything was picked (D7).
        crate::picker::know_roots(data.app_id());
        let mut runner = Runner::boot_with_delivery_linked(
            links.runner,
            plan,
            data,
            kernel,
            carried,
            snapshot,
            facts,
            viewport,
            launch,
        )
        .map_err(HostError::Runner)?;
        runner.set_device_links(links.device);
        // @ref LLP 1079 D1 — a development build measures its work.
        runner.measure_unless_production(compat, true);
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
        prepare(&mut runner)?;

        let has_heads = runner
            .plan()
            .nodes
            .iter()
            .any(|n| n.node_type == NodeType::Head as u8);
        let mut host = Host {
            has_heads,
            head_title: None,
            head_edited: false,
            language: None,
            hatches: paragraph::HatchWords::of(runner.plan()),
            runner,
            mirror: IdMap::default(),
            keys: IdMap::default(),
            inline_runs: IdMap::default(),
            svg: svg::SvgState::new(cfg!(any(target_os = "ios", target_os = "tvos"))),
            canvas_held: IdSet::default(),
            canvas_kept: Default::default(),
            canvas_deferred: false,
            dirty_paragraphs: BTreeSet::new(),
            pending_layout: IdSet::default(),
            layout_withheld: false,
            stickies: IdMap::default(),
            fragments: IdMap::default(),
            ranks: IdMap::default(),
            roots: Vec::new(),
            collections_json: "[]".into(),
            engine: {
                let mut engine = Engine::new();
                engine.set_lowered_properties(&svg::lowered(cfg!(any(
                    target_os = "ios",
                    target_os = "tvos"
                ))));
                engine
            },
            paint: paint::Paint::default(),
            holds: BTreeMap::new(),
            height_owner: None,
            height_handles: BTreeMap::new(),
            height_auto_owned: false,
            height_drag: None,
            transform_drags: TransformDrags::new()?,
            arrange: None,
            group: None,
            presence: presence::Presence::default(),
            flights: flights::Flights::default(),
            content_region,
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
            viewport: (viewport.width as f32, viewport.height as f32),
            now_ms: 0.0,
            data_activated: false,
            preload_wake: Default::default(),
            secrets,
            update_line: None,
            delivery,
            colors_seen: None,
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
        host.emit_language(&mut batch);
        for s in host.runner.take_surface_updates() {
            batch.surface(&s);
        }
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = host.runner.take_router_change() {
            batch.router(&change);
        }
        for c in host.runner.take_commands() {
            batch.command(&c.name, &c.args, c.source);
        }
        batch.sound(host.runner.take_sounds(), Some(host.runner.plan()));
        // The engine hears the whole tree once: values, no transitions; an
        // `animation` starts now, as a browser starts one on a new element.
        let mut sync = MotionSync::default();
        for id in &order {
            if let Some(node) = host.runner.kernel().node(*id) {
                host.runner.kernel().motion_sync_node(node.key, &mut sync);
            }
        }
        let applied = sync.apply(&mut host.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        let runner = &host.runner;
        svg_lower::eligibility(
            runner.kernel(),
            &mut host.engine,
            &sync,
            host.svg.box_motion,
            &|v| !runner.handlers_of(v).is_empty(),
        );
        for view in svg_lower::glass_sampling(host.runner.kernel(), &mut host.engine) {
            host.svg.element(host.runner.kernel(), view);
        }
        host.boot_paint(&order);
        host.reconcile_height_handles(&mut batch, true);
        host.layout(&mut batch).map_err(HostError::Layout)?;
        host.canvas_limits();
        host.canvas_turn(&mut batch);
        // A failed first layout is a refused boot, not a partially committed
        // host. In particular, no candidate secret writes escape before this
        // point on a dev reload.
        host.emit_transform_drags(&mut batch);
        host.present(&mut batch, true);
        let (mut batch, _) = host.resize_rounds(batch, None);
        let timers = host.runner.timer_due_ms();
        let motion = !host.engine.quiescent();
        batch.spatial = host.engine.spatial();
        batch.frames = host.runner.wants_frames();
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

    /// The runner, mutably: an auth session's arm (LLP 1069.006).
    pub fn runner_mut(&mut self) -> &mut Runner<D> {
        &mut self.runner
    }

    /// `tap @t` or `type @t` (LLP 1069.007 D4): the agent answers a held
    /// device request, consumed here; the reply names the requesting node,
    /// where the Swift capability arm delivers it (a picker's `change` or
    /// `cancel`, LLP 1069.002 D9). Also the picker's `showPicker` hold and
    /// `pickedPath`, whose reply adds the file to copy into. A `share` hold
    /// delivers nothing but the journal line the runner writes (LLP 1069.003
    /// D6). `appFile` names the file behind an `app:/` path, what an export
    /// copies from (LLP 1069.010 D3); `appRoots` the three roots, which the
    /// Swift host resolves an image's `app:/` source against (LLP 1069.002
    /// D7). `None` for an ordinary `tap` or `type`.
    pub fn answer_hold(&mut self, request: &str) -> Option<String> {
        let op = exact_runner::agent::field_str(request, "op");
        if op.as_deref() == Some("appFile") {
            let path = exact_runner::agent::field_str(request, "path").unwrap_or_default();
            return Some(crate::picker::app_file(&path));
        }
        if op.as_deref() == Some("appRoots") {
            return Some(crate::picker::roots_reply());
        }
        // The documents a person chose (LLP 1069.010 D1), minted for the
        // session that asks: `openDocument` for a host route, `mintDocument`
        // for what a picker returned, `forgetDocuments` as the session ends.
        if let Some(op @ ("openDocument" | "mintDocument" | "forgetDocuments")) = op.as_deref() {
            let grants = self.runner.data_ref().grants().to_owned();
            return Some(crate::picker::documents(op, request, &grants));
        }
        let picked = op.as_deref() == Some("pickedPath");
        exact_runner::agent::answer(&mut self.runner, request).map(|(reply, _)| {
            if picked {
                crate::picker::with_files(reply)
            } else {
                reply
            }
        })
    }

    /// A command that shows system UI, about to run (`share`, LLP 1069.003;
    /// `saveFile`, LLP 1069.010 D3): the runner's ruling on it
    /// ([`exact_runner::commands::request`]).
    pub fn command(&mut self, request: &str) -> String {
        exact_runner::commands::request(&mut self.runner, request)
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
        let Some(([data, cache, temporary], _)) = crate::picker::app_dirs(source.app_id())? else {
            return Ok(());
        };
        // An authored test's store starts empty every run (`agent --test`):
        // emptied at the boot that read it, so this is a no-op unless that failed.
        crate::picker::empty_fresh_tree(source.app_id())?;
        // What `app:/` names for the picker and an image's source (LLP
        // 1069.002 D4, D7); the last launch's picks go.
        crate::picker::set_roots(data.clone(), cache.clone(), temporary.clone());
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
        self.preload_wake.set(wake.clone());
        self.runner.listen(wake);
    }

    /// The source's native slot, where the host installs the app module.
    pub fn native_slot(&self) -> exact_runner::Native {
        self.runner.native_slot()
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

    /// What the last commit kept or forgot, into the platform's store (LLP
    /// 1018 D6): secrets synchronously, on this thread, milliseconds once per
    /// login; kept answers queued for their writer thread (`store.rs`). A
    /// write that fails is journaled (a kept answer's at a later commit);
    /// the app is otherwise unaffected, as a web app is when `setItem`
    /// throws: the next launch will not remember.
    fn persist(&mut self) {
        for line in self.kept_failures() {
            self.runner.log(line);
        }
        for w in self.runner.take_store_writes() {
            let Some(secrets) = &self.secrets else {
                continue;
            };
            if let Err(e) = secrets.write(&w) {
                self.runner.log(format!("store {} failed: {e}", w.name));
            }
        }
    }

    /// The kept answers the writer failed to write since the last call.
    pub fn kept_failures(&self) -> Vec<String> {
        self.secrets
            .as_ref()
            .map(|s| s.kept_failures())
            .unwrap_or_default()
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
    pub fn fulfill_all(
        &mut self,
        outcomes: Vec<(u64, Outcome, Option<u64>)>,
        now_ms: f64,
    ) -> String {
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

    /// The agent's `tap <list> into <key>` (LLP 1070.000 §5).
    pub fn scroll_into_view(
        &mut self,
        view: ViewId,
        key: &str,
        block: &str,
        inline: &str,
    ) -> String {
        match self.runner.scroll_into_view_at(view, key, block, inline) {
            Ok(receipt) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(error) => self.finish(Batch::new(), Some(format!("scrollIntoView: {error:?}"))),
        }
    }

    /// A presenter's line for the runner's journal (LLP 1012 §3): a refused
    /// intent — a focus that could not be delivered, a route key that names
    /// no route, a presentation the owner refused — with its reason.
    pub fn log(&mut self, line: &str) {
        self.runner.log(line);
    }

    /// Deliver an event at the app's clock (milliseconds); the batch makes
    /// the presenter equal to the tree after the commit, laid out, with any
    /// motion the change started. A refusal is reported in the batch's
    /// `error`, and the presenter is untouched (as the kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        // At the event's time: an action's `now()` is the host's (LLP 1096 D3).
        let a = self.runner.dispatch_at(view, event, self.now_ms);
        self.commit(&a.receipts, a.error.map(|e| format!("{e:?}")))
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

    /// The `then`s an agent's input settled, the clock unmoved
    /// ([`exact_runner::Runner::land_then`]).
    pub fn land_then(&mut self) -> String {
        let a = self.runner.land_then();
        self.advanced(a)
    }

    /// Whether this host's display drives frame tasks (LLP 1073 D4): off when
    /// the agent's clock takes over, so its advances fire virtual frames.
    pub fn present_frames(&mut self, on: bool) {
        self.runner.present_frames(on);
    }

    /// A presented frame (LLP 1073 D2): the timers due by `now_ms`, then
    /// every frame task once at it, in one batch as [`Host::advance`]'s.
    pub fn frame(&mut self, now_ms: f64) -> String {
        // The frame source started: frame tasks are its, not the timers' (LLP 1073 D4).
        self.runner.present_frames(true);
        let a = self.runner.frame(now_ms);
        self.advanced(a)
    }

    fn advanced(&mut self, a: exact_runner::Advanced) -> String {
        self.now_ms = a.now_ms.max(self.now_ms);
        self.runner.canvas_frame();
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

    /// The page's visibility, connectivity and share sheet (LLP 1069.000
    /// D2): re-answer `exactPage` in one commit; the same facts commit
    /// nothing.
    pub fn set_page(&mut self, page: exact_runner::Page) -> String {
        match self.runner.set_page(page) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("page: {e:?}"))),
        }
    }

    /// The root font size `rem` follows (LLP 1069.000 D3): a relayout in
    /// one commit, no resource asked again; the same size commits nothing.
    pub fn set_root_font_size(&mut self, px: f64) -> String {
        match self.runner.set_root_font_size(px) {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.finish(Batch::new(), None),
            Err(e) => self.finish(Batch::new(), Some(format!("root font size: {e:?}"))),
        }
    }

    /// The viewer's locale and zone, beside the date: one commit when it changes.
    pub fn set_place(&mut self, locale: &str, time_zone: &str, seed: Option<f64>) -> String {
        let (receipts, error) = match self.runner.set_place(locale, time_zone, seed) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("place: {e:?}"))),
        };
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

    /// Content has settled: the tree and the motion engine give back what
    /// they hold beyond the live nodes ([`exact_kernel::Kernel::trim`]).
    pub fn trim(&mut self) {
        self.runner.kernel_mut().trim();
        self.engine.trim();
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
        self.runner.canvas_frame();
        // A tick is never waited for where draws are deferred (LLP 1072
        // §9): it draws in its own turn.
        self.canvas_draw_turn(&mut batch);
        // Only suspended ancestor mappings need a settle recheck. Normal
        // photo Translate/Scale frames keep the existing cheap tick path.
        if self.transform_drags.mapping_pending {
            self.emit_transform_drags(&mut batch);
        }
        self.present(&mut batch, false);
        self.finish(batch, error)
    }

    /// The active head's title and edited mark, when a plan with a head may
    /// have moved them (LLP 1048.003 D1, LLP 1069.010 D6). The app owning
    /// the window or scene shows them.
    fn emit_title(&mut self, batch: &mut Batch) {
        if !self.has_heads {
            return;
        }
        let head = self.runner.head();
        if head.title != self.head_title || head.edited != self.head_edited {
            batch.title(head.title.as_deref(), head.edited);
            self.head_title = head.title;
            self.head_edited = head.edited;
        }
    }

    fn emit_language(&mut self, batch: &mut Batch) {
        let language = self.runner.resolved_locale();
        if self.language.as_deref() != Some(language) {
            batch.language(language, self.runner.direction());
            self.language = Some(language.into());
        }
    }

    fn finish(&mut self, batch: Batch, error: Option<String>) -> String {
        let (batch, error) = self.resize_rounds(batch, error);
        if batch.layout_provisional {
            self.withhold_layout();
        }
        self.refused(batch, error)
    }

    /// [`Host::finish`] for a batch that commits nothing, a refusal's.
    fn refused(&self, mut batch: Batch, error: Option<String>) -> String {
        batch.spatial = self.engine.spatial();
        batch.frames = self.runner.wants_frames();
        batch.seq = self.runner.seq_range();
        batch.canvas_frames(self.runner.canvas_wants_frame());
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
        self.group_after_commit(&mut batch);
        self.commit_finish(batch, error.or(arrange))
    }

    /// The receipts into the mirror, the engine and layout; not yet presented.
    fn commit_tree(
        &mut self,
        receipts: &[Timed],
        error: Option<String>,
        mut batch: Batch,
    ) -> (Batch, Option<String>) {
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
            // Flights capture their leavers before an exit takes them
            // out of the presenter's maps (LLP 1013.000 D4.1).
            self.begin_flights(r, &mut batch);
            self.begin_exits(r, &mut batch);
            for key in &r.destroyed {
                if let Some(id) = self.keys.remove(key) {
                    self.ranks.remove(&id);
                    self.paint.runs.remove(&id);
                    if let Some((owner, _)) = self.inline_runs.remove(&id) {
                        self.dirty_paragraphs.insert(owner);
                    } else if self.svg.destroyed(id) {
                        self.mirror.remove(&id);
                    } else if !self.native_selected_id(id) {
                        self.mirror.remove(&id);
                        self.end_flight_of(id);
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
            let runner = &self.runner;
            svg_lower::eligibility(
                runner.kernel(),
                &mut self.engine,
                &sync,
                self.svg.box_motion,
                &|v| !runner.handlers_of(v).is_empty(),
            );
            for view in svg_lower::glass_sampling(self.runner.kernel(), &mut self.engine) {
                self.svg.element(self.runner.kernel(), view);
            }
            self.play_exits(&mut batch);
            self.start_flights(&t.receipt);
            self.seed_layout(&t.receipt, &mut batch);
            self.sync_paint(&t.receipt, &mut batch);
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
        if layout_error.is_some() && !receipts.is_empty() {
            self.emit_ranks(&mut batch);
        }
        self.canvas_turn(&mut batch);
        for s in self.runner.take_surface_updates() {
            batch.surface(&s);
        }
        // The capabilities the actions called, after their commits, in order.
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = self.runner.take_router_change() {
            batch.router(&change);
        }
        for c in self.runner.take_commands() {
            batch.command(&c.name, &c.args, c.source);
        }
        batch.sound(self.runner.take_sounds(), None);
        (batch, error.or(layout_error))
    }

    /// Persist, then the presentation after every commit in the batch.
    fn commit_finish(&mut self, mut batch: Batch, error: Option<String>) -> String {
        self.persist();
        self.emit_language(&mut batch);
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
    extent(node, kernel, node.content)
}

/// A `fit-content` sheet's measure (LLP 1075.003 §9.11): the children's
/// extent and the authored end padding, without Taffy's height, which may
/// count end padding, a native container's bottom cover among it (the
/// sheet's safe area, which UIKit adds below the detent itself).
fn fitted_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    extent(node, kernel, (node.content.0, 0.0))
}

/// `from` floored by the direct children's extent plus the end padding.
fn extent(node: &NodeRef<'_>, kernel: &Kernel, from: (f32, f32)) -> (f32, f32) {
    // Taffy's block containers do not always count end-edge padding in
    // `content_size` (its flex containers do); CSS's `scrollHeight` does.
    // Floor with the direct children's extent plus the end padding.
    let env = kernel.env();
    let pad = |d: exact_kernel::Dimension, against: f32| match d.resolve(&env) {
        exact_kernel::Dimension::Points(p) => p,
        exact_kernel::Dimension::Percent(p) => against * p / 100.0,
        exact_kernel::Dimension::Calc(p, x) => against * p / 100.0 + x,
        exact_kernel::Dimension::Auto
        | exact_kernel::Dimension::Env(..)
        | exact_kernel::Dimension::Segment(..)
        | exact_kernel::Dimension::Viewport(..)
        | exact_kernel::Dimension::Compare(..) => 0.0,
    };
    let pad_right = pad(node.style.padding_right, node.frame.width);
    let pad_bottom = pad(node.style.padding_bottom, node.frame.width);
    let (mut w, mut h) = from;
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
        // Never a view: its `svg`'s scene draws it (LLP 1055 D4).
        NodeType::SvgGroup
        | NodeType::SvgPath
        | NodeType::SvgPolyline
        | NodeType::SvgPolygon
        | NodeType::SvgCircle
        | NodeType::SvgLine
        | NodeType::SvgRect
        | NodeType::SvgEllipse
        | NodeType::SvgViewport
        | NodeType::SvgDefs
        | NodeType::SvgLinearGradient
        | NodeType::SvgRadialGradient
        | NodeType::SvgStop
        | NodeType::SvgUse
        | NodeType::SvgSymbol
        | NodeType::SvgClipPath
        | NodeType::SvgText
        | NodeType::SvgTSpan
        | NodeType::SvgMarker
        | NodeType::SvgMask
        | NodeType::SvgPattern
        | NodeType::SvgForeignObject
        | NodeType::SvgFilter
        | NodeType::SvgFe => "svg-element",
        NodeType::ScrollView => "scroll",
        NodeType::Text => "text",
        NodeType::Image => "image",
        NodeType::TextInput => "input",
        NodeType::Pressable => "button",
        NodeType::Control => "control",
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
    // measured, `text-transform` applied (LLP 1064 D5).
    if let Some(std::borrow::Cow::Owned(shown)) = node.shown_text() {
        out.insert(PropId::Text.name().to_string(), shown);
    }
    style::glass_auto_props(node, &mut out); // LLP 1053.000.000.000 D2
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
                role.strip_prefix("sf/")
                    .or_else(|| exact_kernel::generated::symbol(role).map(|s| s.0))
                    .unwrap_or("")
                    .into(),
            );
        }
    }
    out
}

fn handler_name(e: EventKind) -> Option<&'static str> {
    match e {
        // The runner raises these itself, after collection feedback and
        // after layout: nothing for the presenter to listen for.
        EventKind::Reachstart | EventKind::Reachend | EventKind::Resize => None,
        _ => Some(e.name()),
    }
}
