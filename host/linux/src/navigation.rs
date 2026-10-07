//! @ref LLP 1038 D6 — the browser's route visibility rule, without a view mirror.
use exact_kernel::{Kernel, PropId, ViewId};
use std::collections::{BTreeMap, BTreeSet};

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

/// A navigation root's tabpanels (LLP 1075.003 §3.7): those its own
/// tablist's tabs name with `aria-controls`, in tab order — the browser's
/// `panelsOf`. A tablist inside a route is that route's.
fn tab_panels(kernel: &Kernel, children: &[ViewId]) -> Vec<ViewId> {
    fn walk(kernel: &Kernel, ids: &[ViewId], lists: &mut Vec<ViewId>, panels: &mut Vec<ViewId>) {
        for id in ids {
            let Some(node) = kernel.node(*id) else {
                continue;
            };
            let role = node.props.str(PropId::AccessibilityRole);
            if role == Some("tabpanel") {
                panels.push(*id);
            }
            if role == Some("tablist") {
                lists.push(*id);
            }
            if node.props.str(PropId::NavigationKey).is_none() && role != Some("tablist") {
                walk(kernel, &node.children(), lists, panels);
            }
        }
    }
    let (mut lists, mut panels) = (Vec::new(), Vec::new());
    walk(kernel, children, &mut lists, &mut panels);
    for list in lists {
        let Some(list) = kernel.node(list) else {
            continue;
        };
        let named: Vec<ViewId> = list
            .children()
            .iter()
            .filter_map(|t| kernel.node(*t))
            .filter(|t| t.props.str(PropId::AccessibilityRole) == Some("tab"))
            .filter_map(|t| t.props.str(PropId::AccessibilityControls))
            .filter_map(|name| {
                panels.iter().copied().find(|p| {
                    kernel
                        .node(*p)
                        .is_some_and(|n| n.props.str(PropId::Id) == Some(name))
                })
            })
            .collect();
        if !named.is_empty() {
            return named;
        }
    }
    Vec::new()
}

#[derive(Default)]
pub(crate) struct Navigation {
    routes: BTreeMap<ViewId, (bool, bool)>,
    /// The tabpanels a root last hid or showed: one it stops naming shows again.
    panels: BTreeSet<ViewId>,
    refused: BTreeMap<ViewId, String>,
    popovers: bool,
}

impl Navigation {
    pub fn sync(&mut self, kernel: &Kernel, order: &[ViewId]) -> Vec<String> {
        self.routes.retain(|id, _| kernel.node(*id).is_some());
        self.refused.retain(|id, _| kernel.node(*id).is_some());
        let mut logs = Vec::new();
        let before = std::mem::take(&mut self.panels);
        self.popovers = false;
        for id in order {
            let Some(nav) = kernel.node(*id) else {
                continue;
            };
            self.popovers |= nav.props.str(PropId::Popover).is_some();
            if nav.props.str(PropId::NavigationBack).is_none() {
                continue;
            }
            let route_rows = |ids: &[ViewId]| -> Vec<ViewId> {
                ids.iter()
                    .copied()
                    .filter(|id| {
                        kernel
                            .node(*id)
                            .is_some_and(|n| n.props.str(PropId::NavigationKey).is_some())
                    })
                    .collect()
            };
            // @ref LLP 1075.003 §3.7 — with tabs, each panel is a stack.
            let panels = tab_panels(kernel, &nav.children());
            self.panels.extend(panels.iter().copied());
            let stacks: Vec<Vec<ViewId>> = if panels.is_empty() {
                vec![route_rows(&nav.children())]
            } else {
                panels
                    .iter()
                    .filter_map(|p| kernel.node(*p))
                    .map(|p| route_rows(&p.children()))
                    .collect()
            };
            let key = |id: &ViewId| {
                kernel
                    .node(*id)
                    .and_then(|n| n.props.str(PropId::NavigationKey))
                    .unwrap_or("")
            };
            let selected = nav.props.str(PropId::NavigationKey).unwrap_or("");
            let Some(at) = stacks
                .iter()
                .position(|routes| routes.iter().any(|r| key(r) == selected))
            else {
                if self.refused.get(id).map(String::as_str) != Some(selected) {
                    self.refused.insert(*id, selected.into());
                    logs.push(format!(
                        "navigationKey \"{selected}\" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged"
                    ));
                }
                continue;
            };
            self.refused.remove(id);
            for (index, panel) in panels.iter().enumerate() {
                self.routes.insert(*panel, (index != at, index != at));
            }
            for (stack, routes) in stacks.iter().enumerate() {
                let keys: Vec<&str> = routes.iter().map(key).collect();
                // The selected stack shows the route the root names; another
                // keeps its top laid out.
                let shown = if stack == at {
                    selected
                } else {
                    keys.last().copied().unwrap_or("")
                };
                let modal = routes.iter().find(|r| key(r) == shown).is_some_and(|r| {
                    kernel
                        .node(*r)
                        .and_then(|n| n.props.str(PropId::NavigationPresentation))
                        == Some("modal")
                });
                if let Some(visibility) = route_visibility(&keys, shown, modal) {
                    for (route, state) in routes.iter().zip(visibility) {
                        self.routes.insert(*route, state);
                    }
                }
            }
        }
        for panel in before.difference(&self.panels) {
            self.routes.remove(panel);
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
