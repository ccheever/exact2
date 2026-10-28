//! The runner's half of a differential run: boot the plan, deliver each
//! event, and print what `Contract.Observe.lines` prints for the same
//! configuration (`semantics/Contract/Observe.lean` is the format's
//! definition).

use crate::oracle::Oracle;
use crate::script::Event;
use exact_kernel::{Kernel, PropId, ViewId};
use exact_plan::{Plan, Value};
use exact_runner::{ControlValue, Event as HostEvent, Runner};

/// A number as the observation prints it: its IEEE bits, NaN canonical.
pub fn number(n: f64) -> String {
    let bits = if n.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        n.to_bits()
    };
    format!("n{bits:016x}")
}

/// A string as the observation quotes it.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u00{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A value as the observation prints it.
pub fn value(v: &Value) -> String {
    match v {
        Value::Number(n) => number(*n),
        Value::Bool(b) => b.to_string(),
        v if v.is_str() => quote(v.as_str().unwrap_or_default()),
        Value::Unit => "()".into(),
        Value::Option(None) => "none".into(),
        Value::Option(Some(inner)) => format!("some({})", value(inner)),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(value).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Record(items) => {
            let parts: Vec<String> = items.iter().map(value).collect();
            format!("{{{}}}", parts.join(","))
        }
        _ => "?".into(),
    }
}

/// Every live element in structural preorder.
fn preorder(r: &Runner<Oracle>) -> Vec<ViewId> {
    fn walk(r: &Runner<Oracle>, id: ViewId, out: &mut Vec<ViewId>) {
        out.push(id);
        if let Some(n) = r.kernel().node(id) {
            for c in n.children() {
                walk(r, c, out);
            }
        }
    }
    let mut out = Vec::new();
    for root in r.roots() {
        walk(r, root, &mut out);
    }
    out
}

/// The elements an observation shows, in preorder: every live element but
/// a virtualized list's descendants, which are the rows its window lays out
/// (`Contract.Observe.viewLines` leaves them out too).
fn observed(r: &Runner<Oracle>) -> Vec<ViewId> {
    fn walk(r: &Runner<Oracle>, windowed: &[ViewId], id: ViewId, out: &mut Vec<ViewId>) {
        out.push(id);
        if windowed.contains(&id) {
            return;
        }
        if let Some(n) = r.kernel().node(id) {
            for c in n.children() {
                walk(r, windowed, c, out);
            }
        }
    }
    let windowed: Vec<ViewId> = r.collections().iter().map(|c| c.view).collect();
    let mut out = Vec::new();
    for root in r.roots() {
        walk(r, &windowed, root, &mut out);
    }
    out
}

fn find(r: &Runner<Oracle>, test_id: &str) -> Option<ViewId> {
    preorder(r).into_iter().find(|&id| {
        r.kernel()
            .node(id)
            .is_some_and(|n| n.props.str(PropId::TestId) == Some(test_id))
    })
}

/// Boot `plan` against `oracle`, on the device store the oracle holds,
/// building every tab panel then when the host would (a prerendered page).
pub(crate) fn boot(
    plan: Plan,
    oracle: Oracle,
) -> Result<Runner<Oracle>, exact_runner::RunnerError> {
    let snapshot = oracle.snapshot();
    let every = oracle.prerendered;
    let launch = oracle.launch.clone();
    let mut r = exact_runner::time::launching_with(launch, || {
        Runner::boot_stored(
            plan,
            oracle,
            Kernel::with_monospace(),
            snapshot,
            Default::default(),
            "/",
        )
    })?;
    if every {
        r.build_every_tab()?;
    }
    Ok(r)
}

/// The state, commands and view lines of one configuration.
fn state(r: &mut Runner<Oracle>, plan: &Plan, out: &mut Vec<String>) {
    for (i, row) in plan.slots.iter().enumerate() {
        let id = exact_plan::SlotsId(i as u32);
        if row.owner.is_some() || plan.locale == Some(id) {
            continue;
        }
        let name = plan.str(row.name);
        if let Some(v) = r.slot(name) {
            out.push(format!("slot {name} {}", value(v)));
        }
    }
    // What the store keeps for each persisted state (LLP 1116 D5).
    for row in plan.slots.iter().filter(|s| s.persist) {
        let name = plan.str(row.name);
        let kept = r.persisted(name).map_or("-".to_string(), |v| value(&v));
        out.push(format!("store {name} {kept}"));
    }
    for d in &plan.derives {
        let name = plan.str(d.name);
        if let Some(v) = r.derive(name) {
            out.push(format!("derive {name} {}", value(v)));
        }
    }
    for res in &plan.resources {
        let name = plan.str(res.name);
        if let Some(v) = r.resource(name) {
            out.push(format!("resource {name} {}", value(v)));
        }
    }
    // Each queue's waiting sends (LLP 1092 D12), in declaration order.
    let queued = r.queued();
    for m in plan.mutations.iter().filter(|m| m.queue) {
        let name = plan.str(m.name);
        let n = queued
            .iter()
            .find(|(q, _)| q == name)
            .map_or(0, |(_, n)| *n);
        out.push(format!("queued {name} {n}"));
    }
    for c in r.take_commands() {
        let args: String = c.args.iter().map(|a| format!(" {}", value(a))).collect();
        out.push(format!("command {}{args}", c.name));
    }
    for id in observed(r) {
        let Some(n) = r.kernel().node(id) else {
            continue;
        };
        if let Some(t) = n.props.str(PropId::TestId) {
            let text = n.props.str(PropId::Text).map_or("-".to_string(), quote);
            out.push(format!("view {} {text}", quote(t)));
        }
    }
}

/// Boot `plan` against `oracle`, deliver `events`, and return the
/// observation lines and the source (with its transcript).
///
/// An event is delivered as every host delivers one, at the current time
/// (`Runner::dispatch_at`): the work already due fires first, then the
/// event, which runs even when that work refused (`Contract.Observe.dispatchAt`).
pub fn run(plan: Plan, oracle: Oracle, events: &[Event]) -> (Vec<String>, Option<Oracle>) {
    let (mut driver, mut out) = Driver::new(plan, oracle);
    for event in events {
        out.extend(driver.step(event));
    }
    (out, Some(driver.oracle))
}

/// A booted runner. Each delivery returns exactly one block of [`run`]'s observation.
pub struct Driver {
    plan: Plan,
    runner: Option<Runner<Oracle>>,
    oracle: Oracle,
}

/// A rendered test target and the actions its input handlers name.
#[derive(Clone, Debug)]
pub struct Control {
    pub id: String,
    pub press: Option<String>,
    pub input: Option<String>,
}

impl Driver {
    /// Boot once, retaining the oracle even when boot refuses.
    pub fn new(plan: Plan, oracle: Oracle) -> (Self, Vec<String>) {
        let kept = oracle.handle();
        let mut out = vec!["== boot".into()];
        let runner = match boot(plan.clone(), oracle) {
            Err(e) => {
                out.extend(["outcome refused".into(), format!("# {e:?}")]);
                None
            }
            Ok(r) if r.is_poisoned() => {
                out.push("outcome poisoned".into());
                None
            }
            Ok(mut r) => {
                out.push("outcome ok".into());
                for res in &plan.resources {
                    let source = plan.str(res.source);
                    if !matches!(source, "exactTransport" | "exactMIDI")
                        && !r
                            .data()
                            .transcript
                            .borrow()
                            .iter()
                            .any(|c| c.source == source)
                    {
                        let name = plan.str(res.name);
                        if let Some(v) = r.resource(name).cloned() {
                            r.data()
                                .facts
                                .borrow_mut()
                                .push((name.to_string(), res.ty, v));
                        }
                    }
                }
                state(&mut r, &plan, &mut out);
                Some(r)
            }
        };
        (
            Self {
                plan,
                runner,
                oracle: kept,
            },
            out,
        )
    }

    /// Whether another event can be delivered.
    pub fn live(&self) -> bool {
        self.runner.as_ref().is_some_and(|r| !r.is_poisoned())
    }

    /// Deliver one event; a refused boot or poisoned runner returns no further blocks.
    pub fn step(&mut self, event: &Event) -> Vec<String> {
        if !self.live() {
            return Vec::new();
        }
        let r = self.runner.as_mut().expect("live runner");
        let mut out = vec![format!("== {}", event.label())];
        let ((result, receipts), ran) = r.trace_commits(|r| apply(r, event));
        let traced = ran
            .iter()
            .filter(|l| l.starts_with("ran ") && l.ends_with(" ok"))
            .count();
        if r.is_poisoned() {
            out.push("outcome poisoned".into());
        } else {
            out.push(
                if result.is_ok() {
                    "outcome ok"
                } else {
                    "outcome refused"
                }
                .into(),
            );
        }
        if let Err(e) = result {
            out.push(format!("# {e:?}"));
        }
        out.extend(ran);
        if receipts > traced {
            out.push(OMITTED_COMMIT.into());
        }
        if !r.is_poisoned() {
            state(r, &self.plan, &mut out);
        }
        out
    }

    /// Controls among the same live elements whose `view` lines the observation prints.
    pub fn controls(&self) -> Vec<Control> {
        let Some(r) = self.runner.as_ref().filter(|_| self.live()) else {
            return Vec::new();
        };
        let mut seen = std::collections::BTreeSet::new();
        observed(r)
            .into_iter()
            .filter_map(|view| {
                let id = r
                    .kernel()
                    .node(view)?
                    .props
                    .str(PropId::TestId)?
                    .to_string();
                if !seen.insert(id.clone()) {
                    return None;
                }
                let (node, _) = r.site_of(view)?;
                let action = |kind| {
                    self.plan
                        .node(node)
                        .handlers
                        .iter()
                        .map(|h| self.plan.handler(h))
                        .find(|h| h.event == kind)
                        .map(|h| self.plan.str(self.plan.action(h.action).name).to_string())
                };
                Some(Control {
                    id,
                    press: action(exact_plan::EventKind::Press),
                    input: action(exact_plan::EventKind::Input)
                        .or_else(|| action(exact_plan::EventKind::Change)),
                })
            })
            .collect()
    }
}

/// The plan node of the first element with `test_id` after `events`, by
/// the same boot and delivery as [`run`]: where a step's target or a
/// differing view line was declared (`contract::SourceMap::node`).
pub fn site(plan: Plan, oracle: Oracle, events: &[Event], test_id: &str) -> Option<usize> {
    let mut r = boot(plan, oracle).ok()?;
    for e in events {
        let _ = apply(&mut r, e);
        if r.is_poisoned() {
            return None;
        }
    }
    let view = find(&r, test_id)?;
    r.site_of(view).map(|(node, _)| node.0 as usize)
}

/// One event at the runner's current time, as a host delivers it: a host
/// attaches a listener only where the view handles the event
/// (`Runner::handlers_of`), so an event the view has no handler for reaches
/// nothing and fires nothing due; one it handles fires what is due first
/// (`Runner::dispatch_at`), and the event's refusal, else the due work's, is
/// the outcome (`Contract.Observe.dispatchAt`).
fn deliver(
    r: &mut Runner<Oracle>,
    view: ViewId,
    event: HostEvent,
) -> (Result<(), exact_runner::RunnerError>, usize) {
    let kind = match &event {
        HostEvent::Press => Some(exact_plan::EventKind::Press),
        HostEvent::Change(_) => Some(exact_plan::EventKind::Change),
        HostEvent::Input(_) => Some(exact_plan::EventKind::Input),
        _ => None,
    };
    if !kind.is_some_and(|k| r.handlers_of(view).contains(&k)) {
        let result = r.dispatch(view, event);
        let receipts = usize::from(result.is_ok());
        return (result.map(drop), receipts);
    }
    let now = r.now_ms();
    let advanced = r.dispatch_at(view, event, now);
    (advanced.error.map_or(Ok(()), Err), advanced.receipts.len())
}

/// A receipt with no modeled commit. Property checks must exclude this step from `after`.
pub const OMITTED_COMMIT: &str = "# a commit the semantics has no branch for";

/// Deliver a primitive event, retaining successful receipts even when a later commit refuses.
pub(crate) fn apply(
    r: &mut Runner<Oracle>,
    e: &Event,
) -> (Result<(), exact_runner::RunnerError>, usize) {
    match e {
        Event::Tap(t) | Event::Type(t, _) => match find(r, t) {
            Some(view) => deliver(
                r,
                view,
                match e {
                    Event::Type(_, s)
                        if r.handlers_of(view).contains(&exact_plan::EventKind::Input) =>
                    {
                        HostEvent::Input(ControlValue::Text(s.clone()))
                    }
                    Event::Type(_, s) => HostEvent::Change(ControlValue::Text(s.clone())),
                    _ => HostEvent::Press,
                },
            ),
            None => (Err(exact_runner::RunnerError::UnknownView(0)), 0),
        },
        Event::Clock(ms) => {
            let a = r.advance_timed(r.now_ms() + ms);
            (a.error.map_or(Ok(()), Err), a.receipts.len())
        }
        Event::Land => {
            let a = r.land_then();
            (a.error.map_or(Ok(()), Err), a.receipts.len())
        }
    }
}
