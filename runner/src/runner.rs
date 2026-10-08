//! The runner: boot, actions, events, resources, timers, the clock.
//!
//! @ref LLP 1004 D3 (the refusal tuple) / D4 (the data seam) / D5 (restart)
//!
//! Each event settles derives and changed resource arguments, evaluates all
//! sites, and applies one atomic kernel batch; hosts then lay out and paint.
//! Kernel validation precedes every write; a refusal leaves the kernel untouched.

mod admission;
mod background;
mod commit;
mod control;
mod event;
pub mod faults;
mod field;
pub use field::{FieldSelection, SelectionDirection};
mod host_kinds;
mod pointer;
pub use pointer::{DropEvent, PointerEvent, WheelEvent};
mod reorder;
mod reorder_codec;
mod reorder_group;
mod resize;
pub use resize::{ResizeRect, UNDELIVERED as RESIZE_UNDELIVERED};
mod root_font;
pub use event::{
    ActionBinding, ActionBindingError, ActionBindingRefusal, ControlValue, Event, KeyModifiers,
    KeyboardEvent, ScrollEvent,
};
mod canvas2d;
pub use canvas2d::{
    engine as canvas_engine, CanvasEngine, CanvasList, DrawReply, DrawRequest, Drawn, Geometry,
    Limits,
};
mod carry;
mod checkpoint;
mod collection;
mod source;
pub use source::{
    Announce, BackgroundState, DataError, DataSource, InFlight, Interrupt, Native, NativeCall,
    NativeHandler, PreloadWake, Target, BACKGROUND,
};
mod delivery;
mod links;
pub use links::{CanvasLink, FormatLink, GeometryLink, RouterLink, RunnerLinks, SurfaceAnswer};
mod device;
mod device_links;
mod document;
pub use device::{Hold, HoldAnswer};
pub use device_links::{AuthLinks, DeviceLinks, PickerLinks};
pub mod picker;
pub use picker::{Picked, PickerRequest, PICKED};
mod fragments;
mod gates;
mod into_view;
mod kept;
#[cfg(test)]
mod kept_tests;
mod lines;
mod lists;
mod media_session;
mod page;
mod perf;
mod query;
mod queue;
pub use queue::QUEUE_BOUND;
pub mod router;
pub use lists::ListTextPosition;
mod settlement;
mod surface_record;
mod time;
mod viewport;
pub use carry::Carried;
pub use checkpoint::Checkpoint;
pub use router::{routing, RouterChange, Routing};

use crate::instance::{Ids, InstanceError, InstanceStep, SurfaceUpdate, Tree, Update};
use crate::request::{Answer, Dispatch, Outcome, Request, RequestOut};
use crate::store::{Store, StoreWrite};
use crate::vm::{self, Env, Frame, RowSlots, Trap};
use exact_kernel::{CommitReceipt, Kernel, KernelError, ViewId};
use exact_plan::{ActionsId, Code, EventKind, MutationsId, NodesId, Plan, PlanError, Value};

/// A host-facing effect an action asked for; executed after commit, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// The capability name.
    pub name: String,
    /// Its arguments.
    pub args: Vec<Value>,
    /// The node whose input ran the action, when a host event did: where a command that shows system UI anchors it (LLP 1069.003 D3). `None` for a timer, an answer, or anything else no input dispatched.
    pub source: Option<ViewId>,
}

/// A timer or collection-feedback commit and the runner clock it happened at.
#[derive(Debug, Clone)]
pub struct Timed {
    /// The runner's clock when the receipt committed, milliseconds.
    pub at_ms: f64,
    /// The commit.
    pub receipt: CommitReceipt,
}

/// What one `advance_timed` or collection feedback call did: the commits in order, each at its due
/// time; the clock afterwards — the requested time, or the last time reached
/// before a refusal (or, advancing until a request, the due time of the timer
/// that sent it); and that refusal, if any. Commits before a refusal are
/// kept: they are in the kernel, and a host must show them.
#[derive(Debug)]
pub struct Advanced {
    /// The commits, in order.
    pub receipts: Vec<Timed>,
    /// The clock after the call, milliseconds.
    pub now_ms: f64,
    /// The refusal that stopped the advance or the collection edge action.
    pub error: Option<RunnerError>,
}

/// Why the runner refused. The kernel is unchanged.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum RunnerError {
    KernelSchemaMismatch {
        plan: u64,
        kernel: u64,
    },
    /// The plan belongs to another app (LLP 1023 D5): its header names one identity, this binary's data crate another.
    AppMismatch {
        plan: String,
        host: String,
    },
    Plan(PlanError),
    NotOneRoot(usize),
    Trap(Trap),
    Instance(InstanceError),
    Kernel(KernelError),
    Data {
        resource: String,
        error: DataError,
    },
    Shape {
        resource: String,
        /// Where the answer first differs from the declared shape.
        why: String,
    },
    UnknownView(ViewId),
    /// A typed host event carries invalid numeric values. No action ran.
    InvalidEvent {
        event: &'static str,
    },
    /// A control's `input` or `change` carries a value it could never report (LLP 1069.001 D4): a select's value no enabled option has.
    InvalidValue {
        event: &'static str,
        reason: String,
    },
    NoHandler {
        view: ViewId,
        event: &'static str,
    },
    Arity {
        action: String,
        expected: usize,
        actual: usize,
    },
    /// Derives and resources depend on each other in a cycle; nothing settles.
    Cycle,
    /// An earlier update failed after the instance tree had begun to change; the runner no longer matches its kernel and must be restarted (D5).
    Poisoned,
    /// `advance` was given a non-finite time.
    NonFiniteClock,
    /// A viewport dimension is non-finite or non-positive.
    InvalidViewport,
    /// A date fact is non-finite, negative, or its offset past ±18 hours.
    InvalidTime,
    /// A locale or time zone is empty, over-long, or not in its form.
    InvalidPlace,
    /// The declared router shapes, table, launch fallback or value is invalid.
    Router(String),
    /// A clock value exceeds the exact integer-millisecond domain.
    ClockOutOfRange,
    /// Adding a timer interval did not advance its next due time.
    ClockDidNotAdvance {
        timer: usize,
    },
    /// One seek reached the bounded number of timer commits it may perform.
    TimerFireLimit {
        limit: usize,
    },
    /// A send would be the 65th waiting for a `queue` mutation (LLP 1092 D4).
    QueueFull {
        mutation: String,
    },
    /// A gated task's `key=` is no key (a non-finite number; LLP 1092 D8).
    TaskKey {
        task: String,
    },
    /// A region sits at the plan root; v1 requires one root node.
    RootRegion,
    /// A slot initializer or write does not conform to the slot's declared type.
    SlotType {
        slot: String,
    },
    /// A derive's value does not conform to its declared type.
    DeriveType {
        derive: String,
    },
    /// An action argument does not conform to the parameter's declared type.
    ArgumentType {
        action: String,
        param: String,
    },
    /// An action argument or a slot write is a string longer than [`crate::vm::MAX_STRING`] bytes.
    StringTooLong {
        name: String,
    },
}

impl From<Trap> for RunnerError {
    fn from(t: Trap) -> Self {
        RunnerError::Trap(t)
    }
}

impl From<InstanceError> for RunnerError {
    fn from(e: InstanceError) -> Self {
        match e {
            InstanceError::SlotType { slot } => RunnerError::SlotType { slot },
            other => RunnerError::Instance(other),
        }
    }
}

impl From<KernelError> for RunnerError {
    fn from(e: KernelError) -> Self {
        RunnerError::Kernel(e)
    }
}

#[derive(Clone)]
struct ResourceState {
    args: Vec<Value>,
    value: crate::held::Held,
    /// Store revision this answer observed; checked only for known readers.
    store_revision: u64,
    /// A placeholder shown before an answer, including after failure, never an answer
    /// to reuse, carry or compile (LLP 1054.000.002 D4).
    placeholder: bool,
    /// Only a persisted boot seed can match identifying arguments before
    /// readiness (LLP 1027.005 D3). Settlement and rollback copy this with
    /// its value; activation or losing that identity consumes it.
    kept_seed: bool,
}

/// What a boot starts from besides the plan and the launch.
#[derive(Clone, Copy)]
enum Seed<'a> {
    /// The plan's initial state.
    Fresh,
    /// Fresh state evaluated at the supplied render clock.
    At(f64),
    /// A reload's carried state (LLP 1007 §6).
    Carried(&'a Carried),
    /// A rendered document's answers (LLP 1048.000 D6).
    Checkpoint(&'a Checkpoint),
}

/// A request the host is running: the ticket its reply carries, what it
/// answers, and the arguments it was asked with (what `parse` sees).
#[derive(Clone)]
struct PendingReq {
    refusal: Option<(&'static str, bool)>,
    /// The host refused this ordered request before execution (LLP 1041 §8.4).
    /// Earlier source turns may already have executed effects.
    refused: bool,
    ticket: u64,
    target: Target,
    source: String,
    args: Vec<Value>,
    /// The source's continuation token, when the request is one.
    continuation: Option<u64>,
    /// The request itself when a re-ask may keep it: a resource's plain
    /// HTTP request (LLP 1054.000.000 D3).
    keepable: Option<Request>,
    /// An answer that keeps coming (LLP 1016.000): what it has delivered.
    stream: Option<StreamCount>,
    /// A topic its resource watches changed while it was in flight: its
    /// reply lands, then the resource is asked again (LLP 1016.002 D4).
    ask_again: bool,
}

/// An open stream's messages so far, and those the host coalesced away
/// (LLP 1016.000 D4, D5). One message landed ends its `pending`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamCount {
    /// Messages delivered to the runner.
    pub messages: u64,
    /// Messages the host dropped for a newer one before delivery.
    pub coalesced: u64,
}

impl PendingReq {
    /// In flight: every request, and a stream until its first message.
    fn in_flight(&self) -> bool {
        self.stream.is_none_or(|s| s.messages == 0)
    }
}

#[derive(Clone)]
struct Timer {
    /// The next due time; infinite once a one-shot timer has fired, and
    /// while a gated task is idle (LLP 1092 D8); a frame task's next
    /// virtual frame (LLP 1073 D3), `virtual_frame(base, k)`.
    next_ms: f64,
    /// Whether the task's gate holds (always, for `mount`).
    armed: bool,
    /// The key it was armed with (`key=`), as `each` compares keys.
    key: Option<String>,
    /// A frame task's last presented frame (or mount) …
    base: f64,
    /// … and which virtual frame after it is next.
    k: u32,
}

/// One plan, one data source, one kernel.
pub struct Runner<D: DataSource> {
    plan: Plan,
    /// The plan's string pool, interned once (`vm::intern`).
    strings: Vec<Value>,
    sites: crate::instance::SiteIndex,
    inspection_digest: std::cell::OnceCell<String>,
    action_binding_origin: std::rc::Rc<()>,
    data: D,
    kernel: Kernel,
    slots: Vec<Value>,
    derives: Vec<Option<Value>>,
    resources: Vec<Option<ResourceState>>,
    resource_values: Vec<Option<crate::held::Held>>,
    tree: Option<Tree>,
    reorder_owner: Option<exact_kernel::NodeKey>,
    reorder_ops: Vec<exact_kernel::Op>,
    /// The session the owner's token names (LLP 1094 D4).
    reorder_session: Option<Box<reorder_group::Session>>,
    /// A grouped drop's source while its `reorderdrop` runs (D2).
    dropping_from: Option<exact_kernel::NodeKey>,
    ids: Ids,
    now_ms: f64,
    timers: Vec<Timer>,
    /// The host presents frames: frame tasks fire only at `frame` (LLP 1073 D4).
    presenting: bool,
    batch: u64,
    commands: Vec<Command>,
    /// `scrollIntoView` commands an action stated, run after its update.
    into_view: Vec<crate::instance::collection::IntoView>,
    /// Where the host last showed each scroller (`frame`, LLP 1051.000 D1).
    scrolled: crate::geometry::Scrolled,
    /// Each `resize` handler's node and the content box last delivered.
    resized: resize::Resized,
    /// Refused requests, for `state` (LLP 1070.000 §2.2).
    into_view_refused: std::collections::VecDeque<String>,
    surfaces: Vec<SurfaceUpdate>,
    /// Views the next applied batch renews (LLP 1078): rebound list rows.
    renewed: Vec<ViewId>,
    /// Rows the next batch mounts out of their port, and those that showed (LLP 1055 D13).
    shown: crate::instance::collection::shown::RowsShown,
    /// Retiring list rows are rebound to new items ([`Runner::set_row_reuse`]).
    reuse: bool,
    /// The 2D canvases (LLP 1056 D4), when Canvas 2D is linked.
    canvases: Option<Box<dyn canvas2d::CanvasEngine>>,
    /// Requests in flight (LLP 1016): at most one per resource or mutation.
    pending: Vec<PendingReq>,
    /// This commit let a request go: `conclude` tells the source what is still in flight.
    forgot: bool,
    /// Failed arguments suppress another ask until they change or refresh.
    failed_args: Vec<Option<Vec<Value>>>,
    /// Why each resource last failed, shown in `state.failed` while
    /// `failed_args` holds (app farm round 1: a refusal only in the journal).
    failed_why: Vec<Option<crate::failure::Failure>>,
    /// `pending` as flags, by resource and by mutation, for expressions.
    pending_res: Vec<bool>,
    pending_mut: Vec<bool>,
    /// Mutations whose answer landed in the commit being made; their `then` actions are armed once it stands (LLP 1016.001).
    landed: Vec<usize>,
    /// Sends made before the data source was ready, by mutation, sent at
    /// `data_ready` (LLP 1027 D4): pending meanwhile, one per mutation.
    unsent: Vec<(usize, String, Vec<Value>)>,
    /// When each mutation's `then` action is due, as a one-shot timer: infinite until an answer lands.
    then_due: Vec<f64>,
    /// `queue` mutations' waiting sends, `next`s and stalls (LLP 1092).
    queues: queue::Queues,
    next_ticket: u64,
    /// The module's background work's round, when one is out (LLP 1097 D5).
    background: background::Background,
    /// Files picked this run, for `app:/tmp/picked/` names (LLP 1069.002 D3).
    picked_count: u64,
    /// Second edges waiting for the first action's async targets to settle,
    /// and lists whose edge waits for their covered route to show (`collection.rs`).
    deferred_edges: Vec<(u32, Vec<Target>)>,
    held_edges: Vec<u32>,
    /// Requests for the host, since the last take.
    requests: Vec<RequestOut>,
    /// Resources an action asked to re-request; consumed by the next settle
    /// that can ask them (LLP 1054.000.000 D2).
    refresh_next: Vec<usize>,
    /// Resources a send declared it changes, to read again from the source
    /// without sending anything (LLP 1054.000.000 D1); the next settle's.
    reread_next: Vec<usize>,
    /// Durable client state (LLP 1018 D1): the host's snapshot, and the
    /// writes since for the host to persist.
    store: Store,
    /// Which resources consulted the store when they settled (bake gives
    /// them no compiled value, LLP 1018 D4).
    store_readers: Vec<bool>,
    /// Which resources drew secure randomness when they settled, since boot
    /// (LLP 1069.005 D2): bake compiles no value for them at all.
    entropy_readers: Vec<bool>,
    /// The device topics each resource's current answer watches (LLP
    /// 1016.002): an announced topic asks exactly these again.
    watching: Vec<Vec<String>>,
    /// Topics the source's native module announced since the host last
    /// applied them, from any thread ([`Runner::listen`]).
    announced: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// The native slot for a source that has none of its own (a Rust source):
    /// the host installs the app module here too (LLP 1067.000 Q9).
    native: Native,
    /// Deferred resources shown from a placeholder — a kept answer or
    /// the compiled empty-store value — to ask again at `data_ready`.
    stale: Vec<bool>,
    /// Resources showing their placeholder because their source can't
    /// answer yet (LLP 1048.003 D6): `pending(x)` is true for them.
    awaiting: Vec<bool>,
    /// Whether fresh answers of store-reading resources are kept for the
    /// next boot: only for a source that may not be ready at boot.
    keeps_answers: bool,
    poisoned: bool,
    /// Evaluate every site on every update (the incremental update's
    /// reference; `set_full_evaluation`).
    full: bool,
    /// Row slots actions wrote, for the next update.
    row_writes: crate::instance::RowWrites,
    /// Journal lines an update produced, written once its batch applies.
    notes: Vec<String>,
    /// This commit's work outside the tree: rows the event's lookup
    /// compared, derives settled by evaluation, and the store's copied
    /// bytes when it began.
    lookup_rows: std::cell::Cell<usize>,
    derives_evaluated: usize,
    copied_at_checkpoint: usize,
    /// The inputs the published derives were computed against.
    settled: Option<settlement::Settled>,
    /// Per derive: its value depends on the durable store (bake provenance).
    derive_store_dependent: Vec<bool>,
    /// What this binary and its update store know about delivery (LLP 1030
    /// D4, D7): the embedded answer until a host says otherwise.
    delivery: crate::delivery::Delivery,
    // @ref LLP 1039 D2 — the layout size before settlement.
    viewport: crate::Viewport,
    // @ref LLP 1069.000 D2 — visibility, connectivity, the share sheet.
    page: crate::page::Page,
    // @ref LLP 1027.000.000 — the date, once the host says it.
    time: crate::time::WallTime,
    place: crate::time::Place,
    surface_records: exact_kernel::SortedMap<String, String>,
    surface_refusals: exact_kernel::SortedMap<String, String>,
    /// What the host links of the runner's own answers (LLP 1047 D3).
    links: RunnerLinks,
    router: Option<Box<dyn router::Routing>>,
    /// What happened, one line each, for the agent API's `logs`: the last
    /// [`JOURNAL_RING`] lines, and how many were dropped before them.
    journal: std::collections::VecDeque<String>,
    /// The driver's fetch faults (LLP 1103).
    faults: faults::Faults,
    /// Device requests held for the agent (LLP 1069.007 D3): not I/O.
    device_holds: Vec<device::Hold>,
    /// Notifications the app posted under the agent (`state.notifications`):
    /// the agent's substitute for the system's (`crate::notify`).
    notifications: Vec<crate::notify::Notice>,
    /// The voice table (LLP 1096 D5): what the app's sounds scheduled.
    sounds: crate::sound::Sounds,
    /// The app's `setRootFontSize` over the host's size (LLP 1069.000 D3).
    root_font: root_font::RootFont,
    /// Auth sessions (LLP 1069.006): live ones, and answers to deliver.
    auth: crate::auth::Sessions,
    /// The device capabilities linked (LLP 1047 D3): [`DeviceLinks`].
    device_links: DeviceLinks<D>,
    /// The view whose event is being dispatched, stamped on the commands
    /// its action issues (LLP 1069.003 D3).
    input_source: Option<ViewId>,
    journal_start: usize,
    flow_warned: exact_kernel::SortedSet<exact_kernel::NodeKey>,
    fragment_warned: exact_kernel::SortedSet<exact_kernel::NodeKey>,
    /// The lists already found conforming to their types, so a live answer
    /// is checked where it changed (LLP 1053 §0 G8).
    conformed: std::cell::RefCell<crate::conform::Conformed>,
}

/// How many journal lines the runner retains (about an hour of a one-second
/// timer); older ones are dropped, and `logs` reports where its window starts.
pub const JOURNAL_RING: usize = 4096;

/// Largest accepted clock value: JavaScript's exact integer domain in ms.
pub const MAX_CLOCK_MS: f64 = 9_007_199_254_740_991.0;

/// Maximum timer commits one call to [`Runner::advance_timed`] may perform.
pub const TIMER_FIRE_LIMIT: usize = 4096;

/// The seekable clock's virtual display: a frame every 1000/60 ms after a
/// frame task last fired (LLP 1073 D3).
pub const VIRTUAL_FRAME_MS: f64 = 1000.0 / 60.0;

/// A frame task's `k`th virtual frame after `base`: `base + k·1000/60`, the
/// product first, so sixty frames are exactly a second on every host.
pub fn virtual_frame(base: f64, k: u32) -> f64 {
    base + f64::from(k) * 1000.0 / 60.0
}

impl<D: DataSource> Runner<D> {
    /// This runner, linking every device capability: the plain boots'.
    pub(crate) fn linking_every_device(mut self) -> Self {
        self.device_links = DeviceLinks::ALL;
        self
    }

    /// Boot: refuse a plan built against another kernel schema, evaluate
    /// initial state, settle resources (compiled data first, the source
    /// otherwise), realize the tree, and apply the first frame's ops.
    pub fn boot(
        plan: Plan,
        data: D,
        kernel: Kernel,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(
            RunnerLinks::ALL,
            plan,
            data,
            kernel,
            Seed::Fresh,
            Vec::new(),
            Default::default(),
            viewport,
            launch,
        )
        .map(Runner::linking_every_device)
    }

    /// Boot a new plan with the state of an old runner (a dev reload that
    /// keeps its place — LLP 1007 §6). The tree, ids, timers, and every
    /// derive are fresh; only slots, matching resources, and the clock are
    /// taken, and each only where it still fits the new plan, so carried
    /// state can never be why a boot fails. Nothing compiled into the plan
    /// is trusted over carried state.
    pub fn boot_carrying(
        plan: Plan,
        data: D,
        kernel: Kernel,
        carried: &Carried,
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(
            RunnerLinks::ALL,
            plan,
            data,
            kernel,
            Seed::Carried(carried),
            carried.store.clone(),
            Default::default(),
            viewport,
            launch,
        )
        .map(Runner::linking_every_device)
    }

    /// Everything a reload keeps.
    pub fn carry(&self) -> Carried {
        Carried {
            router: self.carry_router(),
            data_revision: self.data.revision().map(str::to_owned),
            keeps_answers: self.keeps_answers,
            slots: self
                .plan
                .slots
                .iter()
                .zip(&self.slots)
                .filter(|(s, _)| s.owner.is_none())
                .map(|(s, v)| (self.plan.str(s.name).to_string(), v.clone()))
                .collect(),
            resources: self
                .plan
                .resources
                .iter()
                .zip(&self.resources)
                .enumerate()
                // A request in flight or a placeholder shown until the
                // source is ready is not an answer to carry.
                .filter(|(i, _)| !self.pending_res[*i] && !self.stale[*i])
                .filter(|(_, (_, s))| s.as_ref().is_none_or(|s| !s.placeholder))
                .filter_map(|(_, (r, s))| {
                    s.as_ref().map(|s| {
                        (
                            self.plan.str(r.name).to_string(),
                            self.plan.str(r.source).to_string(),
                            s.args.clone(),
                            s.value.get(&self.plan).clone(),
                        )
                    })
                })
                .collect(),
            store_readers: self
                .plan
                .resources
                .iter()
                .enumerate()
                .filter(|(i, _)| self.store_readers[*i])
                .map(|(_, resource)| {
                    (
                        self.plan.str(resource.name).to_string(),
                        self.plan.str(resource.source).to_string(),
                    )
                })
                .collect(),
            now_ms: self.now_ms,
            store: self.store.snapshot(),
            forgot_waiting: self.queued(),
            faults: (!self.faults.is_empty()).then(|| self.faults.spec()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)] // the host boot facts
    fn boot_inner(
        links: RunnerLinks,
        plan: Plan,
        mut data: D,
        kernel: Kernel,
        seed: Seed<'_>,
        snapshot: Vec<(String, String)>,
        delivery: crate::delivery::Delivery,
        // @ref LLP 1039 D2 — the layout size before settlement.
        viewport: crate::Viewport,
        launch: &str,
    ) -> Result<Runner<D>, RunnerError> {
        viewport.validate()?;
        // @ref LLP 1095 D9 — every platform colour the plan can show is known
        // before the host's first report, a branch not yet taken included.
        for text in &plan.strings {
            exact_kernel::style::roles::intern_literals(text);
        }
        let carried = match seed {
            Seed::Carried(carried) => Some(carried),
            _ => None,
        };
        let now_ms = match seed {
            Seed::Fresh => 0.0,
            Seed::At(now_ms) => now_ms,
            Seed::Carried(carried) => carried.now_ms,
            Seed::Checkpoint(checkpoint) => checkpoint.now_ms,
        };
        if !now_ms.is_finite() {
            return Err(RunnerError::NonFiniteClock);
        }
        if !(0.0..=MAX_CLOCK_MS).contains(&now_ms) {
            return Err(RunnerError::ClockOutOfRange);
        }
        // A plan is validated where it is made: `Plan::decode` and
        // `PlanBuilder::finish` (the compiler's and the bake's) both check
        // every cross-reference, so the runner takes it as it is.
        if plan.kernel_schema_digest != exact_kernel::SCHEMA_DIGEST {
            return Err(RunnerError::KernelSchemaMismatch {
                plan: plan.kernel_schema_digest,
                kernel: exact_kernel::SCHEMA_DIGEST,
            });
        }
        // The identity gate (LLP 1023 D5): a plan naming one app against a
        // data crate naming another is a poisoned boot — the seam's names
        // and shapes cannot be trusted to line up. Unnamed (empty, either
        // side) matches anything: fixtures and stand-ins stay bootable.
        if !plan.app_id.is_empty() && !data.app_id().is_empty() && plan.app_id != data.app_id() {
            return Err(RunnerError::AppMismatch {
                plan: plan.app_id.clone(),
                host: data.app_id().to_string(),
            });
        }
        let roots = plan
            .nodes
            .iter()
            .filter(|n| n.parent.is_none() && n.arm.is_none())
            .count()
            + plan
                .regions
                .iter()
                .filter(|r| r.parent.is_none() && r.arm.is_none())
                .count();
        if roots != 1 {
            return Err(RunnerError::NotOneRoot(roots));
        }
        if plan
            .regions
            .iter()
            .any(|r| r.parent.is_none() && r.arm.is_none())
        {
            return Err(RunnerError::RootRegion);
        }
        let same_logic = carried.is_none_or(|c| c.data_revision.as_deref() == data.revision());
        data.bind(&plan);
        // A cold boot forgets the kept answers no declared reader seeds; an
        // app granting Health forgets them all, at every boot (LLP 1069.008.000 D7).
        let private = crate::device::keeps_no_answers(data.grants());
        let obsolete_kept = match carried {
            _ if private => kept::all(&snapshot),
            None => kept::obsolete(&plan, &snapshot),
            Some(_) => Vec::new(),
        };
        let mut store = Store::new_linked(data.grants(), snapshot, links.grants);
        for name in &obsolete_kept {
            store.forget_kept(name);
        }
        if let Some(carried) = carried {
            // Forget incompatible seeds, not just their first use: a pending
            // replacement must not relabel an old answer on the next reload.
            for (name, _) in &carried.store {
                let Some(resource) = name.strip_prefix(Store::KEPT) else {
                    continue;
                };
                let compatible = same_logic
                    && plan.resources.iter().any(|r| {
                        plan.str(r.name) == resource
                            && carried
                                .store_readers
                                .iter()
                                .any(|(n, s)| n == resource && s == plan.str(r.source))
                    });
                if private || !compatible {
                    store.forget_kept(name);
                }
            }
        }
        let store_readers = plan
            .resources
            .iter()
            .map(|resource| {
                resource.reader
                    || plan.str(resource.source) == crate::delivery::SOURCE
                    || plan.str(resource.source) == crate::viewport::SOURCE
                    || plan.str(resource.source) == crate::time::SOURCE
                    || plan.str(resource.source) == crate::page::SOURCE
                    || plan.str(resource.source) == crate::surface_record::SOURCE
                    || (same_logic
                        && carried.is_some_and(|carried| {
                            let name = plan.str(resource.name);
                            let source = plan.str(resource.source);
                            carried
                                .store_readers
                                .iter()
                                .any(|(reader, origin)| reader == name && origin == source)
                        }))
            })
            .collect();
        let router = match links.router {
            Some(routing) => routing(&plan)?,
            None => None,
        };
        let mut runner = Runner {
            sites: crate::instance::SiteIndex::new(&plan),
            strings: vm::intern(&plan),
            plan,
            inspection_digest: std::cell::OnceCell::new(),
            action_binding_origin: std::rc::Rc::new(()),
            data,
            kernel,
            slots: Vec::new(),
            derives: Vec::new(),
            resources: Vec::new(),
            resource_values: Vec::new(),
            tree: None,
            reorder_owner: None,
            reorder_ops: Vec::new(),
            reorder_session: None,
            dropping_from: None,
            ids: Ids::default(),
            now_ms,
            timers: Vec::new(),
            presenting: false,
            batch: 0,
            commands: Vec::new(),
            sounds: Default::default(),
            root_font: Default::default(),
            into_view: Vec::new(),
            scrolled: Default::default(),
            resized: Vec::new(),
            into_view_refused: Default::default(),
            surfaces: Vec::new(),
            renewed: Vec::new(),
            shown: Default::default(),
            reuse: false,
            canvases: links.canvas.map(|engine| engine()),
            pending: Vec::new(),
            pending_res: Vec::new(),
            pending_mut: Vec::new(),
            announced: Default::default(),
            native: Native::default(),
            watching: Vec::new(),
            landed: Vec::new(),
            unsent: Vec::new(),
            then_due: Vec::new(),
            queues: Default::default(),
            next_ticket: 1,
            background: Default::default(),
            picked_count: 0,
            forgot: false,
            failed_args: Vec::new(),
            failed_why: Vec::new(),
            deferred_edges: Vec::new(),
            held_edges: Vec::new(),
            requests: Vec::new(),
            refresh_next: Vec::new(),
            reread_next: Vec::new(),
            store,
            entropy_readers: Vec::new(),
            store_readers,
            stale: Vec::new(),
            awaiting: Vec::new(),
            keeps_answers: false,
            delivery,
            viewport,
            page: Default::default(),
            time: Default::default(),
            place: Default::default(),
            surface_records: Default::default(),
            surface_refusals: Default::default(),
            links,
            router,
            poisoned: false,
            full: false,
            row_writes: Default::default(),
            notes: Vec::new(),
            lookup_rows: Default::default(),
            derives_evaluated: 0,
            copied_at_checkpoint: 0,
            settled: None,
            derive_store_dependent: Vec::new(),
            journal: std::collections::VecDeque::new(),
            faults: faults::Faults::from_env(),
            device_holds: Vec::new(),
            notifications: Vec::new(),
            auth: Default::default(),
            device_links: DeviceLinks::CORE,
            input_source: None,
            journal_start: 0,
            flow_warned: Default::default(),
            fragment_warned: Default::default(),
            conformed: Default::default(),
        };
        runner.init_slots(carried, launch)?;
        runner.derives = vec![None; runner.plan.derives.len()];
        // Resources: carry only the same named source and a value that still
        // fits the declared shape; a carried value can refuse nothing.
        runner.resources = (0..runner.plan.resources.len())
            .map(|i| {
                let name = runner.plan.str(runner.plan.resources[i].name);
                let source = runner.plan.str(runner.plan.resources[i].source);
                carried
                    .filter(|_| same_logic)
                    .and_then(|c| {
                        c.resources
                            .iter()
                            .find(|(n, s, _, _)| n == name && s == source)
                    })
                    .filter(|(_, _, _, value)| runner.check_shape(i, value).is_ok())
                    .map(|(_, _, args, value)| ResourceState {
                        args: args.clone(),
                        value: crate::held::Held::new(value.clone()),
                        store_revision: runner.store.revision(),
                        placeholder: false,
                        kept_seed: false,
                    })
            })
            .collect();
        // @ref LLP 1048.000 D6 — a document's answers seed their resources.
        // The device's own store is an input its render never had.
        let (seeded, note) = match seed {
            Seed::Checkpoint(checkpoint) => runner.seed_checkpoint(checkpoint),
            _ => (vec![false; runner.plan.resources.len()], String::new()),
        };
        let device_state = matches!(seed, Seed::Checkpoint(_)) && runner.has_app_store();
        // Store-reading resources when the data source is not ready (a
        // TypeScript module before its host loads it, LLP 1027 D4): the
        // answer kept from the last launch seeds the first frame if its
        // identifying arguments still match and its value still fits; the compiled
        // empty-store placeholder is the fallback (settlement); either way
        // the resource is asked again at `data_ready`.
        let ready = runner.data.ready();
        runner.keeps_answers = !private && (!ready || carried.is_some_and(|c| c.keeps_answers));
        runner.stale = vec![false; runner.plan.resources.len()];
        runner.entropy_readers = vec![false; runner.plan.resources.len()];
        runner.awaiting = vec![false; runner.plan.resources.len()];
        if !ready {
            for (i, &taken) in seeded.iter().enumerate() {
                if !runner.plan.resources[i].reader
                    || exact_plan::runner_owned_source(
                        runner.plan.str(runner.plan.resources[i].source),
                    )
                {
                    continue;
                }
                // An answer the runtime already has isn't asked again when
                // the module loads, unless the device's store may change it.
                if taken && !device_state {
                    continue;
                }
                runner.stale[i] = true;
                if runner.resources[i].is_some() {
                    continue;
                }
                let name = runner.plan.str(runner.plan.resources[i].name);
                let seed = runner
                    .store
                    .kept(&kept::kept_name(name))
                    .and_then(kept::decode)
                    .filter(|(args, value)| {
                        let row = &runner.plan.resources[i];
                        args.len() == row.args.len as usize - usize::from(row.context)
                            && runner.check_shape(i, value).is_ok()
                    });
                if let Some((args, value)) = seed {
                    runner.resources[i] = Some(ResourceState {
                        args,
                        value: crate::held::Held::new(value),
                        store_revision: runner.store.revision(),
                        placeholder: false,
                        kept_seed: true,
                    });
                }
            }
        }
        if ready && device_state {
            // … and asked now when it can answer, showing the rendered
            // answer until it does.
            runner.refresh_next.extend(
                (0..seeded.len()).filter(|&i| seeded[i] && runner.plan.resources[i].reader),
            );
        }
        runner.resource_values = vec![None; runner.plan.resources.len()];
        runner.pending_res = vec![false; runner.plan.resources.len()];
        runner.pending_mut = vec![false; runner.plan.mutations.len()];
        runner.watching = vec![Vec::new(); runner.plan.resources.len()];
        runner.failed_args = vec![None; runner.plan.resources.len()];
        runner.failed_why = vec![None; runner.plan.resources.len()];
        runner.then_due = vec![f64::INFINITY; runner.plan.mutations.len()];
        runner.queues = queue::Queues::new(runner.plan.mutations.len());
        // A carried boot never takes compiled data: it was baked for the
        // initial state, and the carried state is not that.
        runner.settle(carried.is_none())?;
        runner.init_late_slots(carried)?;
        let now = runner.now_ms;
        // A gated task starts idle; the gate step arms the ones whose gate
        // holds, over the settled state (LLP 1092 D8).
        runner.timers = runner
            .plan
            .timers
            .iter()
            .map(|t| Timer {
                next_ms: if t.gated || t.keyed {
                    f64::INFINITY
                } else if t.frame {
                    virtual_frame(now, 1)
                } else {
                    now + t.interval_ms as f64
                },
                armed: !(t.gated || t.keyed),
                key: None,
                base: now,
                k: 1,
            })
            .collect();
        runner.gate_step()?;
        // First frame.
        let mut ids = std::mem::take(&mut runner.ids);
        let (tree, ops, surfaces, notes, shown) = {
            let mut u = Update::new(runner.env(&[], &[]), &runner.sites, &mut ids);
            u.discard = runner.kernel.is_detached();
            let tree = Tree::create(&mut u)?;
            (tree, u.ops, u.surfaces, u.notes, u.shown)
        };
        runner.notes = notes;
        runner.shown = shown;
        runner.ids = ids;
        runner.tree = Some(tree);
        let receipt = runner.apply(ops)?;
        runner.publish_surfaces(surfaces);
        let line = lines::boot(carried.is_some(), runner.kernel.live_count(), receipt.epoch);
        runner.log(line);
        if let Some(spec) = carried.and_then(|c| c.faults.as_deref()) {
            runner.faults = faults::Faults::parse(spec).unwrap_or_default();
        }
        for (name, n) in carried
            .map(|c| c.forgot_waiting.as_slice())
            .unwrap_or_default()
        {
            runner.log(lines::forgot_waiting(*n, name));
        }
        if !note.is_empty() {
            runner.log(note);
        }
        // @ref LLP 1048.003 D6 — what the first frame shows from the bake
        // until its source is ready (feed F24: say so in the journal).
        for i in 0..runner.stale.len() {
            if runner.stale[i]
                && runner.resources[i]
                    .as_ref()
                    .is_some_and(|s| s.value.is_compiled())
            {
                let line = lines::build_time(runner.plan.str(runner.plan.resources[i].name));
                runner.log(line);
            }
        }
        // Grants that do not parse grant nothing: said once here, and in
        // each refusal (the store's, the host's).
        if let Some(why) = runner.store.unparsed() {
            let line = format!("{why}; nothing is granted");
            runner.log(line);
        }
        Ok(runner)
    }

    /// The auth sessions this runner keeps (LLP 1069.006; [`crate::auth`]).
    pub fn auth_mut(&mut self) -> &mut crate::auth::Sessions {
        &mut self.auth
    }

    /// Append a line to the journal the agent API's `logs` reads, stamped
    /// with the clock. Hosts add their own lines here (an image loaded, a
    /// layout refusal) so one read sees everything in order.
    pub fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        self.journal.push_back(lines::stamped(self.now_ms, &line));
        if self.journal.len() > JOURNAL_RING {
            self.journal.pop_front();
            self.journal_start += 1;
        }
    }

    /// Report unsupported auto-height flow once per leaf per boot.
    /// @ref LLP 1043.000 §4 stage 2 — M9 makes the deferred behavior visible.
    pub fn report_flow_skipped(&mut self, keys: &[exact_kernel::NodeKey]) {
        for &key in keys {
            if self.flow_warned.insert(key) {
                if let Some(node) = self.kernel.node_by_key(key) {
                    let why = node.flow_refusal().map_or("", |r| r.message());
                    self.log(format!(
                        "wrap-flow: text #{} has auto height and is not flowed: {why} (LLP 1043.000 §8)",
                        node.id
                    ));
                }
            }
        }
    }

    /// The retained journal lines, oldest first.
    pub fn journal(&self) -> impl Iterator<Item = &str> {
        self.journal.iter().map(String::as_str)
    }

    /// The index (since boot) of the first retained journal line: how many
    /// were dropped by the ring.
    pub fn journal_start(&self) -> usize {
        self.journal_start
    }

    /// Journal an outcome. `was_poisoned` is the runner's state before the
    /// attempt: only the failure that poisons it is written as such.
    fn log_outcome(
        &mut self,
        what: &str,
        result: &Result<CommitReceipt, RunnerError>,
        was_poisoned: bool,
    ) {
        let line = match result {
            Ok(r) => lines::committed(
                what,
                r.epoch,
                r.created.len(),
                r.destroyed.len(),
                r.touched.len(),
            ),
            Err(e) if self.poisoned && !was_poisoned => {
                format!("{what} poisoned the runner: {e:?}")
            }
            Err(e) => format!("{what} refused: {e:?}"),
        };
        self.log_router_refusals();
        self.log(line);
    }

    /// The kernel, for layout and export.
    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    /// A scroll container the host presents now stands at `(left, top)` CSS
    /// px, or the page does (`None`): what `frame` and `measure` subtract
    /// from the kernel's scroll-free box natively. Noted, never dispatched.
    pub fn scrolled(&mut self, view: Option<ViewId>, left: f64, top: f64) {
        self.scrolled.note(&self.kernel, view, left, top);
    }

    /// The kernel, mutably (a host lays out through it).
    pub fn kernel_mut(&mut self) -> &mut Kernel {
        &mut self.kernel
    }

    /// The plan.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Same immutable plan as a targeted node read. No work until an inspector
    /// requests identity; a replacement runner owns a fresh cache.
    pub(crate) fn inspection_digest(&self) -> &str {
        self.inspection_digest.get_or_init(|| {
            use sha2::{Digest, Sha256};
            use std::fmt::Write;
            let mut out = String::with_capacity(64);
            for byte in Sha256::digest(self.plan.encode()) {
                write!(out, "{byte:02x}").unwrap();
            }
            out
        })
    }

    /// The data source.
    pub fn data(&mut self) -> &mut D {
        &mut self.data
    }

    /// Inspect data-source identity and readiness without executing app logic.
    pub fn data_ref(&self) -> &D {
        &self.data
    }

    /// What the app posted under the agent, oldest first (`crate::notify`).
    pub fn notifications(&self) -> &[crate::notify::Notice] {
        &self.notifications
    }

    pub(crate) fn notifications_mut(&mut self) -> &mut Vec<crate::notify::Notice> {
        &mut self.notifications
    }

    /// The voice table (LLP 1096 D5), for `state.sounds` and a test.
    pub fn sounds(&self) -> &crate::sound::Sounds {
        &self.sounds
    }

    /// What the output plays since the last take: a `Play` per new voice,
    /// an `End` per voice that now ends earlier (LLP 1096 D5).
    pub fn take_sounds(&mut self) -> Vec<crate::sound::SoundOp> {
        self.sounds.take()
    }

    /// End every live voice before this runner is replaced (a dev reload,
    /// LLP 1096 D5); the host drains `take_sounds` before the restart.
    pub fn end_sounds(&mut self) {
        if self.plan.sounds.is_empty() {
            return;
        }
        let mut lines = Vec::new();
        self.sounds.reload(self.now_ms, &mut lines);
        lines.into_iter().for_each(|l| self.log(l));
    }

    /// Current value of a slot by name.
    pub fn slot(&self, name: &str) -> Option<&Value> {
        self.plan
            .slots
            .iter()
            .position(|s| self.plan.str(s.name) == name && s.owner.is_none())
            .map(|i| &self.slots[i])
    }

    /// Current value of a derive by name.
    pub fn derive(&self, name: &str) -> Option<&Value> {
        self.plan
            .derives
            .iter()
            .position(|d| self.plan.str(d.name) == name)
            .and_then(|i| self.derives[i].as_ref())
    }

    /// Each resource whose last request failed, by name, with why: what
    /// `failed(x)` reads, for the agent's `state.failed`.
    pub fn failed_resources(&self) -> Vec<(&str, &str)> {
        let failed = self.failed_args.iter().zip(&self.failed_why).enumerate();
        failed
            .filter(|(_, (args, _))| args.is_some())
            .map(|(i, (_, why))| {
                let name = self.plan.str(self.plan.resources[i].name);
                (
                    name,
                    why.as_ref().map_or("it failed", |w| w.message.as_str()),
                )
            })
            .collect()
    }

    /// Current value of a resource by name.
    pub fn resource(&self, name: &str) -> Option<&Value> {
        self.plan
            .resources
            .iter()
            .position(|r| self.plan.str(r.name) == name)
            .and_then(|i| self.resources[i].as_ref().map(|r| r.value.get(&self.plan)))
    }

    /// The clock, in milliseconds.
    pub fn now_ms(&self) -> f64 {
        self.now_ms
    }

    /// The plan site a view was instantiated from and the instance path to
    /// it — the `each` keys and active arms crossed — for `layout <node>`
    /// (LLP 1035.002 D6). `None` for a view the instance tree does not own.
    pub fn site_of(&self, view: ViewId) -> Option<(NodesId, Vec<InstanceStep>)> {
        self.tree.as_ref().and_then(|t| t.site(view))
    }

    /// The event kinds a view handles, for a host that attaches listeners.
    pub fn handlers_of(&self, view: ViewId) -> Vec<EventKind> {
        let Some(node) = self
            .ids
            .site(view)
            .filter(|_| self.kernel.node(view).is_some())
        else {
            return Vec::new();
        };
        self.plan
            .node(node)
            .handlers
            .iter()
            .map(|h| self.plan.handler(h).event)
            .collect()
    }

    /// All live listener declarations (bulk host creation), from the views
    /// the runner created — no tree walk.
    pub fn handlers(&self) -> exact_kernel::SortedMap<ViewId, Vec<EventKind>> {
        self.ids
            .sites()
            .filter(|(view, node)| {
                self.plan.node(*node).handlers.len > 0 && self.kernel.node(*view).is_some()
            })
            .map(|(view, node)| {
                let handlers = self.plan.node(node).handlers;
                (
                    view,
                    handlers
                        .iter()
                        .map(|h| self.plan.handler(h).event)
                        .collect(),
                )
            })
            .collect()
    }

    /// The site owning `view` and the frames in force there, found along
    /// the kernel's parent chain.
    fn find(&self, view: ViewId) -> Option<(NodesId, Vec<Frame>)> {
        let mut scanned = 0;
        let found = self.tree.as_ref()?.find(
            view,
            |v| self.kernel.node(v).and_then(|n| n.parent),
            &mut scanned,
        );
        self.lookup_rows.set(scanned);
        found
    }

    /// The kernel roots.
    pub fn roots(&self) -> Vec<ViewId> {
        self.tree.as_ref().map(Tree::roots).unwrap_or_default()
    }

    /// Commands produced since the last take.
    /// Surface inputs that changed since the last take — a canvas node's
    /// arguments, evaluated — published only from commits that applied
    /// (LLP 1009 D2). The host hands them to the app's GPU module.
    pub fn take_surface_updates(&mut self) -> Vec<SurfaceUpdate> {
        std::mem::take(&mut self.surfaces)
    }

    /// The commands actions emitted since the last take.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }

    /// The store's writes since the last take, in order, for the host to
    /// persist (LLP 1018 D1) — only from commits that applied.
    pub fn take_store_writes(&mut self) -> Vec<StoreWrite> {
        self.store.take_writes()
    }

    /// The names the store holds a value for — never the values (LLP 1018
    /// D5: a token is not the agent's to see).
    pub fn store_names(&self) -> Vec<String> {
        self.store.names().into_iter().map(str::to_string).collect()
    }

    /// The store, for a test that reads what an action kept.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Whether resource `name` observed device state (secrets, filesystem,
    /// or SQLite). Bake marks its compiled value as a placeholder, refreshed
    /// when the data source becomes ready (LLP 1018 D4 / LLP 1027 D4).
    pub fn resource_reads_store(&self, name: &str) -> bool {
        self.plan
            .resources
            .iter()
            .position(|r| self.plan.str(r.name) == name)
            .is_some_and(|i| self.store_readers[i])
    }

    /// Whether resource `name` drew secure randomness (LLP 1069.005 D2):
    /// a device read whose value bake leaves out of the plan, so no random
    /// value is shared by every install; the device asks it.
    pub fn resource_draws_entropy(&self, name: &str) -> bool {
        self.plan
            .resources
            .iter()
            .position(|r| self.plan.str(r.name) == name)
            .is_some_and(|i| self.entropy_readers[i])
    }

    /// Deterministic instance work counters, separate from layout and host costs.
    pub fn last_instance_work(&self) -> crate::instance::InstanceWork {
        self.tree
            .as_ref()
            .map(|tree| tree.last_work)
            .unwrap_or_default()
    }

    /// Evaluate every site on every update instead of only those whose
    /// inputs changed: the reference an incremental update must equal
    /// (the differential test runs both). A runtime switch, not a feature.
    pub fn set_full_evaluation(&mut self, full: bool) {
        self.full = full;
    }

    /// Rebind a list row that leaves the window to the item the window
    /// needs next, instead of destroying it and building one (LLP 1078; off
    /// by default). The receipts name every view of a rebound row
    /// `renewed`: a host that turns this on resets what it keeps by view
    /// for each of them, as for a new one.
    pub fn set_row_reuse(&mut self, on: bool) {
        self.reuse = on;
    }

    /// Whether an update failed after the tree began to change (see
    /// [`RunnerError::Poisoned`]).
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn apply(&mut self, mut ops: Vec<exact_kernel::Op>) -> Result<CommitReceipt, RunnerError> {
        if !self.reorder_ops.is_empty() {
            let mut prefix = std::mem::take(&mut self.reorder_ops);
            prefix.append(&mut ops);
            ops = prefix;
        }
        let change = self.router_change()?;
        self.batch += 1;
        let language = self.resolved_locale().to_owned();
        let direction = if self.direction() == "rtl" {
            exact_kernel::Direction::Rtl
        } else {
            exact_kernel::Direction::Ltr
        };
        let shown = std::mem::take(&mut self.shown);
        let mut receipt =
            match self
                .kernel
                .apply_document(0, self.batch, &ops, Some((&language, direction)))
            {
                Ok(receipt) => receipt,
                Err(e) => {
                    self.notes.clear();
                    return Err(e.into());
                }
            };
        let renewed = self.settle_shown(shown, &mut receipt);
        self.tally(&ops, &receipt.touched);
        for note in std::mem::take(&mut self.notes) {
            self.log(note);
        }
        let arena = self.kernel.arena();
        receipt.renewed = renewed
            .into_iter()
            .filter_map(|view| arena.key_of(view))
            .collect();
        // Forget destroyed views once they outnumber the live ones (a
        // detached kernel holds none, and its runner asks for none).
        if !self.kernel.is_detached() && self.ids.remembered() > 2 * self.kernel.live_count() + 256
        {
            let kernel = &self.kernel;
            self.ids.retain(|view| kernel.node(view).is_some());
        }
        let cleanup = self.reconcile_reorder()?;
        if !cleanup.is_empty() {
            self.batch += 1;
            let tail = self.kernel.apply(0, self.batch, &cleanup)?;
            self.tally(&cleanup, &tail.touched);
            receipt.batch = tail.batch;
            receipt.epoch = tail.epoch;
            for key in tail.touched {
                if !receipt.created.contains(&key) && !receipt.touched.contains(&key) {
                    receipt.touched.push(key);
                }
            }
            receipt.layout_invalidated |= tail.layout_invalidated;
        }
        self.commit_router(change);
        self.retire_holds();
        Ok(receipt)
    }

    fn env<'a>(&'a self, params: &'a [Value], frames: &'a [Frame]) -> Env<'a> {
        Env {
            plan: &self.plan,
            strings: &self.strings,
            router: self.router.as_deref(),
            lists: self.links.lists,
            format: self.links.format,
            geometry: None,
            slots: &self.slots,
            derives: &self.derives,
            resources: &self.resource_values,
            params,
            frames,
            now_ms: self.now_ms,
            pending_resources: &self.pending_res,
            failed_resources: &self.failed_args,
            failed_why: &self.failed_why,
            pending_mutations: &self.pending_mut,
            store_dependent_derives: &[],
            store_dependent_resources: &[],
        }
    }

    fn eval(&self, code: Code, params: &[Value], frames: &[Frame]) -> Result<Value, RunnerError> {
        let env = self.env(params, frames);
        Ok(vm::eval(self.plan.code(code), &env, &[])?.value)
    }
}

#[cfg(test)]
mod stream_tests;
