//! The state as `exact_plan::Value`s, field for field in the order
//! `shapes.contract` declares them. Every string passes [`clean`] on the
//! way out, so app text never carries a terminal control sequence.

use crate::state::{Approval, Block, Entry, Line, ModelChoice, Run, State};
use exact_plan::Value;
use std::borrow::Cow;

/// Text without control characters: ESC becomes "␛", a tab a space, any
/// other C0/C1 control or DEL is dropped.
pub fn clean(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{1b}' => out.push('␛'),
            '\t' | '\n' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// A link a terminal may open: http(s) or file, with no control or space
/// characters; anything else is no link.
fn link(href: &str) -> String {
    let scheme = ["https://", "http://", "file://"]
        .iter()
        .any(|p| href.starts_with(p));
    if scheme && !href.chars().any(|c| c.is_control() || c == ' ') {
        href.to_string()
    } else {
        String::new()
    }
}

fn s(text: &str) -> Value {
    Value::str(&clean(text))
}

/// `shape Run`.
pub fn run(r: &Run) -> Value {
    Value::record(vec![
        s(&r.text),
        Value::str(&r.fg),
        Value::str(&r.bg),
        Value::Bool(r.bold),
        Value::Bool(r.italic),
        Value::Bool(r.dim),
        Value::Bool(r.under),
        Value::Bool(r.strike),
        Value::str(&link(&r.href)),
    ])
}

fn runs(rs: &[Run]) -> Value {
    Value::list(rs.iter().map(run).collect())
}

/// `shape Line`.
pub fn line(l: &Line) -> Value {
    Value::record(vec![runs(&l.runs)])
}

/// A list of lines.
pub fn lines(ls: &[Line]) -> Value {
    Value::list(ls.iter().map(line).collect())
}

/// `shape Block`.
pub fn block(b: &Block) -> Value {
    Value::record(vec![
        Value::str(&b.kind),
        Value::Number(b.depth),
        s(&b.marker),
        s(&b.lang),
        runs(&b.runs),
        lines(&b.lines),
    ])
}

/// `shape Entry`.
pub fn entry(e: &Entry) -> Value {
    Value::record(vec![
        Value::str(&e.id),
        Value::str(&e.kind),
        Value::Bool(e.busy),
        s(&e.title),
        Value::str(&e.status),
        Value::list(e.blocks.iter().map(block).collect()),
        Value::Number(e.more),
        s(&e.image),
        Value::Number(e.cols),
        Value::Number(e.rows),
        Value::str(&link(&e.link)),
    ])
}

/// `shape Approval`.
pub fn approval(a: &Approval) -> Value {
    Value::record(vec![
        Value::str(&a.id),
        s(&a.tool),
        s(&a.summary),
        lines(&a.lines),
    ])
}

/// `shape ModelChoice`.
pub fn model(m: &ModelChoice) -> Value {
    Value::record(vec![
        Value::str(&m.id),
        s(&m.label),
        Value::str(&m.provider),
        Value::Bool(m.available),
    ])
}

/// The context window the gauge measures against.
pub const CONTEXT: f64 = 200_000.0;

/// `shape Session`.
pub fn session(st: &State) -> Value {
    Value::record(vec![
        s(&st.model),
        Value::str(&st.provider()),
        s(&st.cwd),
        s(&st.branch),
        Value::Bool(st.busy()),
        Value::str(if st.phase.is_empty() {
            "idle"
        } else {
            &st.phase
        }),
        Value::Number(st.tokens_in),
        Value::Number(st.tokens_out),
        Value::Number((st.context / CONTEXT * 100.0).clamp(0.0, 100.0)),
        Value::list(st.entries.iter().map(entry).collect()),
        approval(&st.approval),
        s(&st.toast),
        Value::list(st.models.iter().map(model).collect()),
        Value::Number(st.retired),
        Value::list(st.queue.iter().map(|q| s(q)).collect()),
    ])
}

/// `shape Frame`.
pub fn frame(title: &str, ls: &[Line]) -> Value {
    Value::record(vec![s(title), lines(ls)])
}

/// `shape Ack`.
pub fn ack(ok: bool) -> Value {
    Value::record(vec![Value::Bool(ok)])
}
