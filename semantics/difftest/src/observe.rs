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
pub fn run(plan: Plan, oracle: Oracle, events: &[Event]) -> (Vec<String>, Option<Oracle>) {
    let mut out = vec!["== boot".to_string()];
    let kept = oracle.handle();
    let mut r = match Runner::boot(
        plan.clone(),
        oracle,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    ) {
        Ok(r) => r,
        Err(e) => {
            out.push("outcome refused".into());
            out.push(format!("# {e:?}"));
            return (out, Some(kept));
        }
    };
    if r.is_poisoned() {
        out.push("outcome poisoned".into());
        return (out, Some(kept));
    }
    out.push("outcome ok".into());
    // The host's facts, as the runner answered them at boot.
    for res in &plan.resources {
        let source = plan.str(res.source);
        if crate::oracle::HOST_SOURCES.contains(&source) {
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
    for e in events {
        out.push(format!("== {}", e.label()));
        let was = r.is_poisoned();
        let result = match e {
            Event::Tap(t) => match find(&r, t) {
                Some(view) => r.dispatch(view, HostEvent::Press).map(drop),
                None => Err(exact_runner::RunnerError::UnknownView(0)),
            },
            Event::Type(t, s) => match find(&r, t) {
                Some(view) => r
                    .dispatch(view, HostEvent::Change(ControlValue::Text(s.clone())))
                    .map(drop),
                None => Err(exact_runner::RunnerError::UnknownView(0)),
            },
            Event::Clock(ms) => {
                let to = r.now_ms() + ms;
                r.advance(to).map(drop)
            }
        };
        if r.is_poisoned() && !was {
            out.push("outcome poisoned".into());
            if let Err(e) = &result {
                out.push(format!("# {e:?}"));
            }
            break;
        }
        match &result {
            Ok(()) => out.push("outcome ok".into()),
            Err(e) => {
                out.push("outcome refused".into());
                out.push(format!("# {e:?}"));
            }
        }
        if r.is_poisoned() {
            break;
        }
        state(&mut r, &plan, &mut out);
    }
    (out, Some(kept))
}
