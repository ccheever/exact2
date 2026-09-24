//! @ref LLP 1038 D2–D5, D7, D9 — the plan/value boundary of the router.
//!
//! Chunk (c)'s compiler declares these positional records, in this exact order:
//! Router { tab: string, tabs: list<Tab>, next: number },
//! Tab { name: string, stack: list<Entry> },
//! Entry { id: number, name: string, url: string, tab: string, params: Params },
//! Params { one string field per distinct :name, in first-declaration order }.
//! The header slot's type leads to every shape; global type-row order is immaterial.
//! The compiler checks the route table; its types are checked once at boot.
//! No host interprets slots.

use super::{Carried, DataSource, Runner, RunnerError};
use exact_kernel::SortedSet;
use exact_plan::{Plan, SlotsId, Stdlib, TypeKind, TypesId, Value};
use exact_route::{Entry, Router, Tab, Table};
use std::cell::RefCell;

/// The selected visit and all visit ids removed from any retained tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterChange {
    /// The selected top's visit id.
    pub top: u64,
    /// Its canonical location.
    pub url: String,
    /// Removed ids, in the old value's tab/stack order.
    pub removed: Vec<u64>,
}

/// The router seam (LLP 1047 D4): what the runner and its VM ask of a plan's
/// router. [`RouterContext`] is the one implementation, built by
/// [`routing`]; a host that links no router passes none, and a plan that
/// declares routes is refused before it boots (LLP 1047 D6).
pub trait Routing {
    /// The root slot the router value lives in.
    fn slot(&self) -> SlotsId;
    /// A router verb or read the VM calls; `None` on a type mismatch.
    fn call(&self, plan: &Plan, f: Stdlib, args: &[Value]) -> Option<Value>;
    /// Journal a refused intent once per commit.
    fn refuse(&self, intent: &str, message: &str);
    /// The router value a boot starts from: the carried router when the
    /// table still fits it, else a launch at `launch`.
    fn initial(&self, carried: Option<&Router>, launch: &str) -> Result<Value, RunnerError>;
    /// `value` as a router, when it is a valid one.
    fn read(&self, value: &Value) -> Option<Router>;
    /// What changed since the last commit, for the host (LLP 1038 D7).
    fn change(&self, value: &Value) -> Result<Option<RouterChange>, RunnerError>;
    /// Remember `value` as committed, and keep `change` for the host.
    fn commit(&mut self, value: Value, change: Option<RouterChange>);
    /// Refusals journaled since the last take.
    fn take_refusals(&self) -> Vec<String>;
    /// The pending change, once.
    fn take_change(&mut self) -> Option<RouterChange>;
    /// Whether `location` names a declared route pattern (LLP 1038 §7).
    fn matches_pattern(&self, location: &str) -> bool;
}

/// The plan's router, if it declares one (LLP 1038 D2): what a host that
/// links the router passes as `RunnerLinks.router`.
pub fn routing(plan: &Plan) -> Result<Option<Box<dyn Routing>>, RunnerError> {
    Ok(RouterContext::from_plan(plan)?.map(|r| Box::new(r) as Box<dyn Routing>))
}

/// The checked table and shape metadata shared by the runner's VM evaluations.
/// Constructed by boot from the plan; no additional declaration authority.
pub struct RouterContext {
    slot: SlotsId,
    router_ty: TypesId,
    entry_ty: TypesId,
    param_names: Vec<String>,
    table: Table,
    committed: Option<Value>,
    pending: Option<RouterChange>,
    refusals: RefCell<Vec<String>>,
    /// The last router value a call read, checked and converted once, with
    /// the reads made of it: a binding that calls `stack(nav)` or `top(nav)`
    /// on an unchanged router gets the same object back, not a rebuilt one.
    reads: RefCell<Option<Reads>>,
}

/// One router value, validated, and what has been read of it.
struct Reads {
    value: Value,
    router: Router,
    stack: Option<Value>,
    top: Option<Value>,
}

fn invalid(message: impl Into<String>) -> RunnerError {
    RunnerError::Router(message.into())
}

impl RouterContext {
    pub(super) fn from_plan(plan: &Plan) -> Result<Option<Self>, RunnerError> {
        let Some(slot) = plan.router else {
            return Ok(None);
        };
        let table = Table {
            routes: plan
                .routes
                .iter()
                .map(|r| exact_route::Route {
                    name: plan.str(r.name).into(),
                    pattern: plan.str(r.pattern).into(),
                    parent: r.parent.map(|p| p.0 as usize),
                    tab: r.tab,
                    notfound: r.notfound,
                })
                .collect(),
        };
        // The compiler checked this table (`exact_route::Table::check`, in
        // `contract/types`) before it wrote the plan, as it checked the rest.
        let router_ty = plan.slot(slot).ty;
        let fields = shape(plan, router_ty, "Router", &["tab", "tabs", "next"])?;
        primitive(plan, fields[0], TypeKind::String)?;
        primitive(plan, fields[2], TypeKind::Number)?;
        let tab_ty = element(plan, fields[1])?;
        let fields = shape(plan, tab_ty, "Tab", &["name", "stack"])?;
        primitive(plan, fields[0], TypeKind::String)?;
        let entry_ty = element(plan, fields[1])?;
        let fields = shape(
            plan,
            entry_ty,
            "Entry",
            &["id", "name", "url", "tab", "params"],
        )?;
        primitive(plan, fields[0], TypeKind::Number)?;
        for field in &fields[1..4] {
            primitive(plan, *field, TypeKind::String)?;
        }
        let names = table.param_names();
        let params = shape(plan, fields[4], "Params", &names)?;
        for ty in params {
            primitive(plan, ty, TypeKind::String)?;
        }
        let param_names = names.into_iter().map(str::to_owned).collect();
        Ok(Some(Self {
            slot,
            router_ty,
            entry_ty,
            param_names,
            table,
            committed: None,
            pending: None,
            refusals: RefCell::new(Vec::new()),
            reads: RefCell::new(None),
        }))
    }

    fn entry(&self, value: &Value) -> Option<Entry> {
        let v = record(value)?;
        if v.len() != 5 {
            return None;
        }
        let params = record(&v[4])?;
        if params.len() != self.param_names.len() {
            return None;
        }
        Some(Entry {
            id: integer(&v[0])?,
            name: v[1].as_str()?.into(),
            url: v[2].as_str()?.into(),
            tab: v[3].as_str()?.into(),
            params: self
                .param_names
                .iter()
                .zip(params)
                .map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                .collect::<Option<_>>()?,
        })
    }

    fn router(&self, value: &Value) -> Option<Router> {
        let v = record(value)?;
        if v.len() != 3 {
            return None;
        }
        let tabs = list(&v[1])?
            .iter()
            .map(|tab| {
                let t = record(tab)?;
                if t.len() != 2 {
                    return None;
                }
                Some(Tab {
                    name: t[0].as_str()?.into(),
                    stack: list(&t[1])?
                        .iter()
                        .map(|entry| self.entry(entry))
                        .collect::<Option<_>>()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let r = Router {
            tab: v[0].as_str()?.into(),
            tabs,
            next: integer(&v[2])?,
        };
        // A shape-correct forged value must preserve identities, a total top,
        // and the canonical URL/name/params round trip for every retained entry.
        let mut ids = SortedSet::new();
        let mut tabs = SortedSet::new();
        if exact_route::top(&r).is_none()
            || r.tabs.iter().any(|t| {
                !tabs.insert(&t.name)
                    || t.stack.first().is_none_or(|e| e.name != t.name)
                    || t.stack.iter().any(|e| {
                        e.tab != t.name
                            || e.id >= r.next
                            || !ids.insert(e.id)
                            || exact_route::canonical(&e.url) != e.url
                            || self
                                .table
                                .matches(&e.url)
                                .is_none_or(|m| m.name != e.name || m.params != e.params)
                    })
            })
        {
            return None;
        }
        Some(r)
    }

    fn entry_value(&self, e: &Entry) -> Value {
        Value::record(vec![
            Value::Number(e.id as f64),
            Value::str(&e.name),
            Value::str(&e.url),
            Value::str(&e.tab),
            Value::record(
                self.param_names
                    .iter()
                    .map(|n| Value::str(e.params.get(n).map_or("", String::as_str)))
                    .collect(),
            ),
        ])
    }

    fn value(&self, r: &Router) -> Value {
        Value::record(vec![
            Value::str(&r.tab),
            Value::list(
                r.tabs
                    .iter()
                    .map(|t| {
                        Value::record(vec![
                            Value::str(&t.name),
                            Value::list(t.stack.iter().map(|e| self.entry_value(e)).collect()),
                        ])
                    })
                    .collect(),
            ),
            Value::Number(r.next as f64),
        ])
    }

    fn refuse_once(&self, intent: &str, message: &str) {
        // @ref LLP 1035.001 D6 / LLP 1038 D4 — settlement may retry a read;
        // each distinct refused intent is journaled once in this commit.
        let line = format!("router {intent} refused: {message}");
        let mut lines = self.refusals.borrow_mut();
        if !lines.contains(&line) {
            lines.push(line);
        }
    }

    fn launch(&self, location: &str) -> Result<Router, RunnerError> {
        let (r, refusal) = Router::launch(&self.table, location);
        match refusal {
            None => Ok(r),
            Some(reason) => {
                self.refuse_once("launch", &reason.message);
                let (r, refusal) = Router::launch(&self.table, "/");
                if let Some(reason) = refusal {
                    return Err(invalid(reason.message));
                }
                Ok(r)
            }
        }
    }

    fn call_verb(&self, plan: &Plan, f: Stdlib, args: &[Value]) -> Option<Value> {
        let first = args.first()?;
        if f == Stdlib::SearchParam {
            if !first.conforms(plan, self.entry_ty) {
                return None;
            }
            return Some(Value::str(&exact_route::search_param(
                &self.entry(first)?,
                args.get(1)?.as_str()?,
            )));
        }
        let mut reads = self.reads.borrow_mut();
        let reads = match &mut *reads {
            Some(reads) if crate::compare::same(&reads.value, first) => reads,
            slot => {
                if !first.conforms(plan, self.router_ty) {
                    return None;
                }
                slot.insert(Reads {
                    value: first.clone(),
                    router: self.router(first)?,
                    stack: None,
                    top: None,
                })
            }
        };
        let arg = || args.get(1)?.as_str();
        match f {
            Stdlib::Stack => {
                return Some(
                    reads
                        .stack
                        .get_or_insert_with(|| {
                            Value::list(
                                exact_route::stack(&reads.router)
                                    .iter()
                                    .map(|e| self.entry_value(e))
                                    .collect(),
                            )
                        })
                        .clone(),
                )
            }
            Stdlib::Top => {
                if reads.top.is_none() {
                    reads.top = Some(self.entry_value(exact_route::top(&reads.router)?));
                }
                return reads.top.clone();
            }
            Stdlib::Depth => return Some(Value::Number(exact_route::depth(&reads.router) as f64)),
            Stdlib::Params => {
                return Some(Value::list(
                    exact_route::params(&reads.router, arg()?)
                        .into_iter()
                        .map(Value::str)
                        .collect(),
                ))
            }
            _ => {}
        }
        let r = reads.router.clone();
        let (after, refusal) = match f {
            Stdlib::Open => exact_route::open(&self.table, r, arg()?),
            Stdlib::Push => exact_route::push(&self.table, r, arg()?),
            Stdlib::Replace => exact_route::replace(&self.table, r, arg()?),
            Stdlib::Back => exact_route::back(&self.table, r),
            Stdlib::Select => exact_route::select(&self.table, r, arg()?),
            Stdlib::Go => exact_route::go(&self.table, r, arg()?),
            _ => return None,
        };
        if let Some(reason) = refusal {
            self.refuse_once(f.name(), &reason.message);
            return Some(first.clone());
        }
        Some(self.value(&after))
    }
}

fn shape(
    plan: &Plan,
    ty: TypesId,
    name: &str,
    names: &[&str],
) -> Result<Vec<TypesId>, RunnerError> {
    let row = plan.type_(ty);
    if row.kind != TypeKind::Record
        || plan.str(row.name) != name
        || row.fields.len as usize != names.len()
    {
        return Err(invalid(format!("expected {name} shape")));
    }
    row.fields
        .iter()
        .zip(names)
        .map(|(f, name)| {
            let field = plan.field(f);
            if plan.str(field.name) != *name {
                return Err(invalid(format!("expected {name} field in declared order")));
            }
            Ok(field.ty)
        })
        .collect()
}
fn primitive(plan: &Plan, ty: TypesId, kind: TypeKind) -> Result<(), RunnerError> {
    if plan.type_(ty).kind == kind {
        Ok(())
    } else {
        Err(invalid(format!("expected {} field", kind.name())))
    }
}
fn element(plan: &Plan, ty: TypesId) -> Result<TypesId, RunnerError> {
    primitive(plan, ty, TypeKind::List)?;
    plan.type_(ty)
        .elem
        .ok_or_else(|| invalid("list needs an element type"))
}
fn record(v: &Value) -> Option<&[Value]> {
    if let Value::Record(v) = v {
        Some(v)
    } else {
        None
    }
}
fn list(v: &Value) -> Option<&[Value]> {
    if let Value::List(v) = v {
        Some(v)
    } else {
        None
    }
}
fn integer(v: &Value) -> Option<u64> {
    let n = v.as_number()?;
    (n.fract() == 0.0 && (0.0..=9_007_199_254_740_991.0).contains(&n)).then_some(n as u64)
}

impl Routing for RouterContext {
    fn slot(&self) -> SlotsId {
        self.slot
    }

    fn call(&self, plan: &Plan, f: Stdlib, args: &[Value]) -> Option<Value> {
        self.call_verb(plan, f, args)
    }

    fn refuse(&self, intent: &str, message: &str) {
        self.refuse_once(intent, message);
    }

    fn initial(&self, carried: Option<&Router>, launch: &str) -> Result<Value, RunnerError> {
        let (r, was_carried) =
            match carried {
                Some(old)
                    if old
                        .tabs
                        .iter()
                        .map(|t| t.name.as_str())
                        .eq(self.table.tab_names())
                        && old.tabs.iter().flat_map(|t| &t.stack).all(|e| {
                            self.table.matches(&e.url).is_some_and(|m| m.name == e.name)
                        }) =>
                {
                    // Params may have been reordered/renamed by a table edit.
                    let mut kept = old.clone();
                    for e in kept.tabs.iter_mut().flat_map(|t| &mut t.stack) {
                        e.params = self.table.matches(&e.url).expect("checked").params;
                    }
                    (kept, true)
                }
                Some(old) => (
                    self.launch(exact_route::top(old).map_or(launch, |e| &e.url))?,
                    false,
                ),
                None => (self.launch(launch)?, false),
            };
        let value = self.value(&r);
        // Launched from the table, it is valid by construction; carried across
        // a reload, it is checked against the table it meets.
        let router = if was_carried {
            self.router(&value)
                .ok_or_else(|| invalid("invalid router value at boot"))?
        } else {
            r
        };
        // The view reads it at once, and the first commit publishes it: both
        // are served from what was just built.
        *self.reads.borrow_mut() = Some(Reads {
            value: value.clone(),
            router,
            stack: None,
            top: None,
        });
        Ok(value)
    }

    fn read(&self, value: &Value) -> Option<Router> {
        self.router(value)
    }

    fn change(&self, value: &Value) -> Result<Option<RouterChange>, RunnerError> {
        // A committed value was validated, so it holds no NaN: the same
        // object is an equal one.
        if self
            .committed
            .as_ref()
            .is_some_and(|c| crate::compare::same(c, value) || c == value)
        {
            return Ok(None);
        }
        // A value the reads hold was validated (or built valid) when read.
        let read = self
            .reads
            .borrow()
            .as_ref()
            .filter(|reads| crate::compare::same(&reads.value, value))
            .map(|reads| reads.router.clone());
        let r = match read {
            Some(r) => r,
            None => self
                .router(value)
                .ok_or_else(|| invalid("invalid router value"))?,
        };
        let top = exact_route::top(&r).ok_or_else(|| invalid("router has no top"))?;
        let ids: SortedSet<_> = r.tabs.iter().flat_map(|t| &t.stack).map(|e| e.id).collect();
        let removed = self
            .committed
            .as_ref()
            .and_then(|v| self.router(v))
            .into_iter()
            .flat_map(|r| r.tabs)
            .flat_map(|t| t.stack)
            .filter(|e| !ids.contains(&e.id))
            .map(|e| e.id)
            .collect();
        Ok(Some(RouterChange {
            top: top.id,
            url: top.url.clone(),
            removed,
        }))
    }

    fn commit(&mut self, value: Value, change: Option<RouterChange>) {
        self.committed = Some(value);
        if let Some(mut change) = change {
            if let Some(pending) = self.pending.take() {
                // A clock seek may commit more than once before a host drains
                // effects. Keep every removed id and the latest selected top.
                let mut removed = pending.removed;
                for id in change.removed {
                    if !removed.contains(&id) {
                        removed.push(id);
                    }
                }
                change.removed = removed;
            }
            self.pending = Some(change);
        }
    }

    fn take_refusals(&self) -> Vec<String> {
        std::mem::take(&mut *self.refusals.borrow_mut())
    }

    fn take_change(&mut self) -> Option<RouterChange> {
        self.pending.take()
    }

    fn matches_pattern(&self, location: &str) -> bool {
        self.table.matches_pattern(location).is_some()
    }
}

impl<D: DataSource> Runner<D> {
    pub(super) fn init_slots(
        &mut self,
        carried: Option<&Carried>,
        launch: &str,
    ) -> Result<(), RunnerError> {
        self.slots = vec![Value::Unit; self.plan.slots.len()];
        // The router may be a later slot: fill it before *any* initializer.
        if let Some(context) = &self.router {
            let slot = context.slot();
            let name = self.plan.str(self.plan.slot(slot).name);
            let old = carried
                .and_then(|c| c.router.as_ref())
                .filter(|(n, _)| n == name)
                .map(|(_, old)| old);
            self.slots[slot.0 as usize] = context.initial(old, launch)?;
        }
        for i in 0..self.plan.slots.len() {
            let row = &self.plan.slots[i];
            if row.owner.is_some() || self.plan.router == Some(SlotsId(i as u32)) {
                continue;
            }
            let name = self.plan.str(row.name);
            let kept = carried
                .and_then(|c| c.slots.iter().find(|(n, _)| n == name))
                .map(|(_, v)| v.clone())
                .filter(|v| v.conforms(&self.plan, row.ty));
            let v = match kept {
                Some(v) => v,
                None => self.eval(row.init, &[], &[])?,
            };
            if !v.conforms(&self.plan, row.ty) {
                return Err(RunnerError::SlotType { slot: name.into() });
            }
            self.slots[i] = v;
        }
        Ok(())
    }

    pub(super) fn carry_router(&self) -> Option<(String, Router)> {
        let context = self.router.as_ref()?;
        let slot = context.slot();
        Some((
            self.plan.str(self.plan.slot(slot).name).into(),
            context.read(&self.slots[slot.0 as usize])?,
        ))
    }

    pub(super) fn router_change(&self) -> Result<Option<RouterChange>, RunnerError> {
        let Some(context) = &self.router else {
            return Ok(None);
        };
        context.change(&self.slots[context.slot().0 as usize])
    }

    pub(super) fn commit_router(&mut self, change: Option<RouterChange>) {
        if let Some(context) = &mut self.router {
            let value = self.slots[context.slot().0 as usize].clone();
            context.commit(value, change);
        }
        self.log_router_refusals();
    }

    pub(super) fn log_router_refusals(&mut self) {
        let lines = self
            .router
            .as_ref()
            .map(|r| r.take_refusals())
            .unwrap_or_default();
        for line in lines {
            self.log(line);
        }
    }

    /// Take the navigation change published by successful commits, including boot.
    /// @ref LLP 1038 D7 — drained beside commands; kernel receipts stay unchanged.
    /// Multiple commits before a take retain the latest top/url and every removed
    /// id in first-removal order. Unchanged or refused commits leave it alone.
    /// Returns `None` after a take until navigation changes again, or without a router.
    pub fn take_router_change(&mut self) -> Option<RouterChange> {
        self.router.as_mut()?.take_change()
    }

    /// Whether `location` names a pattern the plan's route table declares —
    /// never only its notfound fallback (LLP 1038 §7). `false` without a router.
    pub fn route_matches(&self, location: &str) -> bool {
        self.router
            .as_ref()
            .is_some_and(|r| r.matches_pattern(location))
    }

    /// The evaluated arguments of a settled resource (the bake's cache key).
    /// @ref LLP 1038 D5 — written beside `resources.initial`.
    pub fn resource_args(&self, name: &str) -> Option<&[Value]> {
        let i = self
            .plan
            .resources
            .iter()
            .position(|r| self.plan.str(r.name) == name)?;
        self.resources[i].as_ref().map(|r| r.args.as_slice())
    }
}
