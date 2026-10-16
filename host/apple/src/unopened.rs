//! Tabs built when first selected (@ref LLP 1075.003 §3.7, Q5, §9.12).
//!
//! On iOS and tvOS each tab is a navigation controller of its own under a
//! `UITabBarController`, which loads a tab's view when the tab is first
//! selected. The runner keeps every tab's stack mounted, so state, the
//! agent's tree and every test read the same, but the presenter is given a
//! tab the navigation root leaves unselected only when it is first selected:
//! the tabpanel and its routes are views, what the routes hold is not.
//! Selected once, a tab stays built, as `UITabBarController` keeps a loaded
//! tab. The panel's contents arrive in the batch that selects it, before
//! UIKit shows it, with what the presenter would have heard meanwhile: the
//! GPU surfaces' inputs and the presented motion and paint values.
//!
//! A waiting panel that is absolutely positioned is also laid out as covered
//! whole (`display: none`), measuring nothing in it: out of flow, it moves
//! no other box (LLP 1001 declares what it leaves out). A panel in flow is
//! laid out as authored, since its size can move its siblings.

use super::*;

/// The tabs not yet selected, and what the presenter has not been given.
#[derive(Debug, Default)]
pub(crate) struct Unopened {
    /// Each tabpanel not yet selected, and whether its layout is skipped.
    panels: Vec<(ViewId, NodeKey, bool)>,
    /// Each view under one of their routes.
    held: IdMap<NodeKey, ViewId>,
    held_ids: IdSet<ViewId>,
    /// The last surface inputs of each held GPU canvas.
    surfaces: IdMap<ViewId, exact_runner::SurfaceUpdate>,
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

    /// Before a commit's views: what it destroyed of the held views is
    /// forgotten, and a tabpanel it made that the root leaves unselected
    /// waits.
    pub(crate) fn close_new_tabs(&mut self, receipts: &[Timed]) {
        let unopened = &mut self.unopened;
        for key in receipts.iter().flat_map(|t| &t.receipt.destroyed) {
            if let Some(id) = unopened.held.remove(key) {
                unopened.held_ids.remove(&id);
                unopened.surfaces.remove(&id);
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
            if let Some(key) = self.runner.kernel().node(panel).map(|n| n.key) {
                self.unopened.panels.push((panel, key, false));
            }
        }
        self.skip_waiting_layout();
    }

    /// Each waiting panel's layout is skipped while it is out of flow, and
    /// restored when it comes into flow.
    fn skip_waiting_layout(&mut self) {
        for i in 0..self.unopened.panels.len() {
            let (panel, _, skipped) = self.unopened.panels[i];
            let out = self
                .runner
                .kernel()
                .node(panel)
                .is_some_and(|n| n.style.position_type == exact_kernel::PositionType::Absolute);
            if out != skipped {
                let cover = out.then_some(exact_kernel::HostCover::Whole);
                // A panel the kernel has is always coverable.
                let _ = self.runner.kernel_mut().set_host_cover(panel, cover);
                self.unopened.panels[i].2 = out;
            }
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
            if self.unopened.panels.iter().any(|(p, _, _)| *p == at) {
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

    /// The commits' GPU surface inputs: a held canvas's latest is kept
    /// for its tab's opening, as the runner sends no input again unchanged.
    pub(crate) fn emit_surfaces(&mut self, batch: &mut Batch) {
        for s in self.runner.take_surface_updates() {
            if self.held(s.view) {
                self.unopened.surfaces.insert(s.view, s);
            } else {
                batch.surface(&s);
            }
        }
    }

    /// After a commit's views: each waiting tab the root now selects is
    /// built — its views made, its children placed, its surfaces' inputs and
    /// presented values sent — and laid out from now on. A tab that left the
    /// tree stops waiting.
    pub(crate) fn open_selected_tabs(&mut self, batch: &mut Batch) {
        if self.unopened.panels.is_empty() {
            return;
        }
        let kernel = self.runner.kernel();
        self.unopened
            .panels
            .retain(|(_, key, _)| kernel.node_by_key(*key).is_some());
        let off = self.runner.unselected_tabs();
        let (open, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.unopened.panels)
            .into_iter()
            .partition(|(panel, _, _)| !off.contains(panel));
        self.unopened.panels = wait;
        self.skip_waiting_layout();
        for (panel, _, skipped) in open {
            self.open(panel, skipped, batch);
        }
    }

    fn open(&mut self, panel: ViewId, skipped: bool, batch: &mut Batch) {
        if skipped {
            let _ = self.runner.kernel_mut().set_host_cover(panel, None);
        }
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
        // The panel's ancestors, whose presented colour its views inherit.
        let mut above = Vec::new();
        let mut at = Some(panel);
        while let Some(id) = at {
            above.push(id);
            at = kernel.node(id).and_then(|n| n.parent);
        }
        let handlers = self.runner.handlers();
        for &id in &order {
            self.create(id, handlers.get(&id).map_or(&[], Vec::as_slice), batch);
        }
        for &id in routes.iter().chain(&order) {
            self.emit_children(id, batch);
        }
        for &id in &order {
            if let Some(s) = self.unopened.surfaces.remove(&id) {
                batch.surface(&s);
            }
            self.present_settled(id, batch);
        }
        for id in above.into_iter().rev().chain(routes).chain(order) {
            self.present_paint(id, batch);
        }
    }

    /// A view made after its motion values were set: what the engine
    /// shows now that is not the property's identity, as a boot sends it.
    fn present_settled(&mut self, id: ViewId, batch: &mut Batch) {
        let Some(key) = self.runner.kernel().node(id).map(|n| n.key) else {
            return;
        };
        let node = motion_node(key);
        for property in [
            Property::Translate,
            Property::Scale,
            Property::Rotate,
            Property::Opacity,
            Property::Layout,
        ] {
            if let Some(value) = self.engine.sampled_value(node, property) {
                self.present_value(id, property, node, value, true, batch);
            }
        }
    }

    /// A box's moving paint over its rows, and its presented `color` on
    /// what inherits it, inline runs among them (`present_colors`).
    fn present_paint(&mut self, id: ViewId, batch: &mut Batch) {
        let Some(key) = self.runner.kernel().node(id).map(|n| n.key) else {
            return;
        };
        let shown = self.shown_paint(motion_node(key));
        if shown.0.iter().any(Option::is_some) {
            self.present_colors(id, batch, shown.get(Property::Color).is_some());
        }
    }
}
