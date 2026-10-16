//! Tabs built when first selected (@ref LLP 1075.003 §3.7, Q5).
//!
//! On iOS and tvOS each tab is a navigation controller of its own under a
//! `UITabBarController`, which loads a tab's view when the tab is first
//! selected. The runner keeps every tab's stack mounted, so state, the
//! agent's tree and every test read the same, but the presenter is given a
//! tab the navigation root leaves unselected only when it is first selected:
//! the tabpanel and its routes are views, their contents are not, and the
//! kernel lays the panel out as covered whole (`display: none`), measuring
//! nothing in it. Selected once, a tab stays built, as `UITabBarController`
//! keeps a loaded tab. The panel's contents arrive in the batch that selects
//! it, before UIKit shows it.

use super::*;

/// The tabs not yet selected, and the views their routes hold.
#[derive(Debug, Default)]
pub(crate) struct Unopened {
    /// Each tabpanel not yet selected.
    panels: Vec<(ViewId, NodeKey)>,
    /// Each view under one of their routes, which the presenter has not been given.
    held: IdMap<NodeKey, ViewId>,
    held_ids: IdSet<ViewId>,
}

#[cfg(test)]
thread_local! {
    /// The policy in a test, which runs on the Mac.
    pub(crate) static TABS_WAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this platform builds a tab when it is first selected.
fn tabs_wait() -> bool {
    #[cfg(test)]
    if TABS_WAIT.get() {
        return true;
    }
    cfg!(any(target_os = "ios", target_os = "tvos"))
}

impl<D: DataSource> Host<D> {
    /// At boot, before any view is made: every tab the root leaves
    /// unselected waits.
    pub(crate) fn close_unopened_tabs(&mut self) {
        if tabs_wait() {
            let panels = self.runner.unselected_tabs();
            self.close(&panels);
        }
    }

    /// Before a commit's views: the held views it destroyed are forgotten,
    /// and a tabpanel it made that the root leaves unselected waits.
    pub(crate) fn close_new_tabs(&mut self, receipts: &[Timed]) {
        let unopened = &mut self.unopened;
        for key in receipts.iter().flat_map(|t| &t.receipt.destroyed) {
            if let Some(id) = unopened.held.remove(key) {
                unopened.held_ids.remove(&id);
            }
        }
        if !tabs_wait() {
            return;
        }
        let kernel = self.runner.kernel();
        let created: IdSet<NodeKey> = receipts
            .iter()
            .flat_map(|t| t.receipt.created.iter().copied())
            .filter(|key| {
                kernel
                    .node_by_key(*key)
                    .is_some_and(|n| n.props.str(PropId::AccessibilityRole) == Some("tabpanel"))
            })
            .collect();
        if created.is_empty() {
            return;
        }
        let panels: Vec<ViewId> = self
            .runner
            .unselected_tabs()
            .into_iter()
            .filter(|p| kernel.node(*p).is_some_and(|n| created.contains(&n.key)))
            .collect();
        self.close(&panels);
    }

    fn close(&mut self, panels: &[ViewId]) {
        for &panel in panels {
            let Some(key) = self.runner.kernel().node(panel).map(|n| n.key) else {
                continue;
            };
            // A panel the kernel has is always coverable.
            let _ = self
                .runner
                .kernel_mut()
                .set_host_cover(panel, Some(exact_kernel::HostCover::Whole));
            self.unopened.panels.push((panel, key));
        }
    }

    /// Whether the presenter has not been given `id`.
    pub(crate) fn held(&self, id: ViewId) -> bool {
        self.unopened.held_ids.contains(&id)
    }

    /// At a view's creation: whether it lies under a route of a tab not yet
    /// selected, and is held until the tab is.
    pub(crate) fn hold(&mut self, id: ViewId) -> bool {
        if self.unopened.panels.is_empty() {
            return false;
        }
        let kernel = self.runner.kernel();
        let Some(node) = kernel.node(id) else {
            return false;
        };
        let (key, mut child, mut parent) = (node.key, id, node.parent);
        while let Some(at) = parent {
            if self.unopened.panels.iter().any(|(p, _)| *p == at) {
                // The panel's own children (its routes) are views.
                if child == id {
                    return false;
                }
                self.unopened.held.insert(key, id);
                self.unopened.held_ids.insert(id);
                return true;
            }
            child = at;
            parent = kernel.node(at).and_then(|n| n.parent);
        }
        false
    }

    /// After a commit's views: each waiting tab the root now selects is
    /// built — its views made, its children placed, its presented motion
    /// values sent — and laid out from now on. A tab that left the tree
    /// stops waiting.
    pub(crate) fn open_selected_tabs(&mut self, batch: &mut Batch) {
        if self.unopened.panels.is_empty() {
            return;
        }
        let kernel = self.runner.kernel();
        self.unopened
            .panels
            .retain(|(_, key)| kernel.node_by_key(*key).is_some());
        let off = self.runner.unselected_tabs();
        let (open, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.unopened.panels)
            .into_iter()
            .partition(|(panel, _)| !off.contains(panel));
        self.unopened.panels = wait;
        for (panel, _) in open {
            self.open(panel, batch);
        }
    }

    fn open(&mut self, panel: ViewId, batch: &mut Batch) {
        let _ = self.runner.kernel_mut().set_host_cover(panel, None);
        let kernel = self.runner.kernel();
        let routes = kernel.node(panel).map(|n| n.children()).unwrap_or_default();
        // Parents before children, as a boot makes them.
        let mut order = Vec::new();
        let mut stack: Vec<ViewId> = routes.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            if let Some(held) = self.unopened.held.remove(&node.key) {
                self.unopened.held_ids.remove(&held);
                order.push(id);
            }
            stack.extend(node.children().into_iter().rev());
        }
        let handlers = self.runner.handlers();
        for &id in &order {
            self.create(id, handlers.get(&id).map_or(&[], Vec::as_slice), batch);
        }
        for &id in routes.iter().chain(&order) {
            self.emit_children(id, batch);
        }
        for &id in &order {
            self.present_settled(id, batch);
        }
    }

    /// A view made after its motion values were set: those that are not
    /// the property's identity, as a boot sends them.
    fn present_settled(&mut self, id: ViewId, batch: &mut Batch) {
        if self.inline_runs.contains_key(&id) || self.svg.presented(id) {
            return;
        }
        let Some(key) = self.runner.kernel().node(id).map(|n| n.key) else {
            return;
        };
        for property in [
            Property::Translate,
            Property::Scale,
            Property::Rotate,
            Property::Opacity,
        ] {
            let Some(v) = self.engine.value(motion_node(key), property) else {
                continue;
            };
            if property.identity() == Some(v) {
                continue;
            }
            if property == Property::Translate && (v.z != 0.0 || v.w != 0.0) {
                batch.present4(id, "translate", [v.x, v.y, v.z, v.w]);
            } else {
                batch.present(id, property.name(), v.x, v.y);
            }
        }
    }
}
