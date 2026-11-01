//! @ref LLP 1038 D2–D5, D7, D9 — the plan/value boundary of the router.
//!
//! Chunk (c)'s compiler declares these positional records, in this exact order:
//! Router { tab: string, tabs: Tab[], next: number },
//! Tab { name: string, stack: Entry[] },
//! Entry { id: number, name: string, url: string, tab: string, params: Params, fragment: string },
//! Params { one string field per distinct :name, in first-declaration order }.
//! The header slot's type leads to every shape; global type-row order is immaterial.
//! The compiler checks the route table; its types are checked once at boot.
//! No host interprets slots.

use super::{Carried, CommitReceipt, DataSource, Runner, RunnerError};
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
    /// Its fragment, `""` when it has none (LLP 1153 D5).
    pub fragment: String,
    /// Whether the host scrolls to that fragment once the route's content
    /// has arrived: the top is a new visit with a fragment, its fragment
    /// changed, or the router was assigned a navigation to it. A return to a
    /// visit (Back, a tab switch) keeps its place.
    pub scroll: bool,
    /// That scroll's activation, counted from 1 for the router's life, `0`
    /// with none: the third argument of the `scrollToFragment` that makes
    /// it, so a host matches the two and takes each once.
    pub activation: u64,
    /// Removed ids, in the old value's tab/stack order.
    pub removed: Vec<u64>,
    /// Every retained visit's canonical location, in tab/stack order.
    pub visits: Vec<(u64, String)>,
}

impl RouterChange {
    /// The top's location as a host writes it back: its url, then `#` and
    /// its fragment when it has one.
    pub fn location(&self) -> String {
        if self.fragment.is_empty() {
            self.url.clone()
        } else {
            format!("{}#{}", self.url, self.fragment)
        }
    }
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
    /// Return to an earlier visit in the selected stack, only from its current top.
    fn back_to(&self, value: &Value, source: u64, destination: u64) -> Option<Value>;
    /// What changed since the last commit, for the host (LLP 1038 D7).
    fn change(&self, value: &Value) -> Result<Option<RouterChange>, RunnerError>;
    /// Remember `value` as committed, and keep `change` for the host.
    fn commit(&mut self, value: Value, change: Option<RouterChange>);
    /// Refusals journaled since the last take.
    fn take_refusals(&self) -> Vec<String>;
    /// The pending change, once.
    fn take_change(&mut self) -> Option<RouterChange>;
    /// The location a native host's Forward goes to, the nearest of the
    /// visits Back left (`history.rs`; LLP 1115.000 D5.5).
    fn ahead(&self) -> Option<&str>;
    /// Whether `location` names a declared route pattern (LLP 1038 §7).
    fn matches_pattern(&self, location: &str) -> bool;
    /// Set what `sharedFiles` answers for a share's token (LLP 1133.024 D3);
    /// false when it already answers that.
    fn share(&self, token: &str, files: Value) -> bool;
    /// This launch's claims (D3), drawn from `seed` the first time.
    fn claims(&self, seed: f64) -> std::cell::RefMut<'_, crate::share_in::Claims>;
    /// Whether a committed change asked for a scroll to its top's fragment
    /// that no host has been told to make yet (LLP 1153 D5).
    fn fragment_due(&self) -> bool;
    /// That visit's id, fragment and activation, once.
    fn take_fragment(&mut self) -> Option<(u64, String, u64)>;
    /// Whether a verb in the commit being made navigated, by a location
    /// with a fragment, to the visit it left on top.
    fn activated(&self) -> bool;
    /// A refused commit's navigation goes with it.
    fn forget_activation(&self);
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
    /// The visit, fragment and activation a host scrolls to once the
    /// content settles.
    fragment: Option<(u64, String, u64)>,
    /// The activations made so far.
    activations: u64,
    /// The locations of the visits Back left, the nearest last: where a
    /// native host's Forward goes (`history.rs`).
    ahead: Vec<String>,
    /// What each `open`, `push`, `replace` or `go` to a location with a
    /// fragment returned in the commit being made, with the visit and
    /// fragment it left on top: an explicit navigation, which scrolls even
    /// to a fragment the visit already shows, where a return by `back` or
    /// `select` does not. It counts only when the router is assigned that
    /// very value (`compare::same`), never for a value merely computed
    /// (LLP 1153 D5; LLP 1038's verbs stay pure).
    navigations: RefCell<Vec<(Value, u64, String)>>,
    refusals: RefCell<Vec<String>>,
    /// The last router value a call read, checked and converted once, with
    /// the reads made of it: a binding that calls `stack(nav)` or `top(nav)`
    /// on an unchanged router gets the same object back, not a rebuilt one.
    reads: RefCell<Option<Reads>>,
    /// Each share token set this launch and its `SharedFiles` record.
    shared: RefCell<std::collections::BTreeMap<String, Value>>,
    /// The counters this launch's shares draw tokens and names from.
    claims: RefCell<Option<crate::share_in::Claims>>,
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
            &["id", "name", "url", "tab", "params", "fragment"],
        )?;
        primitive(plan, fields[0], TypeKind::Number)?;
        for field in fields[1..4].iter().chain(&fields[5..]) {
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
            fragment: None,
            activations: 0,
            ahead: Vec::new(),
            navigations: RefCell::new(Vec::new()),
            refusals: RefCell::new(Vec::new()),
            reads: RefCell::new(None),
            // A cold share's tokens arrive with the boot's other facts.
            shared: RefCell::new(
                crate::time::launch_shared()
                    .into_iter()
                    .map(|(token, files)| (token, files.value()))
                    .collect(),
            ),
            claims: RefCell::new(crate::time::launch_claims()),
        }))
    }

    fn entry(&self, value: &Value) -> Option<Entry> {
        let v = record(value)?;
        if v.len() != 6 {
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
            fragment: v[5].as_str()?.into(),
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
            Value::str(&e.fragment),
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
        if f == Stdlib::Fragment {
            if !first.conforms(plan, self.entry_ty) {
                return None;
            }
            return Some(Value::str(&self.entry(first)?.fragment));
        }
        if f == Stdlib::SearchParam {
            if !first.conforms(plan, self.entry_ty) {
                return None;
            }
            return Some(Value::str(&exact_route::search_param(
                &self.entry(first)?,
                args.get(1)?.as_str()?,
            )));
        }
        if f == Stdlib::SharedFiles {
            if !first.conforms(plan, self.entry_ty) {
                return None;
            }
            let name = args.get(1)?.as_str()?;
            let token = exact_route::search_param(&self.entry(first)?, name);
            return Some(match self.shared.borrow().get(&token) {
                Some(files) => files.clone(),
                None => {
                    if !token.is_empty() {
                        self.refuse_once(
                            "sharedFiles",
                            &format!("no share under `{name}` this launch"),
                        );
                    }
                    crate::share_in::SharedFiles::default().value()
                }
            });
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
        let value = self.value(&after);
        if f != Stdlib::Back && f != Stdlib::Select && !exact_route::fragment_of(arg()?).is_empty()
        {
            if let Some(top) = exact_route::top(&after) {
                let navigation = (value.clone(), top.id, top.fragment.clone());
                self.navigations.borrow_mut().push(navigation);
            }
        }
        Some(value)
    }

    /// Whether the router was assigned `value`, a navigation to `top` by a
    /// location with its fragment, in the commit being made.
    fn activates(&self, value: &Value, top: &Entry) -> bool {
        (self.navigations.borrow().iter()).any(|(v, id, fragment)| {
            crate::compare::same(v, value) && *id == top.id && *fragment == top.fragment
        })
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

    fn share(&self, token: &str, files: Value) -> bool {
        self.shared.borrow_mut().insert(token.into(), files.clone()) != Some(files)
    }

    fn claims(&self, seed: f64) -> std::cell::RefMut<'_, crate::share_in::Claims> {
        std::cell::RefMut::map(self.claims.borrow_mut(), |c| {
            c.get_or_insert_with(|| crate::share_in::Claims::new(seed))
        })
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

    fn back_to(&self, value: &Value, source: u64, destination: u64) -> Option<Value> {
        let mut router = self.router(value)?;
        let tab = router.tabs.iter_mut().find(|tab| tab.name == router.tab)?;
        if tab.stack.last()?.id != source {
            return None;
        }
        let at = tab.stack.iter().position(|entry| entry.id == destination)?;
        if at + 1 == tab.stack.len() {
            return None;
        }
        tab.stack.truncate(at + 1);
        Some(self.value(&router))
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
        let before: Vec<Entry> = self
            .committed
            .as_ref()
            .and_then(|v| self.router(v))
            .into_iter()
            .flat_map(|r| r.tabs)
            .flat_map(|t| t.stack)
            .collect();
        // @ref LLP 1153 D5 — a new visit with a fragment, a fragment that
        // changed, or a navigation to the top's fragment scrolls; a visit
        // returned to keeps its place.
        let scroll = self.activates(value, top)
            || match before.iter().find(|e| e.id == top.id) {
                Some(old) => old.fragment != top.fragment,
                None => !top.fragment.is_empty(),
            };
        let removed = before
            .iter()
            .filter(|e| !ids.contains(&e.id))
            .map(|e| e.id)
            .collect();
        Ok(Some(RouterChange {
            top: top.id,
            url: top.url.clone(),
            fragment: top.fragment.clone(),
            scroll,
            activation: 0,
            removed,
            visits: r
                .tabs
                .iter()
                .flat_map(|t| &t.stack)
                .map(|e| (e.id, e.url.clone()))
                .collect(),
        }))
    }

    fn commit(&mut self, value: Value, change: Option<RouterChange>) {
        // A navigation to the fragment the top already shows changes no
        // value, and scrolls again, as a browser's link to it does: it is
        // published as a change of its own, so a host arms it as any other.
        let change = change.or_else(|| {
            let r = self.router(&value)?;
            let top = exact_route::top(&r)?;
            self.activates(&value, top).then(|| RouterChange {
                top: top.id,
                url: top.url.clone(),
                fragment: top.fragment.clone(),
                scroll: true,
                activation: 0,
                removed: Vec::new(),
                visits: (r.tabs.iter().flat_map(|t| &t.stack))
                    .map(|e| (e.id, e.url.clone()))
                    .collect(),
            })
        });
        self.navigations.borrow_mut().clear();
        if let Some(after) = self.router(&value) {
            let before = self.committed.as_ref().and_then(|v| self.router(v));
            super::history::step(before.as_ref(), &after, &mut self.ahead);
        }
        self.committed = Some(value);
        if let Some(mut change) = change {
            if let Some(pending) = self.pending.take() {
                // A clock seek may commit more than once before a host drains
                // effects. Keep every removed id and the latest selected top,
                // and its scroll while it shows the same fragment.
                let mut removed = pending.removed;
                for id in change.removed {
                    if !removed.contains(&id) {
                        removed.push(id);
                    }
                }
                change.removed = removed;
                if !change.scroll
                    && pending.scroll
                    && pending.top == change.top
                    && pending.fragment == change.fragment
                {
                    change.scroll = true;
                    change.activation = pending.activation;
                }
            }
            if change.scroll && change.activation == 0 {
                self.activations += 1;
                change.activation = self.activations;
            }
            if change.scroll {
                self.fragment = Some((change.top, change.fragment.clone(), change.activation));
            } else if self
                .fragment
                .as_ref()
                .is_some_and(|(id, ..)| *id != change.top)
            {
                self.fragment = None;
            }
            self.pending = Some(change);
        }
    }

    fn fragment_due(&self) -> bool {
        self.fragment.is_some()
    }

    fn take_fragment(&mut self) -> Option<(u64, String, u64)> {
        self.fragment.take()
    }

    fn activated(&self) -> bool {
        !self.navigations.borrow().is_empty()
    }

    fn forget_activation(&self) {
        self.navigations.borrow_mut().clear();
    }

    fn take_refusals(&self) -> Vec<String> {
        std::mem::take(&mut *self.refusals.borrow_mut())
    }

    fn ahead(&self) -> Option<&str> {
        self.ahead.last().map(String::as_str)
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
        // @ref LLP 1060 D4 — the locale slot is the last one lowered, and an
        // initializer may call `t`: it is filled first.
        let locale = self.plan.locale.map(|s| s.0 as usize);
        let rest = (0..self.plan.slots.len()).filter(|i| Some(*i) != locale);
        for i in locale.into_iter().chain(rest) {
            let row = &self.plan.slots[i];
            if row.owner.is_some() || row.late || self.plan.router == Some(SlotsId(i as u32)) {
                continue;
            }
            self.init_slot(i, carried)?;
            // Where the viewer is, known at launch (`launching_with`): the
            // locale `set_place` writes, before any initializer calls `t`.
            if Some(i) == locale && crate::time::launch_place().is_some() {
                if let Some(resolved) = self.plan.resolve_locale(&self.place.locale) {
                    self.slots[i] = Value::str(resolved);
                }
            }
        }
        Ok(())
    }

    /// The root slots of children used outside every region, initialized
    /// as the root instance first renders: after boot settlement, so an
    /// initializer may read derives and resources (LLP 1017 P4c). A carried
    /// value still wins, as for any root slot.
    pub(super) fn init_late_slots(&mut self, carried: Option<&Carried>) -> Result<(), RunnerError> {
        for i in 0..self.plan.slots.len() {
            if self.plan.slots[i].late {
                self.init_slot(i, carried)?;
            }
        }
        Ok(())
    }

    fn init_slot(&mut self, i: usize, carried: Option<&Carried>) -> Result<(), RunnerError> {
        let row = &self.plan.slots[i];
        let name = self.plan.str(row.name);
        let kept = carried
            .and_then(|c| c.slots.iter().find(|(n, _)| n == name))
            .map(|(_, v)| v.clone())
            .filter(|v| v.conforms(&self.plan, row.ty));
        let carried = kept.is_some();
        let v = match kept {
            Some(v) => v,
            None => self.eval(row.init, &[], &[])?,
        };
        if !v.conforms(&self.plan, row.ty) {
            return Err(RunnerError::slot_type(&self.plan, &v, row.ty, name));
        }
        // A persisted slot takes the store's value over its initializer's
        // (LLP 1116 D5); a carried value is this session's and wins.
        self.slots[i] = if carried {
            v
        } else {
            self.restore_persisted(i, v)
        };
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

    /// The host's scroll to the selected visit's fragment, as the command
    /// `scrollToFragment(visit, fragment, activation)`, once the content it
    /// waits for has arrived: the data source is ready and no request is in
    /// flight, a mutation's included and a held one not (`has_pending`; LLP
    /// 1153 D5, LLP 1113). The host finds the element in that visit's route
    /// and holds it in place while pictures load. An author's own
    /// `scrollIntoView` before then wins.
    pub(super) fn fragment_command(&mut self) {
        if !self.router.as_ref().is_some_and(|r| r.fragment_due()) {
            return;
        }
        let authored = self.commands.iter().any(|c| c.name == "scrollIntoView");
        if !authored && (!self.data.ready() || self.has_pending()) {
            return;
        }
        let Some((visit, fragment, activation)) =
            self.router.as_mut().and_then(|r| r.take_fragment())
        else {
            return;
        };
        if authored {
            self.log(format!(
                "fragment #{fragment}: the app's own scrollIntoView took its place"
            ));
            return;
        }
        self.commands.push(super::Command {
            name: "scrollToFragment".into(),
            args: vec![
                Value::Number(visit as f64),
                Value::str(&fragment),
                Value::Number(activation as f64),
            ],
            source: None,
        });
    }

    /// A refused commit's navigation intent goes with it (LLP 1153 D5).
    pub(super) fn conclude_router(&self, refused: bool) {
        if let Some(router) = self.router.as_deref().filter(|_| refused) {
            router.forget_activation();
        }
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

    /// The canonical location of a retained visit, or `None` when it is
    /// absent. Without its fragment: a `go` there returns to the visit as it
    /// was, where a fragment would scroll it again (LLP 1153 D5).
    pub fn visit_location(&self, id: u64) -> Option<String> {
        let (_, router) = self.carry_router()?;
        router
            .tabs
            .iter()
            .flat_map(|tab| &tab.stack)
            .find(|entry| entry.id == id)
            .map(|entry| entry.url.clone())
    }

    /// The location of the visit beneath visit `id` on the stack that holds
    /// it, or `None` (no router, no such visit, or a stack's root). A host's
    /// own Back for a route with no authored Back control delivers it to the
    /// navigation root's `navigate`, as the web's history Back does (LLP
    /// 1115 D5; LLP 1038 D11 as amended 2026-10-03).
    pub fn location_beneath(&self, id: u64) -> Option<String> {
        let (_, router) = self.carry_router()?;
        router.tabs.iter().find_map(|tab| {
            let at = tab.stack.iter().position(|e| e.id == id)?;
            at.checked_sub(1).map(|below| tab.stack[below].url.clone())
        })
    }

    /// Where the platform's own Back goes from the selected visit, or
    /// `None` (no router, or the stack's root): the location of the visit
    /// beneath the selected stack's top, without its fragment, as
    /// [`Runner::location_beneath`] gives it. A host asks it for a
    /// navigation root with no routes that hears `navigate`, whose Back
    /// steps the router's visits as a browser steps session history (LLP
    /// 1115.000 D5.5).
    pub fn location_behind(&self) -> Option<String> {
        let (_, router) = self.carry_router()?;
        let stack = exact_route::stack(&router);
        stack
            .len()
            .checked_sub(2)
            .map(|below| stack[below].url.clone())
    }

    /// Where the platform's own Forward goes, or `None` (no router, or
    /// nothing ahead): the nearest location Back left on the selected
    /// stack, as a browser's session history keeps it. A native host
    /// delivers it as the web host delivers a Forward (LLP 1038 D7): to the
    /// navigation root's `navigate`, else by [`Runner::host_go`] (LLP
    /// 1115.000 D5.5).
    pub fn location_ahead(&self) -> Option<String> {
        self.router.as_deref()?.ahead().map(String::from)
    }

    /// The platform's own Back from the selected visit `id`, for a route
    /// with no authored Back control under a navigation root with no
    /// `navigate` handler (LLP 1115 D5): the router's `back`, written by the
    /// runner as one commit of its own, as the app's `nav = back(nav)`
    /// would be — journaled as `host back`, refused like any commit. `None`
    /// (nothing done) without a router, or when `id` is not the selected
    /// top or is its stack's root: a Back the app already took, or one
    /// with nowhere to go.
    pub fn host_back(&mut self, id: u64) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(context) = self.router.as_deref() else {
            return Ok(None);
        };
        let slot = context.slot().0 as usize;
        let current = self.slots[slot].clone();
        let Some(router) = context.read(&current) else {
            return Ok(None);
        };
        if exact_route::top(&router).map(|e| e.id) != Some(id) || exact_route::depth(&router) < 2 {
            return Ok(None);
        }
        let Some(next) = context.call(&self.plan, Stdlib::Back, &[current]) else {
            return Ok(None);
        };
        self.host_commit(slot, next, "host back")
    }

    /// Complete a platform Back-history jump in one commit. Both endpoints are
    /// visit identities: repeated URLs must retain the visit UIKit selected.
    /// A stale source or a destination outside the selected stack does nothing.
    pub fn host_back_to(
        &mut self,
        source: u64,
        destination: u64,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(context) = self.router.as_deref() else {
            return Ok(None);
        };
        let slot = context.slot().0 as usize;
        let Some(next) = context.back_to(&self.slots[slot], source, destination) else {
            return Ok(None);
        };
        self.host_commit(slot, next, "host back to")
    }

    /// The platform's own navigation to `location` under a navigation root
    /// with no `navigate` handler: the browser's Forward, or a same-origin
    /// link to a declared route (LLP 1115.000 D4.3). It is the router's
    /// `go`, written by the runner as one commit of its own, as the app's
    /// `nav = go(nav, location)` would be — journaled as `host go`, refused
    /// like any commit. `None` (nothing done) without a router, or when
    /// `go` leaves the router as it is: no route matches `location`, or it
    /// is already the selected top.
    pub fn host_go(&mut self, location: &str) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(context) = self.router.as_deref() else {
            return Ok(None);
        };
        let slot = context.slot().0 as usize;
        let current = self.slots[slot].clone();
        let Some(router) = context.read(&current) else {
            return Ok(None);
        };
        let next = context
            .call(&self.plan, Stdlib::Go, &[current, Value::str(location)])
            .filter(|next| context.read(next).is_some_and(|r| r != router) || context.activated());
        self.log_router_refusals();
        match next {
            Some(next) => self.host_commit(slot, next, "host go"),
            None => Ok(None),
        }
    }

    /// One commit of the runner's own that writes the router `next`.
    fn host_commit(
        &mut self,
        slot: usize,
        next: Value,
        what: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let was_poisoned = self.poisoned;
        let checkpoint = self.checkpoint(false);
        let result = if self.poisoned {
            Err(RunnerError::Poisoned)
        } else {
            self.slots[slot] = next;
            self.router_change()
                .and_then(|_| self.settle(false))
                .and_then(|_| self.gate_step())
                .and_then(|_| self.update())
        };
        self.conclude(checkpoint, &result, was_poisoned);
        self.arm_then(result.is_ok());
        self.arm_next(result.is_ok());
        self.log_outcome(what, &result, was_poisoned);
        result.map(Some)
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

    /// Whether resource `name` shows a placeholder, not an answer (LLP
    /// 1054.000.002 D4): the bake compiles no value for it.
    pub fn resource_is_placeholder(&self, name: &str) -> bool {
        self.plan
            .resources
            .iter()
            .position(|r| self.plan.str(r.name) == name)
            .and_then(|i| self.resources[i].as_ref())
            .is_some_and(|r| r.placeholder)
    }
}
