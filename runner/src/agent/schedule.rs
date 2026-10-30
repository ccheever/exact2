//! The agent's view of what is scheduled (LLP 1092 D10): each task's next
//! due time (`null` while idle or spent) and each queue's waiting sends.

use super::{num, quote};
use crate::runner::{DataSource, Runner};

/// `"queued":{"wrote":2}`: each mutation with sends waiting their turn.
pub(super) fn queued<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    s.push_str(",\"queued\":{");
    for (i, (name, n)) in runner.queued().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, s);
        s.push(':');
        s.push_str(&n.to_string());
    }
    s.push('}');
}

/// `"tasks":{"hide":5000,"beat":null}`: each task's next due time.
pub(super) fn tasks<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    s.push_str(",\"tasks\":{");
    for (i, (name, due)) in runner.tasks().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, s);
        s.push(':');
        match due {
            Some(ms) => s.push_str(&num(*ms).to_string()),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"taskTiming\":{");
    for (i, task) in runner.task_timing().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(&task.name, s);
        let due = task.due_ms.map_or("null".into(), |at| num(at).to_string());
        let cap = task.cap_ms.map_or("null".into(), |at| num(at).to_string());
        use std::fmt::Write as _;
        let _ = write!(
            s,
            ":{{\"due\":{due},\"wait\":{},\"held\":{},\"cap\":{cap},\"tolerance\":{}}}",
            task.wait, task.held, task.tolerance_ms
        );
    }
    s.push('}');
}
