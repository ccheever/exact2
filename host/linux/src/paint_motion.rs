//! Paint motion on the Linux host (LLP 1062): the engine samples colours and
//! the shadow like any property; the painter paints a presented value over
//! its row while the two differ. An animating `color` also reaches the views
//! and inline runs that inherit it, as a browser's inheriting element shows
//! it, and each `currentcolor` border side among them. An SVG element's
//! inherited `fill` and `stroke` reach its descendants through the scene's
//! resolver (LLP 1055.000 D15).

use super::Host;
use exact_kernel::motion::{motion_node, node_key};
use exact_kernel::{CommitReceipt, StyleId, ViewId};
use exact_motion::{Presentation, Property, Value};
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// Adopt a commit's paint, after its `motion_sync` set the rows.
    pub(super) fn sync_paint(&mut self, receipt: &CommitReceipt) {
        let retired = self
            .paint
            .sync(self.runner.kernel(), receipt, &mut self.engine);
        self.retire_paint(retired);
    }

    /// Adopt the whole tree's paint at boot.
    pub(super) fn boot_paint(&mut self) {
        self.paint.adopt(
            self.runner.kernel(),
            self.keys.keys().copied(),
            &mut self.engine,
        );
    }

    fn retire_paint(&mut self, retired: Vec<(u64, Property)>) {
        for (node, property) in retired {
            let views = self.inheritors_of(node, property);
            let own = self.keys.get(&node_key(node)).copied();
            for view in own.into_iter().chain(views) {
                match property {
                    Property::Color => self.paint_color(view, None),
                    p => self.paint_over(view, p, None),
                }
            }
        }
    }

    /// Report the session's appearance: first quietly, then transitioning
    /// `light-dark()` targets under their rows (LLP 1062 D4).
    pub fn set_scheme(&mut self, dark: bool) {
        if let Some(retired) = self.paint.set_scheme(
            self.runner.kernel(),
            &mut self.engine,
            dark,
            self.now_ms / 1000.0,
        ) {
            self.flow_damage.repaint();
            self.retire_paint(retired);
            self.present();
        }
    }

    /// Report a view's own appearance, following the same first-report
    /// correction and later transitions as the session's.
    pub fn set_view_scheme(&mut self, view: ViewId, dark: bool) {
        let Some(key) = self.runner.kernel().node(view).map(|n| n.key) else {
            return;
        };
        if let Some(retired) = self.paint.set_view_scheme(
            self.runner.kernel(),
            &mut self.engine,
            key,
            dark,
            self.now_ms / 1000.0,
        ) {
            self.flow_damage.repaint();
            self.retire_paint(retired);
            self.present();
        }
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
                    true => self.presented(view).colors.value(Property::Color),
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
            } else if !self.paint.owns(motion_node(key), side) {
                self.paint_over(view, side, None);
            }
        }
    }

    fn paint_over(&mut self, view: ViewId, property: Property, value: Option<Value>) {
        if let Some(n) = self.runner.kernel().node(view) {
            self.row_dirty.node(n.key);
        }
        let base = self.presented(view);
        let entry = self.presented.entry(view).or_insert(base);
        entry.colors.set(property, value);
    }

    /// The views below `node` whose `color` is `node`'s: no own row on the
    /// way, and no paint motion of their own.
    fn inheritors_of(&self, node: u64, property: Property) -> Vec<ViewId> {
        let row = match property {
            Property::Color => StyleId::TextColor,
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
            if child.style.mask.has(row) || self.paint.owns(motion_node(child.key), property) {
                continue;
            }
            out.push(id);
            stack.extend(child.children());
        }
        out
    }
}
