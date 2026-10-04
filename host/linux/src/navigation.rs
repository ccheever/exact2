//! @ref LLP 1038 D6 — the browser's route visibility rule, without a view mirror.
use exact_kernel::{Kernel, PropId, ViewId};
use std::collections::BTreeMap;

pub(crate) const POPOVER_UNSUPPORTED: &str = "Linux does not support popover presentation";

/// Popover invocation is a host default action, even without a press handler.
/// Refuse it before running an application's accompanying refresh/action.
pub(crate) fn popover_invoker(kernel: &Kernel, id: ViewId) -> bool {
    let mut at = Some(id);
    while let Some(id) = at {
        let Some(node) = kernel.node(id) else { break };
        if node.props.bool(PropId::Disabled) == Some(true) {
            return false;
        }
        // A native button is a button: a press on it is its own, not an
        // enclosing invoker's (LLP 1069.011.000 D9; it invokes no menu).
        if node.node_type == exact_kernel::NodeType::Pressable
            || exact_kernel::ControlKind::of(node.node_type, node.props)
                == Some(exact_kernel::ControlKind::Button)
        {
            return node.props.str(PropId::Popovertarget).is_some();
        }
        at = node.parent;
    }
    false
}

/// Hidden and inert, respectively, for each direct route child. An unmatched
/// selection leaves the previously projected state alone.
pub fn route_visibility(keys: &[&str], selected: &str, modal: bool) -> Option<Vec<(bool, bool)>> {
    let selected = keys.iter().position(|key| *key == selected)?;
    Some(
        (0..keys.len())
            .map(|index| {
                let active = index == selected;
                (
                    !active && !(modal && index.checked_add(1) == Some(selected)),
                    !active,
                )
            })
            .collect(),
    )
}

#[derive(Default)]
pub(crate) struct Navigation {
    routes: BTreeMap<ViewId, (bool, bool)>,
    refused: BTreeMap<ViewId, String>,
    popovers: bool,
}

impl Navigation {
    pub fn sync(&mut self, kernel: &Kernel, order: &[ViewId]) -> Vec<String> {
        self.routes.retain(|id, _| kernel.node(*id).is_some());
        self.refused.retain(|id, _| kernel.node(*id).is_some());
        let mut logs = Vec::new();
        self.popovers = false;
        for id in order {
            let Some(nav) = kernel.node(*id) else {
                continue;
            };
            self.popovers |= nav.props.str(PropId::Popover).is_some();
            if nav.props.str(PropId::NavigationBack).is_none() {
                continue;
            }
            let children = nav.children();
            let routes: Vec<_> = children
                .iter()
                .filter_map(|id| kernel.node(*id))
                .filter(|node| node.props.str(PropId::NavigationKey).is_some())
                .collect();
            let keys: Vec<_> = routes
                .iter()
                .map(|node| node.props.str(PropId::NavigationKey).unwrap())
                .collect();
            let selected = nav.props.str(PropId::NavigationKey).unwrap_or("");
            let modal = routes
                .iter()
                .find(|node| node.props.str(PropId::NavigationKey) == Some(selected))
                .is_some_and(|node| {
                    node.props.str(PropId::NavigationPresentation) == Some("modal")
                });
            let Some(visibility) = route_visibility(&keys, selected, modal) else {
                if self.refused.get(id).map(String::as_str) != Some(selected) {
                    self.refused.insert(*id, selected.into());
                    logs.push(format!(
                        "navigationKey \"{selected}\" matches no route; the stack is unchanged"
                    ));
                }
                continue;
            };
            self.refused.remove(id);
            for (node, state) in routes.iter().zip(visibility) {
                self.routes.insert(node.id, state);
            }
        }
        logs
    }

    pub fn visibility(&self, kernel: &Kernel, id: ViewId) -> (bool, bool) {
        // No route, popover or inert node anywhere: nothing to climb for.
        if self.routes.is_empty()
            && !(self.popovers && kernel.has_prop(PropId::Popover))
            && !kernel.has_prop(PropId::Inert)
        {
            return (false, false);
        }
        let mut result = (false, false);
        let mut at = Some(id);
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else { break };
            let (hidden, inert) = self.routes.get(&id).copied().unwrap_or_default();
            // Linux has no top-layer presenter yet. Closed popovers retain
            // their logical tree but must never paint or intercept input.
            let closed = self.popovers && node.props.str(PropId::Popover).is_some();
            result.0 |= hidden || closed;
            result.1 |= inert || closed || node.props.bool(PropId::Inert) == Some(true);
            at = node.parent;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_route_and_only_its_modal_underlay_are_visible() {
        let keys = ["a", "b", "c", "d"];
        assert_eq!(
            route_visibility(&keys, "c", false),
            Some(vec![
                (true, true),
                (true, true),
                (false, false),
                (true, true)
            ])
        );
        assert_eq!(
            route_visibility(&keys, "c", true),
            Some(vec![
                (true, true),
                (false, true),
                (false, false),
                (true, true)
            ])
        );
        assert_eq!(
            route_visibility(&keys, "a", true),
            Some(vec![
                (false, false),
                (true, true),
                (true, true),
                (true, true)
            ])
        );
        assert_eq!(route_visibility(&keys, "missing", false), None);
        assert_eq!(route_visibility(&[], "missing", false), None);
    }
}
