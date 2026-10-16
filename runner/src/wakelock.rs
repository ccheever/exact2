//! `wakelock="screen"` (LLP 1125 D11): the display does not dim or lock for
//! idleness while any element that is mounted and displayed says so.
//! Displayed is CSS's: neither the node nor an ancestor is `display: none`,
//! so a screen kept mounted in a hidden tab panel holds nothing. This is the
//! one rule: the native hosts take the lock by it, and the agent reads it as
//! `state.wakelock`.

use exact_kernel::{Display, Kernel, PropId, ViewId};
use exact_plan::{BindingKind, Plan};

/// Whether any node of `plan` says `wakelock`: a host that loads its lock on
/// demand (the JS target's wakelock.js) bundles it only then.
pub fn bound(plan: &Plan) -> bool {
    plan.nodes.iter().any(|node| {
        node.bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| b.kind == BindingKind::Prop && b.id == PropId::Wakelock as u16)
    })
}

/// The nodes that hold the lock now, in tree order.
pub fn holders(kernel: &Kernel) -> Vec<ViewId> {
    if !kernel.has_prop(PropId::Wakelock) {
        return Vec::new();
    }
    kernel
        .preorder_where(&kernel.roots(), |_, props| {
            props.str(PropId::Wakelock) == Some("screen")
        })
        .into_iter()
        .filter(|id| displayed(kernel, *id))
        .collect()
}

/// Whether neither `id` nor an ancestor is `display: none`.
fn displayed(kernel: &Kernel, id: ViewId) -> bool {
    let mut at = Some(id);
    while let Some(id) = at {
        let Some(node) = kernel.node(id) else {
            return false;
        };
        if node.style.display == Display::None {
            return false;
        }
        at = node.parent;
    }
    true
}

/// `,"wakelock":{"held":…,"by":[…]}`, the agent's `state.wakelock`.
pub(crate) fn state(kernel: &Kernel, s: &mut String) {
    let by = holders(kernel);
    s.push_str(",\"wakelock\":{\"held\":");
    s.push_str(if by.is_empty() { "false" } else { "true" });
    s.push_str(",\"by\":[");
    for (i, id) in by.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&id.to_string());
    }
    s.push_str("]}");
}

#[cfg(test)]
#[path = "wakelock_tests.rs"]
mod tests;
