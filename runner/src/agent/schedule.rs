//! The agent's view of what is scheduled (LLP 1092 D10): each queue's
//! waiting sends.

use super::quote;
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
