//! @ref LLP 1038 D6 — the browser's route visibility rule, without a view mirror.
use exact_kernel::invocation::Action;
use exact_kernel::{Kernel, PropId, ViewId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const POPOVER_UNSUPPORTED: &str = "Linux does not support popover presentation";
pub(crate) const DIALOG_UNSUPPORTED: &str =
    "unsupported: Linux dialog presentation is not implemented";

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
            return invocation(kernel, id).is_some_and(|action| !dialog_command(action));
        }
        at = node.parent;
    }
    false
}

/// What the button `id` invokes, as HTML resolves it.
fn invocation(kernel: &Kernel, id: ViewId) -> Option<Action> {
    let node = kernel.node(id)?;
    exact_kernel::Invocation::of(kernel, &node).map(|i| i.action)
}

fn dialog_command(action: Action) -> bool {
    matches!(action, Action::ShowModal | Action::Close)
}

/// LLP 1035.001 D1, when the root's key matches no route: the stack is left
/// alone, less the routes that have left the tree. The deepest route of the
/// stack last shown (its keys through its selected route) that is still
/// declared is the selected one, so a closed sheet or a popped screen never
/// leaves the route beneath it covered; `None` (nothing shown yet) changes nothing.
pub fn survivor(shown: &[String], declared: impl Fn(&str) -> bool) -> Option<String> {
    shown.iter().rev().find(|key| declared(key)).cloned()
}

/// Hidden and inert, respectively, for each direct route child, `sheets`
/// saying which routes are sheets. A sheet shows what it sits over: the page
/// (the last route beneath it that is not a sheet) and every sheet between,
/// as navigation.js `covered` does. An unmatched selection leaves the
/// previously projected state alone.
pub fn route_visibility(
    keys: &[&str],
    selected: &str,
    sheets: &[bool],
) -> Option<Vec<(bool, bool)>> {
    let selected = keys.iter().position(|key| *key == selected)?;
    let sheet = |i: usize| sheets.get(i).copied().unwrap_or(false);
    let page = (0..selected).rev().find(|i| !sheet(*i)).unwrap_or(0);
    let covered = |i: usize| sheet(selected) && (page..selected).contains(&i);
    Some(
        (0..keys.len())
            .map(|index| {
                let active = index == selected;
                (!active && !covered(index), !active)
            })
            .collect(),
    )
}

/// A navigation root's stacks (LLP 1075.003 §3.7): with tabs (the kernel's
/// rule, which every host with a tab bar reads, LLP 1115.001 D11) each
/// panel is one, its children that carry a `navigationKey`; with none, the
/// root's own such children are the one stack.
pub(crate) fn stacks(
    kernel: &Kernel,
    root: ViewId,
) -> (Option<exact_kernel::RootTabs>, Vec<Vec<ViewId>>) {
    let route_rows = |id: ViewId| -> Vec<ViewId> {
        let children = kernel.node(id).map(|n| n.children()).unwrap_or_default();
        children
            .into_iter()
            .filter(|id| {
                kernel
                    .node(*id)
                    .is_some_and(|n| n.props.str(PropId::NavigationKey).is_some())
            })
            .collect()
    };
    let tabs = kernel.root_tabs(root);
    let stacks = match &tabs {
        Some(tabs) => tabs.panels.iter().map(|p| route_rows(*p)).collect(),
        None => vec![route_rows(root)],
    };
    (tabs, stacks)
}

#[derive(Default)]
pub(crate) struct Navigation {
    pub(crate) split: crate::presenter::split::State,
    routes: BTreeMap<ViewId, (bool, bool)>,
    /// The tabpanels a root last hid or showed: one it stops naming shows again.
    panels: BTreeSet<ViewId>,
    refused: BTreeMap<ViewId, String>,
    /// Each root's stack as last shown, its keys through its selected route.
    shown: BTreeMap<ViewId, Vec<String>>,
    /// The first root's stack as its key last named it: its routes through
    /// the selected one. What the focus follows (`presenter/arrive.rs`; LLP
    /// 1115.000 D4.2); a key that matches no route leaves it as it was.
    pub(crate) stack: Option<Vec<ViewId>>,
    popovers: bool,
    pub(crate) shown_popover: Option<ViewId>,
    /// A shell presents the top layer (LLP 1115.001 D10: Android's reader,
    /// as windows): a press on an invoker is then the presenter's to act on
    /// (`presenter/top.rs`), not refused here.
    pub(crate) top_layer: bool,
    /// What a shell's bars are made from (LLP 1115.001 D11; `bars.rs`),
    /// and the nodes a bar stands for: a header its top bar replaces, a
    /// tablist its navigation bar replaces. Those are neither painted nor
    /// hit, as iOS hides the header its bar lifts.
    pub(crate) bars: Projection,
    pub(crate) lifted: BTreeSet<ViewId>,
}

/// A route whose `header` is its bar (the kernel's rule,
/// `exact_kernel::bars`).
#[derive(Debug, Clone, PartialEq)]
pub struct RouteBar {
    /// The route.
    pub route: ViewId,
    /// Not the first of its stack, nor presented over it: the bar's Up
    /// arrow goes Back from it, and stands for its Back control.
    pub pushed: bool,
    /// Its header as the bar shows it.
    pub shape: exact_kernel::HeaderShape<ViewId>,
}

/// The structure a shell's bars are made from (LLP 1115.001 D11), read
/// from the first navigation root after each commit (`bars.rs`). Empty on a
/// host whose shell has no bars: desktop Linux, a headless run.
#[derive(Debug, Default)]
pub struct Projection {
    /// The room the shell's bars take of a route: the top bar from its top
    /// edge and the navigation bar from its bottom, in points. `None`: no
    /// shell has bars, and nothing is projected.
    pub(crate) room: Option<(f32, f32)>,
    /// The navigation root.
    pub root: Option<ViewId>,
    /// Its routes whose header is a bar, in every stack.
    pub routes: Vec<RouteBar>,
    /// Its tabs, when its tablist is the navigation bar.
    pub tabs: Option<exact_kernel::RootTabs>,
    /// Its stacks of routes: one a tab, in the tabs' order; else the one.
    pub stacks: Vec<Vec<ViewId>>,
    /// What the bars cover of the tree, as the kernel was last told.
    pub(crate) covers: BTreeMap<ViewId, exact_kernel::HostCover>,
}

impl Navigation {
    /// Why a press on `view` is refused before its handler runs, on a host
    /// that presents no top layer: a popover's invoker (`dialog` false), or
    /// a button that names a dialog with `commandfor` (`dialog` true).
    pub(crate) fn refusal(&self, kernel: &Kernel, view: ViewId, dialog: bool) -> Option<&str> {
        if self.top_layer {
            return None;
        }
        match dialog {
            false => popover_invoker(kernel, view).then_some(POPOVER_UNSUPPORTED),
            true => invocation(kernel, view)
                .is_some_and(dialog_command)
                .then_some(DIALOG_UNSUPPORTED),
        }
    }

    pub fn sync(&mut self, kernel: &Kernel, order: &[ViewId]) -> Vec<String> {
        self.routes.retain(|id, _| kernel.node(*id).is_some());
        self.refused.retain(|id, _| kernel.node(*id).is_some());
        self.shown.retain(|id, _| kernel.node(*id).is_some());
        let mut logs = Vec::new();
        let before = std::mem::take(&mut self.panels);
        self.popovers = false;
        let mut first = true;
        for id in order {
            let Some(nav) = kernel.node(*id) else {
                continue;
            };
            self.popovers |= nav.props.str(PropId::Popover).is_some();
            if nav.props.str(PropId::NavigationBack).is_none() {
                continue;
            }
            let first = std::mem::take(&mut first);
            // @ref LLP 1075.003 §3.7 — with tabs, each panel is a stack.
            let (tabs, stacks) = stacks(kernel, *id);
            let panels = tabs.map_or(Vec::new(), |tabs| tabs.panels);
            self.panels.extend(panels.iter().copied());
            let key = |id: &ViewId| {
                kernel
                    .node(*id)
                    .and_then(|n| n.props.str(PropId::NavigationKey))
                    .unwrap_or("")
            };
            let named = nav.props.str(PropId::NavigationKey).unwrap_or("");
            let declared = |k: &str| {
                stacks
                    .iter()
                    .any(|routes| routes.iter().any(|r| key(r) == k))
            };
            let matched = declared(named);
            let selected = if matched {
                self.refused.remove(id);
                named.to_string()
            } else {
                if self.refused.get(id).map(String::as_str) != Some(named) {
                    self.refused.insert(*id, named.into());
                    logs.push(format!(
                        "navigationKey \"{named}\" matches no route among the root's children or those of the tabpanels its tablist names; the stack is unchanged"
                    ));
                }
                let shown = self.shown.get(id).map(Vec::as_slice).unwrap_or_default();
                let Some(survivor) = survivor(shown, declared) else {
                    continue;
                };
                survivor
            };
            let Some(at) = stacks
                .iter()
                .position(|routes| routes.iter().any(|r| key(r) == selected))
            else {
                continue;
            };
            for (index, panel) in panels.iter().enumerate() {
                self.routes.insert(*panel, (index != at, index != at));
            }
            for (stack, routes) in stacks.iter().enumerate() {
                let keys: Vec<&str> = routes.iter().map(key).collect();
                // The selected stack shows the route the root names; another
                // keeps its top laid out.
                let shown = if stack == at {
                    selected.as_str()
                } else {
                    keys.last().copied().unwrap_or("")
                };
                if stack == at {
                    let through = keys.iter().position(|k| *k == shown).map_or(0, |i| i + 1);
                    self.shown
                        .insert(*id, keys[..through].iter().map(|k| k.to_string()).collect());
                    if first && matched {
                        self.stack = Some(routes[..through].to_vec());
                    }
                }
                let sheets: Vec<bool> = routes
                    .iter()
                    .map(|r| {
                        kernel
                            .node(*r)
                            .and_then(|n| n.props.str(PropId::NavigationPresentation))
                            == Some("modal")
                    })
                    .collect();
                if let Some(visibility) = route_visibility(&keys, shown, &sheets) {
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
        self.visibility_of(kernel, id, true)
    }

    /// [`Navigation::visibility`], and whether what a shell's bar stands
    /// for counts as gone from the page (`lifted`).
    pub(crate) fn visibility_of(&self, kernel: &Kernel, id: ViewId, lifted: bool) -> (bool, bool) {
        self.visibility_to(kernel, id, lifted, None)
    }

    /// A separately presented subtree inherits visibility only through its root.
    pub(crate) fn visibility_to(
        &self,
        kernel: &Kernel,
        id: ViewId,
        lifted: bool,
        root: Option<ViewId>,
    ) -> (bool, bool) {
        // No route, popover or inert node anywhere: nothing to climb for.
        if self.routes.is_empty()
            && (self.lifted.is_empty() || !lifted)
            && !(self.popovers && kernel.has_prop(PropId::Popover))
            && !kernel.has_prop(PropId::Inert)
        {
            return (false, false);
        }
        let mut result = (false, false);
        let mut at = Some(id);
        while let Some(id) = at {
            let Some(node) = kernel.node(id) else { break };
            let (hidden, inert) = self
                .split
                .visibility(id)
                .unwrap_or_else(|| self.routes.get(&id).copied().unwrap_or_default());
            // Closed popovers retain their tree but neither paint nor take input.
            let closed = self.popovers
                && Some(id) != self.shown_popover
                && node.props.str(PropId::Popover).is_some();
            // What a shell's bar stands for shows in the bar, not the page.
            let gone = closed || lifted && self.lifted.contains(&id);
            result.0 |= hidden || gone;
            result.1 |= inert || gone || node.props.bool(PropId::Inert) == Some(true);
            if Some(id) == root {
                break;
            }
            at = node.parent;
        }
        result
    }
}

impl<D: exact_runner::DataSource> crate::Host<D> {
    pub(crate) fn neutral_dialog_invoker(&self, view: ViewId) -> bool {
        let kernel = self.kernel();
        kernel
            .node(view)
            .and_then(|node| exact_kernel::Invocation::of(kernel, &node))
            .and_then(|invocation| kernel.node_by_key(invocation.target))
            .is_some_and(|n| {
                crate::paint::look::neutral()
                    && (n.props.str(PropId::SemanticTag) == Some("dialog")
                        || n.props.str(PropId::Popover).is_some()
                            && n.props.str(PropId::AccessibilityRole) != Some("menu"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_route_and_only_its_modal_underlay_are_visible() {
        let keys = ["a", "b", "c", "d"];
        assert_eq!(
            route_visibility(&keys, "c", &[false; 4]),
            Some(vec![
                (true, true),
                (true, true),
                (false, false),
                (true, true)
            ])
        );
        assert_eq!(
            route_visibility(&keys, "c", &[false, false, true, false]),
            Some(vec![
                (true, true),
                (false, true),
                (false, false),
                (true, true)
            ])
        );
        assert_eq!(
            route_visibility(&keys, "a", &[true, false, false, false]),
            Some(vec![
                (false, false),
                (true, true),
                (true, true),
                (true, true)
            ])
        );
        assert_eq!(route_visibility(&keys, "missing", &[]), None);
        assert_eq!(route_visibility(&[], "missing", &[]), None);
    }

    /// A sheet over a sheet shows both the sheet beneath it and the page
    /// (LLP 1133.029 D3), as navigation.js `covered` does; a page pushed
    /// beneath the first sheet hides the route it was pushed over.
    #[test]
    fn a_sheet_over_a_sheet_shows_the_page_and_the_sheets_between() {
        let keys = ["a", "b", "c", "d"];
        assert_eq!(
            route_visibility(&keys, "d", &[false, false, true, true]),
            Some(vec![
                (true, true),
                (false, true),
                (false, true),
                (false, false)
            ])
        );
        assert_eq!(
            route_visibility(&keys, "d", &[false, true, false, true]),
            Some(vec![
                (true, true),
                (true, true),
                (false, true),
                (false, false)
            ])
        );
    }

    /// LLP 1035.001 D1: a root whose key names no route (visit "0" over a
    /// route keyed "rooms") keeps what is left of the stack it showed: the
    /// sheet closed, the route beneath it is selected, live again.
    #[test]
    fn a_key_that_names_no_route_keeps_what_is_left_of_the_stack_shown() {
        let shown = ["rooms".to_string(), "1".to_string()];
        assert_eq!(survivor(&shown, |k| k == "rooms"), Some("rooms".into()));
        assert_eq!(
            survivor(&shown, |k| k == "rooms" || k == "1"),
            Some("1".into())
        );
        assert_eq!(survivor(&shown, |_| false), None);
        assert_eq!(survivor(&[], |_| true), None);
    }
}
