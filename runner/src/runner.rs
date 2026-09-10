//! The runner: boot, actions, events, resources, timers, the clock.
//!
//! @ref LLP 1004 D3 (the refusal tuple) / D4 (the data seam) / D5 (restart)
//!
//! Each event settles derives and changed resource arguments, evaluates all
//! sites, and applies one atomic kernel batch; hosts then lay out and paint.
//! Kernel validation precedes every write; a refusal leaves the kernel untouched.

mod carry;
mod source;
pub use source::{DataError, DataSource};
mod actions;
mod delivery;
mod effects;
mod kept;
mod requests;
mod settlement;
mod timers;
mod transaction;
pub use carry::Carried;
pub use effects::OwnedResourceSnapshot;

use crate::instance::{Ids, InstanceError, SurfaceUpdate, Tree, Update};
use crate::request::{Answer, Outcome, Request, RequestOut};
use crate::store::{Store, StoreWrite};
use crate::vm::{self, Env, Frame, RowSlots, Trap};
use exact_kernel::{CommitReceipt, Kernel, KernelError, ViewId};
use exact_plan::{ActionsId, Code, EventKind, MutationsId, Plan, PlanError, Value};
use std::fmt::Write as _;

/// A host-facing effect an action asked for; executed after commit, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// The capability name.
    pub name: String,
    /// Its arguments.
    pub args: Vec<Value>,
}

/// One timer's commit and the clock it fired at (`Runner::advance_timed`).
#[derive(Debug, Clone)]
pub struct Timed {
    /// The runner's clock when the timer ran, milliseconds.
    pub at_ms: f64,
    /// The commit.
    pub receipt: CommitReceipt,
}

/// What one `advance_timed` did: the commits in order, each at its due
/// time; the clock afterwards — the requested time, or the last time reached
/// before a refusal; and that refusal, if any. Commits before a refusal are
/// kept: they are in the kernel, and a host must show them.
#[derive(Debug)]
pub struct Advanced {
    /// The commits, in order.
    pub receipts: Vec<Timed>,
    /// The clock after the call, milliseconds.
    pub now_ms: f64,
    /// The refusal that stopped the advance, if one did.
    pub error: Option<RunnerError>,
}

/// A host event aimed at a view.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A press on the view.
    Press,
    /// A text input changed to `value`.
    Change(String),
    /// The pointer came over the view (`true`) or left it (`false`) —
    /// `pointerenter`/`pointerleave`, not a bubbling `mouseover`.
    Hover(bool),
    /// The view took the focus.
    Focus,
    /// The view lost the focus.
    Blur,
    /// A key went down while the view had the focus: the key's name as the
    /// web spells it (`"Enter"`, `"ArrowDown"`, `"a"`).
    Key(String),
    /// Enter in an input with a `submit` handler — the web's implicit
    /// submission (HTML forms §4.10.21.2), without a form.
    Submit,
    /// An iframe finished loading (including an error document on the web).
    Load,
    /// An iframe guest posted a string to its parent (@ref LLP 1020 D2).
    Message(String),
}

/// Why the runner refused. The kernel is unchanged.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum RunnerError {
    KernelSchemaMismatch {
        plan: u64,
        kernel: u64,
    },
    /// The plan belongs to another app (LLP 1023 D5): its header names one
    /// identity, this binary's data crate another.
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
    },
    UnknownView(ViewId),
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
    /// An earlier update failed after the instance tree had begun to change;
    /// the runner no longer matches its kernel and must be restarted (D5).
    Poisoned,
    /// `advance` was given a non-finite time.
    NonFiniteClock,
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
    /// A structural region sits at the plan root; only a chain of component
    /// scopes ending at one visual root is permitted.
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
            InstanceError::Effect(error) => *error,
            other => RunnerError::Instance(other),
        }
    }
}

impl From<KernelError> for RunnerError {
    fn from(e: KernelError) -> Self {
        RunnerError::Kernel(e)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResourceState {
    pub(crate) args: Vec<Value>,
    pub(crate) value: Value,
    /// Store revision this answer observed; checked only for known readers.
    pub(crate) store_revision: u64,
}

/// A resource or a mutation, as the target of a request in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Resource(usize),
    Mutation(usize),
    OwnedResource(usize, u64),
    OwnedMutation(usize, u64),
}

/// A request the host is running: the ticket its reply carries, what it
/// answers, and the arguments it was asked with (what `parse` sees).
#[derive(Clone)]
struct PendingReq {
    ticket: u64,
    context: u64,
    /// Same-action send + assignment dispatches once but discards its reply.
    publish: bool,
    target: Target,
    source: String,
    args: Vec<Value>,
}

struct Timer {
    next_ms: f64,
}

/// One plan, one data source, one kernel.
pub struct Runner<D: DataSource> {
    plan: std::rc::Rc<Plan>,
    data: D,
    kernel: Kernel,
    slots: Vec<Value>,
    derives: Vec<Option<Value>>,
    derive_readers: Vec<bool>,
    resources: Vec<Option<ResourceState>>,
    resource_values: Vec<Option<Value>>,
    tree: Option<Tree>,
    ids: Ids,
    now_ms: f64,
    timers: Vec<Timer>,
    batch: u64,
    commands: Vec<Command>,
    surfaces: Vec<SurfaceUpdate>,
    /// Requests in flight (LLP 1016): at most one per resource or mutation.
    pending: Vec<PendingReq>,
    /// `pending` as flags, by resource and by mutation, for expressions.
    pending_res: Vec<bool>,
    pending_mut: Vec<bool>,
    next_ticket: u64,
    next_context: u64,
    /// Live executor calls, including tentative replacements until commit.
    contexts: std::collections::BTreeSet<u64>,
    /// Requests for the host, since the last take.
    requests: Vec<RequestOut>,
    /// Resources an action asked to re-request; consumed by the next settle.
    refresh_next: Vec<usize>,
    /// Durable client state (LLP 1018 D1): the host's snapshot, and the
    /// writes since for the host to persist.
    store: Store,
    /// Which resources consulted the store when they settled (bake gives
    /// them no compiled value, LLP 1018 D4).
    store_readers: Vec<bool>,
    /// Store-reading resources shown from a placeholder — a kept answer or
    /// the compiled empty-store value — to ask again at `data_ready`.
    stale: Vec<bool>,
    /// Whether fresh answers of store-reading resources are kept for the
    /// next boot: only for a source that may not be ready at boot.
    keeps_answers: bool,
    poisoned: bool,
    booting: bool,
    /// What this binary and its update store know about delivery (LLP 1030
    /// D4, D7): the embedded answer until a host says otherwise.
    delivery: crate::delivery::Delivery,
    /// What happened, one line each, for the agent API's `logs`: the last
    /// [`JOURNAL_RING`] lines, and how many were dropped before them.
    journal: std::collections::VecDeque<String>,
    journal_start: usize,
}

/// How many journal lines the runner retains (about an hour of a one-second
/// timer); older ones are dropped, and `logs` reports where its window starts.
pub const JOURNAL_RING: usize = 4096;

/// Largest accepted clock value: JavaScript's exact integer domain in ms.
pub const MAX_CLOCK_MS: f64 = 9_007_199_254_740_991.0;

/// Maximum timer commits one call to [`Runner::advance_timed`] may perform.
pub const TIMER_FIRE_LIMIT: usize = 4096;

impl<D: DataSource> Runner<D> {
    /// Boot: refuse a plan built against another kernel schema, evaluate
    /// initial state, settle resources (compiled data first, the source
    /// otherwise), realize the tree, and apply the first frame's ops.
    pub fn boot(plan: Plan, data: D, kernel: Kernel) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(plan, data, kernel, None, Vec::new(), Default::default())
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
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(
            plan,
            data,
            kernel,
            Some(carried),
            carried.store.clone(),
            Default::default(),
        )
    }

    /// Everything a reload keeps.
    pub fn carry(&self) -> Carried {
        Carried {
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
                .filter(|(i, (row, _))| row.owner.is_none() && !self.pending_res[*i])
                .filter_map(|(_, (r, s))| {
                    s.as_ref().map(|s| {
                        (
                            self.plan.str(r.name).to_string(),
                            s.args.clone(),
                            s.value.clone(),
                        )
                    })
                })
                .collect(),
            store_readers: self
                .plan
                .resources
                .iter()
                .enumerate()
                .filter(|(i, row)| row.owner.is_none() && self.store_readers[*i])
                .map(|(_, resource)| self.plan.str(resource.name).to_string())
                .collect(),
            now_ms: self.now_ms,
            store: self.store.snapshot(),
        }
    }

    fn boot_inner(
        plan: Plan,
        mut data: D,
        kernel: Kernel,
        carried: Option<&Carried>,
        snapshot: Vec<(String, String)>,
        delivery: crate::delivery::Delivery,
    ) -> Result<Runner<D>, RunnerError> {
        if let Some(carried) = carried {
            if !carried.now_ms.is_finite() {
                return Err(RunnerError::NonFiniteClock);
            }
            if !(0.0..=MAX_CLOCK_MS).contains(&carried.now_ms) {
                return Err(RunnerError::ClockOutOfRange);
            }
        }
        plan.validate().map_err(RunnerError::Plan)?;
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
        if plan.regions.iter().any(|r| {
            r.parent.is_none() && r.arm.is_none() && r.kind != exact_plan::RegionKind::Scope
        }) {
            return Err(RunnerError::RootRegion);
        }
        let same_logic = carried.is_none_or(|c| c.data_revision.as_deref() == data.revision());
        data.bind(&plan);
        let store = Store::new(data.grants(), snapshot);
        let store_readers = plan
            .resources
            .iter()
            .map(|resource| {
                resource.reader
                    || plan.str(resource.source) == crate::delivery::SOURCE
                    || (same_logic
                        && carried.is_some_and(|carried| {
                            let name = plan.str(resource.name);
                            carried.store_readers.iter().any(|reader| reader == name)
                        }))
            })
            .collect();
        let mut runner = Runner {
            plan: std::rc::Rc::new(plan),
            data,
            kernel,
            slots: Vec::new(),
            derives: Vec::new(),
            derive_readers: Vec::new(),
            resources: Vec::new(),
            resource_values: Vec::new(),
            tree: None,
            ids: Ids::default(),
            now_ms: 0.0,
            timers: Vec::new(),
            batch: 0,
            commands: Vec::new(),
            surfaces: Vec::new(),
            pending: Vec::new(),
            pending_res: Vec::new(),
            pending_mut: Vec::new(),
            next_ticket: 1,
            next_context: 1,
            contexts: Default::default(),
            requests: Vec::new(),
            refresh_next: Vec::new(),
            store,
            store_readers,
            stale: Vec::new(),
            keeps_answers: false,
            delivery,
            poisoned: false,
            booting: true,
            journal: std::collections::VecDeque::new(),
            journal_start: 0,
        };
        // Slots: carried values where the name and type still fit, else
        // initial values, in order (an initializer may read earlier slots).
        for i in 0..runner.plan.slots.len() {
            let ty = runner.plan.slots[i].ty;
            let name = runner.plan.str(runner.plan.slots[i].name);
            if runner.plan.slots[i].owner.is_some() {
                // A row slot (LLP 1017 P4c) lives on its rows, initialized as
                // each row is created; nothing to carry, nothing to hold here.
                runner.slots.push(Value::Unit);
                continue;
            }
            let kept = carried
                .and_then(|c| c.slots.iter().find(|(n, _)| n == name))
                .map(|(_, v)| v.clone())
                .filter(|v| v.conforms(&runner.plan, ty));
            let v = match kept {
                Some(v) => v,
                None => runner.eval(runner.plan.slots[i].init, &[], &[])?,
            };
            if !v.conforms(&runner.plan, ty) {
                return Err(RunnerError::SlotType {
                    slot: name.to_string(),
                });
            }
            runner.slots.push(v);
        }
        runner.derives = vec![None; runner.plan.derives.len()];
        // Resources: carried where the name is still declared and the value
        // still fits the declared shape; a carried value can refuse nothing.
        runner.resources = (0..runner.plan.resources.len())
            .map(|i| {
                let name = runner.plan.str(runner.plan.resources[i].name);
                carried
                    .filter(|_| same_logic && runner.plan.resources[i].owner.is_none())
                    .and_then(|c| c.resources.iter().find(|(n, _, _)| n == name))
                    .filter(|(_, _, value)| runner.check_shape(i, value).is_ok())
                    .map(|(_, args, value)| ResourceState {
                        args: args.clone(),
                        value: value.clone(),
                        store_revision: runner.store.revision(),
                    })
            })
            .collect();
        // Store-reading resources when the data source is not ready (a
        // TypeScript module before its host loads it, LLP 1027 D4): the
        // answer kept from the last launch seeds the first frame if its
        // arguments still match and its value still fits; the compiled
        // empty-store placeholder is the fallback (settlement); either way
        // the resource is asked again at `data_ready`.
        let ready = runner.data.ready();
        runner.keeps_answers = !ready || carried.is_some_and(|c| c.keeps_answers);
        runner.stale = vec![false; runner.plan.resources.len()];
        if !ready {
            for i in 0..runner.plan.resources.len() {
                if runner.plan.resources[i].owner.is_some() || !runner.plan.resources[i].reader {
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
                    .filter(|(_, value)| runner.check_shape(i, value).is_ok());
                if let Some((args, value)) = seed {
                    runner.resources[i] = Some(ResourceState {
                        args,
                        value,
                        store_revision: runner.store.revision(),
                    });
                }
            }
        }
        runner.resource_values = vec![None; runner.plan.resources.len()];
        runner.pending_res = vec![false; runner.plan.resources.len()];
        runner.pending_mut = vec![false; runner.plan.mutations.len()];
        runner.now_ms = carried.map_or(0.0, |c| c.now_ms);
        // A carried boot never takes compiled data: it was baked for the
        // initial state, and the carried state is not that.
        runner.settle(carried.is_none())?;
        let now = runner.now_ms;
        runner.timers = runner
            .plan
            .timers
            .iter()
            .map(|t| Timer {
                next_ms: if t.owner.is_none() {
                    now + t.interval_ms as f64
                } else {
                    f64::INFINITY
                },
            })
            .collect();
        // Scope resources settle during the same tree walk as their children.
        let (ops, surfaces) = runner.walk()?;
        runner.booting = false;
        let roots = runner.roots().len();
        if roots != 1 {
            return Err(RunnerError::NotOneRoot(roots));
        }
        let receipt = runner.apply(ops)?;
        runner.surfaces = surfaces;
        let line = format!(
            "boot{}: {} nodes, epoch {}",
            if carried.is_some() { " (carried)" } else { "" },
            runner.kernel.live_count(),
            receipt.epoch
        );
        runner.log(line);
        Ok(runner)
    }

    /// Append a line to the journal the agent API's `logs` reads, stamped
    /// with the clock. Hosts add their own lines here (an image loaded, a
    /// layout refusal) so one read sees everything in order.
    pub fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        self.journal
            .push_back(format!("t={} {line}", crate::agent::num(self.now_ms)));
        if self.journal.len() > JOURNAL_RING {
            self.journal.pop_front();
            self.journal_start += 1;
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
            Ok(r) => format!(
                "{what} → epoch {} (+{} −{} ~{})",
                r.epoch,
                r.created.len(),
                r.destroyed.len(),
                r.touched.len()
            ),
            Err(e) if self.poisoned && !was_poisoned => {
                format!("{what} poisoned the runner: {e:?}")
            }
            Err(e) => format!("{what} refused: {e:?}"),
        };
        self.log(line);
    }

    /// The kernel, for layout and export.
    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    /// The kernel, mutably (a host lays out through it).
    pub fn kernel_mut(&mut self) -> &mut Kernel {
        &mut self.kernel
    }

    /// The plan.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// The data source.
    pub fn data(&mut self) -> &mut D {
        &mut self.data
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

    /// Current value of a resource by name.
    pub fn resource(&self, name: &str) -> Option<&Value> {
        self.plan
            .resources
            .iter()
            .position(|r| self.plan.str(r.name) == name)
            .and_then(|i| self.resources[i].as_ref().map(|r| &r.value))
    }

    /// The clock, in milliseconds.
    pub fn now_ms(&self) -> f64 {
        self.now_ms
    }

    /// The event kinds a view handles, for a host that attaches listeners.
    pub fn handlers_of(&self, view: ViewId) -> Vec<EventKind> {
        let Some((node, _)) = self.tree.as_ref().and_then(|t| t.find(view)) else {
            return Vec::new();
        };
        self.plan
            .node(node)
            .handlers
            .iter()
            .map(|h| self.plan.handler(h).event)
            .collect()
    }

    /// All live listener declarations in one tree walk (bulk host creation).
    pub fn handlers(&self) -> std::collections::BTreeMap<ViewId, Vec<EventKind>> {
        self.tree
            .as_ref()
            .map_or_else(Default::default, |t| t.handlers(&self.plan))
    }

    /// Whether the plan has timers (a host then drives `advance`).
    pub fn has_timers(&self) -> bool {
        !self.plan.timers.is_empty()
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

    /// Deliver a host event to `view`: find its handler, evaluate the curried
    /// arguments in the instance's scope now, run the action, update.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> Result<CommitReceipt, RunnerError> {
        let mut what = format!(
            "{} view {view}",
            match &event {
                Event::Press => "press",
                Event::Change(_) => "change",
                Event::Hover(true) => "hover in",
                Event::Hover(false) => "hover out",
                Event::Focus => "focus",
                Event::Blur => "blur",
                Event::Key(_) => "key",
                Event::Submit => "submit",
                Event::Load => "load",
                Event::Message(_) => "message",
            }
        );
        let was_poisoned = self.poisoned;
        let result = self.dispatch_inner(view, event, &mut what);
        self.log_outcome(&what, &result, was_poisoned);
        result
    }

    fn dispatch_inner(
        &mut self,
        view: ViewId,
        event: Event,
        what: &mut String,
    ) -> Result<CommitReceipt, RunnerError> {
        let (node, frames) = self
            .tree
            .as_ref()
            .and_then(|t| t.find(view))
            .ok_or(RunnerError::UnknownView(view))?;
        let (kind, payload, name) = match &event {
            Event::Press => (EventKind::Press, None, "press"),
            Event::Change(text) => (EventKind::Change, Some(Value::str(text)), "change"),
            Event::Hover(over) => (EventKind::Hover, Some(Value::Bool(*over)), "hover"),
            Event::Focus => (EventKind::Focus, None, "focus"),
            Event::Blur => (EventKind::Blur, None, "blur"),
            Event::Key(key) => (EventKind::Key, Some(Value::str(key)), "key"),
            Event::Submit => (EventKind::Submit, None, "submit"),
            Event::Load => (EventKind::Load, None, "load"),
            Event::Message(message) => (EventKind::Message, Some(Value::str(message)), "message"),
        };
        let row = self.plan.node(node);
        let handler = row
            .handlers
            .iter()
            .map(|h| self.plan.handler(h))
            .find(|h| h.event == kind)
            .cloned()
            .ok_or(RunnerError::NoHandler { view, event: name })?;
        let mut args = Vec::new();
        for a in handler.args.iter() {
            let code = self.plan.arg(a).expr;
            args.push(self.eval(code, &[], &frames)?);
        }
        if let Some(p) = payload {
            args.push(p);
        }
        let _ = write!(
            what,
            " ({})",
            self.plan.str(self.plan.action(handler.action).name)
        );
        self.run_action(handler.action, args, &frames)
    }

    /// Run an action by name with `args` — what a test or an agent does.
    pub fn act(&mut self, name: &str, args: Vec<Value>) -> Result<CommitReceipt, RunnerError> {
        let what = format!("act {name}");
        let was_poisoned = self.poisoned;
        let result = match self
            .plan
            .actions
            .iter()
            .position(|a| self.plan.str(a.name) == name)
            .map(|i| ActionsId(i as u32))
        {
            Some(id) => self.run_action(id, args, &[]),
            None => Err(RunnerError::NoHandler {
                view: 0,
                event: "action",
            }),
        };
        self.log_outcome(&what, &result, was_poisoned);
        result
    }

    /// Re-evaluate every site and apply one batch. A failure here means the
    /// instance tree and the kernel may disagree; the runner is poisoned and
    /// the host restarts it — never a half-applied frame.
    fn update(&mut self) -> Result<CommitReceipt, RunnerError> {
        let (ops, surfaces) = match self.walk() {
            Ok(out) => out,
            Err(error) => {
                self.poison();
                return Err(error);
            }
        };
        match self.apply(ops) {
            Ok(receipt) => {
                self.surfaces.extend(surfaces);
                Ok(receipt)
            }
            Err(e) => {
                self.poison();
                Err(e)
            }
        }
    }

    fn poison(&mut self) {
        self.poisoned = true;
        self.commands.clear();
        self.requests.clear();
        self.pending.clear();
        self.flush_contexts();
        self.refresh_next.clear();
        for frames in self.scope_frames() {
            let mut cell = frames.last().unwrap().scope.as_ref().unwrap().borrow_mut();
            for resource in cell.resources.values_mut() {
                resource.pending = false;
                resource.refresh = false;
            }
            cell.mutations.clear();
            cell.timers.clear();
        }
        self.sync_pending_flags();
    }

    /// Whether an update failed after the tree began to change (see
    /// [`RunnerError::Poisoned`]).
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn apply(&mut self, ops: Vec<exact_kernel::Op>) -> Result<CommitReceipt, RunnerError> {
        self.batch += 1;
        let receipt = self.kernel.apply(0, self.batch, &ops)?;
        self.flush_contexts();
        Ok(receipt)
    }

    fn env<'a>(&'a self, params: &'a [Value], frames: &'a [Frame]) -> Env<'a> {
        Env {
            plan: &self.plan,
            slots: &self.slots,
            derives: &self.derives,
            resources: &self.resource_values,
            params,
            frames,
            now_ms: self.now_ms,
            pending_resources: &self.pending_res,
            pending_mutations: &self.pending_mut,
            store_dependent_derives: &self.derive_readers,
            store_dependent_resources: &self.store_readers,
        }
    }

    fn eval(&self, code: Code, params: &[Value], frames: &[Frame]) -> Result<Value, RunnerError> {
        let env = self.env(params, frames);
        Ok(vm::eval(self.plan.code(code), &env, &[])?.value)
    }

    fn query(&mut self, i: usize, args: &[Value]) -> Result<(Answer, u64), RunnerError> {
        let row = &self.plan.resources[i];
        let source = self.plan.str(row.source).to_string();
        let resource = self.plan.str(row.name).to_string();
        // Delivery is the runner's own (LLP 1030 D7): the data seam never
        // sees it, and a data crate could not answer it if it did.
        if source == crate::delivery::SOURCE {
            return self
                .delivery_answer(i)
                .map(|v| (Answer::Now(v), 0))
                .map_err(|error| RunnerError::Data { resource, error });
        }
        self.answer_call(&source, args)
            .map_err(|error| RunnerError::Data { resource, error })
    }

    /// Recheck the plan relation at the write boundary instead of trusting
    /// that a decoded plan is the only possible caller.
    fn mutation_slot(&self, mutation: usize) -> Result<usize, RunnerError> {
        let mutation = MutationsId(mutation as u32);
        self.plan
            .validate_mutation_slot(mutation)
            .map_err(RunnerError::Plan)?;
        Ok(self.plan.mutation(mutation).slot.0 as usize)
    }
}

impl<D: DataSource> Drop for Runner<D> {
    fn drop(&mut self) {
        for context in std::mem::take(&mut self.contexts) {
            self.data.cancel_scoped(context);
        }
    }
}
