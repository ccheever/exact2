//! The state as `exact_plan::Value`s, through the records `contract rust`
//! generates from `shapes.contract` (LLP 1101.002 P9). Every string passes
//! [`clean`] on the way out, so app text never carries a terminal control
//! sequence.

use crate::shapes::{Ack, ContractValue, Frame, Session};
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

fn c(text: &str) -> String {
    clean(text).into_owned()
}

/// A run with its text cleaned and its link checked.
fn run(r: &Run) -> Run {
    Run {
        text: c(&r.text),
        href: link(&r.href),
        ..r.clone()
    }
}

fn line(l: &Line) -> Line {
    Line {
        runs: l.runs.iter().map(run).collect(),
    }
}

fn block(b: &Block) -> Block {
    Block {
        kind: b.kind.clone(),
        depth: b.depth,
        marker: c(&b.marker),
        lang: c(&b.lang),
        runs: b.runs.iter().map(run).collect(),
        lines: b.lines.iter().map(line).collect(),
    }
}

fn entry(e: &Entry) -> Entry {
    Entry {
        id: e.id.clone(),
        kind: e.kind.clone(),
        busy: e.busy,
        title: c(&e.title),
        status: e.status.clone(),
        blocks: e.blocks.iter().map(block).collect(),
        more: e.more,
        image: c(&e.image),
        cols: e.cols,
        rows: e.rows,
        link: link(&e.link),
        model: c(&e.model),
    }
}

/// The context window the gauge measures against.
pub const CONTEXT: f64 = 200_000.0;

/// `shape Session`: the state mapped onto the generated record, every
/// string cleaned on the way.
pub fn session(st: &State) -> Value {
    Session {
        model: c(&st.model),
        provider: st.provider(),
        cwd: c(&st.cwd),
        branch: c(&st.branch),
        busy: st.busy(),
        phase: if st.phase.is_empty() {
            "idle".into()
        } else {
            st.phase.clone()
        },
        tokens_in: st.tokens_in,
        tokens_out: st.tokens_out,
        context_pct: (st.context / CONTEXT * 100.0).clamp(0.0, 100.0),
        entries: st.entries.iter().map(entry).collect(),
        approval: Approval {
            id: st.approval.id.clone(),
            tool: c(&st.approval.tool),
            summary: c(&st.approval.summary),
            lines: st.approval.lines.iter().map(line).collect(),
        },
        toast: c(&st.toast),
        models: st
            .models
            .iter()
            .map(|m| ModelChoice {
                label: c(&m.label),
                ..m.clone()
            })
            .collect(),
        retired: st.retired,
        queued: st.queue.iter().map(|q| c(q)).collect(),
        epoch: st.epoch,
    }
    .to_value()
}

/// `shape Frame`.
pub fn frame(title: &str, ls: &[Line]) -> Value {
    Frame {
        title: c(title),
        lines: ls.iter().map(line).collect(),
    }
    .to_value()
}

/// `shape Ack`.
pub fn ack(ok: bool) -> Value {
    Ack { ok }.to_value()
}
