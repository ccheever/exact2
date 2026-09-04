//! The runner: boot, actions, events, resources, timers, the clock.
//!
//! @ref LLP 1004 D3 (the refusal tuple) / D4 (the data seam) / D5 (restart)
//!
//! Each event settles derives and changed resource arguments, evaluates all
//! sites, and applies one atomic kernel batch; hosts then lay out and paint.
//! Kernel validation precedes every write; a refusal leaves the kernel untouched.

mod delivery;
mod kept;
mod settlement;

use crate::instance::{Ids, InstanceError, SurfaceUpdate, Tree, Update};
use crate::request::{Answer, Outcome, Request, RequestOut};
use crate::store::{Store, StoreWrite};
use crate::vm::{self, Env, Frame, RowSlots, Trap};
use exact_kernel::{CommitReceipt, Kernel, KernelError, ViewId};
use exact_plan::{ActionsId, Code, EventKind, MutationsId, Plan, PlanError, Value};
use std::fmt::Write as _;

/// The app's data source: the one seam through which computation enters
/// (LLP 1004 D4). Implemented once, in Rust, by the app's data crate.
pub trait DataSource {
    /// Answer a resource's or a mutation's request now. `args` are the
    /// resource's argument expressions evaluated against current state, or
    /// a `send`'s arguments. Bake and every in-process source use this.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError>;

    /// The app identity, reverse-DNS (LLP 1023 D5) — the one declaration:
    /// bake writes it into the plan header, and boot refuses a plan whose
    /// header names a different app. Empty is unnamed — a fixture or a
    /// stand-in — and unnamed matches anything.
    fn app_id(&self) -> &str {
        ""
    }

    /// Answer now, or hand back a request the host will run (LLP 1016 D1:
    /// the runner never does I/O). The default answers `query` now; a source
    /// that reaches outside the process overrides this and [`parse`]. The
    /// [`Store`] is the app's durable state (LLP 1018 D1) — the host's
    /// snapshot, read here synchronously; a write rides the commit out.
    ///
    /// [`parse`]: DataSource::parse
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let _ = store;
        self.query(source, args).map(Answer::Now)
    }

    /// The value of a resource or mutation from what the host brought back
    /// for a request `answer` handed out, in the shape the declaration
    /// names — or one more request (LLP 1027 D1a: a TypeScript `answer`
    /// that awaits a second `fetch` is pending again, on the same target,
    /// with the same arguments). No I/O, no host — the store is the one
    /// thing it may write (a token from a reply, LLP 1018 D5). A failure on
    /// the wire is an `Outcome` too — what the app sees is the source's to
    /// decide (D4). A source that never answers later need not implement it.
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let _ = (store, args, outcome);
        Err(DataError::UnknownSource(source.to_string()))
    }

    /// What the app may reach and keep (LLP 1016 D6, LLP 1018 D3; ibex LLP
    /// 0067): one grant per line — `net.fetch <url prefix>`, `secret.keep
    /// <name>`. A request outside them fails as `Refused` on every host
    /// before any executor sees it; a secret outside them reads as absent
    /// and refuses a write. Empty: nothing.
    fn grants(&self) -> &'static str {
        ""
    }

    /// The plan this source answers for — once, at boot, after the identity
    /// gate and before any answer (LLP 1027 D2). An executor that marshals
    /// by the plan's declared shapes reads the `sources` table here; a Rust
    /// crate has nothing to learn and ignores it.
    fn bind(&mut self, plan: &Plan) {
        let _ = plan;
    }

    /// Whether answers are available now. A TypeScript module before its
    /// host loads it is not (LLP 1027 D4): the runner then boots every
    /// store-reading resource from its kept answer or its compiled
    /// empty-store placeholder, and asks again at [`Runner::data_ready`].
    fn ready(&self) -> bool {
        true
    }
}

/// Why a data source could not answer.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum DataError {
    UnknownSource(String),
    BadArguments(String),
    Unavailable(String),
}

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
    value: Value,
    /// Store revision this answer observed; checked only for known readers.
    store_revision: u64,
}

/// A resource or a mutation, as the target of a request in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Resource(usize),
    Mutation(usize),
}

/// A request the host is running: the ticket its reply carries, what it
/// answers, and the arguments it was asked with (what `parse` sees).
#[derive(Clone)]
struct PendingReq {
    ticket: u64,
    target: Target,
    source: String,
    args: Vec<Value>,
}

struct Timer {
    next_ms: f64,
}

/// What survives a reload: state by name, settled resources by name with
/// the arguments they answered and their store dependency, and the clock. A new
/// plan takes each slot
/// whose name it still declares and whose carried value conforms to the
/// slot's (possibly new) type; everything else starts from its initializer.
/// Resources are reused only where their arguments still match, so a
/// carried `stationId` gets its own board, never the baked one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Carried {
    /// Slot name → value.
    pub slots: Vec<(String, Value)>,
    /// Resource name → (arguments, value).
    pub resources: Vec<(String, Vec<Value>, Value)>,
    /// Names of carried resources whose answer depends on the store.
    pub store_readers: Vec<String>,
    /// The clock, milliseconds.
    pub now_ms: f64,
    /// The store's kept values (LLP 1018): what the host has persisted.
    pub store: Vec<(String, String)>,
}

/// One plan, one data source, one kernel.
pub struct Runner<D: DataSource> {
    plan: Plan,
    data: D,
    kernel: Kernel,
    slots: Vec<Value>,
    derives: Vec<Option<Value>>,
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
        Runner::boot_inner(plan, data, kernel, None, Vec::new())
    }

    /// Boot with the host's snapshot of the app's kept secrets (LLP 1018
    /// D1): what the platform's store holds under the names the data
    /// crate's grants allow, read by the host before this call. A resource
    /// with no compiled value — one that read the store at bake — answers
    /// from it now, so the first frame is a returning user's.
    pub fn boot_stored(
        plan: Plan,
        data: D,
        kernel: Kernel,
        snapshot: Vec<(String, String)>,
    ) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(plan, data, kernel, None, snapshot)
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
        Runner::boot_inner(plan, data, kernel, Some(carried), carried.store.clone())
    }

    /// Everything a reload keeps.
    pub fn carry(&self) -> Carried {
        Carried {
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
                .filter_map(|(r, s)| {
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
                .filter(|(i, _)| self.store_readers[*i])
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
        if plan
            .regions
            .iter()
            .any(|r| r.parent.is_none() && r.arm.is_none())
        {
            return Err(RunnerError::RootRegion);
        }
        data.bind(&plan);
        let store = Store::new(data.grants(), snapshot);
        let store_readers = plan
            .resources
            .iter()
            .map(|resource| {
                resource.reader
                    || carried.is_some_and(|carried| {
                        let name = plan.str(resource.name);
                        carried.store_readers.iter().any(|reader| reader == name)
                    })
            })
            .collect();
        let mut runner = Runner {
            plan,
            data,
            kernel,
            slots: Vec::new(),
            derives: Vec::new(),
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
            requests: Vec::new(),
            refresh_next: Vec::new(),
            store,
            store_readers,
            stale: Vec::new(),
            keeps_answers: false,
            delivery: crate::delivery::Delivery::default(),
            poisoned: false,
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
        runner.keeps_answers = !ready;
        runner.stale = vec![false; runner.plan.resources.len()];
        if !ready {
            for i in 0..runner.plan.resources.len() {
                if !runner.plan.resources[i].reader {
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
                next_ms: now + t.interval_ms as f64,
            })
            .collect();
        // First frame.
        let mut ids = std::mem::take(&mut runner.ids);
        let (tree, ops, surfaces) = {
            let mut u = Update {
                env: runner.env(&[], &[]),
                ids: &mut ids,
                ops: Vec::new(),
                surfaces: Vec::new(),
            };
            let tree = Tree::create(&mut u)?;
            (tree, u.ops, u.surfaces)
        };
        runner.ids = ids;
        runner.tree = Some(tree);
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

    /// Whether resource `name` consulted the store when it settled: bake
    /// gives such a resource no compiled value (LLP 1018 D4).
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

    /// Move the clock to `now_ms`, firing every timer due, in order — the
    /// commits alone, or the refusal that stopped it. Tests use this; a host
    /// uses [`Runner::advance_timed`], which keeps the commits before a
    /// refusal and the time each was made.
    pub fn advance(&mut self, now_ms: f64) -> Result<Vec<CommitReceipt>, RunnerError> {
        let a = self.advance_timed(now_ms);
        match a.error {
            Some(e) => Err(e),
            None => Ok(a.receipts.into_iter().map(|t| t.receipt).collect()),
        }
    }

    /// Move the clock to `now_ms`, firing every timer due, in order, each at
    /// its own due time. A refusal stops the advance there: the commits so
    /// far are returned with their times, the clock stays at the refusing
    /// timer's due time, and the refusal rides along.
    pub fn advance_timed(&mut self, now_ms: f64) -> Advanced {
        let mut receipts = Vec::new();
        if !now_ms.is_finite() {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::NonFiniteClock),
            };
        }
        if now_ms < self.now_ms {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: None,
            };
        }
        if now_ms > MAX_CLOCK_MS {
            return Advanced {
                receipts,
                now_ms: self.now_ms,
                error: Some(RunnerError::ClockOutOfRange),
            };
        }
        loop {
            // The earliest due timer, deterministic by index on ties.
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|(_, t)| t.next_ms <= now_ms)
                .min_by(|(ia, a), (ib, b)| {
                    a.next_ms.partial_cmp(&b.next_ms).unwrap().then(ia.cmp(ib))
                })
                .map(|(i, t)| (i, t.next_ms));
            let Some((i, at)) = due else { break };
            if receipts.len() == TIMER_FIRE_LIMIT {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::TimerFireLimit {
                        limit: TIMER_FIRE_LIMIT,
                    }),
                };
            }
            let interval = self.plan.timers[i].interval_ms as f64;
            let next_ms = at + interval;
            if !next_ms.is_finite() || next_ms <= at {
                return Advanced {
                    receipts,
                    now_ms: self.now_ms,
                    error: Some(RunnerError::ClockDidNotAdvance { timer: i }),
                };
            }
            self.now_ms = at;
            self.timers[i].next_ms = next_ms;
            let action = self.plan.timers[i].action;
            let was_poisoned = self.poisoned;
            let result = self.run_action(action, Vec::new(), &[]);
            match result {
                Ok(receipt) => receipts.push(Timed { at_ms: at, receipt }),
                Err(e) => {
                    let what = format!(
                        "timer {} ({})",
                        i,
                        self.plan.str(self.plan.action(action).name)
                    );
                    let failed = Err(e);
                    self.log_outcome(&what, &failed, was_poisoned);
                    return Advanced {
                        receipts,
                        now_ms: self.now_ms,
                        error: failed.err(),
                    };
                }
            }
        }
        self.now_ms = now_ms;
        if !receipts.is_empty() {
            let line = format!(
                "advance → {} timer{} fired, epoch {}",
                receipts.len(),
                if receipts.len() == 1 { "" } else { "s" },
                receipts.last().map_or(0, |t| t.receipt.epoch)
            );
            self.log(line);
        }
        Advanced {
            receipts,
            now_ms,
            error: None,
        }
    }

    /// Run an action: its body, its sends, its writes, settlement, the
    /// update — one commit. What it kept in the store is journaled once the
    /// commit stands and rolled back with everything else when it does not
    /// (LLP 1018 D1): nothing reaches the host from a refused action.
    fn run_action(
        &mut self,
        action: ActionsId,
        args: Vec<Value>,
        frames: &[Frame],
    ) -> Result<CommitReceipt, RunnerError> {
        let kept = self.store.checkpoint();
        let since = kept.writes;
        let result = self.run_action_inner(action, args, frames);
        match &result {
            Ok(_) => self.log_store_writes(since),
            Err(_) => self.store.restore(kept),
        }
        result
    }

    /// Journal the store's writes from index `since`: the names, never the
    /// values.
    fn log_store_writes(&mut self, since: usize) {
        let writes = self.store.writes();
        let lines: Vec<String> = writes[since.min(writes.len())..]
            .iter()
            .map(|w| match &w.value {
                Some(_) => format!("store {}", w.name),
                None => format!("forget {}", w.name),
            })
            .collect();
        for line in lines {
            self.log(line);
        }
    }

    fn run_action_inner(
        &mut self,
        action: ActionsId,
        args: Vec<Value>,
        frames: &[Frame],
    ) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let row = self.plan.action(action).clone();
        let expected = row.params.len as usize;
        if args.len() != expected {
            return Err(RunnerError::Arity {
                action: self.plan.str(row.name).to_string(),
                expected,
                actual: args.len(),
            });
        }
        for (i, p) in row.params.iter().enumerate() {
            let param = self.plan.param(p);
            if !args[i].conforms(&self.plan, param.ty) {
                return Err(RunnerError::ArgumentType {
                    action: self.plan.str(row.name).to_string(),
                    param: self.plan.str(param.name).to_string(),
                });
            }
        }
        let allowed: Vec<u32> = row
            .writes
            .iter()
            .map(|w| self.plan.write(w).slot.0)
            .collect();
        let outcome = {
            // The frames in force at the view the event hit (LLP 1017 P4c):
            // a row action reads and writes its row through them.
            let env = self.env(&args, frames);
            vm::eval(self.plan.code(row.body), &env, &allowed)?
        };
        // Sends (LLP 1016 §4): each asks the source now. An answer lands in
        // the mutation's slot inside this commit; a request goes to the host
        // once the commit stands.
        let mut later: Vec<(usize, String, Vec<Value>, Request)> = Vec::new();
        let mut answered: Vec<(u32, Value)> = Vec::new();
        for (m, source, sargs) in &outcome.sends {
            let m = *m as usize;
            let mrow = self.plan.mutations[m].clone();
            let name = self.plan.str(mrow.name).to_string();
            let answer = self
                .data
                .answer(&mut self.store, source, sargs)
                .map_err(|error| RunnerError::Data {
                    resource: name.clone(),
                    error,
                })?;
            match answer {
                Answer::Now(v) => {
                    if !v.conforms(&self.plan, mrow.ty) {
                        return Err(RunnerError::Shape { resource: name });
                    }
                    let slot = self.mutation_slot(m)?;
                    answered.push((slot as u32, Value::some(v)));
                }
                Answer::Later(request) => later.push((m, source.clone(), sargs.clone(), request)),
            }
        }
        // Commit the writes, then everything downstream. If settlement refuses
        // (a data source or shape refusal), the writes and commands roll back
        // and the kernel is exactly as it was.
        for (slot, value) in outcome
            .writes
            .iter()
            .map(|(s, v)| (s, v))
            .chain(outcome.row_writes.iter().map(|(s, v, _)| (s, v)))
        {
            if !value.conforms(&self.plan, self.plan.slots[*slot as usize].ty) {
                return Err(RunnerError::SlotType {
                    slot: self
                        .plan
                        .str(self.plan.slots[*slot as usize].name)
                        .to_string(),
                });
            }
        }
        // Row writes land in their rows now, remembered for a rollback.
        let mut row_undo: Vec<(RowSlots, u32, Option<Value>)> = Vec::new();
        for (slot, value, rows) in outcome.row_writes {
            let old = rows.borrow_mut().insert(slot, value);
            row_undo.push((rows, slot, old));
        }
        let saved_slots = self.slots.clone();
        let saved_commands = self.commands.len();
        let saved_pending_mut = self.pending_mut.clone();
        for (slot, value) in answered {
            self.slots[slot as usize] = value;
        }
        let written: Vec<u32> = outcome.writes.iter().map(|(s, _)| *s).collect();
        for (slot, value) in outcome.writes {
            self.slots[slot as usize] = value;
        }
        for (name, args) in outcome.commands {
            self.commands.push(Command { name, args });
        }
        // An assignment to a mutation's slot tentatively makes it not
        // pending. The pending map is changed only after settlement stands:
        // a refused assignment did not change what reply the view wants.
        let assigned: Vec<usize> = (0..self.plan.mutations.len())
            .filter(|m| written.contains(&self.plan.mutations[*m].slot.0))
            .collect();
        for m in &assigned {
            self.pending_mut[*m] = false;
        }
        for (m, _, _, _) in &later {
            if !assigned.contains(m) {
                self.pending_mut[*m] = true;
            }
        }
        self.refresh_next = outcome.refreshes.iter().map(|r| *r as usize).collect();
        if let Err(e) = self.settle(false) {
            self.slots = saved_slots;
            for (rows, slot, old) in row_undo.into_iter().rev() {
                match old {
                    Some(v) => rows.borrow_mut().insert(slot, v),
                    None => rows.borrow_mut().remove(&slot),
                };
            }
            self.commands.truncate(saved_commands);
            self.pending_mut = saved_pending_mut;
            return Err(e);
        }
        for m in &assigned {
            self.forget(Target::Mutation(*m));
        }
        for (m, source, args, request) in later {
            self.enqueue(Target::Mutation(m), source, args, request, false);
            if assigned.contains(&m) {
                self.forget(Target::Mutation(m));
            }
        }
        let commands: Vec<String> = self.commands[saved_commands..]
            .iter()
            .map(|c| {
                let mut s = format!("command {}(", c.name);
                for (i, a) in c.args.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    crate::agent::untyped_json(a, &mut s);
                }
                s.push(')');
                s
            })
            .collect();
        let result = self.update();
        // Commands are journaled only once the update committed: a failure
        // there poisons the runner and clears them.
        if result.is_ok() {
            for line in commands {
                self.log(line);
            }
        }
        result
    }

    /// Re-evaluate every site and apply one batch. A failure here means the
    /// instance tree and the kernel may disagree; the runner is poisoned and
    /// the host restarts it — never a half-applied frame.
    fn update(&mut self) -> Result<CommitReceipt, RunnerError> {
        let mut tree = self.tree.take().expect("booted");
        let mut ids = std::mem::take(&mut self.ids);
        let result = {
            let mut u = Update {
                env: self.env(&[], &[]),
                ids: &mut ids,
                ops: Vec::new(),
                surfaces: Vec::new(),
            };
            tree.update(&mut u).map(|_| (u.ops, u.surfaces))
        };
        self.ids = ids;
        self.tree = Some(tree);
        let (ops, surfaces) = match result {
            Ok(x) => x,
            Err(e) => {
                self.poison();
                return Err(e.into());
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
        self.sync_pending_flags();
    }

    /// Whether an update failed after the tree began to change (see
    /// [`RunnerError::Poisoned`]).
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn apply(&mut self, ops: Vec<exact_kernel::Op>) -> Result<CommitReceipt, RunnerError> {
        self.batch += 1;
        Ok(self.kernel.apply(0, self.batch, &ops)?)
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
            store_dependent_derives: &[],
            store_dependent_resources: &[],
        }
    }

    fn eval(&self, code: Code, params: &[Value], frames: &[Frame]) -> Result<Value, RunnerError> {
        let env = self.env(params, frames);
        Ok(vm::eval(self.plan.code(code), &env, &[])?.value)
    }

    fn query(&mut self, i: usize, args: &[Value]) -> Result<Answer, RunnerError> {
        let row = &self.plan.resources[i];
        let source = self.plan.str(row.source).to_string();
        let resource = self.plan.str(row.name).to_string();
        // Delivery is the runner's own (LLP 1030 D7): the data seam never
        // sees it, and a data crate could not answer it if it did.
        if source == crate::delivery::SOURCE {
            return self
                .delivery_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        self.data
            .answer(&mut self.store, &source, args)
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

    fn target_name(&self, t: Target) -> String {
        match t {
            Target::Resource(i) => self.plan.str(self.plan.resources[i].name).to_string(),
            Target::Mutation(m) => self.plan.str(self.plan.mutations[m].name).to_string(),
        }
    }

    /// Drop the request in flight for `target`, if any: its reply, when it
    /// comes, is dropped too (a `POST` already sent is not unsent — LLP 1016 D5).
    fn forget(&mut self, target: Target) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.log(format!("forget request {t} ({})", self.target_name(target)));
        }
        self.sync_pending_flags();
    }

    /// Hand `request` to the host under a fresh ticket, replacing any
    /// request in flight for the same target.
    fn enqueue(
        &mut self,
        target: Target,
        source: String,
        args: Vec<Value>,
        request: Request,
        forced: bool,
    ) {
        if let Some(pos) = self.pending.iter().position(|p| p.target == target) {
            let t = self.pending.remove(pos).ticket;
            self.log(format!("forget request {t} ({})", self.target_name(target)));
        }
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let name = self.target_name(target);
        self.log(format!(
            "request {ticket} ({name}): {} {}",
            request.method, request.url
        ));
        self.pending.push(PendingReq {
            ticket,
            target,
            source,
            args,
        });
        self.requests.push(RequestOut {
            ticket,
            target: name,
            request,
            forced,
        });
        self.sync_pending_flags();
    }

    fn sync_pending_flags(&mut self) {
        self.pending_res = vec![false; self.plan.resources.len()];
        self.pending_mut = vec![false; self.plan.mutations.len()];
        for p in &self.pending {
            match p.target {
                Target::Resource(i) => self.pending_res[i] = true,
                Target::Mutation(m) => self.pending_mut[m] = true,
            }
        }
    }

    /// The requests the host is to run since the last take (LLP 1016 D2).
    pub fn take_requests(&mut self) -> Vec<RequestOut> {
        std::mem::take(&mut self.requests)
    }

    /// Every request in flight: the resource's or mutation's name and its ticket.
    pub fn pending(&self) -> Vec<(String, u64)> {
        self.pending
            .iter()
            .map(|p| (self.target_name(p.target), p.ticket))
            .collect()
    }

    /// Whether any request is in flight (the agent's `settle` waits on it).
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The host brought back the outcome of request `ticket`: the source
    /// parses it, the resource takes its value or the mutation's slot its
    /// `some`, and everything downstream settles as after an action — one
    /// commit. A ticket no longer held (forgotten, D5) is dropped with a
    /// journal line and no commit.
    pub fn fulfill(
        &mut self,
        ticket: u64,
        outcome: Outcome,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(pos) = self.pending.iter().position(|p| p.ticket == ticket) else {
            self.log(format!("reply {ticket} dropped: no such request in flight"));
            return Ok(None);
        };
        let saved_pending = self.pending.clone();
        let p = self.pending.remove(pos);
        self.sync_pending_flags();
        let what = format!("fulfil {ticket} ({})", self.target_name(p.target));
        let was_poisoned = self.poisoned;
        let kept = self.store.checkpoint();
        let since = kept.writes;
        let result = self.fulfill_inner(p, outcome);
        if result.is_err() && !self.poisoned {
            self.pending = saved_pending;
            self.sync_pending_flags();
        }
        match &result {
            Ok(_) => self.log_store_writes(since),
            Err(_) => self.store.restore(kept),
        }
        self.log_outcome(&what, &result, was_poisoned);
        result.map(Some)
    }

    fn fulfill_inner(
        &mut self,
        p: PendingReq,
        outcome: Outcome,
    ) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let name = self.target_name(p.target);
        let ty = match p.target {
            Target::Resource(i) => self.plan.resources[i].ty,
            Target::Mutation(m) => self.plan.mutations[m].ty,
        };
        let value = match self
            .data
            .parse(&mut self.store, &p.source, &p.args, outcome)
            .map_err(|error| RunnerError::Data {
                resource: name.clone(),
                error,
            })? {
            Answer::Now(value) => value,
            Answer::Later(request) => {
                // One more round (LLP 1027 D1a): the target keeps its value,
                // a new ticket goes out for the same arguments, and this
                // commit changes nothing but the pending set.
                self.log(format!("{name}: the reply asks for one more request"));
                self.enqueue(p.target, p.source, p.args, request, false);
                return self.update();
            }
        };
        if !value.conforms(&self.plan, ty) {
            return Err(RunnerError::Shape { resource: name });
        }
        let saved_slots = self.slots.clone();
        let saved_resources = self.resources.clone();
        match p.target {
            Target::Resource(i) => {
                self.stale[i] = false;
                self.keep_answer(i, &p.args, &value);
                self.resources[i] = Some(ResourceState {
                    args: p.args,
                    value,
                    store_revision: self.store.revision(),
                });
            }
            Target::Mutation(m) => {
                let slot = self.mutation_slot(m)?;
                self.slots[slot] = Value::some(value);
            }
        }
        if let Err(e) = self.settle(false) {
            self.slots = saved_slots;
            self.resources = saved_resources;
            return Err(e);
        }
        self.update()
    }
}
