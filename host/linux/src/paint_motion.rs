//! Paint motion on the Linux host (LLP 1062): the engine samples colours and
//! the shadow like any property; the painter paints a presented value over
//! its row while the two differ. An animating `color` also reaches the views
//! and inline runs that inherit it, as a browser's inheriting element shows
//! it, and each `currentcolor` border side among them; so do a path's
//! inherited `fill` and `stroke` (LLP 1065).

use super::Host;
use exact_kernel::motion::{motion_node, MotionSync};
use exact_kernel::{CommitReceipt, NodeKey, StyleId, ViewId};
use exact_motion::{Presentation, Property, Value};
use exact_runner::DataSource;

fn node_key(node: u64) -> NodeKey {
    NodeKey {
        index: node as u32,
        generation: (node >> 32) as u32,
    }
}

impl<D: DataSource> Host<D> {
    /// Adopt a commit's paint, after its `motion_sync` set the rows.
    pub(super) fn sync_paint(&mut self, receipt: &CommitReceipt) {
        let sync = self
            .runner
            .kernel()
            .paint_sync(receipt, self.dark, &mut self.paint_owners);
        self.apply_paint(sync);
    }

    /// Adopt the whole tree's paint at boot.
    pub(super) fn boot_paint(&mut self) {
        let kernel = self.runner.kernel();
        let keys: Vec<NodeKey> = self.keys.keys().copied().collect();
        let sync = kernel.paint_adopt(keys, self.dark, &mut self.paint_owners);
        self.apply_paint(sync);
    }

    fn apply_paint(&mut self, sync: MotionSync) {
        // A side that was and stays `currentcolor` has no transition of its
        // own (CSS: its computed value never changed): it takes the new
        // `color` at once and paints the presented one.
        let kernel = self.runner.kernel();
        let mut nodes: Vec<u64> = sync.changes.iter().map(|c| c.node).collect();
        nodes.dedup();
        for node in nodes {
            let now = kernel.current_color_sides(node_key(node));
            let was = self.paint_current.remove(&node).unwrap_or_default();
            for side in now.iter().filter(|s| was.contains(s)) {
                if !self.engine.is_active(node, *side) {
                    self.engine.remove_property(node, *side);
                }
            }
            if !now.is_empty() {
                self.paint_current.insert(node, now);
            }
        }
        for (node, property) in &sync.retired {
            self.paint_current.remove(node);
            let views = self.inheritors_of(*node, *property);
            let own = self.keys.get(&node_key(*node)).copied();
            for view in own.into_iter().chain(views) {
                match property {
                    Property::Color => self.paint_color(view, None),
                    p => self.paint_over(view, *p, None),
                }
            }
        }
        let applied = sync.apply(&mut self.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
    }

    /// The appearance `light-dark()` resolves to (the app's `setScheme`):
    /// every owner re-targets, transitioning under its row (LLP 1062 D4).
    pub fn set_scheme(&mut self, dark: bool) {
        if self.dark == dark {
            return;
        }
        self.dark = dark;
        self.engine.set_dark(dark, false);
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        let sync = self
            .runner
            .kernel()
            .paint_resync(dark, &mut self.paint_owners);
        self.apply_paint(sync);
    }

    /// One paint presentation, over its row while it differs from the target.
    pub(super) fn present_paint(&mut self, p: Presentation) {
        let settled = self.engine.target(p.node, p.property) == Some(p.value);
        let value = (!settled).then_some(p.value);
        let mut views: Vec<ViewId> = self
            .keys
            .get(&node_key(p.node))
            .copied()
            .into_iter()
            .collect();
        if p.property != Property::Color {
            views.extend(self.inheritors_of(p.node, p.property));
            for view in views {
                // A settled `currentcolor` side paints the presented `color`.
                let follows = value.is_none()
                    && self.runner.kernel().node(view).is_some_and(|n| {
                        self.runner
                            .kernel()
                            .current_color_sides(n.key)
                            .contains(&p.property)
                    });
                let value = match follows {
                    true => self.presented(view).paint.value(Property::Color),
                    false => value,
                };
                self.paint_over(view, p.property, value);
            }
            return;
        }
        views.extend(self.inheritors_of(p.node, Property::Color));
        for view in views {
            self.paint_color(view, value);
        }
    }

    /// `color` on `view`, and on its `currentcolor` border sides; a side
    /// that has its own colour and no motion of its own shows its row.
    fn paint_color(&mut self, view: ViewId, value: Option<Value>) {
        self.paint_over(view, Property::Color, value);
        let kernel = self.runner.kernel();
        let Some(key) = kernel.node(view).map(|n| n.key) else {
            return;
        };
        let current = kernel.current_color_sides(key);
        for side in [
            Property::BorderTopColor,
            Property::BorderRightColor,
            Property::BorderBottomColor,
            Property::BorderLeftColor,
        ] {
            // A side moving under its own row shows its own value.
            if self.engine.is_active(motion_node(key), side) {
                continue;
            }
            if current.contains(&side) {
                self.paint_over(view, side, value);
            } else if !self.paint_owners.owns(motion_node(key), side) {
                self.paint_over(view, side, None);
            }
        }
    }

    fn paint_over(&mut self, view: ViewId, property: Property, value: Option<Value>) {
        let base = self.presented(view);
        let entry = self.presented.entry(view).or_insert(base);
        entry.paint.set(property, value);
    }

    /// The views below `node` whose `property` is `node`'s, when it is an
    /// inherited one (`color`, `fill`, `stroke`): no own row on the way, and
    /// no paint motion of their own.
    fn inheritors_of(&self, node: u64, property: Property) -> Vec<ViewId> {
        let row = match property {
            Property::Color => StyleId::TextColor,
            Property::Fill => StyleId::Fill,
            Property::Stroke => StyleId::Stroke,
            _ => return Vec::new(),
        };
        let kernel = self.runner.kernel();
        let Some(source) = kernel.node_by_key(node_key(node)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = source.children();
        while let Some(id) = stack.pop() {
            let Some(child) = kernel.node(id) else {
                continue;
            };
            if child.style.mask.has(row) || self.paint_owners.owns(motion_node(child.key), property)
            {
                continue;
            }
            out.push(id);
            stack.extend(child.children());
        }
        out
    }
}
