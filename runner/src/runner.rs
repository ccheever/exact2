//! The runner: boot, actions, events, resources, timers, the clock.
//!
//! @ref LLP 1004 D3 (the refusal tuple) / D4 (the data seam) / D5 (restart)
//!
//! One frame on a native host: `dispatch` or `advance` → the runner
//! re-evaluates derives, re-requests resources whose arguments changed,
//! re-evaluates every site, and applies one atomic op batch to the kernel →
//! the host lays out and paints. Every write is validated by the kernel before
//! anything changes; a trap or refusal leaves the kernel exactly as it was.

use crate::instance::{Ids, InstanceError, SurfaceUpdate, Tree, Update};
use crate::vm::{self, Env, Frame, Trap};
use exact_kernel::{CommitReceipt, Kernel, KernelError, ViewId};
use exact_plan::{ActionsId, Code, EventKind, Plan, PlanError, Value};

/// The app's data source: the one seam through which computation enters
/// (LLP 1004 D4). Implemented once, in Rust, by the app's data crate.
pub trait DataSource {
    /// Answer a resource's request. `args` are the resource's argument
    /// expressions evaluated against current state.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError>;
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

/// A host event aimed at a view.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A press on the view.
    Press,
    /// A text input changed to `value`.
    Change(String),
}

/// Why the runner refused. The kernel is unchanged.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum RunnerError {
    KernelSchemaMismatch {
        plan: u64,
        kernel: u64,
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
        RunnerError::Instance(e)
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
}

struct Timer {
    next_ms: f64,
}

/// What survives a reload: state by name, settled resources by name with
/// the arguments they answered, and the clock. A new plan takes each slot
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
    /// The clock, milliseconds.
    pub now_ms: f64,
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
    poisoned: bool,
}

impl<D: DataSource> Runner<D> {
    /// Boot: refuse a plan built against another kernel schema, evaluate
    /// initial state, settle resources (compiled data first, the source
    /// otherwise), realize the tree, and apply the first frame's ops.
    pub fn boot(plan: Plan, data: D, kernel: Kernel) -> Result<Runner<D>, RunnerError> {
        Runner::boot_inner(plan, data, kernel, None)
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
        Runner::boot_inner(plan, data, kernel, Some(carried))
    }

    /// Everything a reload keeps.
    pub fn carry(&self) -> Carried {
        Carried {
            slots: self
                .plan
                .slots
                .iter()
                .zip(&self.slots)
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
            now_ms: self.now_ms,
        }
    }

    fn boot_inner(
        plan: Plan,
        data: D,
        kernel: Kernel,
        carried: Option<&Carried>,
    ) -> Result<Runner<D>, RunnerError> {
        plan.validate().map_err(RunnerError::Plan)?;
        if plan.kernel_schema_digest != exact_kernel::SCHEMA_DIGEST {
            return Err(RunnerError::KernelSchemaMismatch {
                plan: plan.kernel_schema_digest,
                kernel: exact_kernel::SCHEMA_DIGEST,
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
            poisoned: false,
        };
        // Slots: carried values where the name and type still fit, else
        // initial values, in order (an initializer may read earlier slots).
        for i in 0..runner.plan.slots.len() {
            let ty = runner.plan.slots[i].ty;
            let name = runner.plan.str(runner.plan.slots[i].name);
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
                    })
            })
            .collect();
        runner.resource_values = vec![None; runner.plan.resources.len()];
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
        runner.apply(ops)?;
        runner.surfaces = surfaces;
        Ok(runner)
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
            .position(|s| self.plan.str(s.name) == name)
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

    /// Deliver a host event to `view`: find its handler, evaluate the curried
    /// arguments in the instance's scope now, run the action, update.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> Result<CommitReceipt, RunnerError> {
        let (node, frames) = self
            .tree
            .as_ref()
            .and_then(|t| t.find(view))
            .ok_or(RunnerError::UnknownView(view))?;
        let (kind, payload, name) = match &event {
            Event::Press => (EventKind::Press, None, "press"),
            Event::Change(text) => (EventKind::Change, Some(Value::str(text)), "change"),
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
        self.run_action(handler.action, args)
    }

    /// Run an action by name with `args` — what a test or an agent does.
    pub fn act(&mut self, name: &str, args: Vec<Value>) -> Result<CommitReceipt, RunnerError> {
        let id = self
            .plan
            .actions
            .iter()
            .position(|a| self.plan.str(a.name) == name)
            .map(|i| ActionsId(i as u32))
            .ok_or(RunnerError::NoHandler {
                view: 0,
                event: "action",
            })?;
        self.run_action(id, args)
    }

    /// Move the clock to `now_ms`, firing every timer due, in order.
    pub fn advance(&mut self, now_ms: f64) -> Result<Vec<CommitReceipt>, RunnerError> {
        let mut receipts = Vec::new();
        if !now_ms.is_finite() {
            return Err(RunnerError::NonFiniteClock);
        }
        if now_ms < self.now_ms {
            return Ok(receipts);
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
            self.now_ms = at;
            let interval = self.plan.timers[i].interval_ms as f64;
            self.timers[i].next_ms += interval;
            let action = self.plan.timers[i].action;
            receipts.push(self.run_action(action, Vec::new())?);
        }
        self.now_ms = now_ms;
        Ok(receipts)
    }

    fn run_action(
        &mut self,
        action: ActionsId,
        args: Vec<Value>,
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
            let env = self.env(&args, &[]);
            vm::eval(self.plan.code(row.body), &env, &allowed)?
        };
        // Commit the writes, then everything downstream. If settlement refuses
        // (a data source or shape refusal), the writes and commands roll back
        // and the kernel is exactly as it was.
        for (slot, value) in &outcome.writes {
            if !value.conforms(&self.plan, self.plan.slots[*slot as usize].ty) {
                return Err(RunnerError::SlotType {
                    slot: self
                        .plan
                        .str(self.plan.slots[*slot as usize].name)
                        .to_string(),
                });
            }
        }
        let saved_slots = self.slots.clone();
        let saved_commands = self.commands.len();
        for (slot, value) in outcome.writes {
            self.slots[slot as usize] = value;
        }
        for (name, args) in outcome.commands {
            self.commands.push(Command { name, args });
        }
        if let Err(e) = self.settle(false) {
            self.slots = saved_slots;
            self.commands.truncate(saved_commands);
            return Err(e);
        }
        self.update()
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
                self.poisoned = true;
                self.commands.clear();
                return Err(e.into());
            }
        };
        match self.apply(ops) {
            Ok(receipt) => {
                self.surfaces.extend(surfaces);
                Ok(receipt)
            }
            Err(e) => {
                self.poisoned = true;
                self.commands.clear();
                Err(e)
            }
        }
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
        }
    }

    fn eval(&self, code: Code, params: &[Value], frames: &[Frame]) -> Result<Value, RunnerError> {
        let env = self.env(params, frames);
        Ok(vm::eval(self.plan.code(code), &env, &[])?.value)
    }

    fn query(&mut self, i: usize, args: &[Value]) -> Result<Value, RunnerError> {
        let row = &self.plan.resources[i];
        let source = self.plan.str(row.source).to_string();
        let resource = self.plan.str(row.name).to_string();
        self.data
            .query(&source, args)
            .map_err(|error| RunnerError::Data { resource, error })
    }

    fn check_shape(&self, i: usize, value: &Value) -> Result<(), RunnerError> {
        let row = &self.plan.resources[i];
        if value.conforms(&self.plan, row.ty) {
            Ok(())
        } else {
            Err(RunnerError::Shape {
                resource: self.plan.str(row.name).to_string(),
            })
        }
    }

    /// Settle every derive and resource against current state, in plan
    /// order, to a fixpoint: an expression that reads something not yet
    /// settled this pass is retried after it settles. Deterministic, and a
    /// cycle is a typed refusal. On boot a resource takes its compiled value
    /// if it has one; afterwards it is re-requested only when its arguments
    /// changed, so every derive that reads it sees the new value in the same
    /// pass.
    fn settle(&mut self, boot: bool) -> Result<(), RunnerError> {
        let mut derives: Vec<Option<Value>> = vec![None; self.plan.derives.len()];
        let mut resources: Vec<Option<Value>> = vec![None; self.plan.resources.len()];
        // Work on a copy of the committed resource states; publish only when
        // the whole pass succeeds, so a failure leaves every cache as it was.
        let mut states: Vec<Option<ResourceState>> = self.resources.clone();
        let mut settled_res = vec![false; states.len()];
        loop {
            let mut progress = false;
            let mut all = true;
            for i in 0..self.plan.derives.len() {
                if derives[i].is_some() {
                    continue;
                }
                let code = self.plan.derives[i].body;
                let result = {
                    let env = Env {
                        plan: &self.plan,
                        slots: &self.slots,
                        derives: &derives,
                        resources: &resources,
                        params: &[],
                        frames: &[],
                        now_ms: self.now_ms,
                    };
                    vm::eval(self.plan.code(code), &env, &[])
                };
                match result {
                    Ok(o) => {
                        if !o.value.conforms(&self.plan, self.plan.derives[i].ty) {
                            return Err(RunnerError::DeriveType {
                                derive: self.plan.str(self.plan.derives[i].name).to_string(),
                            });
                        }
                        derives[i] = Some(o.value);
                        progress = true;
                    }
                    Err(Trap::Pending { .. }) => all = false,
                    Err(t) => return Err(t.into()),
                }
            }
            for i in 0..self.plan.resources.len() {
                if settled_res[i] {
                    continue;
                }
                let row = self.plan.resources[i].clone();
                let mut args = Vec::with_capacity(row.args.len as usize);
                let mut pending = false;
                for a in row.args.iter() {
                    let code = self.plan.arg(a).expr;
                    let result = {
                        let env = Env {
                            plan: &self.plan,
                            slots: &self.slots,
                            derives: &derives,
                            resources: &resources,
                            params: &[],
                            frames: &[],
                            now_ms: self.now_ms,
                        };
                        vm::eval(self.plan.code(code), &env, &[])
                    };
                    match result {
                        Ok(o) => args.push(o.value),
                        Err(Trap::Pending { .. }) => {
                            pending = true;
                            break;
                        }
                        Err(t) => return Err(t.into()),
                    }
                }
                if pending {
                    all = false;
                    continue;
                }
                let reuse = states[i]
                    .as_ref()
                    .filter(|s| s.args == args)
                    .map(|s| s.value.clone());
                let value = match reuse {
                    Some(v) => v,
                    None if boot && row.initial.len > 0 => {
                        Value::from_bytes(self.plan.bytes(row.initial))
                            .map_err(RunnerError::Plan)?
                    }
                    None => self.query(i, &args)?,
                };
                self.check_shape(i, &value)?;
                resources[i] = Some(value.clone());
                states[i] = Some(ResourceState { args, value });
                settled_res[i] = true;
                progress = true;
            }
            if all && settled_res.iter().all(|s| *s) {
                break;
            }
            if !progress {
                return Err(RunnerError::Cycle);
            }
        }
        self.derives = derives;
        self.resource_values = resources;
        self.resources = states;
        Ok(())
    }
}
