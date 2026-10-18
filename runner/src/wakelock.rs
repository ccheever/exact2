//! `wakelock="screen"` (LLP 1125 D11): the display does not dim or lock for
//! idleness while any element that is mounted and displayed says so.
//! Displayed means neither the node nor an ancestor is `display: none` or
//! laid out as such by its host (a whole [`HostCover`]: an unopened tab, a
//! closed top-layer box), and it is under no route its navigation root
//! hides: a tab not selected, or a screen a stack keeps under its top (the
//! one a sheet covers stays shown). So a screen kept mounted in a hidden tab
//! panel holds nothing. A whole cover marks both what a host hides and what
//! it shows as its own control (a header in the navigation bar, a
//! confirmation as an alert, a menu as a native menu), which the kernel cannot
//! tell apart, so natively the latter holds nothing too: a rare false release
//! rather than a lock left on in a closed dialog. This is the one rule: the
//! native hosts take the lock by it, and the agent reads it as
//! `state.wakelock`.

use exact_kernel::{Display, HostCover, Kernel, PropId, ViewId};
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

/// Whether `id` is displayed (the module's rule).
fn displayed(kernel: &Kernel, id: ViewId) -> bool {
    let arena = kernel.arena();
    let mut at = Some(id);
    while let Some(id) = at {
        let Some(node) = kernel.node(id) else {
            return false;
        };
        let whole = arena.slot_of(id).and_then(|slot| arena.cover(slot)) == Some(HostCover::Whole);
        if node.style.display == Display::None || whole {
            return false;
        }
        at = node.parent;
    }
    !crate::head::hidden_route(kernel, id)
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
