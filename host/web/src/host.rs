//! The runner wrapped for a DOM: receipts → batches.
//!
//! @ref LLP 1007 §2 (the DOM mirrors the kernel tree)
//!
//! After every commit the host walks the receipt: destroyed keys become
//! `destroy`, created keys become `create` (with the node's tag, props, CSS,
//! and handler kinds), touched keys become `props`/`style`/`children` ops
//! only where the host's per-view cache says something changed. The kernel
//! is the single source of truth; the cache is a memo of what the page has
//! already been told.

use crate::batch::Batch;
use crate::css;
use crate::motion::{Lowered, Motion, Still};
use exact_kernel::{CommitReceipt, Kernel, NodeKey, NodeType, PropId, ViewId};
use exact_motion::{EngineError, HoldEnd, HoldStart, Property, Value as MotionValue};
use exact_plan::{EventKind, Plan, StackMemberKind, StacksId};
use exact_runner::{
    Carried, DataSource, Dispatch, Event, FailureKind, Outcome, RequestOut, Response, Runner,
    RunnerError, SurfaceOutcome, Timed, Work,
};

#[path = "auth.rs"]
mod auth;
#[path = "canvas2d.rs"]
mod canvas2d;
pub mod fonts;
pub(crate) use fonts::{decode_plan, plan_font_catalog};
use fonts::{font_catalog, font_faces, font_names};
#[path = "document.rs"]
pub mod document;
#[path = "element.rs"]
mod element;
#[path = "template.rs"]
pub mod template;
use element::{css_style, host_css, in_button, props_for, svg_props, tag_for};
#[path = "height_drag.rs"]
mod height_drag;
pub use height_drag::HeightDragBinding;
#[path = "flow_host.rs"]
mod flow_host;
mod fold;
#[path = "layers.rs"]
pub mod layers;
#[path = "reorder_drag.rs"]
mod reorder_drag;
#[path = "transform_drag.rs"]
mod transform_drag;
/// A reply as the ABI carries it, as the runner's `Outcome`. Kind 8 is one
/// message of a stream: `event`, `id` and `coalesced` as header lines, the
/// data as the body (LLP 1016.000).
pub fn outcome_from(kind: u32, status: u32, headers: &str, body: Vec<u8>) -> Outcome {
    match kind {
        8 => {
            let field = |name: &str| {
                headers
                    .lines()
                    .find_map(|l| l.strip_prefix(name)?.strip_prefix(": "))
                    .unwrap_or("")
                    .to_string()
            };
            Outcome::Message(exact_runner::Message {
                event: field("event"),
                id: field("id"),
                coalesced: field("coalesced").parse().unwrap_or(0),
                data: String::from_utf8_lossy(&body).into_owned(),
            })
        }
        5 => Outcome::Storage(body),
        6 if body.len() <= exact_runner::MAX_HOST_WORK_BYTES => {
            Outcome::Surface(SurfaceOutcome::Captured(body))
        }
        6 => Outcome::Failed {
            kind: FailureKind::Refused,
            message: "surface capture exceeds 16 MiB".into(),
        },
        7 => Outcome::Surface(SurfaceOutcome::Restored),
        0 => Outcome::Response(Response {
            status: status as u16,
            headers: headers
                .lines()
                .filter_map(|l| {
                    l.split_once(':')
                        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                })
                .collect(),
            body,
        }),
        k => Outcome::Failed {
            kind: match k {
                2 => FailureKind::Refused,
                3 => FailureKind::Unsupported,
                4 => FailureKind::Aborted,
                _ => FailureKind::Network,
            },
            message: String::from_utf8_lossy(&body).into_owned(),
        },
    }
}
use exact_kernel::id::IdMap;
use exact_kernel::{SortedMap, SortedSet};
use std::fmt::Write as _;

/// Why the host refused.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    RuntimeIdExhausted,
    /// The plan uses these capabilities, which this artifact doesn't link
    /// (LLP 1047 D6).
    Unlinked(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Mirror {
    props: SortedMap<String, String>,
    css: String,
    children: Vec<ViewId>,
    /// Created inside a `<button>`, where a container is a `<span>`.
    in_button: bool,
    /// Created with handlers (a text that has some is never folded).
    handled: bool,
}

/// What a host links that is generic over its data source (LLP 1047 D3):
/// drags' hooks into every commit. An entry passes [`HostLinks::of`] its
/// `EXACT_LINKED`, so an app without drags carries none of them; tests and
/// native tools take [`HostLinks::ALL`] through the plain boots.
pub struct HostLinks<D: DataSource> {
    /// Height, transform and reorder handles, tracked and published.
    pub drag: Option<DragHooks<D>>,
    /// The agent API's reads (LLP 1012): `exact_runner::agent::handle`.
    pub inspect: Option<fn(&Runner<D>, &str) -> String>,
    /// The runner's device capabilities (LLP 1069, linked by use).
    pub device: exact_runner::DeviceLinks<D>,
    /// The page's word on an auth session (LLP 1069.006 D4): `auth.rs`.
    pub auth: Option<fn(&mut Host<D>, &str) -> String>,
}

impl<D: DataSource> Clone for HostLinks<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: DataSource> Copy for HostLinks<D> {}

impl<D: DataSource> HostLinks<D> {
    /// Every capability.
    pub const ALL: HostLinks<D> = HostLinks {
        drag: Some(DragHooks::LINKED),
        inspect: Some(exact_runner::agent::handle::<D>),
        device: exact_runner::DeviceLinks::ALL,
        auth: Some(Host::<D>::auth_linked),
    };

    /// The core alone.
    pub const CORE: HostLinks<D> = HostLinks {
        drag: None,
        inspect: None,
        device: exact_runner::DeviceLinks::CORE,
        auth: None,
    };

    /// What `linked` names.
    pub const fn of(linked: crate::Linked) -> HostLinks<D> {
        HostLinks {
            drag: if linked.drag {
                Some(DragHooks::LINKED)
            } else {
                None
            },
            inspect: if linked.inspection {
                Some(exact_runner::agent::handle::<D>)
            } else {
                None
            },
            device: exact_runner::DeviceLinks {
                auth: if linked.auth {
                    Some(exact_runner::AuthLinks::LINKED)
                } else {
                    None
                },
                share: if linked.share {
                    exact_runner::DeviceLinks::<D>::ALL.share
                } else {
                    None
                },
                documents: if linked.documents {
                    exact_runner::DeviceLinks::<D>::ALL.documents
                } else {
                    None
                },
                picker: if linked.picker.is_some() {
                    Some(exact_runner::PickerLinks::LINKED)
                } else {
                    None
                },
            },
            auth: if linked.auth {
                Some(Host::<D>::auth_linked)
            } else {
                None
            },
        }
    }
}

/// Drags' hooks into every commit: a handle is tracked when its node is
/// created or updated and forgotten when destroyed; bindings are reconciled
/// with each receipt and published with each batch.
pub struct DragHooks<D: DataSource> {
    open: fn(&mut Host<D>, &mut Batch),
    receipt: fn(&mut Host<D>, &mut Batch),
    publish: fn(&mut Host<D>, &mut Batch),
    created: fn(&mut Host<D>, ViewId, NodeKey, &[EventKind]),
    updated: fn(&mut Host<D>, ViewId, NodeKey),
    destroyed: fn(&mut Host<D>, ViewId),
    valid_event: fn(&Event) -> bool,
}

impl<D: DataSource> Clone for DragHooks<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: DataSource> Copy for DragHooks<D> {}

impl<D: DataSource> DragHooks<D> {
    const LINKED: DragHooks<D> = DragHooks {
        open: |host, batch| {
            host.reconcile_height_drags(batch);
            (DragHooks::LINKED.publish)(host, batch);
        },
        receipt: |host, batch| {
            host.reconcile_height_drags(batch);
            host.reconcile_transform_drags(batch);
        },
        publish: |host, batch| {
            host.emit_height_drags(batch);
            host.emit_transform_drags(batch);
            host.emit_reorder_drags(batch);
        },
        created: |host, id, key, kinds| {
            let node = host.runner.kernel().node(id).expect("live");
            if node.props.str(PropId::ReorderFor).is_some() {
                host.reorder_drags.track(id, key);
            }
            if kinds.contains(&EventKind::Heightrelease) {
                host.height_drags.insert(id, key);
            }
            if kinds.contains(&EventKind::Transformgeometry)
                || kinds.contains(&EventKind::Transformrelease)
            {
                host.transform_drags.insert(
                    id,
                    key,
                    kinds.contains(&EventKind::Transformgeometry),
                    kinds.contains(&EventKind::Transformrelease),
                );
            }
        },
        updated: |host, id, key| {
            let node = host.runner.kernel().node(id).expect("live");
            if node.props.str(PropId::ReorderFor).is_some() {
                host.reorder_drags.track(id, key);
            }
        },
        destroyed: |host, id| {
            host.height_drags.remove(id);
            host.transform_drags.remove(id);
            host.reorder_drags.remove(id);
        },
        valid_event: transform_drag::valid_event,
    };
}

/// The browser measures and lays out text, so the kernel never asks this.
struct BrowserMeasures;

impl exact_kernel::TextMeasurer for BrowserMeasures {
    fn measure(&mut self, _: &exact_kernel::TextMeasureRequest<'_>) -> exact_kernel::TextMetrics {
        exact_kernel::TextMetrics::default()
    }
}

/// The browser's kernel. The browser lays out: the kernel builds its layout
/// engine's tree only if a layout is ever asked for, and links no text
/// measurer of its own (LLP 1047 §6).
pub(crate) fn browser_kernel() -> Kernel {
    Kernel::on_demand(Box::new(BrowserMeasures))
}

/// One runner, one page.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    mirror: IdMap<ViewId, Mirror>,
    keys: IdMap<NodeKey, ViewId>,
    roots: Vec<ViewId>,
    /// Springs and holds, or [`Still`] when the artifact doesn't link motion.
    springs: Box<dyn Motion>,
    height_drags: height_drag::HeightDrags,
    transform_drags: transform_drag::TransformDrags,
    reorder_drags: reorder_drag::ReorderDrags,
    /// Drags' hooks, when the artifact links them.
    drag: Option<DragHooks<D>>,
    /// The agent API's reads, when the artifact links inspection.
    inspect: Option<fn(&Runner<D>, &str) -> String>,
    /// The page's word on an auth session, when the artifact links auth.
    auth_word: Option<fn(&mut Host<D>, &str) -> String>,
    /// The page's clock at the last call, milliseconds from script start.
    now_ms: f64,
    /// Stack id → opaque CSS family name, scoped to this plan.
    font_names: Vec<String>,
    /// The plan-owned face catalog, queried separately from op batches.
    font_catalog: String,
    exclusions: SortedSet<ViewId>,
    textflow: String,
    /// Requests whose continuation a source held at dispatch (LLP 1027.002
    /// D3): released after a later commit, by token.
    parked: SortedMap<u64, RequestOut>,
    /// `exact-auth:` requests handed to the page, until it asks (`auth.rs`).
    auth_out: Vec<RequestOut>,
    location: String,
    collections: String,
    /// Head nodes (LLP 1048.003 D1): the page's `<head>`, never an element.
    heads: SortedSet<ViewId>,
    /// The head the page was last told, and whether a commit may move it.
    head: exact_runner::Head,
    language: Option<String>,
    head_dirty: bool,
    /// While the first batch is made: what a page's projection computed for
    /// each view (LLP 1048.000 D6), taken instead of computing it again.
    computed: document::Computed,
    /// `@keyframes` rules already in the page's stylesheet (LLP 1055 D7).
    keyframes: SortedSet<String>,
    /// The 2D canvases the page watches (LLP 1056 D4).
    canvas2d: canvas2d::Watch,
    /// Which views the page makes `relative` (LLP 1001 §1).
    layers: layers::Layers,
}

impl<D: DataSource> Host<D> {
    /// Boot from plan bytes: decode (a validation pass), boot the runner, and
    /// produce the first batch, which creates the whole tree.
    pub fn boot(
        plan_bytes: &[u8],
        data: D,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_delivered(plan_bytes, data, None, Vec::new(), None, viewport, launch)
    }

    /// Whether `location` names a pattern the plan's route table declares —
    /// never only its notfound fallback. The page follows a same-origin link
    /// to one in place (LLP 1038 §7) instead of loading a document.
    pub fn route_matches(&self, location: &str) -> bool {
        self.runner.route_matches(location)
    }

    /// Boot with the page's snapshot of the app's kept secrets (LLP 1018
    /// D6): what `localStorage` holds under `exact.secret.<name>`, read by
    /// the glue before boot; the runner keeps the granted names.
    pub fn boot_stored(
        plan_bytes: &[u8],
        data: D,
        snapshot: Vec<(String, String)>,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_delivered(plan_bytes, data, None, snapshot, None, viewport, launch)
    }

    /// Boot carrying an earlier host's state (the dev loop's reload, LLP
    /// 1007 §6): slots by name where their types still fit, settled
    /// resources where their arguments still match, the clock. Carried state
    /// is never why a boot fails — what no longer fits starts fresh.
    pub fn boot_with(
        plan_bytes: &[u8],
        data: D,
        carried: Option<&Carried>,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_delivered(
            plan_bytes,
            data,
            carried,
            Vec::new(),
            None,
            viewport,
            launch,
        )
    }

    /// Boot knowing what this wasm was built as (LLP 1030 D7): `compat` is
    /// the archive's `compat.json`, which the `delivery` resource answers
    /// from. The other three boots are this one with nothing to say.
    pub fn boot_delivered(
        plan_bytes: &[u8],
        data: D,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        compat: Option<&str>,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_linked(
            HostLinks::ALL,
            plan_bytes,
            data,
            carried,
            snapshot,
            compat,
            viewport,
            launch,
        )
    }

    /// [`Host::boot_delivered`], with what the artifact links (LLP 1047 D3).
    #[allow(clippy::too_many_arguments)]
    pub fn boot_linked(
        links: HostLinks<D>,
        plan_bytes: &[u8],
        data: D,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        compat: Option<&str>,
        viewport: exact_runner::Viewport,
        launch: &str,
    ) -> Result<(Host<D>, String), HostError> {
        let plan = decode_plan(plan_bytes).map_err(HostError::Plan)?;
        crate::link::admit(&plan)?;
        let kernel = browser_kernel();
        // @ref LLP 1039 D3 — both host facts precede the first settlement.
        let delivery = compat.map_or_else(Default::default, |json| {
            exact_runner::Delivery::default().with_compat(json)
        });
        let runner = Runner::boot_with_delivery_linked(
            crate::link::runner_links(),
            plan,
            data,
            kernel,
            carried,
            snapshot,
            delivery,
            viewport,
            launch,
        )
        .map_err(HostError::Runner)?;
        Host::open(links, runner, launch, Batch::new(), Default::default())
    }

    /// The host over a booted runner, and its first batch — `batch`'s ops,
    /// then everything live, new to the page.
    fn open(
        links: HostLinks<D>,
        runner: Runner<D>,
        launch: &str,
        mut batch: Batch,
        computed: document::Computed,
    ) -> Result<(Host<D>, String), HostError> {
        let mut runner = runner;
        runner.set_device_links(links.device);
        let font_names = font_names(runner.plan());
        let font_catalog = font_catalog(&font_faces(runner.plan()));
        let mut host = Host {
            runner,
            mirror: Default::default(),
            keys: Default::default(),
            roots: Vec::new(),
            springs: crate::link::linked().motion.map_or_else(
                || Box::new(Still::default()) as Box<dyn Motion>,
                |springs| springs(),
            ),
            height_drags: height_drag::HeightDrags::default(),
            transform_drags: transform_drag::TransformDrags::new()?,
            reorder_drags: reorder_drag::ReorderDrags::new()?,
            drag: links.drag,
            inspect: links.inspect,
            auth_word: links.auth,
            now_ms: 0.0,
            font_names,
            font_catalog,
            parked: SortedMap::new(),
            auth_out: Vec::new(),
            location: launch.into(),
            collections: String::new(),
            exclusions: Default::default(),
            textflow: String::new(),
            heads: Default::default(),
            head: Default::default(),
            language: None,
            head_dirty: false,
            computed,
            keyframes: Default::default(),
            canvas2d: Default::default(),
            layers: Default::default(),
        };
        // Everything live is new to the page.
        let roots = host.runner.roots();
        let mut stack: Vec<ViewId> = roots.iter().rev().copied().collect();
        let mut order = Vec::new();
        while let Some(id) = stack.pop() {
            order.push(id);
            let node = host.runner.kernel().node(id).expect("live");
            let mut children = node.children();
            children.reverse();
            stack.extend(children);
        }
        host.relayer(&order, &mut batch);
        let handlers = host.runner.handlers();
        for id in &order {
            host.create(*id, &mut batch, handlers.get(id).map_or(&[], Vec::as_slice));
        }
        host.computed = Default::default();
        for id in &order {
            host.emit_children(*id, &mut batch);
        }
        host.springs.adopt(host.runner.kernel(), &order);
        // A boot's consumers are sought as a commit's are (LLP 1057.003 D4).
        if host.runner.kernel().has_timelines() {
            batch.timelines();
        }
        let roots = host.page_roots();
        host.roots = roots.clone();
        batch.roots(&roots);
        host.emit_head(&mut batch);
        if let Some(drag) = host.drag {
            (drag.open)(&mut host, &mut batch);
        }
        // Surfaces after roots: the canvas is in the page when its surface is made.
        for s in host.runner.take_surface_updates() {
            batch.surface(&s);
        }
        host.canvas_turn(&mut batch);
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = host.runner.take_router_change() {
            host.location = change.url.clone();
            batch.router(&change);
        }
        for c in host.runner.take_commands() {
            batch.command(&c.name, &c.args, c.source);
        }
        for w in host.runner.take_store_writes() {
            batch.store(&w);
        }
        batch.grants(host.runner.data().grants());
        host.emit_textflow(&mut batch);
        let batch = host.complete(batch, None);
        Ok((host, batch))
    }

    /// The latest top URL, also the module re-boot's launch fact.
    /// @ref LLP 1038 D5/D7 — a replacement keeps the host's current location.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// The runner.
    pub fn runner(&self) -> &Runner<D> {
        &self.runner
    }

    /// `tap @t` or `type @t` (LLP 1069.007 D4): the agent answers a held
    /// device request, consumed here. The one capability admitted, `share`, has nothing to
    /// deliver but the journal line the runner writes (LLP 1069.003 D6);
    /// `None` for an ordinary `tap` or `type`.
    pub fn answer_hold(&mut self, request: &str) -> Option<String> {
        exact_runner::agent::answer(&mut self.runner, request).map(|(reply, _)| reply)
    }

    /// The runner, mutably — for tests that drive it past the host.
    pub fn runner_mut(&mut self) -> &mut Runner<D> {
        &mut self.runner
    }

    /// The current plan's declared face catalog for the host-owned web
    /// readiness barrier. This is plan data, never a transient op batch.
    pub fn font_catalog(&self) -> &str {
        &self.font_catalog
    }

    /// Deliver an event at the page's clock (milliseconds from script
    /// start); the batch makes the page equal to the tree after the commit,
    /// and any spring the change releases is in it as frames. A refusal is
    /// reported in the batch's `error`, and the page is untouched (as the
    /// kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> String {
        if self.drag.is_some_and(|drag| !(drag.valid_event)(&event)) {
            return self.finish(Batch::new(), Some("invalid transform event"));
        }
        if let Event::HeightRelease { height, velocity } = &event {
            if !height.is_finite()
                || !(0.0..=f32::MAX as f64).contains(height)
                || !velocity.is_finite()
            {
                return self.finish(Batch::new(), Some("invalid height release"));
            }
        }
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.dispatch(view, event) {
            Ok(receipt) => {
                let at_ms = self.now_ms;
                self.batch_for(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => self.batch_for(&[], Some(&format!("{e:?}"))),
        }
    }

    /// [`Host::dispatch_at`] at the clock's last value.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> String {
        self.dispatch_at(view, event, self.now_ms)
    }

    /// What a reload keeps (`Runner::carry`).
    pub fn carry(&self) -> Carried {
        self.runner.carry()
    }

    /// Springs and holds: presentation values as the page shows them.
    pub fn springs(&self) -> &dyn Motion {
        self.springs.as_ref()
    }

    /// Register the one numeric-height trial owner (or clear it). Registration
    /// adopts the current authored target without animating from an invented
    /// zero height. Invalid replacement preserves the previous hold and clock.
    /// The returned batch retires old DOM ownership before lowering new work.
    pub fn set_height_owner(&mut self, view: Option<ViewId>) -> Result<String, &'static str> {
        let previous = self.springs.height_owner();
        let retired = self.springs.set_height_owner(self.runner.kernel(), view)?;
        if previous == self.springs.height_owner() {
            // Same live registration preserves its provenance and pending work.
            return Ok(self.finish(Batch::new(), None));
        }
        self.height_drags.programmatic();
        let mut batch = Batch::new();
        for item in retired {
            if let Lowered::Retire { view, property } = item {
                batch.retire_motion(view, property.name());
            }
        }
        // Explicit clearing publishes unbound handles now. A later receipt may
        // auto-admit authored handles again; this setter must not undo itself.
        self.cancel_invalid_height_drag();
        self.emit_springs(&mut batch, &[], self.now_ms / 1000.0);
        self.emit_height_drags(&mut batch);
        Ok(self.finish(batch, None))
    }

    /// The page's line for the runner's journal (LLP 1012 §3): a refused
    /// intent and its reason.
    pub fn log(&mut self, line: &str) {
        self.runner.log(line);
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, and
    /// `logs` from the runner; `settle` — the clock at which the last spring
    /// in flight ends, milliseconds, `null` when none — from the engine here.
    /// CSS transitions are the browser's; the glue folds their end times in.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self.springs.settle_time() {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t * 1000.0)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        match self.inspect {
            Some(inspect) => inspect(&self.runner, request),
            None => exact_runner::agent::error("this build links no inspection (LLP 1047 D6)"),
        }
    }

    /// Move the clock; every timer due fires at its own time; one batch for
    /// all of them, each commit's ops behind an `at` marker carrying the
    /// time it was made, so a page that owns time attributes the transitions
    /// they start to that instant (LLP 1012; LLP 1002 D3). A timer's refusal
    /// stops the clock there: the commits before it are in the batch, the
    /// refusal in `error`, and `clock` says where the runner stands.
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
        // LLP 1056 D5: a canvas that asked for a frame draws at the landed time.
        self.runner.canvas_frame();
        let error = a.error.map(|e| format!("{e:?}"));
        self.batch_for(&a.receipts, error.as_deref())
    }

    /// Layout viewport changes re-answer the app in the same returned batch.
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
        self.batch_for(&receipts, error.as_deref())
    }

    /// The size and the display preferences, in one batch.
    /// @ref LLP 1039 D2; LLP 1061 D4
    pub fn resize(&mut self, viewport: exact_runner::Viewport, now_ms: f64) -> String {
        let a = self.runner.advance_timed(now_ms);
        self.now_ms = a.now_ms.max(self.now_ms);
        let mut receipts = a.receipts;
        let mut error = a.error.map(|e| format!("{e:?}"));
        let answers = [
            self.runner.set_viewport(viewport.width, viewport.height),
            self.runner.set_preferences(viewport.preferences),
        ];
        for answer in answers {
            match answer {
                Ok(Some(receipt)) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Ok(None) => {}
                Err(e) => {
                    let viewport_error = format!("viewport: {e:?}");
                    error = Some(match error {
                        Some(timer_error) => format!("{timer_error}; {viewport_error}"),
                        None => viewport_error,
                    });
                }
            }
        }
        self.batch_for(&receipts, error.as_deref())
    }

    /// The date (LLP 1027.000.000): re-answer `exactTime` in one batch.
    pub fn set_time(&mut self, epoch_at_zero: f64, utc_offset: f64) -> String {
        let (receipts, error) = match self.runner.set_time(epoch_at_zero, utc_offset) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("time: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }

    /// The page's visibility, connectivity or share sheet (LLP 1069.000 D2):
    /// the `exactPage()` readers, in one batch.
    pub fn set_page(&mut self, page: exact_runner::Page) -> String {
        let (receipts, error) = match self.runner.set_page(page) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("page: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }

    /// The root font size `rem` follows (LLP 1069.000 D3): a relayout in one
    /// batch, no resource asked again.
    pub fn set_root_font_size(&mut self, px: f64) -> String {
        let (receipts, error) = match self.runner.set_root_font_size(px) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("root font size: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }

    /// The page module says `topic` changed (LLP 1016.002): the resources
    /// watching it are asked again, in one batch.
    pub fn changed(&mut self, topic: &str) -> String {
        let (receipts, error) = match self.runner.changed(topic) {
            Ok(Some(receipt)) => (
                vec![Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => (vec![], None),
            Err(e) => (vec![], Some(format!("changed: {e:?}"))),
        };
        self.batch_for(&receipts, error.as_deref())
    }

    /// The viewer's locale and zone, beside the date.
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
        self.batch_for(&receipts, error.as_deref())
    }

    /// Actual nested scrollport and mounted row geometry, in the shared LE wire
    /// format. Edge actions may settle resources; geometry never advances timers.
    pub fn collection_feedback(&mut self, bytes: &[u8]) -> String {
        match self.runner.collection_feedback_bytes(bytes) {
            Ok(mut result) => {
                for timed in &mut result.receipts {
                    timed.at_ms = self.now_ms;
                }
                let error = result.error.map(|e| format!("collection: {e:?}"));
                let mut batch = Batch::new();
                batch.accept_collection();
                self.batch_from(batch, &result.receipts, error.as_deref())
            }
            Err(error) => self.finish(Batch::new(), Some(&format!("collection: {error:?}"))),
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
                let at_ms = self.runner.now_ms();
                self.batch_for(&[Timed { at_ms, receipt }], None)
            }
            Err(error) => self.batch_for(&[], Some(&format!("scrollIntoView: {error:?}"))),
        }
    }

    /// Activate deferred logic after the page's first rendering opportunity.
    pub fn data_ready(&mut self) -> String {
        if let Err(error) = self.runner.data().activate() {
            return self.batch_for(&[], Some(&format!("module: {error:?}")));
        }
        match self.runner.data_ready() {
            Ok(Some(receipt)) => self.batch_for(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.batch_for(&[], None),
            Err(error) => self.batch_for(&[], Some(&format!("data ready: {error:?}"))),
        }
    }

    fn batch_for(&mut self, receipts: &[Timed], error: Option<&str>) -> String {
        self.batch_from(Batch::new(), receipts, error)
    }

    fn batch_from(&mut self, mut batch: Batch, receipts: &[Timed], error: Option<&str>) -> String {
        self.emit_receipts(receipts, &mut batch);
        self.complete(batch, error.map(str::to_string))
    }

    fn emit_receipts(&mut self, receipts: &[Timed], batch: &mut Batch) {
        // Which views are `relative` in the tree the receipts end at (layers.rs).
        let changed: Vec<ViewId> = (receipts.iter())
            .flat_map(|t| t.receipt.created.iter().chain(&t.receipt.touched))
            .filter_map(|key| self.runner.kernel().node_by_key(*key).map(|n| n.id))
            .collect();
        self.relayer(&changed, batch);
        for t in receipts {
            let r = &t.receipt;
            batch.at(t.at_ms);
            // Before the destroys that follow it (LLP 1063): the page reads
            // the leaving view's geometry before any op of the batch moves it.
            for exit in &r.exits {
                let link = crate::link::linked().animations;
                if let (Some(id), Some(link)) = (self.keys.get(&exit.key), link) {
                    let press = self
                        .runner
                        .kernel()
                        .node(*id)
                        .is_some_and(|n| crate::css::press_composes(n.style));
                    batch.exit(*id, &(link.list)(&exit.animations, press));
                }
            }
            for key in &r.destroyed {
                if let Some(id) = self.keys.remove(key) {
                    if self.heads.remove(&id) {
                        self.head_dirty = true;
                        continue;
                    }
                    self.mirror.remove(&id);
                    self.layers.forget(id);
                    self.exclusions.remove(&id);
                    if let Some(drag) = self.drag {
                        (drag.destroyed)(self, id);
                    }
                    batch.destroy(id);
                }
            }
            let handlers = if r.created.is_empty() {
                SortedMap::new()
            } else {
                self.runner.handlers()
            };
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    self.create(id, batch, handlers.get(&id).map_or(&[], Vec::as_slice));
                }
            }
            // `touched` excludes created nodes (the kernel's receipt contract),
            // so walk it alone: a search of it per key was quadratic.
            for key in &r.touched {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    self.update(id, batch);
                }
            }
            // This commit's springs, at its own time: the style (the target)
            // is in the page before the frames that approach it start playing.
            // Adopt this receipt's time/targets/transitions while the hold is
            // live, then cancel invalid bindings and lower dirty frames once.
            let synced = self.springs.synchronize(
                self.runner.kernel(),
                std::slice::from_ref(r),
                t.at_ms / 1000.0,
            );
            Self::emit_lowered(batch, synced);
            if let Some(drag) = self.drag {
                (drag.receipt)(self, batch);
            }
            self.emit_springs(batch, &[], t.at_ms / 1000.0);
        }
        // Earlier receipts also read the final tree, whose children can be
        // created by a later receipt in this seek. Attach only after all creates.
        for t in receipts {
            for key in t.receipt.created.iter().chain(t.receipt.touched.iter()) {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let (id, parent) = (node.id, node.parent);
                    self.emit_children(id, batch);
                    for box_ in std::iter::once(id).chain(parent) {
                        self.refold(box_, batch);
                    }
                }
            }
        }
        let roots = self.page_roots();
        if roots != self.roots {
            self.roots = roots.clone();
            batch.roots(&roots);
        }
        self.emit_head(batch);
        if let Some(drag) = self.drag.filter(|_| !receipts.is_empty()) {
            (drag.publish)(self, batch);
        }
        if !receipts.is_empty() {
            self.emit_textflow(batch);
        }
        // A canvas's inputs (LLP 1009 D2): the runner's side-output, only
        // from commits that applied.
        for s in self.runner.take_surface_updates() {
            batch.surface(&s);
        }
        self.canvas_turn(batch);
        // @ref LLP 1038 D7 — drain once, after all commits in this batch.
        if let Some(change) = self.runner.take_router_change() {
            self.location = change.url.clone();
            batch.router(&change);
        }
        for c in self.runner.take_commands() {
            batch.command(&c.name, &c.args, c.source);
        }
        // What the commit kept or forgot (LLP 1018 D1), for the page to persist.
        for w in self.runner.take_store_writes() {
            batch.store(&w);
        }
    }

    /// Hand the last commit's requests to the page and finish the batch. A
    /// continuation is dispatched here, on this thread, with the store as
    /// committed (LLP 1027.002 D3); one a source holds is parked and
    /// released after a later commit. Work a source answers at once — a
    /// main member's turn in an ordered set — is fulfilled here, and the
    /// commits it makes join the batch.
    fn complete(&mut self, mut batch: Batch, error: Option<String>) -> String {
        let mut error = error;
        loop {
            let mut immediate = Vec::new();
            for r in self.runner.take_requests() {
                self.emit_request(r, &mut batch, &mut immediate);
            }
            for (token, dispatch) in self.runner.release_work() {
                if let Some(r) = self.parked.remove(&token) {
                    self.emit_dispatch(r, dispatch, &mut batch, &mut immediate);
                }
            }
            if immediate.is_empty() || error.is_some() {
                break;
            }
            let mut receipts = Vec::new();
            for (ticket, outcome) in immediate {
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
            self.emit_receipts(&receipts, &mut batch);
        }
        let collections = self.runner.collections_json();
        if collections != self.collections {
            self.collections = collections;
            batch.collections(&self.collections);
        }
        let language = self.runner.resolved_locale();
        if self.language.as_deref() != Some(language) {
            batch.language(language, self.runner.direction());
            self.language = Some(language.into());
        }
        self.finish(batch, error.as_deref())
    }

    /// Close a batch with the runner's deadline, whether it wants each
    /// animation frame (LLP 1073 D5), and its clock.
    pub(crate) fn finish(&self, batch: Batch, error: Option<&str>) -> String {
        let due = self.runner.timer_due_ms();
        batch.finish(due, self.runner.wants_frames(), self.runner.now_ms(), error)
    }

    fn emit_request(
        &mut self,
        r: RequestOut,
        batch: &mut Batch,
        immediate: &mut Vec<(u64, Outcome)>,
    ) {
        if let Some(message) = crate::batch::request_refusal(&r.request) {
            batch.refuse(r.ticket, message);
            return;
        }
        if r.request.is_auth() {
            return self.emit_auth(r, batch);
        }
        let Some(token) = r.request.continuation else {
            batch.request(&r);
            return;
        };
        let dispatch = self.runner.dispatch_work(token);
        self.emit_dispatch(r, dispatch, batch, immediate);
    }

    fn emit_dispatch(
        &mut self,
        mut r: RequestOut,
        dispatch: Dispatch,
        batch: &mut Batch,
        immediate: &mut Vec<(u64, Outcome)>,
    ) {
        match dispatch {
            Dispatch::Host(registry) => {
                r.request.continuation = Some(registry);
                batch.request(&r);
            }
            Dispatch::Run(Work::Now(work)) => immediate.push((r.ticket, work())),
            Dispatch::Run(Work::Later(_)) => immediate.push((
                r.ticket,
                Outcome::Failed {
                    kind: FailureKind::Unsupported,
                    message: "an owner thread is unavailable on this host".into(),
                },
            )),
            Dispatch::Held => {
                if let Some(token) = r.request.continuation {
                    self.parked.insert(token, r);
                }
            }
            Dispatch::Missing => immediate.push((
                r.ticket,
                Outcome::Failed {
                    kind: FailureKind::Unsupported,
                    message: "missing or consumed browser continuation".into(),
                },
            )),
        }
    }

    /// Capture a live browser presentation. Reply includes cancellation and
    /// any other properties advanced by the same engine clock.
    /// At this boundary `InvalidValueShape` also refuses Height positions
    /// outside the native layout range 0..=f32::MAX, before clock mutation.
    pub fn begin_hold(
        &mut self,
        view: ViewId,
        property: Property,
        presented: MotionValue,
        now_ms: f64,
    ) -> Result<Option<(HoldStart, String)>, EngineError> {
        let Some(start) = self.springs.begin_hold(
            self.runner.kernel(),
            view,
            property,
            presented,
            now_ms / 1000.0,
        )?
        else {
            return Ok(None);
        };
        self.now_ms = now_ms;
        let mut batch = Batch::new();
        self.reconcile_transform_drags(&mut batch);
        self.emit_springs(&mut batch, &[], now_ms / 1000.0);
        batch.animate(
            view,
            property.name(),
            0.0,
            0.0,
            &[],
            property == Property::Translate,
        );
        Ok(Some((start, self.finish(batch, None))))
    }

    /// Check before any action, clock change, or presentation mutation.
    pub fn has_hold(&self, serial: u64) -> bool {
        self.height_hold_valid(serial)
            && self.springs.token(serial).is_some_and(|token| {
                self.runner
                    .kernel()
                    .node_by_key(NodeKey {
                        index: token.node() as u32,
                        generation: (token.node() >> 32) as u32,
                    })
                    .is_some()
            })
    }

    /// Apply an input sample and drain common lowering once; no timer advance.
    /// Height position outside 0..=f32::MAX is `InvalidValueShape`; stale tokens
    /// are refused before that validation. Release velocity remains signed.
    pub fn update_hold(
        &mut self,
        serial: u64,
        value: MotionValue,
        now_ms: f64,
    ) -> Result<Option<String>, EngineError> {
        self.validate_height_delivery(serial);
        if !self.has_hold(serial) || !self.springs.update_hold(serial, value, now_ms / 1000.0)? {
            return Ok(None);
        }
        Ok(Some(self.hold_batch(now_ms)))
    }

    /// Record what the display shows for a live hold (LLP 1057.001 §3).
    pub fn track_hold(&mut self, serial: u64, shown: exact_motion::Value, now_ms: f64) -> bool {
        self.springs.track_hold(serial, now_ms / 1000.0, shown)
    }

    /// Release at the velocity the engine measured over the hold's values,
    /// for input the platform gives no velocity (LLP 1057.001 §3).
    pub fn end_hold_measured(
        &mut self,
        serial: u64,
        now_ms: f64,
    ) -> Result<Option<String>, EngineError> {
        let velocity = self
            .springs
            .hold_velocity(serial, now_ms / 1000.0)
            .filter(|v| v.x.is_finite() && v.y.is_finite())
            .unwrap_or(exact_motion::Value::ZERO);
        self.end_hold(serial, HoldEnd::Release { velocity }, now_ms)
    }

    /// Return to the latest authored target, even without a kernel receipt.
    pub fn end_hold(
        &mut self,
        serial: u64,
        end: HoldEnd,
        now_ms: f64,
    ) -> Result<Option<String>, EngineError> {
        self.validate_height_delivery(serial);
        if !self.has_hold(serial) || !self.springs.end_hold(serial, end, now_ms / 1000.0)? {
            return Ok(None);
        }
        self.transform_member_ended(serial);
        Ok(Some(self.hold_batch(now_ms)))
    }

    fn hold_batch(&mut self, now_ms: f64) -> String {
        self.now_ms = now_ms;
        let mut batch = Batch::new();
        // Geometry-only invalidation has no authored receipt. Seek while held
        // before cancellation, just as batch_for does for accepted receipts.
        let synced = self
            .springs
            .synchronize(self.runner.kernel(), &[], now_ms / 1000.0);
        Self::emit_lowered(&mut batch, synced);
        self.reconcile_transform_drags(&mut batch);
        self.emit_springs(&mut batch, &[], now_ms / 1000.0);
        self.finish(batch, None)
    }

    /// Complete the authored swipe while its translate hold still owns the
    /// live node. An action may destroy that node; its later end is then stale.
    pub fn dispatch_held(&mut self, serial: u64, now_ms: f64) -> Option<String> {
        if !self.has_hold(serial) || !now_ms.is_finite() || now_ms / 1000.0 < self.springs.now() {
            return None;
        }
        let token = self.springs.token(serial)?;
        if token.property() != Property::Translate {
            return None;
        }
        let view = self
            .runner
            .kernel()
            .node_by_key(NodeKey {
                index: token.node() as u32,
                generation: (token.node() >> 32) as u32,
            })?
            .id;
        Some(self.dispatch_at(view, Event::Swiperight, now_ms))
    }

    /// The page brought back request `ticket`'s outcome (LLP 1016 D2):
    /// `kind` 0 is a response with `status`, `headers` as `name: value`
    /// lines, and `body`; 1–4 are failures, 5 is storage, 6/7 are a
    /// captured/restored surface, and 8 is a stream's message. The batch is the commit
    /// the reply made — or nothing, for a ticket no longer held.
    pub fn fulfill_at(
        &mut self,
        ticket: u64,
        kind: u32,
        status: u32,
        headers: &str,
        body: Vec<u8>,
        now_ms: f64,
    ) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        // Kind 9: the answer the runner settled for an auth session (`auth.rs`).
        let outcome = match kind {
            9 => match exact_runner::auth::take_settled(&mut self.runner, ticket) {
                Some(outcome) => outcome,
                None => return self.batch_for(&[], None),
            },
            _ => outcome_from(kind, status, headers, body),
        };
        match self.runner.fulfill(ticket, outcome) {
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                self.batch_for(&[Timed { at_ms, receipt }], None)
            }
            Ok(None) => self.batch_for(&[], None),
            Err(e) => self.batch_for(&[], Some(&format!("{e:?}"))),
        }
    }

    fn emit_springs(&mut self, batch: &mut Batch, receipts: &[CommitReceipt], now_s: f64) {
        let lowered = self.springs.commit(self.runner.kernel(), receipts, now_s);
        Self::emit_lowered(batch, lowered);
    }

    fn emit_lowered(batch: &mut Batch, lowered: Vec<Lowered>) {
        for lowered in lowered {
            match lowered {
                Lowered::Start {
                    view,
                    property,
                    at,
                    delay,
                    duration,
                    values,
                } => {
                    let pairs: Vec<(f64, f64)> = values.iter().map(|v| (v.x, v.y)).collect();
                    batch.spring(
                        at * 1000.0,
                        view,
                        property.name(),
                        (delay * 1000.0, duration * 1000.0),
                        &pairs,
                    );
                }
                Lowered::Cancel { view, property } => {
                    batch.animate(view, property.name(), 0.0, 0.0, &[], false);
                }
                Lowered::Retire { view, property } => {
                    batch.retire_motion(view, property.name());
                }
                Lowered::Timelines => batch.timelines(),
            }
        }
    }

    fn create(&mut self, id: ViewId, batch: &mut Batch, kinds: &[EventKind]) {
        let node = self.runner.kernel().node(id).expect("live");
        if node.node_type.is_metadata() {
            let key = node.key;
            self.keys.insert(key, id);
            self.heads.insert(id);
            self.head_dirty = true;
            return;
        }
        self.track_exclusion(id);
        let node = self.runner.kernel().node(id).expect("live");
        let key = node.key;
        if let Some(drag) = self.drag {
            (drag.created)(self, id, key, kinds);
        }
        let node = self.runner.kernel().node(id).expect("live");
        // An exit's rules too, while its node lives (LLP 1063 D7).
        crate::css::send_keyframes(&mut self.keyframes, node.style, batch);
        let in_button = in_button(self.runner.kernel(), &node);
        let tag = tag_for(&node, in_button);
        let kept = match self.computed.last() {
            Some((view, ..)) if *view == id => self.computed.pop(),
            _ => None,
        };
        let (props, css) = match kept {
            // The projection's, for this view of this tree: the same values.
            Some((_, kept, props, css)) if kept == tag => (props, css),
            _ => {
                let kernel = self.runner.kernel();
                let (css, _skipped) = css::css_text(&css_style(kernel, &node), &self.font_names);
                let mut props = props_for(&node);
                svg_props(kernel, &node, &mut props);
                let css = element::contents(
                    host_css(&node, css, tag),
                    element::folded(kernel, &node, !kinds.is_empty()),
                );
                let handled = |c| self.mirror.get(&c).is_some_and(|m| m.handled);
                let css = element::blocks(css, element::holds_folded(kernel, &node, &handled));
                (props, layers::with_isolation(css, self.layers.isolated(id)))
            }
        };
        let handlers: Vec<&str> = kinds
            .iter()
            .filter(|e| !matches!(e, EventKind::Reachstart | EventKind::Reachend))
            .map(|e| e.name())
            .collect();
        let pairs: Vec<(&str, String)> =
            props.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        batch.create(id, tag, &pairs, &css, &handlers);
        self.mirror.insert(
            id,
            Mirror {
                props,
                css,
                children: Vec::new(),
                in_button,
                handled: !kinds.is_empty(),
            },
        );
        self.keys.insert(key, id);
    }

    fn update(&mut self, id: ViewId, batch: &mut Batch) {
        // A head's fields, or a route selection that may hide one, moved.
        if self.heads.contains(&id) {
            self.head_dirty = true;
            return;
        }
        if !self.heads.is_empty() {
            let props = self.runner.kernel().node(id).expect("live").props;
            if props.str(PropId::NavigationBack).is_some()
                || props.str(PropId::NavigationKey).is_some()
            {
                self.head_dirty = true;
            }
        }
        self.track_exclusion(id);
        if let Some(drag) = self.drag {
            let key = self.runner.kernel().node(id).expect("live").key;
            (drag.updated)(self, id, key);
        }
        let node = self.runner.kernel().node(id).expect("live");
        // An exit's rules too, while its node lives (LLP 1063 D7).
        crate::css::send_keyframes(&mut self.keyframes, node.style, batch);
        // @ref LLP 1053.000 D4 — a computed name the table lacks draws
        // ultra-thin, and says so once.
        let note = node
            .props
            .str(PropId::BackgroundMaterial)
            .zip(crate::link::linked().materials)
            .and_then(|(name, material)| (material.1)(name));
        let mut props = props_for(&node);
        svg_props(self.runner.kernel(), &node, &mut props);
        let css = self.view_css(&node);
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
        if css != m.css {
            batch.style(id, &css);
            m.css = css;
        }
        if let Some(note) = note {
            self.runner.log(note);
        }
    }

    fn emit_children(&mut self, id: ViewId, batch: &mut Batch) {
        if self.heads.contains(&id) {
            return;
        }
        let mut children = self.runner.kernel().node(id).expect("live").children();
        children.retain(|child| !self.heads.contains(child));
        let m = self.mirror.entry(id).or_default();
        if children != m.children {
            batch.children(id, &children);
            m.children = children;
        }
    }
}

impl<D: DataSource> Host<D> {
    /// The roots the page holds: every root but a head.
    fn page_roots(&self) -> Vec<ViewId> {
        let mut roots = self.runner.roots();
        roots.retain(|root| !self.heads.contains(root));
        roots
    }

    /// The page's `<head>`, when a commit may have moved it (LLP 1048.003
    /// D1): the runner's active head, sent only when it changed.
    fn emit_head(&mut self, batch: &mut Batch) {
        if !std::mem::take(&mut self.head_dirty) {
            return;
        }
        let head = self.runner.head();
        if head != self.head {
            batch.head(&head);
            self.head = head;
        }
    }
}
